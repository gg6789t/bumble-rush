//! Game state and rules. All positions are in screen pixels; physics constants
//! scale with the screen height so the game feels the same in any terminal size.

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use crate::rng::Rng;
use crate::screen::Rgb;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Title,
    Ready,
    Playing,
    Dying,
    GameOver,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Power {
    Shield,
    Slow,
    Magnet,
    Double,
}

impl Power {
    pub fn letter(self) -> &'static str {
        match self {
            Power::Shield => "S",
            Power::Slow => "T",
            Power::Magnet => "M",
            Power::Double => "2",
        }
    }

    pub fn color(self) -> Rgb {
        match self {
            Power::Shield => Rgb(70, 170, 255),
            Power::Slow => Rgb(175, 105, 255),
            Power::Magnet => Rgb(255, 75, 95),
            Power::Double => Rgb(60, 205, 100),
        }
    }

    fn announce(self) -> &'static str {
        match self {
            Power::Shield => "SHIELD UP!",
            Power::Slow => "SLOW-MO!",
            Power::Magnet => "HONEY MAGNET!",
            Power::Double => "DOUBLE POINTS!",
        }
    }
}

pub const TIPS: [&str; 7] = [
    "Tip: graze a pillar's edge for a CLOSE CALL bonus!",
    "Tip: purple crystal pillars move. Learn their rhythm.",
    "Tip: a SHIELD saves you from one crash - even the ground!",
    "Tip: the MAGNET pulls honey drops right to you.",
    "Tip: keep going and the sky turns to sunset, then night...",
    "Tip: short calm taps beat long panicky ones.",
    "Tip: SLOW-MO slows the world, not you. Use it wisely!",
];

pub struct Bee {
    pub x: f32,
    pub y: f32,
    pub vy: f32,
    pub landed: bool,
}

pub struct Pillar {
    pub id: u32,
    pub x: f32,
    pub w: f32,
    pub gap: f32,
    pub gap_y: f32,
    pub base_y: f32,
    pub amp: f32,
    pub phase: f32,
    pub freq: f32,
    pub scored: bool,
    /// Smallest distance between the bee and the gap edges while passing.
    pub min_clear: f32,
}

impl Pillar {
    pub fn gap_top(&self) -> f32 {
        self.gap_y - self.gap * 0.5
    }
    pub fn gap_bot(&self) -> f32 {
        self.gap_y + self.gap * 0.5
    }
    pub fn moving(&self) -> bool {
        self.amp > 0.0
    }
}

pub struct Coin {
    pub x: f32,
    pub y: f32,
    /// Coins sitting in a moving pillar's gap ride along with it.
    pub pillar: Option<u32>,
}

pub struct PowerUp {
    pub x: f32,
    pub y: f32,
    pub kind: Power,
    pub phase: f32,
}

impl PowerUp {
    pub fn draw_y(&self) -> f32 {
        self.y + (self.phase * 3.0).sin() * 2.0
    }
}

pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max: f32,
    pub color: Rgb,
    pub grav: f32,
}

pub struct Popup {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub color: Rgb,
    pub life: f32,
    pub max: f32,
    pub big: bool,
}

pub struct Cloud {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub depth: f32,
}

pub struct Star {
    pub x: f32,
    pub y: f32,
    pub phase: f32,
}

pub struct Game {
    pub w: f32,
    pub h: f32,
    pub ground_y: f32,
    rng: Rng,

    pub state: State,
    pub state_t: f32,
    pub paused: bool,
    pub time: f32,

    pub bee: Bee,
    pub trail: VecDeque<f32>,
    pub pillars: Vec<Pillar>,
    pub coins: Vec<Coin>,
    pub powerups: Vec<PowerUp>,
    pub particles: Vec<Particle>,
    pub popups: Vec<Popup>,
    pub clouds: Vec<Cloud>,
    pub stars: Vec<Star>,

    pub score: u32,
    pub best: u32,
    pub new_best: bool,
    pub passed: u32,
    pub honey: u32,
    pub close_calls: u32,

    pub scroll: f32,
    last_gap: f32,
    next_id: u32,
    spawned: u32,
    since_power: u32,
    power_due: u32,

    pub shield: bool,
    pub invuln: f32,
    pub slow: f32,
    pub magnet: f32,
    pub double: f32,

    pub shake: f32,
    pub flash: f32,
    pub sky_phase: f32,
    pub tip: usize,
}

impl Game {
    pub fn new(w: i32, h: i32, best: u32) -> Self {
        let mut g = Game {
            w: 0.0,
            h: 0.0,
            ground_y: 0.0,
            rng: Rng::seeded(),
            state: State::Title,
            state_t: 0.0,
            paused: false,
            time: 0.0,
            bee: Bee { x: 0.0, y: 0.0, vy: 0.0, landed: false },
            trail: VecDeque::new(),
            pillars: Vec::new(),
            coins: Vec::new(),
            powerups: Vec::new(),
            particles: Vec::new(),
            popups: Vec::new(),
            clouds: Vec::new(),
            stars: Vec::new(),
            score: 0,
            best,
            new_best: false,
            passed: 0,
            honey: 0,
            close_calls: 0,
            scroll: 0.0,
            last_gap: 0.0,
            next_id: 0,
            spawned: 0,
            since_power: 0,
            power_due: 5,
            shield: false,
            invuln: 0.0,
            slow: 0.0,
            magnet: 0.0,
            double: 0.0,
            shake: 0.0,
            flash: 0.0,
            sky_phase: 0.0,
            tip: 0,
        };
        g.resize(w, h);
        g.tip = g.rng.below(TIPS.len());
        g
    }

    pub fn resize(&mut self, w: i32, h: i32) {
        self.w = w as f32;
        self.h = h as f32;
        self.ground_y = (self.h - (self.h * 0.09).max(4.0)).floor();
        self.bee.x = (self.w * 0.25).floor();
        self.gen_scenery();
        if matches!(self.state, State::Playing | State::Dying) {
            // The world no longer fits; start a fresh round.
            self.reset_round();
            self.state = State::Ready;
            self.state_t = 0.0;
        }
        if self.state == State::GameOver {
            self.bee.y = self.ground_y - 6.0;
        } else {
            self.bee.y = self.hover_y();
        }
    }

    fn gen_scenery(&mut self) {
        let (w, h, gy) = (self.w, self.h, self.ground_y);
        self.clouds.clear();
        let n = ((w / 22.0) as usize).max(3);
        for _ in 0..n {
            let c = Cloud {
                x: self.rng.range(0.0, w + 60.0),
                y: self.rng.range(h * 0.05, h * 0.42),
                r: self.rng.range(2.5, 2.5 + h * 0.05),
                depth: self.rng.range(0.04, 0.18),
            };
            self.clouds.push(c);
        }
        self.clouds.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        self.stars.clear();
        let n = ((w * gy) / 70.0) as usize;
        for _ in 0..n {
            let s = Star {
                x: self.rng.range(0.0, w),
                y: self.rng.range(0.0, gy * 0.7),
                phase: self.rng.range(0.0, std::f32::consts::TAU),
            };
            self.stars.push(s);
        }
    }

    // ---- tuning -----------------------------------------------------------

    fn hover_y(&self) -> f32 {
        self.h * 0.45
    }
    fn gravity(&self) -> f32 {
        self.h * 2.9
    }
    fn flap_v(&self) -> f32 {
        self.h * 0.85
    }
    fn max_fall(&self) -> f32 {
        self.h * 1.1
    }
    /// 0 at the start, 1 when fully ramped up.
    pub fn difficulty(&self) -> f32 {
        (self.passed as f32 / 45.0).min(1.0)
    }
    pub fn speed(&self) -> f32 {
        self.h * (0.5 + 0.32 * self.difficulty())
    }
    fn spacing(&self) -> f32 {
        (self.h * (1.05 - 0.15 * self.difficulty())).max(36.0)
    }

    /// The bee's (forgiving) hitbox: x0, y0, x1, y1.
    pub fn bee_box(&self) -> (f32, f32, f32, f32) {
        (self.bee.x + 1.0, self.bee.y + 2.0, self.bee.x + 8.0, self.bee.y + 5.0)
    }
    fn bee_center(&self) -> (f32, f32) {
        (self.bee.x + 4.5, self.bee.y + 3.5)
    }

    // ---- input ------------------------------------------------------------

    pub fn flap(&mut self) {
        if self.paused {
            self.paused = false;
            return;
        }
        match self.state {
            State::Title | State::Ready => {
                self.start();
                self.do_flap();
            }
            State::Playing => self.do_flap(),
            State::GameOver if self.state_t > 0.6 => {
                self.reset_round();
                self.state = State::Ready;
                self.state_t = 0.0;
            }
            _ => {}
        }
    }

    pub fn toggle_pause(&mut self) {
        if self.state == State::Playing || self.paused {
            self.paused = !self.paused;
        }
    }

    fn do_flap(&mut self) {
        self.bee.vy = -self.flap_v();
        let (x, y) = (self.bee.x + 1.0, self.bee.y + 4.0);
        for _ in 0..4 {
            let vx = -self.rng.range(10.0, 30.0);
            let vy = self.rng.range(5.0, 25.0);
            self.particles.push(Particle {
                x,
                y,
                vx,
                vy,
                life: 0.35,
                max: 0.35,
                color: Rgb(235, 245, 255),
                grav: 0.0,
            });
        }
    }

    // ---- round management -------------------------------------------------

    fn reset_round(&mut self) {
        self.pillars.clear();
        self.coins.clear();
        self.powerups.clear();
        self.popups.clear();
        self.score = 0;
        self.passed = 0;
        self.honey = 0;
        self.close_calls = 0;
        self.new_best = false;
        self.shield = false;
        self.invuln = 0.0;
        self.slow = 0.0;
        self.magnet = 0.0;
        self.double = 0.0;
        self.last_gap = self.ground_y * 0.5;
        self.spawned = 0;
        self.since_power = 0;
        self.power_due = 5;
        self.sky_phase = 0.0;
        self.bee.vy = 0.0;
        self.bee.landed = false;
        self.trail.clear();
        self.tip = self.rng.below(TIPS.len());
    }

    fn start(&mut self) {
        self.reset_round();
        self.state = State::Playing;
        self.state_t = 0.0;
        let spacing = self.spacing();
        let mut x = self.bee.x + (self.h * 1.2).max(30.0);
        while x < self.w + 2.0 {
            self.spawn_pillar(x);
            x += spacing;
        }
        if self.pillars.is_empty() {
            self.spawn_pillar(self.w + 1.0);
        }
    }

    fn spawn_pillar(&mut self, x: f32) {
        let d = self.difficulty();
        let h = self.h;
        let gap = (h * (0.36 - 0.11 * d)).max(15.0).min(self.ground_y * 0.6);
        let margin = h * 0.06 + 1.0;
        let lo = gap * 0.5 + margin;
        let hi = self.ground_y - gap * 0.5 - margin;
        let shift = h * (0.26 + 0.22 * d);
        let a = lo.max(self.last_gap - shift);
        let b = hi.min(self.last_gap + shift);
        let c = if a < b { self.rng.range(a, b) } else { (lo + hi) * 0.5 };
        self.last_gap = c;

        let moving = self.spawned >= 7 && self.rng.chance(0.12 + 0.38 * d);
        let mut amp = if moving { self.rng.range(h * 0.05, h * 0.11) } else { 0.0 };
        amp = amp.min(c - lo).min(hi - c).max(0.0);

        let id = self.next_id;
        self.next_id += 1;
        let w = (h * 0.14).clamp(6.0, 14.0).round();
        let phase = self.rng.range(0.0, std::f32::consts::TAU);
        let freq = self.rng.range(1.2, 2.2);
        self.pillars.push(Pillar {
            id,
            x,
            w,
            gap,
            gap_y: c + phase.sin() * amp,
            base_y: c,
            amp,
            phase,
            freq,
            scored: false,
            min_clear: f32::MAX,
        });
        self.spawned += 1;

        let spacing = self.spacing();
        // Power-up between this pillar and the next.
        self.since_power += 1;
        let mut spawned_power = false;
        if self.spawned >= 4 && self.since_power >= self.power_due {
            let mut options = vec![Power::Slow, Power::Magnet, Power::Double];
            if !self.shield {
                options.push(Power::Shield);
                options.push(Power::Shield);
            }
            let kind = options[self.rng.below(options.len())];
            self.powerups.push(PowerUp { x: x + w + spacing * 0.5, y: c, kind, phase: 0.0 });
            self.since_power = 0;
            self.power_due = 7 + self.rng.below(5) as u32;
            spawned_power = true;
        }

        // Honey drops.
        if moving || self.rng.chance(0.5) {
            self.coins.push(Coin { x: x + w * 0.5, y: c, pillar: Some(id) });
        } else if !spawned_power && self.rng.chance(0.55) {
            for (i, lift) in [0.5f32, 1.0, 0.5].iter().enumerate() {
                self.coins.push(Coin {
                    x: x + w + spacing * (0.3 + 0.17 * i as f32),
                    y: c - h * 0.08 * lift,
                    pillar: None,
                });
            }
        }
    }

    fn add_points(&mut self, n: u32) -> u32 {
        let gain = if self.double > 0.0 { n * 2 } else { n };
        self.score += gain;
        gain
    }

    fn popup(&mut self, x: f32, y: f32, text: impl Into<String>, color: Rgb, life: f32, big: bool) {
        self.popups.push(Popup { x, y, text: text.into(), color, life, max: life, big });
    }

    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, x: f32, y: f32, n: usize, colors: &[Rgb], speed: f32, life: f32, grav: f32) {
        for _ in 0..n {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let v = self.rng.range(speed * 0.3, speed);
            let l = self.rng.range(life * 0.5, life);
            let color = colors[self.rng.below(colors.len())];
            self.particles.push(Particle {
                x,
                y,
                vx: a.cos() * v,
                vy: a.sin() * v,
                life: l,
                max: l,
                color,
                grav,
            });
        }
    }

    // ---- update -----------------------------------------------------------

    pub fn update(&mut self, real_dt: f32) {
        self.time += real_dt;
        if self.paused {
            return;
        }
        let dt = if self.slow > 0.0 { real_dt * 0.55 } else { real_dt };
        self.state_t += real_dt;
        self.shake = (self.shake - real_dt).max(0.0);
        self.flash = (self.flash - real_dt * 3.0).max(0.0);
        self.invuln = (self.invuln - real_dt).max(0.0);
        self.slow = (self.slow - real_dt).max(0.0);
        self.magnet = (self.magnet - real_dt).max(0.0);
        self.double = (self.double - real_dt).max(0.0);

        let target = match self.state {
            State::Title | State::Ready => 0.0,
            _ => self.passed as f32 / 12.0,
        };
        self.sky_phase += (target - self.sky_phase) * (real_dt * 0.8).min(1.0);

        match self.state {
            State::Title | State::Ready => {
                self.scroll += self.h * 0.35 * dt;
                self.bee.y = self.hover_y() + (self.time * 4.0).sin() * self.h * 0.03;
                self.bee.vy = 0.0;
            }
            State::Playing => self.update_playing(dt),
            State::Dying => self.update_dying(dt),
            State::GameOver => {}
        }

        self.trail.push_front(self.bee.y);
        self.trail.truncate(10);

        for p in &mut self.particles {
            p.vy += p.grav * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
        for p in &mut self.popups {
            p.life -= real_dt;
            if !p.big {
                p.y -= 7.0 * real_dt;
            }
        }
        self.popups.retain(|p| p.life > 0.0);
    }

    fn update_playing(&mut self, dt: f32) {
        let h = self.h;
        let dx = self.speed() * dt;
        self.scroll += dx;

        // Bee physics.
        self.bee.vy = (self.bee.vy + self.gravity() * dt).min(self.max_fall());
        self.bee.y += self.bee.vy * dt;
        if self.bee.y < -2.0 {
            self.bee.y = -2.0;
            self.bee.vy = self.bee.vy.max(0.0);
        }

        // Pillars.
        for p in &mut self.pillars {
            p.x -= dx;
            if p.amp > 0.0 {
                p.phase += p.freq * dt;
                p.gap_y = p.base_y + p.phase.sin() * p.amp;
            }
        }
        self.pillars.retain(|p| p.x + p.w > -4.0);
        let spacing = self.spacing();
        let next_x = self.pillars.last().map(|p| p.x + spacing).unwrap_or(self.w + 1.0);
        if next_x <= self.w + 1.0 {
            self.spawn_pillar(next_x);
        }

        // Honey drops (with magnet pull).
        let (cx, cy) = self.bee_center();
        let magnet = self.magnet > 0.0;
        let pull = h * 1.8 * dt;
        let pull_r = h * 0.5;
        let pillars = &self.pillars;
        let mut got: Vec<(f32, f32)> = Vec::new();
        self.coins.retain_mut(|c| {
            c.x -= dx;
            if let Some(id) = c.pillar {
                if let Some(p) = pillars.iter().find(|p| p.id == id) {
                    c.y = p.gap_y;
                }
            }
            let (ddx, ddy) = (cx - c.x, cy - c.y);
            let d = (ddx * ddx + ddy * ddy).sqrt();
            if magnet && d < pull_r && d > 0.01 {
                c.pillar = None;
                let s = pull.min(d);
                c.x += ddx / d * s;
                c.y += ddy / d * s;
            }
            if d < 4.5 {
                got.push((c.x, c.y));
                return false;
            }
            c.x > -10.0
        });
        for (x, y) in got {
            self.honey += 1;
            let gain = self.add_points(1);
            self.popup(x, y - 3.0, format!("+{gain}"), Rgb(255, 215, 80), 0.7, false);
            self.burst(x, y, 8, &[Rgb(255, 220, 90), Rgb(255, 250, 200)], h * 0.5, 0.4, 0.0);
        }

        // Power-ups.
        for p in &mut self.powerups {
            p.x -= dx;
            p.phase += dt;
        }
        let mut picked = None;
        self.powerups.retain(|p| {
            let py = p.draw_y();
            let d = ((cx - p.x).powi(2) + (cy - py).powi(2)).sqrt();
            if picked.is_none() && d < 6.0 {
                picked = Some((p.kind, p.x, py));
                return false;
            }
            p.x > -10.0
        });
        if let Some((kind, x, y)) = picked {
            match kind {
                Power::Shield => self.shield = true,
                Power::Slow => self.slow = 5.0,
                Power::Magnet => self.magnet = 8.0,
                Power::Double => self.double = 8.0,
            }
            let c = kind.color();
            self.burst(x, y, 16, &[c, Rgb(255, 255, 255)], h * 0.6, 0.6, 0.0);
            self.popup(x, y - 5.0, kind.announce(), c, 1.2, false);
        }

        // Collisions & scoring.
        let (bx0, by0, bx1, by1) = self.bee_box();
        let mut hit = false;
        let mut passed_now = 0;
        let mut close_now = 0;
        for p in &mut self.pillars {
            let (x0, x1) = (p.x, p.x + p.w);
            if bx1 > x0 && bx0 < x1 {
                let (top, bot) = (p.gap_top(), p.gap_bot());
                p.min_clear = p.min_clear.min((by0 - top).min(bot - by1));
                if by0 < top || by1 > bot {
                    hit = true;
                }
            }
            if !p.scored && bx0 > x1 {
                p.scored = true;
                passed_now += 1;
                if (0.0..2.0).contains(&p.min_clear) {
                    close_now += 1;
                }
            }
        }
        for _ in 0..passed_now {
            self.passed += 1;
            self.add_points(1);
            if let Some(text) = milestone_text(self.passed) {
                self.popup(self.w * 0.5, self.h * 0.3, text, Rgb(255, 200, 60), 1.6, true);
                self.flash = self.flash.max(0.25);
            }
        }
        for _ in 0..close_now {
            self.close_calls += 1;
            let gain = self.add_points(1);
            self.popup(cx, by0 - 4.0, format!("CLOSE CALL +{gain}"), Rgb(255, 120, 210), 1.0, false);
            self.burst(cx, cy, 10, &[Rgb(255, 120, 210), Rgb(255, 255, 255)], h * 0.5, 0.4, 0.0);
        }

        if hit && self.invuln <= 0.0 {
            if self.shield {
                self.break_shield();
            } else {
                self.die();
                return;
            }
        }

        if by1 >= self.ground_y {
            if self.shield || self.invuln > 0.0 {
                if self.shield {
                    self.break_shield();
                }
                self.bee.y = self.ground_y - 5.5;
                self.bee.vy = -self.flap_v() * 1.1;
            } else {
                self.die();
            }
        }
    }

    fn break_shield(&mut self) {
        self.shield = false;
        self.invuln = 1.5;
        self.shake = 0.2;
        let (cx, cy) = self.bee_center();
        self.burst(cx, cy, 24, &[Power::Shield.color(), Rgb(200, 235, 255)], self.h * 0.8, 0.6, 0.0);
        self.popup(cx, cy - 6.0, "SHIELD SAVED YOU!", Power::Shield.color(), 1.2, false);
    }

    fn die(&mut self) {
        self.state = State::Dying;
        self.state_t = 0.0;
        self.shake = 0.4;
        self.flash = 1.0;
        self.slow = 0.0;
        self.magnet = 0.0;
        self.double = 0.0;
        self.bee.vy = -self.flap_v() * 0.5;
        self.bee.landed = false;
        let (cx, cy) = self.bee_center();
        let h = self.h;
        self.burst(
            cx,
            cy,
            30,
            &[Rgb(255, 205, 40), Rgb(40, 30, 20), Rgb(235, 245, 255), Rgb(255, 140, 40)],
            h * 0.9,
            1.1,
            h * 1.5,
        );
        if self.score > self.best {
            self.best = self.score;
            self.new_best = true;
            save_best(self.best);
        }
    }

    fn update_dying(&mut self, dt: f32) {
        if !self.bee.landed {
            self.bee.vy = (self.bee.vy + self.gravity() * dt).min(self.max_fall() * 1.4);
            self.bee.y += self.bee.vy * dt;
            if self.bee.y + 6.0 >= self.ground_y {
                self.bee.y = self.ground_y - 6.0;
                self.bee.vy = 0.0;
                self.bee.landed = true;
                self.shake = self.shake.max(0.15);
                let (cx, gy, h) = (self.bee.x + 4.5, self.ground_y, self.h);
                self.burst(cx, gy - 1.0, 12, &[Rgb(170, 125, 75), Rgb(130, 95, 55)], h * 0.4, 0.5, h);
            }
        }
        if self.bee.landed && self.state_t > 1.0 {
            self.state = State::GameOver;
            self.state_t = 0.0;
        }
    }
}

fn milestone_text(n: u32) -> Option<&'static str> {
    match n {
        10 => Some("NICE!"),
        20 => Some("GREAT!"),
        30 => Some("AWESOME!"),
        50 => Some("UNSTOPPABLE!"),
        75 => Some("LEGENDARY!"),
        100 => Some("BEE-YOND GODLIKE!"),
        n if n > 100 && n % 50 == 0 => Some("STILL BUZZING?!"),
        _ => None,
    }
}

fn best_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".bumble_rush_best"))
}

pub fn load_best() -> u32 {
    best_path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn save_best(n: u32) {
    if let Some(p) = best_path() {
        let _ = fs::write(p, n.to_string());
    }
}
