//! BUMBLE RUSH - a buzzy little sky-dodging game for your terminal.

mod draw;
mod font;
mod game;
mod rng;
mod screen;

use std::env;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
    execute,
    style::ResetColor,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use game::Game;
use screen::Screen;

const FPS: f64 = 60.0;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("BUMBLE RUSH - guide a bee between the pillars!\n");
        println!("Usage: bumble-rush [--256 | --truecolor]\n");
        println!("  --256        force 256-color mode (use if colors look wrong)");
        println!("  --truecolor  force 24-bit color mode\n");
        println!("Controls: SPACE / W / UP / click = flap, P = pause, Q / Esc = quit");
        return Ok(());
    }
    let truecolor = detect_truecolor(&args);

    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, EnterAlternateScreen, Hide, EnableMouseCapture, EnableFocusChange, Clear(ClearType::All))?;

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    let result = run(&mut out, truecolor);
    restore_terminal();

    match result {
        Ok(best) => {
            println!("Thanks for playing BUMBLE RUSH! Best score: {best}");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn restore_terminal() {
    let mut out = io::stdout();
    let _ = execute!(out, ResetColor, DisableFocusChange, DisableMouseCapture, Show, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    let _ = out.flush();
}

/// Use 24-bit color only where the terminal says it supports it; otherwise fall
/// back to the 256-color palette (e.g. macOS Terminal.app before macOS 26).
fn detect_truecolor(args: &[String]) -> bool {
    if args.iter().any(|a| a == "--256") {
        return false;
    }
    if args.iter().any(|a| a == "--truecolor") {
        return true;
    }
    let colorterm = env::var("COLORTERM").unwrap_or_default().to_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return true;
    }
    let program = env::var("TERM_PROGRAM").unwrap_or_default();
    if matches!(
        program.as_str(),
        "iTerm.app" | "WezTerm" | "vscode" | "ghostty" | "Hyper" | "Tabby" | "rio" | "WarpTerminal"
    ) {
        return true;
    }
    let term = env::var("TERM").unwrap_or_default();
    ["kitty", "alacritty", "ghostty", "direct", "wezterm"].iter().any(|t| term.contains(t))
}

fn run(out: &mut impl Write, truecolor: bool) -> io::Result<u32> {
    let (cols, rows) = terminal::size()?;
    let mut screen = Screen::new(cols as usize, rows as usize, truecolor);
    let mut game = Game::new(screen.w, screen.h, game::load_best());

    let frame = Duration::from_secs_f64(1.0 / FPS);
    let mut next = Instant::now();

    'main: loop {
        // Handle input until it's time for the next frame.
        loop {
            let timeout = next.saturating_duration_since(Instant::now());
            if !event::poll(timeout)? {
                break;
            }
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => match k.code {
                    KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => break 'main,
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break 'main,
                    KeyCode::Char(' ')
                    | KeyCode::Up
                    | KeyCode::Enter
                    | KeyCode::Char('w')
                    | KeyCode::Char('W')
                    | KeyCode::Char('k') => game.flap(),
                    KeyCode::Char('p') | KeyCode::Char('P') => game.toggle_pause(),
                    _ => {}
                },
                Event::Mouse(m) if matches!(m.kind, MouseEventKind::Down(_)) => game.flap(),
                Event::FocusLost => {
                    if !game.paused {
                        game.toggle_pause();
                    }
                }
                Event::Resize(c, r) => {
                    screen.resize(c as usize, r as usize);
                    game.resize(screen.w, screen.h);
                }
                _ => {}
            }
        }

        if !draw::too_small(&screen) {
            game.update((1.0 / FPS) as f32);
        }
        draw::draw(&game, &mut screen);
        screen.present(out)?;

        next += frame;
        let now = Instant::now();
        if next + frame * 4 < now {
            // We fell far behind (e.g. the terminal stalled); don't try to catch up.
            next = now;
        }
    }

    Ok(game.best)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple autopilot: flap when below the next gap's center.
    fn autopilot(g: &Game) -> bool {
        let target = g
            .pillars
            .iter()
            .find(|p| p.x + p.w > g.bee.x)
            .map(|p| p.gap_y)
            .unwrap_or(g.h * 0.45);
        g.bee.y + 3.5 > target + 2.0 && g.bee.vy > 0.0
    }

    #[test]
    fn simulate_many_sizes() {
        for &(cols, rows, tc) in &[(40, 16, false), (80, 24, true), (120, 40, true), (220, 60, false), (30, 10, true)] {
            let mut screen = Screen::new(cols, rows, tc);
            let mut game = Game::new(screen.w, screen.h, 0);
            let mut sink: Vec<u8> = Vec::new();
            let mut bytes = 0usize;
            let mut max_score = 0;
            let mut deaths = 0;
            let frames = 60 * 120;
            for f in 0..frames {
                match game.state {
                    game::State::Title | game::State::Ready => game.flap(),
                    game::State::Playing => {
                        if autopilot(&game) {
                            game.flap();
                        }
                    }
                    game::State::GameOver => {
                        deaths += 1;
                        game.flap();
                    }
                    _ => {}
                }
                if f == 3000 {
                    game.toggle_pause();
                }
                if f == 3030 {
                    game.toggle_pause();
                }
                if f == 4000 {
                    screen.resize(cols + 7, rows + 3);
                    game.resize(screen.w, screen.h);
                }
                game.update(1.0 / 60.0);
                draw::draw(&game, &mut screen);
                sink.clear();
                screen.present(&mut sink).unwrap();
                bytes += sink.len();
                max_score = max_score.max(game.score);
            }
            eprintln!(
                "{cols}x{rows} truecolor={tc}: avg {:.1} KB/frame, best score {max_score}, game-overs {deaths}",
                bytes as f64 / frames as f64 / 1024.0
            );
        }
    }
}
