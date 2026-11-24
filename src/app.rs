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
use mettgine::app::Stage;
use mettgine::audio::{Audio, Sound};
use mettgine::canvas::Canvas;
use mettgine::crash;
use mettgine::input::{Bind, Input, MouseButton};
use mettgine::net::mett::{Launch, MettTransport};
use mettgine::net::udp::UdpTransport;
use mettgine::presence::Status;
use mettgine::render::{Config, FrameInput, Renderer, Resolution, UI_HEIGHT};
use std::collections::HashMap;
use winit::event::WindowEvent;
use winit::window::Window;

const CLICK: &[Bind] = &[Bind::Mouse(MouseButton::Left)];

pub struct App {
    settings: Settings,
    audio: Audio,
    sfx: HashMap<Sfx, Sound>,
    music: Sound,
    canvas: Canvas,
    screen: Screen,
    sel: usize,
    item_rects: Vec<[f32; 4]>,
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
    quit: bool,
    /// Fenster muss noch an die Einstellungen angepasst werden
    apply: bool,
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
            audio,
            sfx,
            music: sounds::music(),
            canvas: font,
            screen: Screen::Main,
            sel: 0,
            item_rects: Vec::new(),
            session: None,
            launcher: None,
            notice: None,
            attract: attract_round(),
            cam: Camera::default(),
            anims: Animators::default(),
            time: 0.0,
            last_countdown: 99,
            quit: false,
            apply: true,
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

    fn apply_settings(&mut self) {
        self.audio
            .set_volumes(self.settings.volume, self.settings.music);
        self.apply = true;
    }

    /// Geaenderte Einstellungen ans Fenster weitergeben.
    fn sync(&mut self, stage: &mut Stage) {
        if std::mem::take(&mut self.apply) {
            stage.renderer.set_render_scale(self.settings.render_scale);
            stage.set_fullscreen(self.settings.fullscreen);
        }
        if self.quit {
            stage.quit();
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

    fn menu_input(&mut self, input: &Input, mouse: (f32, f32)) {
        let items = menus::items(self.screen, &self.settings, self.launcher.is_some());
        if items.is_empty() {
            return;
        }
        let n = items.len();
        if input.pressed(k::UP) {
            self.sel = (self.sel + n - 1) % n;
            self.play(Sfx::Blip, 0.5);
        }
        if input.pressed(k::DOWN) {
            self.sel = (self.sel + 1) % n;
            self.play(Sfx::Blip, 0.5);
        }
        // Maus: ueber einem Eintrag auswaehlen, Klick loest aus
        if let Some(i) = self.item_rects.iter().position(|r| {
            mouse.0 >= r[0] && mouse.0 <= r[0] + r[2] && mouse.1 >= r[1] && mouse.1 <= r[1] + r[3]
        }) {
            if i != self.sel && i < n {
                self.sel = i;
            }
            if input.pressed(CLICK) {
                self.activate(items[self.sel.min(n - 1)].1, 1);
                return;
            }
        }
        let sel = self.sel.min(n - 1);
        let act = items[sel].1;
        let adjustable = matches!(act, Action::Setting(_) | Action::Bots);
        if adjustable && input.pressed(k::LEFT) {
            self.activate(act, -1);
        } else if adjustable && input.pressed(k::RIGHT) {
            self.activate(act, 1);
        } else if input.pressed(k::CONFIRM) {
            self.activate(act, 1);
        } else if input.pressed(k::BACK) {
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

    fn lobby_input(&mut self, input: &Input) {
        let Some(s) = self.session.as_mut() else {
            return;
        };
        if input.pressed(k::BACK) {
            self.leave_session();
            return;
        }
        if s.host {
            if input.pressed(k::CONFIRM) {
                if s.members.len() < 2 {
                    s.fill_to = 2;
                    s.fill_bots();
                }
                s.start_round();
            }
            if input.pressed(k::LEFT) && s.fill_to > s.humans() {
                s.fill_to -= 1;
                s.fill_bots();
            }
            if input.pressed(k::RIGHT) && s.fill_to < crate::game::MAX_PLAYERS {
                s.fill_to = s.fill_to.max(s.members.len()) + 1;
                s.fill_bots();
            }
        }
    }

    /// Ein Bild: Eingabe, Spiel, Klang, Zeichnen.
    fn run_frame(&mut self, stage: &mut Stage, dt: f32) {
        // Startbild der Engine: Eingaben verfallen, die Verbindung laeuft weiter
        if stage.splash_active() {
            stage.input.release_all();
            self.update_session(&stage.input, dt);
            self.render(stage, dt);
            return;
        }
        self.time += dt;
        // Overlay des Launchers offen: wie Pause
        if stage.overlay_open() && self.screen == Screen::Playing {
            self.go(Screen::Paused);
        }
        if stage.input.pressed(k::FULLSCREEN) {
            self.settings.fullscreen = !self.settings.fullscreen;
            self.settings.save();
            self.apply_settings();
        }
        match self.screen {
            Screen::Lobby => self.lobby_input(&stage.input),
            Screen::Playing => {
                if stage.input.pressed(k::PAUSE) {
                    self.go(Screen::Paused);
                }
            }
            _ => self.menu_input(&stage.input, stage.cursor()),
        }
        self.update_session(&stage.input, dt);
        self.sync(stage);
        self.render(stage, dt);
        self.smoke_check(stage);
    }

    /// Kurztest: nach der halben Zeit ein Bildschirmfoto (HACKEPETER_SHOT), am Ende Bildrate
    /// ins Protokoll und beenden.
    fn smoke_check(&mut self, stage: &mut Stage) {
        let Some(limit) = self.smoke else {
            return;
        };
        self.frames += 1;
        let half = self.time >= limit * 0.5 && self.time - 1.0 / 30.0 < limit * 0.5;
        if half {
            if let Ok(p) = std::env::var("HACKEPETER_SHOT") {
                stage.renderer.capture_next(p.into());
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

    fn update_session(&mut self, input: &Input, dt: f32) {
        let playing = self.screen == Screen::Playing;
        let controls = if playing {
            Controls {
                left: input.pressed(k::LEFT),
                right: input.pressed(k::RIGHT),
                jump: input.pressed(k::JUMP),
                slide: input.pressed(k::SLIDE),
            }
        } else {
            Controls::default()
        };
        let ability = if playing {
            ABILITIES
                .iter()
                .zip(k::ABILITY)
                .find(|(_, b)| input.pressed(b))
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

    fn render(&mut self, stage: &mut Stage, dt: f32) {
        let aspect = stage.renderer.aspect();
        self.canvas.begin(stage.renderer.ui_width(), UI_HEIGHT);
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
                    crate::hud::ingame(c, s, &look.view.view_proj, stage.input.using_pad);
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
                        crate::hud::ingame(c, s, &look.view.view_proj, stage.input.using_pad);
                    }
                }
                self.item_rects = crate::hud::menu(c, &names, self.sel, &footer);
            }
        }
        let st = &self.settings;
        let rays = st.raytracing && stage.renderer.ray_tracing();
        let modern = (st.modern)
            .then(|| crate::modern::scene(&self.cam, aspect, &look, st.shadows, rays, dt));
        stage.draw(&FrameInput {
            scene: modern.as_ref(),
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
            ..Default::default()
        });
    }

    /// Fokus verloren: im Spiel pausieren, Tasten loslassen.
    fn focus_lost(&mut self) {
        // Kurztest: Fenster startet evtl. ohne Fokus, trotzdem weiterspielen
        if self.screen == Screen::Playing && self.smoke.is_none() {
            self.go(Screen::Paused);
        }
    }
}

impl mettgine::app::Game for App {
    fn renderer(&mut self, window: &Window) -> mettgine::app::Result<Renderer> {
        let meshes = crate::assets::load_all();
        let skins = crate::people::load_skins();
        let f = &self.canvas.font;
        let config = Config {
            resolution: Resolution::Scale(self.settings.render_scale),
            ray_tracing: self.settings.raytracing,
            ..Config::new(&meshes, &skins, (&f.pixels, f.width, f.height))
        };
        Renderer::new(window, &config)
    }

    fn ready(&mut self, stage: &mut Stage) {
        self.sync(stage);
    }

    fn frame(&mut self, stage: &mut Stage, dt: f32) {
        self.run_frame(stage, dt);
        self.sync(stage);
    }

    fn event(&mut self, _stage: &mut Stage, event: &WindowEvent) {
        if let WindowEvent::Focused(false) = event {
            self.focus_lost();
        }
    }

    fn crash_context(&self) {
        crash::context("Bildschirm", format!("{:?}", self.screen));
        if let Some(s) = &self.session {
            crash::context("Spieler", s.members.len().to_string());
            crash::context("Host", s.host.to_string());
            if let Some(r) = &s.round {
                crash::context("Seed", r.track.seed.to_string());
                crash::context("Rundenzeit", format!("{:.1}", r.time));
            }
        }
    }

    /// Was der Launcher in Discord zeigt (oeffentlich, keine Namen und keine Codes).
    fn status(&self) -> Option<Status> {
        let Some(s) = &self.session else {
            return Some(Status::new("Im Menü", ""));
        };
        let details = match (self.screen, &s.round) {
            (Screen::Playing | Screen::Paused, Some(r)) => {
                match r.player(s.my_id).map(|p| p.role) {
                    Some(crate::game::Role::Thief) => "Auf der Flucht als Dieb",
                    Some(crate::game::Role::Cop) => "Jagt den Dieb fürs Ordnungsamt",
                    None => "Schaut zu",
                }
            }
            _ => "In der Lobby",
        };
        let humans = s.humans();
        Some(if humans > 1 || s.code.is_some() {
            Status::new(details, "Mit Freunden").party(humans.max(1), crate::game::MAX_PLAYERS)
        } else {
            Status::new(details, "Solo")
        })
    }
}
