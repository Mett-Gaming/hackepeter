//! Faehigkeiten des Ordnungsamts: Absperrband, Dackel, Trillerpfeife. Der Host entscheidet,
//! wann sie ausgeloest werden; ihre Wirkung ist aus Startzeit und -ort berechenbar, damit alle
//! Rechner dasselbe sehen.

use super::track::{Kind, Obstacle};
use super::{lane_x, LANES};

/// Eine Faehigkeit der Cops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ability {
    /// Absperrband vor den Dieb (drueberspringen)
    Tape,
    /// Dackel rennt dem Dieb hinterher (drueberspringen)
    Dog,
    /// kurzer Sprint fuer den Cop
    Whistle,
}

pub const ABILITIES: [Ability; 3] = [Ability::Tape, Ability::Dog, Ability::Whistle];

impl Ability {
    /// Abklingzeit (Sekunden).
    pub fn cooldown(self) -> f32 {
        match self {
            Ability::Tape => 12.0,
            Ability::Dog => 15.0,
            Ability::Whistle => 10.0,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Ability::Tape => 0,
            Ability::Dog => 1,
            Ability::Whistle => 2,
        }
    }

    pub fn from_index(i: u8) -> Option<Ability> {
        ABILITIES.get(i as usize).copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            Ability::Tape => "Absperrband",
            Ability::Dog => "Dackel",
            Ability::Whistle => "Trillerpfeife",
        }
    }
}

/// So weit vor dem Dieb wird das Absperrband gespannt (Meter)
pub const TAPE_AHEAD: f32 = 32.0;
/// Pfeifen-Sprint: Dauer (s) und Tempo-Faktor
pub const WHISTLE_TIME: f32 = 2.5;
pub const WHISTLE_FACTOR: f32 = 1.3;
/// Der Dackel ist so viel schneller als der Dieb beim Loslaufen (m/s) ...
const DOG_EXTRA: f32 = 7.0;
/// ... und gibt nach dieser Zeit auf (s)
pub const DOG_LIFE: f32 = 7.0;

/// Ein gespanntes Absperrband (Hindernis auf Zeit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tape {
    pub id: u32,
    pub lane: i8,
    pub z: f32,
    /// Rundenzeit, zu der es gespannt wurde
    pub t0: f32,
}

impl Tape {
    pub fn obstacle(&self) -> Obstacle {
        Obstacle {
            kind: Kind::Tape,
            lane: self.lane,
            z: self.z,
            id: self.id,
        }
    }
}

/// Ein losgelassener Dackel: laeuft gerade auf seiner Spur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dog {
    pub id: u32,
    pub lane: i8,
    pub z0: f32,
    pub t0: f32,
    pub speed: f32,
    /// hat den Dieb schon erwischt oder verpasst
    pub done: bool,
}

impl Dog {
    pub fn new(id: u32, lane: i8, z0: f32, t0: f32, thief_speed: f32) -> Self {
        Self {
            id,
            lane: lane.clamp(-1, LANES - 2),
            z0,
            t0,
            speed: thief_speed + DOG_EXTRA,
            done: false,
        }
    }

    /// Position zur Rundenzeit `t`.
    pub fn z(&self, t: f32) -> f32 {
        self.z0 + self.speed * (t - self.t0).max(0.0)
    }

    pub fn x(&self) -> f32 {
        lane_x(self.lane)
    }

    pub fn alive(&self, t: f32) -> bool {
        !self.done && t - self.t0 < DOG_LIFE
    }

    /// Erwischt der Dackel einen Dieb (Fuesse bei Hoehe `y`, Spur-x `x`, Position `z`)?
    /// Er rennt durch ihn hindurch; wer gerade springt, bleibt verschont.
    pub fn bites(&self, t: f32, dt: f32, x: f32, y: f32, z: f32) -> bool {
        let (now, before) = (self.z(t), self.z(t - dt));
        self.alive(t) && (x - self.x()).abs() < 1.0 && before < z && now >= z && y < 0.45
    }
}

/// Abklingzeiten eines Cops.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cooldowns(pub [f32; 3]);

impl Cooldowns {
    pub fn ready(&self, a: Ability) -> bool {
        self.0[a.index()] <= 0.0
    }
    pub fn start(&mut self, a: Ability) {
        self.0[a.index()] = a.cooldown();
    }
    pub fn tick(&mut self, dt: f32) {
        for c in &mut self.0 {
            *c = (*c - dt).max(0.0);
        }
    }
    /// Anteil der Abklingzeit, der noch fehlt (1 = gerade benutzt, 0 = bereit).
    pub fn fraction(&self, a: Ability) -> f32 {
        (self.0[a.index()] / a.cooldown()).clamp(0.0, 1.0)
    }
}
