//! Everything visual: sky, parallax scenery, pillars, the bee, effects and UI.

use crate::font;
use crate::game::{Game, Pillar, Power, PowerUp, State, TIPS};
use crate::rng::hash;
use crate::screen::{Rgb, Screen};

pub const MIN_COLS: usize = 40;
pub const MIN_ROWS: usize = 16;

const WHITE: Rgb = Rgb(255, 255, 255);
const GOLD: Rgb = Rgb(255, 205, 50);
const SHADOW: Rgb = Rgb(30, 20, 40);
const SOFT: Rgb = Rgb(225, 225, 235);

// ---- sky palettes (day -> sunset -> night -> dawn) -------------------------

#[derive(Clone, Copy)]
struct Pal {
    top: Rgb,
    bot: Rgb,
    far: Rgb,
    near: Rgb,
    cloud: Rgb,
    orb: Rgb,
    orb_y: f32,
    night: f32,
}

const PALS: [Pal; 4] = [
    Pal {
        top: Rgb(70, 150, 235),
        bot: Rgb(175, 220, 250),
        far: Rgb(120, 160, 205),
        near: Rgb(80, 165, 90),
        cloud: Rgb(250, 250, 255),
        orb: Rgb(255, 235, 140),
        orb_y: 0.16,
        night: 0.0,
    },
    Pal {
        top: Rgb(95, 75, 160),
        bot: Rgb(255, 155, 105),
        far: Rgb(150, 95, 135),
        near: Rgb(85, 105, 75),
        cloud: Rgb(255, 195, 175),
        orb: Rgb(255, 140, 70),
        orb_y: 0.52,
        night: 0.15,
    },
    Pal {
        top: Rgb(8, 10, 32),
        bot: Rgb(35, 45, 95),
        far: Rgb(35, 40, 78),
        near: Rgb(22, 48, 42),
        cloud: Rgb(75, 85, 125),
        orb: Rgb(235, 240, 255),
        orb_y: 0.18,
        night: 1.0,
    },
    Pal {
        top: Rgb(80, 120, 200),
        bot: Rgb(255, 205, 170),
        far: Rgb(125, 125, 175),
        near: Rgb(65, 135, 85),
        cloud: Rgb(255, 232, 222),
        orb: Rgb(255, 200, 120),
        orb_y: 0.45,
        night: 0.25,
    },
];

fn palette(phase: f32) -> Pal {
    let p = phase.rem_euclid(4.0);
    let i = p.floor() as usize % 4;
    let f = p - p.floor();
    // Hold each palette for a while, then blend smoothly into the next.
    let t = ((f - 0.55) / 0.45).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    let (a, b) = (PALS[i], PALS[(i + 1) % 4]);
    Pal {
        top: a.top.lerp(b.top, t),
        bot: a.bot.lerp(b.bot, t),
        far: a.far.lerp(b.far, t),
        near: a.near.lerp(b.near, t),
        cloud: a.cloud.lerp(b.cloud, t),
        orb: a.orb.lerp(b.orb, t),
        orb_y: a.orb_y + (b.orb_y - a.orb_y) * t,
        night: a.night + (b.night - a.night) * t,
    }
}

// ---- entry point ------------------------------------------------------------

pub fn too_small(s: &Screen) -> bool {
    s.cols < MIN_COLS || s.rows < MIN_ROWS
}

pub fn draw(g: &Game, s: &mut Screen) {
    s.ox = 0;
    s.oy = 0;
    s.clear(Rgb(0, 0, 0));
    if too_small(s) {
        s.clear(Rgb(25, 20, 35));
        let r = s.rows as i32 / 2;
        s.text_center(r - 1, "Terminal too small!", GOLD, true);
        let msg = format!("need {}x{}, have {}x{}", MIN_COLS, MIN_ROWS, s.cols, s.rows);
        s.text_center(r, &msg, SOFT, false);
        s.text_center(r + 1, "(Q to quit)", SOFT, false);
        return;
    }

    let pal = palette(g.sky_phase);
    let light = 1.0 - 0.45 * pal.night;
    draw_sky(g, s, &pal);

    if g.shake > 0.0 {
        let k = g.shake * 8.0;
        s.ox = ((g.time * 91.0).sin() * k).round() as i32;
        s.oy = ((g.time * 73.0).cos() * k * 0.6).round() as i32;
    }

    draw_clouds(g, s, &pal);
    draw_mountains(g, s, &pal);
    draw_hills(g, s, &pal);
    for p in &g.pillars {
        draw_pillar(g, s, p, light);
    }
    draw_ground(g, s, light);
    for c in &g.coins {
        draw_coin(s, c.x, c.y, g.time);
    }
    for p in &g.powerups {
        draw_powerup(g, s, p);
    }
    draw_bee(g, s);
    for p in &g.particles {
        let a = (p.life / p.max).clamp(0.0, 1.0);
        s.blend(p.x.round() as i32, p.y.round() as i32, p.color, a);
    }

    s.ox = 0;
    s.oy = 0;
    if g.slow > 0.0 {
        s.tint(Rgb(120, 60, 200), 0.14 * (g.slow * 2.0).min(1.0));
    }
    if g.flash > 0.0 {
        s.tint(WHITE, g.flash * 0.7);
    }

    match g.state {
        State::Title => draw_title(g, s),
        State::Ready => draw_ready(g, s),
        State::Playing | State::Dying => draw_hud(g, s),
        State::GameOver => {
            draw_best(g, s);
            draw_game_over(g, s);
        }
    }
    draw_popups(g, s);
    if g.paused {
        draw_paused(s);
    }
}

// ---- background -------------------------------------------------------------

fn draw_sky(g: &Game, s: &mut Screen, pal: &Pal) {
    let gy = g.ground_y.max(1.0);
    for y in 0..s.h {
        let t = (y as f32 / gy).min(1.0);
        s.hline_raw(y, pal.top.lerp(pal.bot, t));
    }
    if pal.night > 0.02 {
        for st in &g.stars {
            let x = (st.x - g.scroll * 0.01).rem_euclid(g.w);
            let tw = 0.55 + 0.45 * (g.time * 2.3 + st.phase).sin();
            s.blend(x as i32, st.y as i32, Rgb(255, 255, 235), pal.night * tw);
        }
    }
    // Sun / moon.
    let r = (g.h * 0.06).clamp(2.5, 7.0);
    let (cx, cy) = (g.w * 0.78, g.h * pal.orb_y);
    s.blend_circle(cx, cy, r * 2.3, pal.orb, 0.10);
    s.blend_circle(cx, cy, r * 1.5, pal.orb, 0.18);
    s.fill_circle(cx, cy, r, pal.orb);
    if pal.night > 0.6 {
        let sky = pal.top.lerp(pal.bot, cy / gy);
        s.blend_circle(cx + r * 0.5, cy - r * 0.3, r * 0.8, sky, (pal.night - 0.6) / 0.4);
    }
}

fn puff(s: &mut Screen, x: f32, y: f32, r: f32, c: Rgb) {
    s.fill_circle(x, y, r, c);
    s.fill_circle(x + r * 0.95, y + r * 0.3, r * 0.75, c);
    s.fill_circle(x - r * 0.95, y + r * 0.35, r * 0.65, c);
    s.fill_rect(
        (x - r * 1.5) as i32,
        (y + r * 0.3) as i32,
        (r * 3.1) as i32,
        (r * 0.55).max(1.0) as i32,
        c,
    );
}

fn draw_clouds(g: &Game, s: &mut Screen, pal: &Pal) {
    let span = g.w + 60.0;
    for c in &g.clouds {
        let x = (c.x - g.scroll * c.depth).rem_euclid(span) - 30.0;
        puff(s, x, c.y + 1.0, c.r, pal.cloud.lerp(pal.top, 0.3));
        puff(s, x, c.y, c.r, pal.cloud);
    }
}

fn draw_mountains(g: &Game, s: &mut Screen, pal: &Pal) {
    let gy = g.ground_y as i32;
    let h = g.h;
    for x in 0..s.w {
        let wx = x as f32 + g.scroll * 0.12;
        let hgt = (h * 0.17
            + (wx * 0.045).sin() * h * 0.05
            + (wx * 0.11 + 1.7).sin() * h * 0.022
            + (wx * 0.019 + 0.3).sin() * h * 0.05)
            .max(1.0);
        let top = gy - hgt as i32;
        for y in top..gy {
            let t = (y - top) as f32 / hgt;
            let mut c = pal.far.lerp(pal.bot, 0.35 * (1.0 - t));
            if hgt > h * 0.25 && y - top < 2 {
                c = c.lerp(WHITE, 0.6 * (1.0 - pal.night * 0.5));
            }
            s.set(x, y, c);
        }
    }
}

fn hill_height(h: f32, wx: f32) -> f32 {
    h * 0.08 + (wx * 0.06).sin() * h * 0.03 + (wx * 0.15 + 2.0).sin() * h * 0.012
}

fn draw_hills(g: &Game, s: &mut Screen, pal: &Pal) {
    let gy = g.ground_y as i32;
    let off = g.scroll * 0.4;
    for x in 0..s.w {
        let hgt = hill_height(g.h, x as f32 + off).max(1.0);
        let top = gy - hgt as i32;
        for y in top..gy {
            let c = if y == top {
                pal.near.scale(1.18)
            } else {
                let t = (y - top) as f32 / hgt;
                pal.near.lerp(pal.near.scale(0.7), t)
            };
            s.set(x, y, c);
        }
    }
    // Little round trees.
    let tile = 11.0;
    let start = (off / tile).floor() as i64 - 1;
    let count = (g.w / tile) as i64 + 3;
    for k in start..start + count {
        let hs = hash(k * 31 + 7);
        if hs % 3 != 0 {
            continue;
        }
        let wx = k as f32 * tile + ((hs >> 8) % 7) as f32;
        let sx = wx - off;
        let base = gy as f32 - hill_height(g.h, wx);
        let r = 1.4 + ((hs >> 12) % 3) as f32 * 0.6;
        s.fill_rect(sx as i32, (base - r) as i32, 1, r as i32 + 2, pal.near.scale(0.45));
        s.fill_circle(sx + 0.5, base - r * 1.4, r, pal.near.scale(0.8));
        s.set(sx as i32 - 1, (base - r * 1.9) as i32, pal.near.scale(1.05));
    }
}

fn draw_ground(g: &Game, s: &mut Screen, light: f32) {
    let gy = g.ground_y as i32;
    let scroll = g.scroll.floor() as i32;
    for y in gy..s.h + 3 {
        let d = y - gy;
        for x in -3..s.w + 3 {
            let wx = x + scroll;
            let c = match d {
                0 => Rgb(125, 205, 85),
                1 => {
                    if (wx + y).rem_euclid(6) < 3 {
                        Rgb(85, 165, 60)
                    } else {
                        Rgb(68, 140, 48)
                    }
                }
                _ => {
                    let base = Rgb(155, 110, 62).lerp(Rgb(105, 70, 38), (d as f32 / 8.0).min(1.0));
                    if hash(wx as i64 * 7919 + y as i64) % 9 == 0 {
                        base.scale(0.78)
                    } else {
                        base
                    }
                }
            };
            s.set(x, y, c.scale(light));
        }
    }
    // Flowers along the grass.
    const PETALS: [Rgb; 4] = [Rgb(255, 120, 170), Rgb(255, 255, 255), Rgb(190, 130, 255), Rgb(255, 170, 60)];
    let tile = 7;
    let start = scroll.div_euclid(tile) - 1;
    for k in start..start + s.w / tile + 3 {
        let hs = hash(k as i64 * 13 + 5);
        if hs % 3 != 0 {
            continue;
        }
        let fx = k * tile + ((hs >> 4) % 5) as i32 - scroll;
        s.set(fx, gy - 1, Rgb(60, 150, 50).scale(light));
        s.set(fx, gy - 2, PETALS[((hs >> 9) % 4) as usize].scale(light));
    }
}

// ---- world objects ----------------------------------------------------------

fn draw_pillar(g: &Game, s: &mut Screen, p: &Pillar, light: f32) {
    let x0 = p.x.round() as i32;
    let w = p.w as i32;
    let top = p.gap_top().round() as i32;
    let bot = p.gap_bot().round() as i32;
    let gy = g.ground_y as i32;
    let moving = p.moving();
    let (hi, mid, lo) = if moving {
        (Rgb(215, 170, 255), Rgb(150, 95, 215), Rgb(80, 45, 130))
    } else {
        (Rgb(215, 200, 170), Rgb(160, 145, 118), Rgb(90, 80, 64))
    };
    let shade = |i: i32| -> Rgb {
        if i == 0 {
            hi
        } else if i == w - 1 {
            lo
        } else if i == 1 {
            mid.lerp(hi, 0.5)
        } else {
            mid.lerp(lo, i as f32 / (w - 1) as f32 * 0.7)
        }
    };

    let body = (0..top - 2).chain(bot + 2..gy);
    for y in body {
        for i in 0..w {
            let mut c = shade(i);
            if moving {
                let sheen = (y as f32 * 0.35 - g.time * 3.0 + i as f32 * 0.5).sin();
                if sheen > 0.93 {
                    c = c.lerp(WHITE, 0.5);
                }
            } else {
                let brick = y.div_euclid(4);
                let joint = (i + (brick % 2) * (w / 2)) % w == 0;
                if y.rem_euclid(4) == 0 || (joint && i > 0) {
                    c = c.scale(0.8);
                }
            }
            s.set(x0 + i, y, c.scale(light));
        }
    }

    // Caps.
    let cap = if moving { Rgb(235, 205, 255) } else { Rgb(225, 212, 185) };
    let glow = 0.3 + 0.3 * (g.time * 6.0).sin();
    for (cy0, edge) in [(top - 2, top - 1), (bot, bot)] {
        for y in cy0..cy0 + 2 {
            for i in -1..=w {
                let mut c = if i == -1 {
                    hi
                } else if i == w {
                    lo
                } else {
                    cap.lerp(lo, i as f32 / w as f32 * 0.5)
                };
                if y == edge {
                    c = if moving { c.lerp(WHITE, glow) } else { Rgb(95, 175, 70) };
                }
                s.set(x0 + i, y, c.scale(light));
            }
        }
    }
    // Dangling moss under the top cap.
    if !moving {
        for i in 0..w {
            if hash(p.id as i64 * 31 + i as i64) % 3 == 0 {
                s.set(x0 + i, top, Rgb(80, 155, 60).scale(light));
            }
        }
    }
}

fn draw_coin(s: &mut Screen, x: f32, y: f32, t: f32) {
    let spin = (t * 5.0 + x * 0.13).sin().abs().max(0.2);
    let (cx, cy) = (x.round() as i32, y.round() as i32);
    for dy in -2..=2i32 {
        let half = if dy.abs() == 2 { 1.0 } else { 2.0 } * spin;
        let hw = half.round() as i32;
        for dx in -hw..=hw {
            let edge = dx.abs() == hw || dy.abs() == 2;
            let c = if edge { Rgb(205, 135, 20) } else { Rgb(255, 205, 55) };
            s.set(cx + dx, cy + dy, c);
        }
    }
    if spin > 0.5 {
        s.set(cx - 1, cy - 1, Rgb(255, 250, 210));
    }
}

fn draw_powerup(g: &Game, s: &mut Screen, p: &PowerUp) {
    let y = p.draw_y();
    let c = p.kind.color();
    let pulse = 0.5 + 0.5 * (g.time * 5.0).sin();
    s.blend_circle(p.x, y, 6.0, c, 0.12 + 0.18 * pulse);
    s.fill_circle(p.x, y, 3.3, c.scale(0.7));
    s.fill_circle(p.x - 0.4, y - 0.4, 2.5, c);
    s.set(p.x as i32 - 1, y as i32 - 2, WHITE);
    let col = p.x.floor() as i32;
    let row = y.floor() as i32 / 2;
    s.text_bg(col, row, p.kind.letter(), WHITE, Some(c.scale(0.8)), true);
}

const BEE_UP: [&str; 6] = ["...ww....", "..wWWw.K.", ".YKYKYYY.", "SYKYKYYPY", ".YKYKYYY.", "..y...y.."];
const BEE_DN: [&str; 6] = [".........", ".......K.", ".YKwWwYY.", "SYKYKYYPY", ".YKYKYYY.", "..y...y.."];

fn bee_color(ch: u8) -> Option<Rgb> {
    Some(match ch {
        b'Y' => Rgb(255, 205, 40),
        b'K' => Rgb(45, 32, 22),
        b'S' => Rgb(130, 65, 20),
        b'P' => Rgb(15, 15, 15),
        b'y' => Rgb(95, 62, 22),
        b'w' => Rgb(240, 248, 255),
        b'W' => Rgb(175, 215, 245),
        _ => return None,
    })
}

fn draw_bee(g: &Game, s: &mut Screen) {
    let (bx, by) = (g.bee.x, g.bee.y);
    let alive = !matches!(g.state, State::Dying | State::GameOver);

    // Motion trail.
    if g.state == State::Playing {
        let step = g.speed() / 60.0;
        for (i, ty) in g.trail.iter().enumerate().skip(1) {
            let a = 0.45 * (1.0 - i as f32 / 10.0);
            let tx = bx + 1.0 - i as f32 * step;
            s.blend(tx.round() as i32, (ty + 3.5).round() as i32, Rgb(255, 240, 170), a);
        }
    }

    let blink = g.invuln > 0.0 && ((g.time * 14.0) as i32) % 2 == 0;
    if !blink {
        let frame = if alive && ((g.time * 18.0) as i32) % 2 == 0 { &BEE_UP } else { &BEE_DN };
        let (x0, y0) = (bx.round() as i32, by.round() as i32);
        for (r, line) in frame.iter().enumerate() {
            let row = if alive { r } else { 5 - r };
            for (c, ch) in line.bytes().enumerate() {
                if let Some(col) = bee_color(ch) {
                    let col = if !alive && ch == b'P' { Rgb(200, 40, 40) } else { col };
                    s.set(x0 + c as i32, y0 + row as i32, col);
                }
            }
        }
    }

    if g.shield {
        let (cx, cy) = (bx + 4.5, by + 3.5);
        let r = 6.5;
        let c = Power::Shield.color().lerp(WHITE, 0.3);
        let shimmer = g.time * 4.0;
        for y in (cy - r - 1.0) as i32..=(cy + r + 1.0) as i32 {
            for x in (cx - r - 1.0) as i32..=(cx + r + 1.0) as i32 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt();
                if (d - r).abs() < 0.6 {
                    let a = 0.55 + 0.35 * (dy.atan2(dx) * 2.0 + shimmer).sin();
                    s.blend(x, y, c, a);
                } else if d < r {
                    s.blend(x, y, c, 0.1);
                }
            }
        }
    }
}

// ---- UI -----------------------------------------------------------------------

fn spans_center(s: &mut Screen, row: i32, spans: &[(&str, Rgb, Option<Rgb>)]) {
    let total: usize = spans.iter().map(|(t, _, _)| t.chars().count()).sum();
    let mut col = (s.cols as i32 - total as i32) / 2;
    for (t, fg, bg) in spans {
        s.text_bg(col, row, t, *fg, *bg, bg.is_some());
        col += t.chars().count() as i32;
    }
}

fn blink(g: &Game, hz: f32) -> bool {
    ((g.time * hz) as i32) % 2 == 0
}

fn draw_best(g: &Game, s: &mut Screen) {
    s.text(1, 0, &format!(" BEST {} ", g.best), Rgb(255, 230, 150), true);
}

fn draw_title(g: &Game, s: &mut Screen) {
    let scale = if s.w >= 100 && s.h >= 44 { 2 } else { 1 };
    let w1 = font::width("BUMBLE", scale);
    let w2 = font::width("RUSH", scale);
    let gap = 4 * scale;
    let x = (s.w - (w1 + gap + w2)) / 2;
    let bob = ((g.time * 2.0).sin() * 1.5).round() as i32;
    let y = ((g.h * 0.08) as i32).max(2) + bob;
    font::draw(s, x, y, "BUMBLE", scale, Rgb(255, 210, 50), Some(Rgb(90, 50, 10)));
    font::draw(s, x + w1 + gap, y, "RUSH", scale, Rgb(255, 120, 50), Some(Rgb(90, 30, 10)));
    let sub_row = (y + 5 * scale) / 2 + 2;
    s.text_center(sub_row, " ~ a buzzy little sky adventure ~ ", Rgb(255, 245, 220), false);

    let rows = s.rows as i32;
    let sh = Power::Shield.color();
    let sl = Power::Slow.color();
    let mg = Power::Magnet.color();
    let db = Power::Double.color();
    if s.cols >= 62 {
        spans_center(
            s,
            rows - 5,
            &[
                (" o", GOLD, None),
                (" honey +1   ", WHITE, None),
                ("S", WHITE, Some(sh)),
                (" shield  ", WHITE, None),
                ("T", WHITE, Some(sl)),
                (" slow-mo  ", WHITE, None),
                ("M", WHITE, Some(mg)),
                (" magnet  ", WHITE, None),
                ("2", WHITE, Some(db)),
                (" double ", WHITE, None),
            ],
        );
    } else {
        spans_center(
            s,
            rows - 5,
            &[
                (" o", GOLD, None),
                (" +1 ", WHITE, None),
                ("S", WHITE, Some(sh)),
                (" shld ", WHITE, None),
                ("T", WHITE, Some(sl)),
                (" slow ", WHITE, None),
                ("M", WHITE, Some(mg)),
                (" mag ", WHITE, None),
                ("2", WHITE, Some(db)),
                (" x2 ", WHITE, None),
            ],
        );
    }
    if blink(g, 2.0) {
        s.text_center(rows - 3, " Press SPACE to start buzzing! ", WHITE, true);
    }
    let controls = if s.cols >= 60 {
        " SPACE / W / UP / click = flap    P = pause    Q = quit "
    } else {
        " SPACE flap   P pause   Q quit "
    };
    s.text_center(rows - 1, controls, SOFT, false);
    draw_best(g, s);
}

fn draw_ready(g: &Game, s: &mut Screen) {
    let scale = if s.w >= 80 && s.h >= 44 { 2 } else { 1 };
    let text = "GET READY";
    let x = (s.w - font::width(text, scale)) / 2;
    let y = ((g.h * 0.1) as i32).max(2);
    font::draw(s, x, y, text, scale, Rgb(120, 230, 255), Some(SHADOW));
    let rows = s.rows as i32;
    let tip = TIPS[g.tip % TIPS.len()];
    if tip.len() + 2 <= s.cols {
        s.text_center(rows - 5, &format!(" {tip} "), Rgb(255, 240, 200), false);
    }
    if blink(g, 2.0) {
        s.text_center(rows - 3, " Press SPACE to fly! ", WHITE, true);
        let col = (g.bee.x + 11.0) as i32;
        let row = ((g.bee.y + 3.0) / 2.0) as i32;
        s.text(col, row, "< tap!", GOLD, true);
    }
    draw_best(g, s);
}

fn draw_hud(g: &Game, s: &mut Screen) {
    let scale = if s.h >= 44 && s.cols >= 50 { 2 } else { 1 };
    let txt = g.score.to_string();
    let x = (s.w - font::width(&txt, scale)) / 2;
    let color = if g.double > 0.0 { Power::Double.color().lerp(WHITE, 0.4) } else { WHITE };
    font::draw(s, x, 2, &txt, scale, color, Some(SHADOW));
    draw_best(g, s);

    let mut items: Vec<(String, Rgb, f32)> = Vec::new();
    if g.shield {
        items.push(("SHIELD".into(), Power::Shield.color(), 99.0));
    }
    if g.slow > 0.0 {
        items.push((format!("SLOW {:.1}", g.slow), Power::Slow.color(), g.slow));
    }
    if g.magnet > 0.0 {
        items.push((format!("MAGNET {:.1}", g.magnet), Power::Magnet.color(), g.magnet));
    }
    if g.double > 0.0 {
        items.push((format!("x2 {:.1}", g.double), Power::Double.color(), g.double));
    }
    for (row, (label, c, left)) in items.iter().enumerate() {
        if *left < 1.5 && blink(g, 8.0) {
            continue;
        }
        let label = format!(" {label} ");
        let col = s.cols as i32 - label.len() as i32 - 1;
        s.text_bg(col, row as i32, &label, WHITE, Some(c.scale(0.7)), true);
    }
}

fn draw_popups(g: &Game, s: &mut Screen) {
    for p in &g.popups {
        if p.big {
            // Sits just under the score so it never hides the pillars for long.
            let score_scale = if s.h >= 44 && s.cols >= 50 { 2 } else { 1 };
            let y = 2 + 5 * score_scale + 3;
            let scale = if s.h >= 80 && font::width(&p.text, 2) < s.w - 4 { 2 } else { 1 };
            let wpx = font::width(&p.text, scale);
            if wpx < s.w - 2 {
                if p.life < 0.4 && blink(g, 10.0) {
                    continue;
                }
                font::draw(s, (s.w - wpx) / 2, y, &p.text, scale, p.color, Some(SHADOW));
                continue;
            }
            s.text_center(y / 2, &p.text, p.color, true);
        } else {
            let fade = (p.life / p.max).clamp(0.0, 1.0);
            let n = p.text.chars().count() as i32;
            let col = (p.x as i32 - n / 2).clamp(0, (s.cols as i32 - n).max(0));
            s.text(col, p.y as i32 / 2, &p.text, p.color.lerp(WHITE, 1.0 - fade), true);
        }
    }
}

fn draw_paused(s: &mut Screen) {
    s.tint(Rgb(0, 0, 0), 0.45);
    let x = (s.w - font::width("PAUSED", 2)) / 2;
    let y = s.h / 2 - 8;
    font::draw(s, x, y, "PAUSED", 2, WHITE, Some(SHADOW));
    s.text_center(s.rows as i32 / 2 + 2, " P or SPACE to resume    Q to quit ", SOFT, false);
}

fn draw_game_over(g: &Game, s: &mut Screen) {
    let cols = s.cols as i32;
    let rows = s.rows as i32;
    let pw = 42.min(cols - 2);
    let ph = 14.min(rows - 2);
    let c0 = (cols - pw) / 2;
    let anim = (g.state_t * 3.0).min(1.0);
    let ease = 1.0 - (1.0 - anim).powi(3);
    let r0 = (rows - ph) / 2 + ((1.0 - ease) * rows as f32) as i32;

    s.blend_rect(c0, r0 * 2, pw, ph * 2, Rgb(22, 16, 34), 0.85);
    let border = Rgb(255, 200, 60);
    for x in c0..c0 + pw {
        s.set(x, r0 * 2, border);
        s.set(x, (r0 + ph) * 2 - 1, border);
    }
    for y in r0 * 2..(r0 + ph) * 2 {
        s.set(c0, y, border);
        s.set(c0 + pw - 1, y, border);
    }

    let title = "GAME OVER";
    font::draw(s, (s.w - font::width(title, 1)) / 2, r0 * 2 + 2, title, 1, Rgb(255, 95, 75), Some(Rgb(70, 10, 10)));

    let bg = Some(Rgb(22, 16, 34));
    let center = |s: &mut Screen, row: i32, t: &str, c: Rgb, bold: bool| {
        let n = t.chars().count() as i32;
        s.text_bg((cols - n) / 2, row, t, c, bg, bold);
    };
    center(s, r0 + 5, &format!("SCORE  {:>5}", g.score), WHITE, true);
    let best = format!("BEST   {:>5}", g.best);
    center(s, r0 + 6, &best, Rgb(255, 230, 150), false);
    if g.new_best && blink(g, 3.0) {
        let col = (cols + best.len() as i32) / 2 + 1;
        s.text_bg(col, r0 + 6, "NEW!", GOLD, bg, true);
    }
    let stats = format!("pillars {}  honey {}  close calls {}", g.passed, g.honey, g.close_calls);
    center(s, r0 + 8, &stats, Rgb(180, 180, 200), false);

    let medal = match g.score {
        100.. => Some(("DIAMOND", Rgb(120, 230, 255))),
        50.. => Some(("GOLD", Rgb(255, 205, 50))),
        25.. => Some(("SILVER", Rgb(205, 210, 220))),
        10.. => Some(("BRONZE", Rgb(205, 127, 50))),
        _ => None,
    };
    match medal {
        Some((name, c)) => {
            let text = format!("MEDAL: {name}");
            let n = text.len() as i32;
            let start = (cols - n) / 2;
            center(s, r0 + 10, &text, c, true);
            let (mx, my) = ((start - 4) as f32, ((r0 + 10) * 2) as f32 + 1.0);
            s.fill_rect(mx as i32 - 1, my as i32 - 3, 1, 2, Rgb(220, 60, 60));
            s.fill_rect(mx as i32 + 1, my as i32 - 3, 1, 2, Rgb(60, 90, 220));
            s.fill_circle(mx + 0.5, my + 0.5, 2.3, c);
            s.set(mx as i32 - 1, my as i32 - 1, WHITE);
        }
        None => center(s, r0 + 10, "no medal yet - bronze at 10!", Rgb(160, 160, 180), false),
    }
    if g.state_t > 0.6 && blink(g, 2.0) {
        center(s, r0 + 12, "SPACE: try again    Q: quit", WHITE, true);
    }
}
