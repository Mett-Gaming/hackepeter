//! Erfolge (Beschreibung: `erfolge/erfolge.json`): beobachtet die Runden und meldet dem Launcher.
//! Zaehler ueber mehrere Runden liegen in `%APPDATA%\.mettlauncher\hackepeter\erfolge.txt`.

use crate::game::mover::MoveEvent;
use crate::game::round::{Phase, Round, RoundEvent};
use crate::game::Role;
use mettgine::launcher::Achievements;
use mettgine::settings::{self, KeyValues};

const FILE: &str = "erfolge.txt";
/// So nah war der naechste Cop beim Entkommen (Meter)
const CLOSE_CALL: f32 = 4.0;
/// Ringe in einer Runde fuer "Zwiebelkoenig"
const RINGS_IN_ROUND: u32 = 100;
/// Anzahl verschiedener Power-ups
const POWERS: u32 = 6;

/// Zaehler ueber alle Runden.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Totals {
    escapes: u32,
    catches: u32,
    rings: u32,
}

/// Was in der laufenden Runde schon war.
#[derive(Clone, Copy, Debug, Default)]
struct RoundState {
    seed: u64,
    over: bool,
    hits: u32,
    powers: u32,
}

/// Meldung an den Launcher; `None` = freischalten, sonst der Stand.
type Report = (&'static str, Option<u32>);

/// Beobachter der Runden.
#[derive(Default)]
pub struct Tracker {
    totals: Totals,
    round: RoundState,
    /// in `erfolge.txt` schreiben
    keep: bool,
}

impl Tracker {
    pub fn load() -> Self {
        let kv = settings::load(crate::settings::APP, FILE);
        let totals = Totals {
            escapes: kv.u32("escapes", 0, 0, u32::MAX),
            catches: kv.u32("catches", 0, 0, u32::MAX),
            rings: kv.u32("rings", 0, 0, u32::MAX),
        };
        Self {
            totals,
            keep: true,
            ..Default::default()
        }
    }

    fn save(&self) {
        if !self.keep {
            return;
        }
        let mut kv = KeyValues::default();
        kv.set("escapes", self.totals.escapes);
        kv.set("catches", self.totals.catches);
        kv.set("rings", self.totals.rings);
        settings::save(crate::settings::APP, FILE, &kv);
    }

    /// Stand aus frueheren Runden melden (einmal nach dem Start): So geht nichts verloren, was
    /// ohne Launcher gespielt wurde.
    pub fn report_totals(&self, a: &Achievements) {
        send(a, &self.totals_reports());
    }

    fn totals_reports(&self) -> Vec<Report> {
        let t = &self.totals;
        let mut out = Vec::new();
        for (count, first, many) in [
            (t.escapes, "erste_flucht", "fluchtkoenig"),
            (t.catches, "erster_fang", "dienst_nach_vorschrift"),
        ] {
            if count > 0 {
                out.push((first, None));
                out.push((many, Some(count)));
            }
        }
        if t.rings > 0 {
            out.push(("zwiebelvorrat", Some(t.rings)));
        }
        out
    }

    /// Nach jedem Schritt: was in der Runde passiert ist. `humans` = echte Spieler.
    pub fn step(
        &mut self,
        ev: &[RoundEvent],
        round: &Round,
        me: u16,
        humans: usize,
        a: &Achievements,
    ) {
        send(a, &self.observe(ev, round, me, humans));
    }

    fn observe(&mut self, ev: &[RoundEvent], round: &Round, me: u16, humans: usize) -> Vec<Report> {
        let mut out = Vec::new();
        let Some(player) = round.player(me) else {
            return out;
        };
        let fresh = self.round.over && !matches!(round.phase, Phase::Over { .. });
        if self.round.seed != round.track.seed || fresh {
            self.round = RoundState {
                seed: round.track.seed,
                ..Default::default()
            };
        }
        for e in ev {
            match *e {
                RoundEvent::Ring(id) if id == me => self.totals.rings += 1,
                RoundEvent::Power(id, p) if id == me => {
                    self.round.powers |= 1 << p as u32;
                    if p == crate::game::track::Power::Rocket {
                        out.push(("abgehoben", None));
                    }
                    if self.round.powers.count_ones() == POWERS {
                        out.push(("alles_drauf", None));
                    }
                }
                RoundEvent::Move(id, m) if id == me => match m {
                    MoveEvent::Shielded(_) => out.push(("broetchen_panzer", None)),
                    MoveEvent::Crash(_) | MoveEvent::Stumble => self.round.hits += 1,
                    _ => {}
                },
                // Gebissen wird immer der Dieb
                RoundEvent::Bitten(_) if player.role == Role::Thief => self.round.hits += 1,
                _ => {}
            }
        }
        if player.rings >= RINGS_IN_ROUND {
            out.push(("zwiebelkoenig", None));
        }
        if let Phase::Over { thief_won, catcher } = round.phase {
            if !std::mem::replace(&mut self.round.over, true) {
                let close = round.closest_gap() < CLOSE_CALL;
                out.extend(self.finish(player.role, thief_won, catcher == Some(me), close));
                if humans > 1 {
                    out.push(("gemeinsam", None));
                }
            }
        }
        out
    }

    fn finish(&mut self, role: Role, thief_won: bool, caught: bool, close: bool) -> Vec<Report> {
        let mut out = Vec::new();
        if role == Role::Thief && thief_won {
            self.totals.escapes += 1;
            if self.round.hits == 0 {
                out.push(("unberuehrt", None));
            }
            if close {
                out.push(("haarscharf", None));
            }
        }
        if role == Role::Cop && caught {
            self.totals.catches += 1;
        }
        self.save();
        out.extend(self.totals_reports());
        out
    }
}

fn send(a: &Achievements, reports: &[Report]) {
    for (id, value) in reports {
        match value {
            None => a.unlock(id),
            Some(v) => a.progress(id, *v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::round::Entry;
    use crate::game::track::Power;

    const JSON: &str = include_str!("../erfolge/erfolge.json");

    fn round(thief: u16) -> Round {
        let entry = |id| Entry {
            id,
            name: String::new(),
            bot: id != 0,
            local: true,
        };
        let mut r = Round::new(5, &[entry(0), entry(100)], thief);
        r.phase = Phase::Running;
        r
    }

    fn over(r: &mut Round, thief_won: bool, catcher: Option<u16>) {
        r.phase = Phase::Over { thief_won, catcher };
    }

    fn has(out: &[Report], id: &str) -> bool {
        assert!(
            JSON.contains(&format!("\"id\": \"{id}\"")),
            "{id} fehlt in erfolge.json"
        );
        out.iter().any(|r| r.0 == id)
    }

    #[test]
    fn escaping_counts_once_and_untouched_needs_a_clean_run() {
        let (mut t, mut r) = (Tracker::default(), round(0));
        t.observe(&[], &r, 0, 1);
        over(&mut r, true, None);
        let out = t.observe(&[], &r, 0, 1);
        assert!(has(&out, "erste_flucht") && has(&out, "fluchtkoenig") && has(&out, "unberuehrt"));
        assert!(!has(&out, "gemeinsam") && !has(&out, "haarscharf"));
        assert!(t.observe(&[], &r, 0, 1).is_empty(), "nur einmal");
        assert_eq!(t.totals.escapes, 1);
        // naechste Runde, diesmal mit Stolpern und Freunden
        r.phase = Phase::Running;
        t.observe(&[RoundEvent::Bitten(0)], &r, 0, 2);
        over(&mut r, true, None);
        let out = t.observe(&[], &r, 0, 2);
        assert!(!has(&out, "unberuehrt") && has(&out, "gemeinsam"));
        assert!(out.contains(&("fluchtkoenig", Some(2))));
    }

    #[test]
    fn catches_need_the_own_hand() {
        let (mut t, mut r) = (Tracker::default(), round(100));
        over(&mut r, false, Some(100));
        assert!(!has(&t.observe(&[], &r, 0, 1), "erster_fang"));
        r.phase = Phase::Running;
        t.observe(&[], &r, 0, 1);
        over(&mut r, false, Some(0));
        let out = t.observe(&[], &r, 0, 1);
        assert!(has(&out, "erster_fang") && has(&out, "dienst_nach_vorschrift"));
    }

    #[test]
    fn powers_rings_and_shield() {
        let (mut t, mut r) = (Tracker::default(), round(0));
        let all = [
            Power::Mustard,
            Power::Magnet,
            Power::Shield,
            Power::Rocket,
            Power::Springs,
            Power::Multiplier,
        ];
        let ev: Vec<_> = all.iter().map(|&p| RoundEvent::Power(0, p)).collect();
        let out = t.observe(&ev[..5], &r, 0, 1);
        assert!(has(&out, "abgehoben") && !has(&out, "alles_drauf"));
        assert!(has(&t.observe(&ev[5..], &r, 0, 1), "alles_drauf"));
        let other = [RoundEvent::Ring(100), RoundEvent::Ring(0)];
        t.observe(&other, &r, 0, 1);
        assert_eq!(t.totals.rings, 1);
        assert!(has(&t.totals_reports(), "zwiebelvorrat"));
        let shield = [RoundEvent::Move(0, MoveEvent::Shielded(1))];
        assert!(has(&t.observe(&shield, &r, 0, 1), "broetchen_panzer"));
        r.player_mut(0).unwrap().rings = RINGS_IN_ROUND;
        assert!(has(&t.observe(&[], &r, 0, 1), "zwiebelkoenig"));
    }
}
