//! Bewegung eines Laeufers (Dieb oder Cop): Spurwechsel, Springen, Rutschen, Daecher,
//! Zusammenstoesse. Jeder Rechner rechnet seine eigene Figur (sofortige Reaktion).

use super::lane_x;
use super::track::{Kind, Obstacle, Track, BEAM_HIGH};
use crate::people::Clip;

/// Fallbeschleunigung (m/s^2) und Absprunggeschwindigkeit (m/s): Sprunghoehe ~1.9 m,
/// etwa 0.75 s in der Luft (schwebig, wie im Genre ueblich)
pub const GRAVITY: f32 = 26.0;
pub const JUMP_V: f32 = 9.9;
/// Mit Sprungfedern: ~4 m hoch, reicht aufs Zugdach und auf Bruecken
pub const SUPER_JUMP_V: f32 = 14.5;
/// Flughoehe mit der Pfeffer-Rakete (Fuesse, m)
pub const FLY_H: f32 = 7.0;
/// Schnell nach unten druecken in der Luft
const DIVE_V: f32 = -16.0;
/// Seitliche Geschwindigkeit beim Spurwechsel (m/s)
const LANE_SPEED: f32 = 13.0;
/// Rutschdauer (s)
pub const SLIDE_TIME: f32 = 0.75;
/// Stolpern (seitlich angeeckt): Dauer und Tempo-Faktor
pub const STUMBLE_TIME: f32 = 0.6;
const STUMBLE_FACTOR: f32 = 0.55;
/// Zusammenstoss von vorn: so lange steht man still und rappelt sich auf (s)
pub const CRASH_TIME: f32 = 1.3;
/// Beschleunigung zum Zieltempo (m/s^2)
const ACCEL: f32 = 9.0;
/// halbe Breite der Figur (m)
pub const BODY_HALF: f32 = 0.35;
/// Stufe, die man noch hochlaeuft statt anzustossen (m)
const STEP: f32 = 0.35;

/// Aktive Power-ups, die die Bewegung beeinflussen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Boost {
    /// Broetchen-Schild: naechster Zusammenstoss wird abgefangen
    pub shield: bool,
    /// Sprungfedern
    pub springs: bool,
    /// Pfeffer-Rakete: fliegen
    pub flying: bool,
}

/// Eingabe eines Bildes (Ereignisse, nicht gehaltene Tasten).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    /// eine Spur nach links bzw. rechts, so wie es im Bild aussieht (links = +x)
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub slide: bool,
}

/// Was in diesem Schritt passiert ist (fuer Klang, HUD, Netz).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MoveEvent {
    Jump,
    Land,
    Slide,
    /// seitlich angeeckt
    Stumble,
    /// frontal gegen ein Hindernis (mit dessen Kennung)
    Crash(u32),
    /// Schild hat einen Zusammenstoss abgefangen
    Shielded(u32),
}

/// Zustand eines Laeufers.
#[derive(Clone, Debug, PartialEq)]
pub struct Mover {
    pub lane: i8,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vy: f32,
    pub speed: f32,
    pub on_ground: bool,
    pub slide: f32,
    pub stumble: f32,
    pub crash: f32,
    /// Spur vor dem letzten Wechsel (fuer das Zurueckprallen)
    prev_lane: i8,
    /// Kennungen der Hindernisse, die ein Schild schon "verbraucht" hat
    pub ignore: Vec<u32>,
}

impl Mover {
    pub fn new(lane: i8, z: f32) -> Self {
        Self {
            lane,
            x: lane_x(lane),
            y: 0.0,
            z,
            vy: 0.0,
            speed: 0.0,
            on_ground: true,
            slide: 0.0,
            stumble: 0.0,
            crash: 0.0,
            prev_lane: lane,
            ignore: Vec::new(),
        }
    }

    pub fn crashed(&self) -> bool {
        self.crash > 0.0
    }

    /// Hoehe des Bodens unter der Figur (Strasse, Dach, Rampe), fuer den Betrachter `self.z`.
    fn ground(&self, obstacles: &[Obstacle]) -> f32 {
        obstacles
            .iter()
            .filter(|o| o.overlaps_x(self.x, BODY_HALF * 0.5))
            .filter_map(|o| o.surface(self.z, self.z))
            .filter(|&h| h <= self.y + STEP)
            .fold(0.0, f32::max)
    }

    /// Ein Zeitschritt. `target` = Zieltempo (m/s), `boost` = aktive Power-ups.
    pub fn step(
        &mut self,
        dt: f32,
        c: Controls,
        track: &Track,
        extra: &[Obstacle],
        target: f32,
        boost: Boost,
    ) -> Vec<MoveEvent> {
        let mut ev = Vec::new();
        if boost.flying {
            return self.fly(dt, c, target);
        }
        if self.crash > 0.0 {
            self.crash -= dt;
            self.speed = 0.0;
            if self.crash <= 0.0 {
                self.speed = target * 0.4;
            }
            self.fall(dt, track, extra, &mut ev);
            return ev;
        }
        self.controls(c, boost.springs, &mut ev);
        // Tempo
        self.stumble = (self.stumble - dt).max(0.0);
        self.slide = (self.slide - dt).max(0.0);
        let want = if self.stumble > 0.0 {
            target * STUMBLE_FACTOR
        } else {
            target
        };
        self.speed += (want - self.speed).clamp(-ACCEL * 3.0 * dt, ACCEL * dt);
        let old_x = self.x;
        let tx = lane_x(self.lane);
        let dx = (tx - self.x).clamp(-LANE_SPEED * dt, LANE_SPEED * dt);
        self.x += dx;
        self.z += self.speed * dt;
        self.fall(dt, track, extra, &mut ev);
        self.collide(old_x, track, extra, boost.shield, &mut ev);
        ev
    }

    /// Flug mit der Pfeffer-Rakete: steigt auf Flughoehe, nur Spurwechsel, keine Hindernisse.
    fn fly(&mut self, dt: f32, c: Controls, target: f32) -> Vec<MoveEvent> {
        self.controls(
            Controls {
                jump: false,
                slide: false,
                ..c
            },
            false,
            &mut Vec::new(),
        );
        self.crash = 0.0;
        self.slide = 0.0;
        self.on_ground = false;
        self.vy = (FLY_H - self.y) * 3.0;
        self.y += self.vy * dt;
        self.speed += (target - self.speed).clamp(-ACCEL * dt, ACCEL * dt);
        let tx = lane_x(self.lane);
        self.x += (tx - self.x).clamp(-LANE_SPEED * dt, LANE_SPEED * dt);
        self.z += self.speed * dt;
        // nach dem Flug faellt man ganz normal (vy wird dann von der Schwerkraft uebernommen)
        self.vy = self.vy.min(0.0);
        Vec::new()
    }

    fn controls(&mut self, c: Controls, springs: bool, ev: &mut Vec<MoveEvent>) {
        let change = |d: i8, m: &mut Mover| {
            let n = (m.lane + d).clamp(-1, 1);
            if n != m.lane {
                m.prev_lane = m.lane;
                m.lane = n;
            }
        };
        // Die Kamera schaut entlang +z: links im Bild ist +x (Spur +1)
        if c.left {
            change(1, self);
        }
        if c.right {
            change(-1, self);
        }
        if c.jump && self.on_ground {
            self.vy = if springs { SUPER_JUMP_V } else { JUMP_V };
            self.on_ground = false;
            self.slide = 0.0;
            ev.push(MoveEvent::Jump);
        }
        if c.slide {
            if self.on_ground {
                if self.slide <= 0.0 {
                    ev.push(MoveEvent::Slide);
                }
                self.slide = SLIDE_TIME;
            } else {
                // in der Luft: schnell runter, unten direkt rutschen
                self.vy = self.vy.min(DIVE_V);
                self.slide = SLIDE_TIME;
            }
        }
    }

    fn fall(&mut self, dt: f32, track: &Track, extra: &[Obstacle], ev: &mut Vec<MoveEvent>) {
        let mut obs = track.obstacles_in(self.z - 1.0, self.z + 1.0, self.z);
        obs.extend_from_slice(extra);
        let ground = self.ground(&obs);
        if self.on_ground && self.y > ground + 0.05 {
            // Dachkante: runterfallen
            self.on_ground = false;
            self.vy = 0.0;
        }
        if self.on_ground {
            self.y = ground;
            return;
        }
        self.vy -= GRAVITY * dt;
        self.y += self.vy * dt;
        if self.y <= ground && self.vy <= 0.0 {
            self.y = ground;
            self.vy = 0.0;
            self.on_ground = true;
            ev.push(MoveEvent::Land);
        }
    }

    fn collide(
        &mut self,
        old_x: f32,
        track: &Track,
        extra: &[Obstacle],
        shield: bool,
        ev: &mut Vec<MoveEvent>,
    ) {
        let mut obs = track.obstacles_in(self.z - 0.5, self.z + 0.5, self.z);
        obs.extend(extra.iter().filter(|o| o.end(self.z) >= self.z - 0.5));
        for o in obs {
            if self.ignore.contains(&o.id) || !o.overlaps_x(self.x, BODY_HALF) {
                continue;
            }
            let (s, e) = (o.start(self.z), o.end(self.z));
            if self.z < s - 0.3 || self.z > e {
                continue;
            }
            if self.blocked_by(&o) {
                let from_side = !o.overlaps_x(old_x, BODY_HALF) && self.z > s + 0.4;
                if shield {
                    self.ignore.push(o.id);
                    ev.push(MoveEvent::Shielded(o.id));
                } else if from_side {
                    // zurueck auf die alte Spur
                    self.lane = self.prev_lane;
                    self.x = old_x;
                    self.stumble = STUMBLE_TIME;
                    ev.push(MoveEvent::Stumble);
                } else {
                    self.z = s - 0.35;
                    self.crash = CRASH_TIME;
                    self.speed = 0.0;
                    self.slide = 0.0;
                    self.ignore.push(o.id);
                    ev.push(MoveEvent::Crash(o.id));
                }
                return;
            }
        }
        // ignorierte Hindernisse weit hinter uns vergessen
        if self.ignore.len() > 32 {
            self.ignore.drain(..16);
        }
    }

    /// Steht das Hindernis bei der jetzigen Hoehe und Haltung im Weg?
    fn blocked_by(&self, o: &Obstacle) -> bool {
        match o.kind {
            // Balken: nur drunter durch (zu hoch zum Drueberspringen, ausser mit Sprungfedern)
            Kind::Beam => !(self.slide > 0.0 && self.on_ground) && self.y < BEAM_HIGH + 0.4,
            // Bruecken: unten frei, oben ist Boden
            k if k.overhead() => false,
            Kind::Ramp => false,
            k if k.low() => self.y < k.height() - 0.1,
            k => self.y < k.height() - STEP,
        }
    }

    /// Passende Animation zum Zustand.
    pub fn clip(&self, fast: bool) -> Clip {
        if self.crash > 0.0 {
            if self.crash > super::mover::CRASH_TIME * 0.45 {
                Clip::Hit
            } else {
                Clip::GetUp
            }
        } else if !self.on_ground {
            if self.vy > 2.0 {
                Clip::JumpStart
            } else {
                Clip::JumpLoop
            }
        } else if self.slide > 0.0 {
            Clip::SlideLoop
        } else if self.speed < 0.5 {
            Clip::Idle
        } else if fast {
            Clip::Sprint
        } else {
            Clip::Run
        }
    }
}
