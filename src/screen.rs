//! A pixel framebuffer that renders to the terminal using the upper-half-block
//! character: each terminal cell shows two vertically stacked pixels
//! (foreground = top pixel, background = bottom pixel). Only cells that changed
//! since the previous frame are re-sent, which keeps output small.

use std::fmt::Write as _;
use std::io::{self, Write};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn lerp(self, o: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(f(self.0, o.0), f(self.1, o.1), f(self.2, o.2))
    }

    pub fn scale(self, k: f32) -> Rgb {
        let f = |a: u8| (a as f32 * k).round().clamp(0.0, 255.0) as u8;
        Rgb(f(self.0), f(self.1), f(self.2))
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Cell {
    ch: char,
    fg: Rgb,
    bg: Rgb,
    bold: bool,
}

/// A text character drawn on top of the pixel layer.
#[derive(Clone, Copy)]
struct Over {
    ch: char,
    fg: Rgb,
    bg: Option<Rgb>,
    bold: bool,
}

pub struct Screen {
    pub cols: usize,
    pub rows: usize,
    /// Pixel width (== cols).
    pub w: i32,
    /// Pixel height (== rows * 2).
    pub h: i32,
    /// Drawing offset, used for screen shake.
    pub ox: i32,
    pub oy: i32,
    px: Vec<Rgb>,
    over: Vec<Option<Over>>,
    prev: Vec<Option<Cell>>,
    truecolor: bool,
    needs_clear: bool,
    buf: String,
}

impl Screen {
    pub fn new(cols: usize, rows: usize, truecolor: bool) -> Self {
        let mut s = Screen {
            cols: 0,
            rows: 0,
            w: 0,
            h: 0,
            ox: 0,
            oy: 0,
            px: Vec::new(),
            over: Vec::new(),
            prev: Vec::new(),
            truecolor,
            needs_clear: true,
            buf: String::new(),
        };
        s.resize(cols, rows);
        s
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
        self.w = cols as i32;
        self.h = rows as i32 * 2;
        self.px = vec![Rgb::default(); cols * rows * 2];
        self.over = vec![None; cols * rows];
        self.prev = vec![None; cols * rows];
        self.needs_clear = true;
    }

    pub fn clear(&mut self, c: Rgb) {
        self.px.fill(c);
        self.over.fill(None);
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            Some((y * self.w + x) as usize)
        } else {
            None
        }
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.idx(x + self.ox, y + self.oy) {
            self.px[i] = c;
        }
    }

    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if a <= 0.0 {
            return;
        }
        if let Some(i) = self.idx(x + self.ox, y + self.oy) {
            self.px[i] = self.px[i].lerp(c, a);
        }
    }

    /// Fill a whole pixel row, ignoring the shake offset.
    pub fn hline_raw(&mut self, y: i32, c: Rgb) {
        if y >= 0 && y < self.h {
            let start = (y * self.w) as usize;
            self.px[start..start + self.w as usize].fill(c);
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    pub fn blend_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb, a: f32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.blend(xx, yy, c, a);
            }
        }
    }

    fn circle_pixels(cx: f32, cy: f32, r: f32, mut f: impl FnMut(i32, i32)) {
        let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
        let (y0, y1) = ((cy - r).floor() as i32, (cy + r).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy <= r * r {
                    f(x, y);
                }
            }
        }
    }

    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        Self::circle_pixels(cx, cy, r, |x, y| self.set(x, y, c));
    }

    pub fn blend_circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
        Self::circle_pixels(cx, cy, r, |x, y| self.blend(x, y, c, a));
    }

    /// Blend the whole pixel layer toward a color.
    pub fn tint(&mut self, c: Rgb, a: f32) {
        if a <= 0.0 {
            return;
        }
        for p in &mut self.px {
            *p = p.lerp(c, a);
        }
    }

    pub fn text(&mut self, col: i32, row: i32, s: &str, fg: Rgb, bold: bool) {
        self.text_bg(col, row, s, fg, None, bold);
    }

    /// Draw text on the overlay layer. `bg = None` uses a darkened version of
    /// the pixels underneath so text stays readable on any background.
    pub fn text_bg(&mut self, col: i32, row: i32, s: &str, fg: Rgb, bg: Option<Rgb>, bold: bool) {
        if row < 0 || row >= self.rows as i32 {
            return;
        }
        for (i, ch) in s.chars().enumerate() {
            let c = col + i as i32;
            if c >= 0 && c < self.cols as i32 {
                self.over[row as usize * self.cols + c as usize] = Some(Over { ch, fg, bg, bold });
            }
        }
    }

    pub fn text_center(&mut self, row: i32, s: &str, fg: Rgb, bold: bool) -> i32 {
        let n = s.chars().count() as i32;
        let col = (self.cols as i32 - n) / 2;
        self.text(col, row, s, fg, bold);
        col
    }

    pub fn present(&mut self, out: &mut impl Write) -> io::Result<()> {
        let mut buf = std::mem::take(&mut self.buf);
        buf.clear();
        // Synchronized update: terminals that support it draw the frame atomically.
        buf.push_str("\x1b[?2026h");
        if self.needs_clear {
            buf.push_str("\x1b[0m\x1b[2J");
            self.needs_clear = false;
        }
        buf.push_str("\x1b[22m");

        let truecolor = self.truecolor;
        let cols = self.cols;
        let mut fg: Option<Rgb> = None;
        let mut bg: Option<Rgb> = None;
        let mut bold = false;
        let mut cursor: Option<(usize, usize)> = None;

        for row in 0..self.rows {
            for col in 0..cols {
                let i = row * cols + col;
                let top = self.px[(row * 2) * cols + col];
                let bot = self.px[(row * 2 + 1) * cols + col];
                let cell = match self.over[i] {
                    Some(o) => Cell {
                        ch: o.ch,
                        fg: o.fg,
                        bg: o.bg.unwrap_or_else(|| top.lerp(bot, 0.5).scale(0.45)),
                        bold: o.bold,
                    },
                    None if top == bot => Cell { ch: ' ', fg: top, bg: top, bold: false },
                    None => Cell { ch: '▀', fg: top, bg: bot, bold: false },
                };
                if self.prev[i] == Some(cell) {
                    continue;
                }
                self.prev[i] = Some(cell);

                if cursor != Some((col, row)) {
                    let _ = write!(buf, "\x1b[{};{}H", row + 1, col + 1);
                }
                if cell.bold != bold {
                    buf.push_str(if cell.bold { "\x1b[1m" } else { "\x1b[22m" });
                    bold = cell.bold;
                }
                if cell.ch != ' ' && fg != Some(cell.fg) {
                    push_color(&mut buf, cell.fg, true, truecolor);
                    fg = Some(cell.fg);
                }
                if bg != Some(cell.bg) {
                    push_color(&mut buf, cell.bg, false, truecolor);
                    bg = Some(cell.bg);
                }
                buf.push(cell.ch);
                cursor = Some((col + 1, row));
            }
        }

        buf.push_str("\x1b[?2026l");
        out.write_all(buf.as_bytes())?;
        out.flush()?;
        self.buf = buf;
        Ok(())
    }
}

fn push_color(buf: &mut String, c: Rgb, fg: bool, truecolor: bool) {
    let base = if fg { 38 } else { 48 };
    if truecolor {
        let _ = write!(buf, "\x1b[{};2;{};{};{}m", base, c.0, c.1, c.2);
    } else {
        let _ = write!(buf, "\x1b[{};5;{}m", base, ansi256(c));
    }
}

/// Map an RGB color to the nearest xterm-256 palette entry.
fn ansi256(c: Rgb) -> u8 {
    const LV: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let q = |v: u8| -> usize {
        let v = v as i32;
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            ((v - 35) / 40) as usize
        }
    };
    let (r, g, b) = (q(c.0), q(c.1), q(c.2));
    let dist = |a: (i32, i32, i32)| {
        let (dr, dg, db) = (a.0 - c.0 as i32, a.1 - c.1 as i32, a.2 - c.2 as i32);
        dr * dr + dg * dg + db * db
    };
    let cube = (LV[r], LV[g], LV[b]);
    let avg = (c.0 as i32 + c.1 as i32 + c.2 as i32) / 3;
    let gi = if avg > 238 { 23 } else { ((avg - 3).max(0) / 10).min(23) };
    let gv = 8 + 10 * gi;
    if dist((gv, gv, gv)) < dist(cube) {
        232 + gi as u8
    } else {
        (16 + 36 * r + 6 * g + b) as u8
    }
}
