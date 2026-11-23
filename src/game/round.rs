//! Eine Runde: Rollen, Zeit, Spieler, Faehigkeiten, Einsammeln, Fangen, Punkte.
//!
//! Jeder Rechner simuliert die Spieler, die er steuert (`Player::local`: eigener Spieler, beim
//! Host zusaetzlich die Bots). Ueber Fang, Rundenende und Faehigkeiten entscheidet der Host
//! (`judge`, `use_ability`); die anderen bekommen das Ergebnis ueber das Netz.

use super::bots::Brain;
use super::cops::{Ability, Cooldowns, Dog, Tape, TAPE_AHEAD, WHISTLE_FACTOR, WHISTLE_TIME};
use super::mover::{Boost, Controls, MoveEvent, Mover, FLY_H};
use super::track::{Obstacle, Pickup, Power, Track};
use super::*;
use std::collections::HashSet;

/// Senf-Turbo: Dauer (s) und Tempo-Faktor; Zwiebelmagnet: Dauer (s)
pub const MUSTARD_TIME: f32 = 4.0;
pub const MUSTARD_FACTOR: f32 = 1.3;
pub const MAGNET_TIME: f32 = 8.0;
/// Pfeffer-Rakete: Flugdauer (s) und Tempo-Faktor; Sprungfedern und Mett-Stern: Dauer (s)
pub const ROCKET_TIME: f32 = 5.5;
pub const ROCKET_FACTOR: f32 = 1.15;
pub const SPRINGS_TIME: f32 = 10.0;
pub const MULTIPLIER_TIME: f32 = 15.0;
/// Kennungen der Luftringe beginnen hier (getrennt von den Strecken-Kennungen)
const SKY_IDS: u32 = 3_000_000_000;
/// Ab diesem Rueckstand holen Cops schneller auf (Gummiband, Meter)
const RUBBER_GAP: f32 = 45.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Countdown,
    Running,
    /// vorbei: hat der Dieb durchgehalten, wer hat gefangen
    Over {
        thief_won: bool,
        catcher: Option<u16>,
    },
}

/// Was in der Runde passiert ist (Klang, HUD, Netz).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoundEvent {
    Move(u16, MoveEvent),
    Ring(u16),
    Power(u16, Power),
    Bitten(u16),
    Ability(u16, Ability),
    Caught { cop: u16 },
    Escaped,
}

#[derive(Clone, Debug)]
pub struct Player {
    /// Netz-Kennung (0 = Host); Bots bekommen Nummern ab 100
    pub id: u16,
    pub name: String,
    pub role: Role,
    pub mover: Mover,
    /// Figurvariante (people::PEOPLE)
    pub variant: usize,
    /// wird auf diesem Rechner simuliert
    pub local: bool,
    pub bot: Option<Brain>,
    pub rings: u32,
    pub mustard: f32,
    pub magnet: f32,
    pub shield: bool,
    pub rocket: f32,
    pub springs: f32,
    pub multiplier: f32,
    pub whistle: f32,
    pub cooldowns: Cooldowns,
    /// eingesammelte Dinge (Kennungen)
    pub taken: HashSet<u32>,
    /// Punkte dieser Runde
    pub points: i32,
    /// Animation fremder Figuren (vom Netz, people::Clip als Zahl)
    pub remote_clip: u8,
}

/// Eintrag fuer eine neue Runde.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u16,
    pub name: String,
    pub bot: bool,
    pub local: bool,
}

pub struct Round {
    pub track: Track,
    /// Rundenzeit (s), beginnt bei -COUNTDOWN
    pub time: f32,
    pub phase: Phase,
    pub players: Vec<Player>,
    pub tapes: Vec<Tape>,
    pub dogs: Vec<Dog>,
    /// Luftringe der Pfeffer-Rakete (nur beim Dieb bekannt, der sie eingesammelt hat)
    pub sky: Vec<Pickup>,
    /// Funk-Sperre des Ordnungsamts: nach jeder Faehigkeit wartet das ganze Team (s)
    pub team_lock: f32,
    /// Von Bot-Faehigkeiten erzeugt, seit dem letzten Abholen (der Host schickt sie an alle)
    pub spawned: Vec<Spawn>,
    next_id: u32,
}

/// Funk-Sperre nach einer Faehigkeit (s); verhindert, dass 7 Cops gleichzeitig loslegen
pub const TEAM_LOCK: f32 = 3.5;
/// Hoechstens so viele Absperrbaender gleichzeitig
const MAX_TAPES: usize = 2;

impl Round {
    /// Neue Runde mit Seed; `thief` = Kennung des Diebs, alle anderen sind Cops.
    pub fn new(seed: u64, entries: &[Entry], thief: u16) -> Self {
        let mut cops = 0usize;
        let players = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let role = if e.id == thief {
                    Role::Thief
                } else {
                    Role::Cop
                };
                let (mover, variant) = if role == Role::Thief {
                    (
                        Mover::new(0, START_GAP),
                        crate::people::variant_for(crate::people::Look::Thief, seed as usize),
                    )
                } else {
                    let lane = (cops % 3) as i8 - 1;
                    let z = -1.6 * (cops / 3) as f32;
                    cops += 1;
                    (
                        Mover::new(lane, z),
                        crate::people::variant_for(crate::people::Look::Cop, i),
                    )
                };
                Player {
                    id: e.id,
                    name: e.name.clone(),
                    role,
                    mover,
                    variant,
                    local: e.local,
                    bot: e.bot.then(|| Brain::new(seed ^ e.id as u64)),
                    rings: 0,
                    mustard: 0.0,
                    magnet: 0.0,
                    shield: false,
                    rocket: 0.0,
                    springs: 0.0,
                    multiplier: 0.0,
                    whistle: 0.0,
                    cooldowns: Cooldowns::default(),
                    taken: HashSet::new(),
                    points: 0,
                    remote_clip: 1,
                }
            })
            .collect();
        Self {
            track: Track::new(seed),
            time: -COUNTDOWN,
            phase: Phase::Countdown,
            players,
            tapes: Vec::new(),
            dogs: Vec::new(),
            sky: Vec::new(),
            team_lock: 0.0,
            spawned: Vec::new(),
            next_id: 900_000,
        }
    }

    pub fn thief(&self) -> Option<&Player> {
        self.players.iter().find(|p| p.role == Role::Thief)
    }

    pub fn player(&self, id: u16) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    pub fn player_mut(&mut self, id: u16) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    pub fn cop_count(&self) -> usize {
        self.players.iter().filter(|p| p.role == Role::Cop).count()
    }

    /// Abstand des naechsten Cops hinter dem Dieb (Meter).
    pub fn closest_gap(&self) -> f32 {
        let Some(t) = self.thief() else { return 0.0 };
        self.players
            .iter()
            .filter(|p| p.role == Role::Cop)
            .map(|p| t.mover.z - p.mover.z)
            .fold(f32::MAX, f32::min)
    }

    /// Zeit weiterlaufen lassen und lokale Spieler bewegen. `controls(id)` liefert die Eingabe
    /// menschlicher Spieler; Bots denken selbst.
    pub fn step(&mut self, dt: f32, controls: &dyn Fn(u16) -> Controls) -> Vec<RoundEvent> {
        let mut ev = Vec::new();
        if matches!(self.phase, Phase::Over { .. }) {
            return ev;
        }
        self.time += dt;
        if self.phase == Phase::Countdown && self.time >= 0.0 {
            self.phase = Phase::Running;
        }
        if self.phase != Phase::Running {
            return ev;
        }
        self.team_lock = (self.team_lock - dt).max(0.0);
        let extra: Vec<Obstacle> = self.tapes.iter().map(|t| t.obstacle()).collect();
        let base = base_speed(self.time);
        let cop_factor = cop_speed_factor(self.cop_count());
        let thief_z = self.thief().map_or(0.0, |t| t.mover.z);
        let thief_lane = self.thief().map_or(0, |t| t.mover.lane);
        let dogs = self.dogs.clone();
        for i in 0..self.players.len() {
            if !self.players[i].local {
                continue;
            }
            let p = &mut self.players[i];
            p.cooldowns.tick(dt);
            p.mustard = (p.mustard - dt).max(0.0);
            p.magnet = (p.magnet - dt).max(0.0);
            p.whistle = (p.whistle - dt).max(0.0);
            p.rocket = (p.rocket - dt).max(0.0);
            p.springs = (p.springs - dt).max(0.0);
            p.multiplier = (p.multiplier - dt).max(0.0);
            let target = match p.role {
                Role::Thief => {
                    let boost = if p.mustard > 0.0 { MUSTARD_FACTOR } else { 1.0 };
                    let fly = if p.rocket > 0.0 { ROCKET_FACTOR } else { 1.0 };
                    base * boost * fly
                }
                Role::Cop => {
                    let rubber = if thief_z - p.mover.z > RUBBER_GAP {
                        1.12
                    } else {
                        1.0
                    };
                    let whistle = if p.whistle > 0.0 { WHISTLE_FACTOR } else { 1.0 };
                    base * cop_factor * rubber * whistle
                }
            };
            let c = match &mut p.bot {
                Some(brain) => brain.think(
                    &self.track,
                    &extra,
                    &p.mover,
                    p.role,
                    thief_lane,
                    &dogs,
                    self.time,
                ),
                None => controls(p.id),
            };
            let id = p.id;
            let boost = Boost {
                shield: p.shield,
                springs: p.springs > 0.0,
                flying: p.rocket > 0.0,
            };
            for e in p.mover.step(dt, c, &self.track, &extra, target, boost) {
                if let MoveEvent::Shielded(_) = e {
                    p.shield = false;
                }
                ev.push(RoundEvent::Move(id, e));
            }
            if p.role == Role::Thief {
                Self::collect(&self.track, &mut self.sky, p, &mut ev);
                for d in &mut self.dogs {
                    let m = &p.mover;
                    if d.bites(self.time, dt, m.x, m.y, m.z) {
                        d.done = true;
                        if p.shield {
                            p.shield = false;
                        } else {
                            p.mover.stumble = super::mover::STUMBLE_TIME;
                            ev.push(RoundEvent::Bitten(id));
                        }
                    }
                }
            }
        }
        let t = self.time;
        self.dogs.retain(|d| d.alive(t));
        self.tapes.retain(|tp| tp.z > thief_z - 60.0);
        ev
    }

    fn collect(track: &Track, sky: &mut Vec<Pickup>, p: &mut Player, ev: &mut Vec<RoundEvent>) {
        let m = p.mover.clone();
        let near = |k: &Pickup| k.z >= m.z - 1.2 && k.z <= m.z + 1.2;
        let mut found = track.pickups_in(m.z - 1.2, m.z + 1.2);
        found.extend(sky.iter().filter(|k| near(k)));
        for k in found {
            if p.taken.contains(&k.id) {
                continue;
            }
            let dx = (lane_x(k.lane) - m.x).abs();
            let dy = (k.y - 0.9 - m.y).abs();
            let magnet = p.magnet > 0.0 && k.power.is_none();
            if (dx < 1.0 && dy < 1.2) || (magnet && dy < 4.0) {
                p.taken.insert(k.id);
                match k.power {
                    None => {
                        p.rings += if p.multiplier > 0.0 { 2 } else { 1 };
                        ev.push(RoundEvent::Ring(p.id));
                    }
                    Some(pw) => {
                        match pw {
                            Power::Mustard => p.mustard = MUSTARD_TIME,
                            Power::Magnet => p.magnet = MAGNET_TIME,
                            Power::Shield => p.shield = true,
                            Power::Rocket => {
                                p.rocket = ROCKET_TIME;
                                sky.extend(sky_rings(k.id, m.z, m.speed.max(12.0)));
                            }
                            Power::Springs => p.springs = SPRINGS_TIME,
                            Power::Multiplier => p.multiplier = MULTIPLIER_TIME,
                        }
                        ev.push(RoundEvent::Power(p.id, pw));
                    }
                }
            }
        }
        // Luftringe weit hinter dem Dieb vergessen
        sky.retain(|k| k.z > m.z - 20.0);
    }

    /// Nur Host: Fang und Rundenende pruefen, Bots Faehigkeiten nutzen lassen.
    pub fn judge(&mut self) -> Vec<RoundEvent> {
        let mut ev = Vec::new();
        if self.phase != Phase::Running {
            return ev;
        }
        let Some(thief) = self.thief().cloned() else {
            return ev;
        };
        let tm = &thief.mover;
        let catcher = self.players.iter().find(|p| {
            p.role == Role::Cop
                && (p.mover.z - tm.z).abs() < CATCH_DIST
                && (p.mover.x - tm.x).abs() < 1.2
                && (p.mover.y - tm.y).abs() < 1.5
        });
        if let Some(c) = catcher {
            let cop = c.id;
            self.finish(false, Some(cop));
            ev.push(RoundEvent::Caught { cop });
            return ev;
        }
        if self.time >= ROUND_TIME {
            self.finish(true, None);
            ev.push(RoundEvent::Escaped);
            return ev;
        }
        // Bot-Cops nutzen Faehigkeiten
        let wishes: Vec<(u16, Ability)> = self
            .players
            .iter_mut()
            .filter(|p| p.role == Role::Cop)
            .filter_map(|p| {
                let gap = tm.z - p.mover.z;
                let id = p.id;
                p.bot
                    .as_mut()?
                    .wants_ability(gap, &p.cooldowns)
                    .map(|a| (id, a))
            })
            .collect();
        for (id, a) in wishes {
            if let Some(s) = self.use_ability(id, a) {
                self.spawned.push(s);
                ev.push(RoundEvent::Ability(id, a));
            }
        }
        ev
    }

    /// Nur Host: Faehigkeit eines Cops ausloesen (wenn bereit). Liefert, was entstanden ist.
    pub fn use_ability(&mut self, cop: u16, a: Ability) -> Option<Spawn> {
        if self.phase != Phase::Running || (a != Ability::Whistle && self.team_lock > 0.0) {
            return None;
        }
        let busy = match a {
            Ability::Dog => self.dogs.iter().any(|d| d.alive(self.time)),
            Ability::Tape => self.tapes.len() >= MAX_TAPES,
            Ability::Whistle => false,
        };
        if busy {
            return None;
        }
        let thief = self.thief()?.mover.clone();
        let t = self.time;
        self.next_id += 1;
        let id = self.next_id;
        let p = self.player_mut(cop)?;
        if p.role != Role::Cop || !p.cooldowns.ready(a) {
            return None;
        }
        p.cooldowns.start(a);
        // Pfeife ist nur fuer einen selbst und sperrt den Funk nicht
        if a != Ability::Whistle {
            self.team_lock = TEAM_LOCK;
        }
        let p = self.player_mut(cop)?;
        let spawn = match a {
            Ability::Tape => Spawn::Tape(Tape {
                id,
                lane: thief.lane,
                z: thief.z + TAPE_AHEAD,
                t0: t,
            }),
            Ability::Dog => Spawn::Dog(Dog::new(id, thief.lane, p.mover.z, t, thief.speed)),
            Ability::Whistle => {
                p.whistle = WHISTLE_TIME;
                Spawn::Whistle(cop)
            }
        };
        self.apply_spawn(spawn);
        Some(spawn)
    }

    /// Entstandenes uebernehmen (auch beim Mitspieler, wenn es vom Host kommt).
    pub fn apply_spawn(&mut self, s: Spawn) {
        match s {
            Spawn::Tape(tp) => {
                if !self.tapes.iter().any(|x| x.id == tp.id) {
                    self.tapes.push(tp)
                }
            }
            Spawn::Dog(d) => {
                if !self.dogs.iter().any(|x| x.id == d.id) {
                    self.dogs.push(d)
                }
            }
            Spawn::Whistle(cop) => {
                if let Some(p) = self.player_mut(cop) {
                    p.whistle = WHISTLE_TIME;
                }
            }
        }
    }

    /// Runde beenden und Punkte verteilen.
    pub fn finish(&mut self, thief_won: bool, catcher: Option<u16>) {
        if matches!(self.phase, Phase::Over { .. }) {
            return;
        }
        let survived = self.time.clamp(0.0, ROUND_TIME);
        for p in &mut self.players {
            p.points = match p.role {
                Role::Thief => {
                    survived as i32 + p.rings as i32 / 5 + if thief_won { 50 } else { 0 }
                }
                Role::Cop if Some(p.id) == catcher => 100,
                Role::Cop if catcher.is_some() => 30,
                Role::Cop => 0,
            };
        }
        self.phase = Phase::Over { thief_won, catcher };
    }
}

/// Von einer Faehigkeit erzeugt (geht vom Host an alle).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spawn {
    Tape(Tape),
    Dog(Dog),
    Whistle(u16),
}

/// Naechster Dieb nach `current` in der Reihenfolge der Spieler (reihum).
pub fn next_thief(ids: &[u16], current: Option<u16>) -> u16 {
    match current.and_then(|c| ids.iter().position(|&i| i == c)) {
        Some(i) => ids[(i + 1) % ids.len()],
        None => ids.first().copied().unwrap_or(0),
    }
}

/// Ringspur in der Luft fuer den Raketenflug: schlaengelt sich ueber die Spuren.
/// Aus der Kennung des Power-ups berechnet (gleiche Spur auf allen Rechnern).
pub fn sky_rings(pickup_id: u32, from_z: f32, speed: f32) -> Vec<Pickup> {
    let len = speed * ROCKET_FACTOR * ROCKET_TIME;
    let n = (len / 3.0) as u32;
    (0..n)
        .map(|k| {
            let z = from_z + 12.0 + k as f32 * 3.0;
            let lane = (((k / 6) + pickup_id) % 3) as i8 - 1;
            Pickup {
                id: SKY_IDS
                    .wrapping_add(pickup_id.wrapping_mul(100))
                    .wrapping_add(k),
                lane,
                z,
                y: FLY_H + 0.9,
                power: None,
            }
        })
        .collect()
}
