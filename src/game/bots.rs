//! Bots: steuern Dieb oder Cops, wenn Plaetze frei sind (Einzelspieler oder aufgefuellte
//! Lobby). Sie schauen auf ihrer Spur voraus, springen, rutschen und weichen aus. Ab und zu
//! reagieren sie zu spaet, damit man sie schlagen kann.

use super::cops::{Ability, Cooldowns, Dog};
use super::mover::{Controls, Mover};
use super::track::{Kind, Obstacle, Track};
use super::{lane_x, Role};
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;

/// Wie weit ein Bot vorausschaut (Sekunden Laufzeit)
const LOOKAHEAD: f32 = 1.4;
/// Wahrscheinlichkeit, ein Hindernis zu verschlafen (je Hindernis)
const MISS_CHANCE: f64 = 0.06;
/// Mindestabstand zwischen zwei Entscheidungen (s), damit Bots nicht zappeln
const THINK_GAP: f32 = 0.12;

#[derive(Clone, Debug)]
pub struct Brain {
    rng: Pcg32,
    last_think: f32,
    /// Hindernisse, die dieser Bot absichtlich verschlaeft
    sleepy: Vec<u32>,
    decided: Vec<u32>,
    /// Zufaellige Pause, bevor eine Faehigkeit genutzt wird (s)
    ability_delay: f32,
}

impl Brain {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Pcg32::seed_from_u64(seed),
            last_think: -10.0,
            sleepy: Vec::new(),
            decided: Vec::new(),
            ability_delay: 2.0,
        }
    }

    /// Erstes Hindernis auf Spur `lane` vor `m` (Abstand und Hindernis), das im Weg waere.
    fn first_on_lane(m: &Mover, lane: i8, obs: &[Obstacle], range: f32) -> Option<(f32, Obstacle)> {
        obs.iter()
            // Bruecken sind von unten kein Hindernis
            .filter(|o| o.lane == lane && !o.kind.overhead())
            .filter_map(|o| {
                let s = o.start(m.z);
                let d = s - m.z;
                // Daecher, auf denen wir schon stehen, sind kein Hindernis
                let roof = o.kind.walkable() && m.y >= o.kind.height() - 0.4;
                (d > -0.2 && d < range && !roof).then_some((d, *o))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// Wie gut eine Spur ist: Abstand bis zum ersten echten Hindernis (Rampen, Niedriges und
    /// Balken zaehlen als machbar).
    fn lane_score(m: &Mover, lane: i8, obs: &[Obstacle], range: f32) -> f32 {
        let blocking = obs.iter().filter(|o| {
            o.lane == lane
                && o.kind.blocks_lane()
                && !(o.kind.walkable() && m.y >= o.kind.height() - 0.4)
        });
        blocking
            .filter_map(|o| {
                let d = o.start(m.z) - m.z;
                // Bahn mit Rampe davor ist begehbar
                let ramped = obs.iter().any(|r| {
                    r.kind == Kind::Ramp
                        && r.lane == lane
                        && (r.end(m.z) - o.start(m.z)).abs() < 1.0
                });
                (d > -o.kind.length() && !ramped).then_some(d.max(0.0))
            })
            .fold(range, f32::min)
    }

    /// Eingabe fuer diesen Schritt.
    #[allow(clippy::too_many_arguments)]
    pub fn think(
        &mut self,
        track: &Track,
        extra: &[Obstacle],
        m: &Mover,
        role: Role,
        thief_lane: i8,
        dogs: &[Dog],
        time: f32,
    ) -> Controls {
        let mut c = Controls::default();
        if m.crashed() || time - self.last_think < THINK_GAP {
            return c;
        }
        self.last_think = time;
        let range = (m.speed * LOOKAHEAD).max(8.0) + 6.0;
        let mut obs = track.obstacles_in(m.z - 1.0, m.z + range + 15.0, m.z);
        obs.extend_from_slice(extra);
        // Hindernis direkt voraus
        if let Some((d, o)) = Self::first_on_lane(m, m.lane, &obs, range) {
            if !self.decided.contains(&o.id) {
                self.decided.push(o.id);
                if self.rng.gen_bool(MISS_CHANCE) {
                    self.sleepy.push(o.id);
                }
            }
            if !self.sleepy.contains(&o.id) {
                let reach = m.speed.max(4.0);
                match o.kind {
                    k if k.low() && d < reach * 0.32 => c.jump = true,
                    Kind::Beam if d < reach * 0.3 => c.slide = true,
                    Kind::Ramp => {}
                    k if !k.low() && k != Kind::Beam => {
                        let here = Self::lane_score(m, m.lane, &obs, range);
                        if here < range * 0.8 {
                            c = self.dodge(m, &obs, range, role, thief_lane);
                        }
                    }
                    _ => {}
                }
            }
        } else if role == Role::Cop && m.lane != thief_lane {
            // freie Bahn: Cops ziehen zur Spur des Diebs
            let target = m.lane + (thief_lane - m.lane).signum();
            if Self::lane_score(m, target, &obs, range) >= range * 0.9 {
                c.left = target > m.lane;
                c.right = target < m.lane;
            }
        }
        // Dieb: ueber den Dackel springen
        if role == Role::Thief {
            let danger = dogs.iter().any(|d| {
                d.alive(time) && (d.x() - lane_x(m.lane)).abs() < 1.0 && {
                    // springen, sodass man beim Durchrennen in der Luft ist
                    let gap = m.z - d.z(time);
                    let closing = (d.speed - m.speed).max(1.0);
                    gap > 0.0 && gap < closing * 0.38
                }
            });
            if danger && m.on_ground {
                c.jump = true;
            }
        }
        if self.decided.len() > 64 {
            self.decided.drain(..32);
            self.sleepy.retain(|id| self.decided.contains(id));
        }
        c
    }

    fn dodge(
        &mut self,
        m: &Mover,
        obs: &[Obstacle],
        range: f32,
        role: Role,
        thief: i8,
    ) -> Controls {
        let mut best = m.lane;
        let mut best_score = Self::lane_score(m, m.lane, obs, range);
        for d in [-1i8, 1] {
            let l = m.lane + d;
            if !(-1..=1).contains(&l) {
                continue;
            }
            let mut s = Self::lane_score(m, l, obs, range);
            if role == Role::Cop && l == thief {
                s += 2.0;
            }
            if s > best_score + 0.5 {
                best = l;
                best_score = s;
            }
        }
        Controls {
            left: best > m.lane,
            right: best < m.lane,
            ..Default::default()
        }
    }

    /// Cop-Bot: welche Faehigkeit jetzt (gap = Abstand zum Dieb in Metern)?
    pub fn wants_ability(&mut self, gap: f32, cd: &Cooldowns) -> Option<Ability> {
        self.ability_delay -= 1.0 / 60.0;
        if self.ability_delay > 0.0 || !(3.0..80.0).contains(&gap) {
            return None;
        }
        let pick = if gap < 18.0 && cd.ready(Ability::Whistle) {
            Some(Ability::Whistle)
        } else if gap < 45.0 && cd.ready(Ability::Dog) {
            Some(Ability::Dog)
        } else if cd.ready(Ability::Tape) {
            Some(Ability::Tape)
        } else {
            None
        };
        if pick.is_some() {
            self.ability_delay = self.rng.gen_range(2.0..6.0);
        }
        pick
    }
}
