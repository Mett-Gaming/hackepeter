//! Das Spiel als Ganzes: Bildschirme, Eingabe, Sitzung, Klang und Zeichnen pro Bild.

use crate::controls as k;
use crate::game::cops::ABILITIES;
use crate::game::mover::{Controls, MoveEvent};
use crate::game::round::{Entry, Phase, Round, RoundEvent};
use crate::menus::{self, Action, Screen};
use crate::net::session::Session;
use crate::scene::{self, Animators, Camera};
use crate::settings::Settings;
use crate::sounds::{self, Sfx, ALL_SFX};
use mettgine::audio::{Audio, Sound};
use mettgine::canvas::Canvas;
use mettgine::crash;
use mettgine::input::Input;
use mettgine::net::mett::{Launch, MettTransport};
use mettgine::net::udp::UdpTransport;
use mettgine::render::{FrameInput, Renderer, UI_HEIGHT};
use std::collections::HashMap;
use winit::window::{Fullscreen, Window};

/// Fenster und Renderer (erst nach `resumed` vorhanden).
pub struct Running {
    pub window: Window,
    pub renderer: Renderer,
}

pub struct App {
    pub settings: Settings,
    pub input: Input,
    audio: Audio,
    sfx: HashMap<Sfx, Sound>,
    music: Sound,
    pub running: Option<Running>,
    pub canvas: Canvas,
    pub screen: Screen,
    sel: usize,
    item_rects: Vec<[f32; 4]>,
    /// Maus in UI-Einheiten
    pub mouse: (f32, f32),
    pub clicked: bool,
    session: Option<Session>,
    launcher: Option<MettTransport>,
    /// Meldung im Hauptmenue (z. B. "Verbindung verloren")
    notice: Option<String>,
    /// Hintergrund-Runde der Menues (Bots laufen durch die Stadt)
    attract: Round,
    cam: Camera,
    anims: Animators,
    time: f32,
    last_countdown: i32,
    pub quit: bool,
    crash_ctx_t: f32,
    /// Kurztest (HACKEPETER_SMOKE=Sekunden): direkt eine Einzelspieler-Runde, dann beenden
    smoke: Option<f32>,
    frames: u32,
}

fn attract_round() -> Round {
    let entries: Vec<Entry> = (0..4)
        .map(|i| Entry {
            id: 100 + i,
            name: String::new(),
            bot: true,
            local: true,
        })
        .collect();
    let mut r = Round::new(7, &entries, 100);
    r.time = 0.0;
    r.phase = Phase::Running;
    r
}

impl App {
    pub fn new(font: Canvas) -> Self {
        let settings = Settings::load();
        let audio = Audio::new(settings.volume, settings.music);
        let sfx = ALL_SFX.iter().map(|&s| (s, sounds::make(s))).collect();
        let mut app = Self {
            input: Input::new(),
            audio,
            sfx,
            music: sounds::music(),
            running: None,
            canvas: font,
            screen: Screen::Main,
            sel: 0,
            item_rects: Vec::new(),
            mouse: (0.0, 0.0),
            clicked: false,
            session: None,
            launcher: None,
            notice: None,
            attract: attract_round(),
            cam: Camera::default(),
            anims: Animators::default(),
            time: 0.0,
            last_countdown: 99,
            quit: false,
            crash_ctx_t: 0.0,
            smoke: std::env::var("HACKEPETER_SMOKE")
                .ok()
                .and_then(|s| s.parse().ok()),
            frames: 0,
            settings,
        };
        app.connect_from_start();
        if app.smoke.is_some() {
            app.activate(Action::StartSolo, 1);
        }
        app.audio.play_music(&app.music);
        app
    }

    /// Ueber den Launcher (oder zum Testen per lokalem UDP) gestartet? Dann gleich in die Lobby.
    fn connect_from_start(&mut self) {
        let name = self.settings.name.clone();
        if let Ok(port) = std::env::var("HACKEPETER_DEV_HOST") {
            match UdpTransport::host(port.parse().unwrap_or(47800), &name) {
                Ok(t) => self.enter_session(Session::network(Box::new(t), &name)),
                Err(e) => self.notice = Some(format!("Hosten fehlgeschlagen: {e}")),
            }
            return;
        }
        if let Ok(addr) = std::env::var("HACKEPETER_DEV_JOIN") {
            match UdpTransport::join(&addr, &name) {
                Ok(t) => self.enter_session(Session::network(Box::new(t), &name)),
                Err(e) => self.notice = Some(format!("Beitreten fehlgeschlagen: {e}")),
            }
            return;
        }
        match MettTransport::from_launcher() {
            Ok(Some((mut t, launch))) => match launch {
                Launch::Host => {
                    t.host((crate::game::MAX_PLAYERS - 1) as u16);
                    self.enter_session(Session::network(Box::new(t), &name));
                }
                Launch::Join(code) => {
                    t.join(&code);
                    self.enter_session(Session::network(Box::new(t), &name));
                }
                Launch::None => self.launcher = Some(t),
            },
            Ok(None) => {}
            Err(e) => crash::log(&format!("Netzdienst des Launchers nicht erreichbar: {e}")),
        }
    }

    fn enter_session(&mut self, s: Session) {
        self.session = Some(s);
        self.screen = Screen::Lobby;
        self.sel = 0;
    }

    fn play(&mut self, s: Sfx, gain: f32) {
        if let Some(snd) = self.sfx.get(&s) {
            self.audio.play(snd, gain, 1.0);
        }
    }

    pub fn apply_settings(&mut self) {
        self.audio
            .set_volumes(self.settings.volume, self.settings.music);
        if let Some(r) = &mut self.running {
            r.renderer.set_render_scale(self.settings.render_scale);
            let want = self.settings.fullscreen;
            if r.window.fullscreen().is_some() != want {
                r.window
                    .set_fullscreen(want.then_some(Fullscreen::Borderless(None)));
            }
        }
    }

    fn go(&mut self, s: Screen) {
        self.screen = s;
        self.sel = 0;
    }

    fn leave_session(&mut self) {
        if let Some(s) = self.session.as_mut() {
            s.leave();
        }
        self.session = None;
        self.go(Screen::Main);
    }

    fn activate(&mut self, a: Action, dir: i32) {
        match a {
            Action::Go(s) => self.go(s),
            Action::StartSolo => {
                let mut s = Session::solo(&self.settings.name, self.settings.bots as usize);
                s.start_round();
                self.session = Some(s);
                self.go(Screen::Playing);
            }
            Action::Host => {
                if let Some(mut t) = self.launcher.take() {
                    t.host((crate::game::MAX_PLAYERS - 1) as u16);
                    let name = self.settings.name.clone();
                    self.enter_session(Session::network(Box::new(t), &name));
                }
            }
            Action::Quit => self.quit = true,
            Action::Resume => self.go(Screen::Playing),
            Action::LeaveToMenu => self.leave_session(),
            Action::Setting(n) => {
                menus::change_setting(&mut self.settings, n, dir);
                self.settings.save();
                self.apply_settings();
            }
            Action::Bots => {
                // 1..7 Bots, am Ende wieder von vorn
                let b = self.settings.bots as i32 - 1 + dir;
                self.settings.bots = b.rem_euclid(7) as u32 + 1;
                self.settings.save();
            }
            Action::None => {}
        }
        self.play(Sfx::Select, 0.6);
    }

    fn menu_input(&mut self) {
        let items = menus::items(self.screen, &self.settings, self.launcher.is_some());
        if items.is_empty() {
            return;
        }
        let n = items.len();
        if self.input.pressed(k::UP) {
            self.sel = (self.sel + n - 1) % n;
            self.play(Sfx::Blip, 0.5);
        }
        if self.input.pressed(k::DOWN) {
            self.sel = (self.sel + 1) % n;
            self.play(Sfx::Blip, 0.5);
        }
        // Maus: ueber einem Eintrag auswaehlen, Klick loest aus
        if let Some(i) = self.item_rects.iter().position(|r| {
            self.mouse.0 >= r[0]
                && self.mouse.0 <= r[0] + r[2]
                && self.mouse.1 >= r[1]
                && self.mouse.1 <= r[1] + r[3]
        }) {
            if i != self.sel && i < n {
                self.sel = i;
            }
            if self.clicked {
                self.activate(items[self.sel.min(n - 1)].1, 1);
                return;
            }
        }
        let sel = self.sel.min(n - 1);
        let act = items[sel].1;
        let adjustable = matches!(act, Action::Setting(_) | Action::Bots);
        if adjustable && self.input.pressed(k::LEFT) {
            self.activate(act, -1);
        } else if adjustable && self.input.pressed(k::RIGHT) {
            self.activate(act, 1);
        } else if self.input.pressed(k::CONFIRM) {
            self.activate(act, 1);
        } else if self.input.pressed(k::BACK) {
            let back = match self.screen {
                Screen::Paused => Action::Resume,
                Screen::Main => Action::None,
                _ => Action::Go(Screen::Main),
            };
            if back != Action::None {
                self.activate(back, 1);
            }
        }
    }

    fn lobby_input(&mut self) {
        let Some(s) = self.session.as_mut() else {
            return;
        };
        if self.input.pressed(k::BACK) {
            self.leave_session();
            return;
        }
        if s.host {
            if self.input.pressed(k::CONFIRM) {
                if s.members.len() < 2 {
                    s.fill_to = 2;
                    s.fill_bots();
                }
                s.start_round();
            }
            if self.input.pressed(k::LEFT) && s.fill_to > s.humans() {
                s.fill_to -= 1;
                s.fill_bots();
            }
            if self.input.pressed(k::RIGHT) && s.fill_to < crate::game::MAX_PLAYERS {
                s.fill_to = s.fill_to.max(s.members.len()) + 1;
                s.fill_bots();
            }
        }
    }

    /// Ein Bild: Eingabe, Spiel, Klang, Zeichnen.
    pub fn frame(&mut self, dt: f32) {
        // Startbild der Engine: Eingaben verfallen, die Verbindung laeuft weiter
        if self
            .running
            .as_ref()
            .is_some_and(|r| r.renderer.splash_active())
        {
            self.input.release_all();
            self.clicked = false;
            self.update_session(dt);
            self.render(dt);
            return;
        }
        self.time += dt;
        if self.input.pressed(k::FULLSCREEN) {
            self.settings.fullscreen = !self.settings.fullscreen;
            self.settings.save();
            self.apply_settings();
        }
        match self.screen {
            Screen::Lobby => self.lobby_input(),
            Screen::Playing => {
                if self.input.pressed(k::PAUSE) {
                    self.go(Screen::Paused);
                }
            }
            _ => self.menu_input(),
        }
        self.clicked = false;
        self.update_session(dt);
        self.update_crash_context(dt);
        self.render(dt);
        self.smoke_check();
    }

    /// Kurztest: nach der halben Zeit ein Bildschirmfoto (HACKEPETER_SHOT), am Ende Bildrate
    /// ins Protokoll und beenden.
    fn smoke_check(&mut self) {
        let Some(limit) = self.smoke else {
            return;
        };
        self.frames += 1;
        let half = self.time >= limit * 0.5 && self.time - 1.0 / 30.0 < limit * 0.5;
        if half {
            if let (Ok(p), Some(r)) = (std::env::var("HACKEPETER_SHOT"), self.running.as_mut()) {
                r.renderer.capture_next(p.into());
            }
        }
        if self.time >= limit {
            let info = self
                .session
                .as_ref()
                .and_then(|s| s.round.as_ref().map(|r| (s.my_id, r)))
                .map_or("keine Runde".to_string(), |(me, r)| {
                    let p = r.player(me);
                    format!(
                        "Phase {:?}, Zeit {:.1}, Rolle {:?}, z = {:.0} m",
                        r.phase,
                        r.time,
                        p.map(|p| p.role),
                        p.map_or(0.0, |p| p.mover.z)
                    )
                });
            crash::log(&format!(
                "Kurztest: {} Bilder in {:.1} s ({:.0} fps), Bildschirm {:?}, {info}",
                self.frames,
                self.time,
                self.frames as f32 / self.time,
                self.screen
            ));
            self.quit = true;
        }
    }

    fn update_session(&mut self, dt: f32) {
        let playing = self.screen == Screen::Playing;
        let controls = if playing {
            Controls {
                left: self.input.pressed(k::LEFT),
                right: self.input.pressed(k::RIGHT),
                jump: self.input.pressed(k::JUMP),
                slide: self.input.pressed(k::SLIDE),
            }
        } else {
            Controls::default()
        };
        let ability = if playing {
            ABILITIES
                .iter()
                .zip(k::ABILITY)
                .find(|(_, b)| self.input.pressed(b))
                .map(|(a, _)| *a)
        } else {
            None
        };
        let Some(s) = self.session.as_mut() else {
            self.attract.step(dt, &|_| Controls::default());
            if self
                .attract
                .thief()
                .is_some_and(|t| t.mover.crashed() || t.mover.z > 3000.0)
            {
                self.attract = attract_round();
            }
            return;
        };
        // Einzelspieler pausiert wirklich, Mehrspieler laeuft weiter
        let solo_paused =
            self.screen == Screen::Paused && s.code.is_none() && s.host && s.humans() == 1;
        let ev = if solo_paused {
            Vec::new()
        } else {
            s.update(dt, controls, ability)
        };
        if let Some(reason) = s.ended.take() {
            self.notice = Some(reason);
            self.session = None;
            self.go(Screen::Main);
            return;
        }
        let started = s.round.is_some();
        let me = s.my_id;
        let countdown = s
            .round
            .as_ref()
            .filter(|r| r.phase == Phase::Countdown)
            .map(|r| (-r.time).ceil() as i32);
        if self.screen == Screen::Lobby && started {
            self.go(Screen::Playing);
        }
        self.sounds_for(&ev, me, countdown);
    }

    fn sounds_for(&mut self, ev: &[RoundEvent], me: u16, countdown: Option<i32>) {
        match countdown {
            Some(n) if n != self.last_countdown => {
                self.last_countdown = n;
                self.play(Sfx::Beep, 0.8);
            }
            None if self.last_countdown <= 3 => {
                self.last_countdown = 99;
                self.play(Sfx::Go, 0.8);
            }
            _ => {}
        }
        for e in ev {
            let (s, gain) = match *e {
                RoundEvent::Move(id, m) if id == me => match m {
                    MoveEvent::Jump => (Sfx::Jump, 0.6),
                    MoveEvent::Land => (Sfx::Land, 0.4),
                    MoveEvent::Slide => (Sfx::Slide, 0.6),
                    MoveEvent::Stumble => (Sfx::Stumble, 0.8),
                    MoveEvent::Crash(_) => (Sfx::Crash, 1.0),
                    MoveEvent::Shielded(_) => (Sfx::Power, 0.7),
                },
                RoundEvent::Ring(id) if id == me => (Sfx::Ring, 0.5),
                RoundEvent::Power(id, _) if id == me => (Sfx::Power, 0.8),
                RoundEvent::Bitten(_) => (Sfx::Bark, 0.9),
                RoundEvent::Ability(_, a) => match a {
                    crate::game::cops::Ability::Tape => (Sfx::Tape, 0.7),
                    crate::game::cops::Ability::Dog => (Sfx::Bark, 0.8),
                    crate::game::cops::Ability::Whistle => (Sfx::Whistle, 0.6),
                },
                RoundEvent::Caught { .. } => (Sfx::Caught, 1.0),
                RoundEvent::Escaped => (Sfx::Escaped, 1.0),
                _ => continue,
            };
            self.play(s, gain);
        }
    }

    fn update_crash_context(&mut self, dt: f32) {
        self.crash_ctx_t -= dt;
        if self.crash_ctx_t > 0.0 {
            return;
        }
        self.crash_ctx_t = 1.0;
        crash::context("Bildschirm", format!("{:?}", self.screen));
        if let Some(s) = &self.session {
            crash::context("Spieler", s.members.len().to_string());
            crash::context("Host", s.host.to_string());
            if let Some(r) = &s.round {
                crash::context("Seed", r.track.seed.to_string());
                crash::context("Rundenzeit", format!("{:.1}", r.time));
            }
        }
        crash::context("Bild", format!("{:.1} ms", dt * 1000.0));
    }

    fn render(&mut self, dt: f32) {
        let Some(run) = self.running.as_mut() else {
            return;
        };
        let aspect = run.renderer.aspect();
        self.canvas.begin(run.renderer.ui_width(), UI_HEIGHT);
        let (round, me) = match &self.session {
            Some(s) if s.round.is_some() => (s.round.as_ref().unwrap(), s.my_id),
            _ => (&self.attract, 100),
        };
        let (frame, focus) = scene::build(round, me, &mut self.anims, self.time, dt);
        self.cam.follow(focus, dt);
        let look = scene::look(&self.cam, aspect, focus, self.settings.shadows);
        let c = &mut self.canvas;
        self.item_rects.clear();
        match self.screen {
            Screen::Playing => {
                if let Some(s) = &self.session {
                    crate::hud::ingame(c, s, &look.view.view_proj, self.input.using_pad);
                }
            }
            Screen::Lobby => {
                if let Some(s) = &self.session {
                    crate::hud::lobby(c, s);
                }
            }
            Screen::Friends => {
                crate::hud::info(c, &menus::friends_text(self.launcher.is_some()));
                let items = menus::items(self.screen, &self.settings, self.launcher.is_some());
                let names: Vec<String> = items.into_iter().map(|i| i.0).collect();
                // Eintraege unter den Text
                let rects = crate::hud::menu_at(c, &names, self.sel, 500.0);
                self.item_rects = rects;
            }
            screen => {
                let items = menus::items(screen, &self.settings, self.launcher.is_some());
                let names: Vec<String> = items.into_iter().map(|i| i.0).collect();
                let footer = match &self.notice {
                    Some(n) if screen == Screen::Main => n.clone(),
                    _ => format!(
                        "Pfeiltasten/Controller · Enter · Esc · F11 Vollbild · {}",
                        crate::VERSION
                    ),
                };
                if screen == Screen::Paused {
                    if let Some(s) = &self.session {
                        crate::hud::ingame(c, s, &look.view.view_proj, self.input.using_pad);
                    }
                }
                self.item_rects = crate::hud::menu(c, &names, self.sel, &footer);
            }
        }
        if let Err(e) = run.renderer.draw(&FrameInput {
            view: look.view,
            second_camera: None,
            time: self.time,
            lighting: look.lighting,
            post: look.post,
            meshes: &frame.meshes,
            ghosts: &[],
            skins: &frame.skins,
            camera_skins: &[],
            bones: &frame.bones,
            ui: &self.canvas.verts,
        }) {
            crash::fatal(
                "Zeichnen",
                &format!("Bild konnte nicht gezeichnet werden: {e}"),
                "Die Grafik ist ausgefallen. Bitte den Grafiktreiber aktualisieren.",
            );
            self.quit = true;
        }
    }

    /// Fokus verloren: im Spiel pausieren, Tasten loslassen.
    pub fn focus_lost(&mut self) {
        self.input.release_all();
        // Kurztest: Fenster startet evtl. ohne Fokus, trotzdem weiterspielen
        if self.screen == Screen::Playing && self.smoke.is_none() {
            self.go(Screen::Paused);
        }
    }
}
