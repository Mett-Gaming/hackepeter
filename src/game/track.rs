//! Die Strecke: Stadtstrasse mit Strassenbahngleisen, in Abschnitten (Chunks) aus dem Seed
//! gebaut. Gleicher Seed = gleiche Strecke auf allen Rechnern, unabhaengig davon, in welcher
//! Reihenfolge die Abschnitte gebraucht werden.
//!
//! Jede Zeile laesst mindestens eine Spur frei oder ueberwindbar (Sprung, Rutschen, Dach).

use super::{lane_x, LANE_W};
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;
use std::cell::RefCell;
use std::collections::HashMap;

/// Laenge eines Abschnitts (Meter)
pub const CHUNK_LEN: f32 = 60.0;
/// Bis hier ist die Strecke frei (Start am Mettbroetchen-Stand)
pub const SAFE_START: f32 = 70.0;
/// Hoehe der Strassenbahn (Dach zum Drueberlaufen)
pub const TRAIN_H: f32 = 3.3;
pub const TRAIN_LEN: f32 = 13.0;
/// Unter- und Oberkante des Baustellenbalkens (drunter durchrutschen)
pub const BEAM_LOW: f32 = 1.15;
pub const BEAM_HIGH: f32 = 1.65;
/// Entgegenkommende Bahn: faehrt los, wenn der Betrachter so nah ist (Meter) ...
const MOVE_TRIGGER: f32 = 90.0;
/// ... und legt pro Meter des Betrachters so viel Weg zurueck
const MOVE_RATIO: f32 = 0.6;
/// halbe Breite eines Hindernisses auf seiner Spur (Meter)
pub const HALF_W: f32 = 1.12;

/// Art eines Hindernisses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// stehende Strassenbahn (Dach begehbar)
    Train,
    /// entgegenkommende Strassenbahn
    MovingTrain,
    Truck,
    Van,
    PoliceCar,
    /// niedrige Absperrung: drueberspringen
    Barrier,
    BarrierLarge,
    /// Baustellenbalken: drunter durchrutschen
    Beam,
    /// Rampe aufs Strassenbahndach
    Ramp,
    /// Absperrband des Ordnungsamts (Faehigkeit): drueberspringen
    Tape,
    /// Fussgaengerbruecke: unten durchlaufen, oben drauf (vom Zugdach oder mit Sprungfedern)
    Bridge,
    /// lange Hochbruecke (Hochweg ueber mehrere Zuege)
    BridgeLong,
}

/// Unterkante der Bruecken (Meter); oben liegen sie auf Zughoehe
pub const BRIDGE_LOW: f32 = 2.95;

impl Kind {
    /// Hoehe der Oberkante (Meter).
    pub fn height(self) -> f32 {
        match self {
            Kind::Train | Kind::MovingTrain | Kind::Ramp | Kind::Bridge | Kind::BridgeLong => {
                TRAIN_H
            }
            Kind::Truck => 2.7,
            Kind::Van => 2.2,
            Kind::PoliceCar => 1.6,
            Kind::Barrier => 0.9,
            Kind::BarrierLarge => 1.0,
            Kind::Beam => BEAM_HIGH,
            Kind::Tape => 1.0,
        }
    }

    /// Laenge entlang der Strecke (Meter).
    pub fn length(self) -> f32 {
        match self {
            Kind::Train | Kind::MovingTrain => TRAIN_LEN,
            Kind::Truck => 5.6,
            Kind::Van => 4.8,
            Kind::PoliceCar => 4.4,
            Kind::Barrier | Kind::Tape => 0.45,
            Kind::BarrierLarge => 0.7,
            Kind::Beam => 0.5,
            Kind::Ramp => 6.0,
            Kind::Bridge => 4.0,
            Kind::BridgeLong => 14.0,
        }
    }

    /// Man kann darauf stehen (Dach, Rampe, Bruecke).
    pub fn walkable(self) -> bool {
        matches!(
            self,
            Kind::Train
                | Kind::MovingTrain
                | Kind::Truck
                | Kind::Van
                | Kind::Ramp
                | Kind::Bridge
                | Kind::BridgeLong
        )
    }

    /// Niedrig: mit einem Sprung zu schaffen.
    pub fn low(self) -> bool {
        matches!(self, Kind::Barrier | Kind::BarrierLarge | Kind::Tape)
    }

    /// Unten frei: man laeuft drunter durch (Bruecken).
    pub fn overhead(self) -> bool {
        matches!(self, Kind::Bridge | Kind::BridgeLong)
    }

    /// Sperrt die Spur am Boden (weder Sprung, Rutschen, Rampe noch Durchgang hilft).
    pub fn blocks_lane(self) -> bool {
        !self.low() && !self.overhead() && !matches!(self, Kind::Beam | Kind::Ramp)
    }
}

/// Ein Hindernis. `z` = Anfang (dem Laeufer zugewandte Seite).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    pub kind: Kind,
    pub lane: i8,
    pub z: f32,
    /// Kennung (Abschnitt * 1000 + Nummer), z. B. damit ein Schild ein Hindernis "verbraucht"
    pub id: u32,
}

impl Obstacle {
    /// Anfang fuer einen Betrachter bei `observer_z` (entgegenkommende Bahnen fahren).
    pub fn start(&self, observer_z: f32) -> f32 {
        if self.kind == Kind::MovingTrain {
            self.z - MOVE_RATIO * (observer_z - (self.z - MOVE_TRIGGER)).max(0.0)
        } else {
            self.z
        }
    }

    /// Ende entlang der Strecke fuer einen Betrachter.
    pub fn end(&self, observer_z: f32) -> f32 {
        self.start(observer_z) + self.kind.length()
    }

    /// Hoehe der begehbaren Oberflaeche bei `z` (Rampe steigt an), None ausserhalb.
    pub fn surface(&self, z: f32, observer_z: f32) -> Option<f32> {
        let (s, e) = (self.start(observer_z), self.end(observer_z));
        if !self.kind.walkable() || z < s || z > e {
            return None;
        }
        Some(if self.kind == Kind::Ramp {
            self.kind.height() * ((z - s) / (e - s)).clamp(0.0, 1.0)
        } else {
            self.kind.height()
        })
    }

    pub fn overlaps_x(&self, x: f32, half: f32) -> bool {
        (x - lane_x(self.lane)).abs() < HALF_W + half
    }
}

/// Art eines Power-ups des Diebs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Power {
    /// Senf-Turbo: schneller
    Mustard,
    /// Zwiebelmagnet: sammelt Ringe auf allen Spuren
    Magnet,
    /// Broetchen-Schild: faengt einen Zusammenstoss ab
    Shield,
    /// Pfeffer-Rakete: fliegt eine Weile ueber alles hinweg, mit Ringspur in der Luft
    Rocket,
    /// Sprungfedern: viel hoehere Spruenge (bis aufs Zugdach)
    Springs,
    /// Mett-Stern: Zwiebelringe zaehlen doppelt
    Multiplier,
}

pub const ALL_POWERS: [Power; 6] = [
    Power::Mustard,
    Power::Magnet,
    Power::Shield,
    Power::Rocket,
    Power::Springs,
    Power::Multiplier,
];

/// Etwas zum Einsammeln.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pickup {
    pub id: u32,
    pub lane: i8,
    pub z: f32,
    pub y: f32,
    /// None = Zwiebelring
    pub power: Option<Power>,
}

#[derive(Clone, Debug, Default)]
pub struct Chunk {
    pub obstacles: Vec<Obstacle>,
    pub pickups: Vec<Pickup>,
}

/// Die ganze Strecke; Abschnitte werden bei Bedarf gebaut und gemerkt.
pub struct Track {
    pub seed: u64,
    cache: RefCell<HashMap<i32, std::rc::Rc<Chunk>>>,
}

impl Track {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// Abschnitt `i` (z von i*CHUNK_LEN bis (i+1)*CHUNK_LEN).
    pub fn chunk(&self, i: i32) -> std::rc::Rc<Chunk> {
        if let Some(c) = self.cache.borrow().get(&i) {
            return c.clone();
        }
        let c = std::rc::Rc::new(generate(self.seed, i));
        let mut cache = self.cache.borrow_mut();
        if cache.len() > 64 {
            cache.retain(|k, _| (k - i).abs() < 8);
        }
        cache.insert(i, c.clone());
        c
    }

    /// Alle Abschnitte, die den Bereich z0..z1 beruehren (entgegenkommende Bahnen beachtet).
    pub fn chunks_in(&self, z0: f32, z1: f32) -> Vec<std::rc::Rc<Chunk>> {
        let a = ((z0 - MOVE_TRIGGER) / CHUNK_LEN).floor() as i32 - 1;
        let b = ((z1 + MOVE_TRIGGER) / CHUNK_LEN).floor() as i32 + 1;
        (a.max(0)..=b).map(|i| self.chunk(i)).collect()
    }

    /// Hindernisse, die fuer einen Betrachter bei `observer_z` zwischen z0 und z1 liegen.
    pub fn obstacles_in(&self, z0: f32, z1: f32, observer_z: f32) -> Vec<Obstacle> {
        let mut out = Vec::new();
        for c in self.chunks_in(z0, z1) {
            for o in &c.obstacles {
                if o.end(observer_z) >= z0 && o.start(observer_z) <= z1 {
                    out.push(*o);
                }
            }
        }
        out
    }

    pub fn pickups_in(&self, z0: f32, z1: f32) -> Vec<Pickup> {
        let mut out = Vec::new();
        let a = (z0 / CHUNK_LEN).floor().max(0.0) as i32;
        let b = (z1 / CHUNK_LEN).floor() as i32;
        for i in a..=b {
            out.extend(
                self.chunk(i)
                    .pickups
                    .iter()
                    .filter(|p| p.z >= z0 && p.z <= z1),
            );
        }
        out
    }
}

/// Schwierigkeit 0..1 eines Abschnitts (waechst mit der Strecke).
fn difficulty(i: i32) -> f32 {
    (i as f32 / 14.0).clamp(0.0, 1.0)
}

fn generate(seed: u64, i: i32) -> Chunk {
    let mut rng = Pcg32::seed_from_u64(seed ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mut c = Chunk::default();
    let z0 = i as f32 * CHUNK_LEN;
    let d = difficulty(i);
    let mut next_id = i.max(0) as u32 * 1000;
    let mut id = || {
        next_id += 1;
        next_id
    };
    let mut z = z0 + rng.gen_range(4.0..10.0);
    // Abstand zwischen Hindernis-Zeilen schrumpft mit der Schwierigkeit
    let gap = |rng: &mut Pcg32| rng.gen_range(14.0..22.0) - 5.0 * d;
    while z < z0 + CHUNK_LEN - 6.0 {
        if z < SAFE_START {
            z += 10.0;
            continue;
        }
        let used = row(&mut rng, &mut c, z, d, &mut id);
        z += used + gap(&mut rng);
    }
    // Ringe in Lauflinien ohne Hindernisse
    rings(&mut rng, &mut c, z0, &mut id);
    if z0 >= SAFE_START && rng.gen_bool(0.55) {
        let lane = rng.gen_range(-1..=1);
        let pz = z0 + rng.gen_range(10.0..50.0);
        if free_at(&c, lane, pz - 2.0, pz + 2.0) {
            let power = ALL_POWERS[rng.gen_range(0..ALL_POWERS.len())];
            c.pickups.push(Pickup {
                id: id(),
                lane,
                z: pz,
                y: 1.0,
                power: Some(power),
            });
        }
    }
    c
}

/// Eine Hindernis-Zeile ab z. Liefert ihre Laenge.
fn row(rng: &mut Pcg32, c: &mut Chunk, z: f32, d: f32, id: &mut impl FnMut() -> u32) -> f32 {
    let mut push = |c: &mut Chunk, kind, lane, z| {
        c.obstacles.push(Obstacle {
            kind,
            lane,
            z,
            id: id(),
        })
    };
    let pick = rng.gen_range(0.0..1.0f32);
    let free = rng.gen_range(-1..=1i8);
    let others: Vec<i8> = (-1..=1).filter(|&l| l != free).collect();
    if pick < 0.28 {
        // Strassenbahnen: eine oder zwei Spuren, manchmal mit Rampe aufs Dach
        let n = if rng.gen_bool(0.3 + 0.4 * d as f64) {
            2
        } else {
            1
        };
        for &l in others.iter().take(n) {
            let ramp = rng.gen_bool(0.45);
            if ramp {
                push(c, Kind::Ramp, l, z - Kind::Ramp.length());
            }
            push(c, Kind::Train, l, z);
        }
        TRAIN_LEN
    } else if pick < 0.36 && d > 0.25 {
        // entgegenkommende Bahn auf einer Spur
        push(c, Kind::MovingTrain, others[0], z + 25.0);
        8.0
    } else if pick < 0.44 {
        // Fussgaengerbruecke ueber alle Spuren, darunter manchmal ein Hindernis
        for l in -1..=1 {
            push(c, Kind::Bridge, l, z);
        }
        if rng.gen_bool(0.5) {
            let k = if rng.gen_bool(0.5) {
                Kind::Barrier
            } else {
                Kind::Beam
            };
            push(c, k, others[0], z + 1.5);
        }
        Kind::Bridge.length()
    } else if pick < 0.5 {
        // Hochweg: Rampe, Zug und direkt dahinter eine lange Bruecke ueber die Strasse.
        // Wer oben bleibt, laeuft ueber die Bruecke weiter; unten geht es drunter durch.
        let l = others[0];
        push(c, Kind::Ramp, l, z - Kind::Ramp.length());
        push(c, Kind::Train, l, z);
        let bz = z + TRAIN_LEN;
        for b in -1..=1 {
            push(c, Kind::BridgeLong, b, bz);
        }
        // unter der Bruecke: ein Fahrzeug auf einer Nachbarspur (oben laeuft man drueber)
        if rng.gen_bool(0.5 + 0.4 * d as f64) {
            push(c, Kind::Truck, others[1], bz + 4.0);
        }
        TRAIN_LEN + Kind::BridgeLong.length()
    } else if pick < 0.62 {
        // niedrige Absperrungen, auf schweren Strecken ueber zwei Spuren
        let kind = if rng.gen_bool(0.5) {
            Kind::Barrier
        } else {
            Kind::BarrierLarge
        };
        push(c, kind, others[0], z);
        if rng.gen_bool(0.2 + 0.5 * d as f64) {
            push(c, kind, others[1], z);
        }
        if rng.gen_bool(0.25 * d as f64) {
            push(c, Kind::Beam, free, z);
        }
        2.0
    } else if pick < 0.75 {
        // Baustellenbalken
        push(c, Kind::Beam, others[0], z);
        if rng.gen_bool(0.4) {
            push(c, Kind::Beam, others[1], z);
        }
        2.0
    } else {
        // Fahrzeuge auf einer oder zwei Spuren
        let kinds = [Kind::Truck, Kind::Van, Kind::PoliceCar];
        let n = if rng.gen_bool(0.3 + 0.3 * d as f64) {
            2
        } else {
            1
        };
        let mut len = 0.0f32;
        for &l in others.iter().take(n) {
            let k = kinds[rng.gen_range(0..kinds.len())];
            push(c, k, l, z);
            len = len.max(k.length());
        }
        len
    }
}

/// Liegt auf der Spur zwischen z0 und z1 nichts im Weg?
fn free_at(c: &Chunk, lane: i8, z0: f32, z1: f32) -> bool {
    c.obstacles.iter().all(|o| {
        o.lane != lane
            || o.kind == Kind::MovingTrain
            || o.kind.overhead()
            || o.z > z1
            || o.z + o.kind.length() < z0
    })
}

fn rings(rng: &mut Pcg32, c: &mut Chunk, z0: f32, id: &mut impl FnMut() -> u32) {
    if z0 < 20.0 {
        return;
    }
    for _ in 0..2 {
        let lane = rng.gen_range(-1..=1i8);
        let start = z0 + rng.gen_range(0.0..30.0);
        let count = rng.gen_range(5..10);
        for k in 0..count {
            let z = start + k as f32 * 2.6;
            // Ring ueber einem Hindernis: auf dem Dach oder im Sprungbogen
            let top = c
                .obstacles
                .iter()
                .filter(|o| o.lane == lane && o.kind != Kind::MovingTrain)
                .filter(|o| z >= o.z - 1.5 && z <= o.z + o.kind.length() + 1.5)
                .map(|o| match o.kind {
                    Kind::Beam => 0.0,
                    Kind::Ramp => o.surface(z, 0.0).unwrap_or(0.0),
                    // Bruecke: oben nur, wenn man vom Zugdach draufkommt, sonst darunter
                    k if k.overhead() => {
                        let from_roof = c.obstacles.iter().any(|t| {
                            t.lane == lane
                                && t.kind == Kind::Train
                                && (t.z + TRAIN_LEN - o.z).abs() < 1.0
                        });
                        if from_roof {
                            k.height()
                        } else {
                            0.0
                        }
                    }
                    k => k.height(),
                })
                .fold(0.0f32, f32::max);
            let roof_lane = c
                .obstacles
                .iter()
                .any(|o| o.lane == lane && (o.kind == Kind::Train || o.kind == Kind::Ramp));
            if top > 2.0 && !roof_lane {
                continue;
            }
            c.pickups.push(Pickup {
                id: id(),
                lane,
                z,
                y: top + 0.9,
                power: None,
            });
        }
    }
}

/// Breite der Strasse (alle Spuren plus Rand), fuer die Grafik
pub const ROAD_HALF: f32 = 1.5 * LANE_W + 0.4;
