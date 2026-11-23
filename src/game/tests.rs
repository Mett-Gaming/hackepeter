//! Tests der Spielregeln (ohne Fenster): Strecke, Bewegung, Faehigkeiten, Bots, Runde.

use super::cops::{Ability, Dog};
use super::mover::{Boost, Controls, MoveEvent, Mover};
use super::round::{next_thief, Entry, Phase, Round, RoundEvent, Spawn};
use super::track::{Kind, Obstacle, Track, CHUNK_LEN, SAFE_START, TRAIN_H};
use super::*;

const DT: f32 = 1.0 / 60.0;

fn entries(n: usize, bots: bool) -> Vec<Entry> {
    (0..n)
        .map(|i| Entry {
            id: i as u16,
            name: format!("P{i}"),
            bot: bots,
            local: true,
        })
        .collect()
}

#[test]
fn same_seed_same_track_in_any_order() {
    let a = Track::new(42);
    let b = Track::new(42);
    let forward: Vec<_> = (0..12).map(|i| a.chunk(i).obstacles.clone()).collect();
    let backward: Vec<_> = (0..12)
        .rev()
        .map(|i| b.chunk(i).obstacles.clone())
        .collect();
    for i in 0..12 {
        assert_eq!(
            forward[i],
            backward[11 - i],
            "Abschnitt {i} unterschiedlich"
        );
    }
    assert_ne!(
        Track::new(43).chunk(5).obstacles,
        forward[5],
        "anderer Seed, andere Strecke"
    );
}

#[test]
fn start_is_free_and_every_row_leaves_a_way_through() {
    for seed in 0..20u64 {
        let t = Track::new(seed);
        assert!(t.obstacles_in(0.0, SAFE_START - 1.0, 0.0).is_empty());
        for i in 1..30 {
            let c = t.chunk(i);
            for o in &c.obstacles {
                if o.kind == Kind::MovingTrain {
                    continue;
                }
                // Alle Hindernisse, die zur selben Zeile gehoeren (ueberlappen in z)
                let blocked = (-1..=1i8)
                    .filter(|&l| {
                        c.obstacles.iter().any(|p| {
                            p.lane == l
                                && p.kind.blocks_lane()
                                && p.kind != Kind::MovingTrain
                                && p.z < o.z + o.kind.length()
                                && p.z + p.kind.length() > o.z
                                // Bahn mit Rampe ist begehbar
                                && !c.obstacles.iter().any(|r| {
                                    r.kind == Kind::Ramp
                                        && r.lane == l
                                        && (r.z + r.kind.length() - p.z).abs() < 0.5
                                })
                        })
                    })
                    .count();
                assert!(blocked < 3, "Seed {seed}, Abschnitt {i}: alle Spuren zu");
            }
        }
    }
}

#[test]
fn moving_tram_comes_closer_as_the_observer_runs() {
    let o = Obstacle {
        kind: Kind::MovingTrain,
        lane: 0,
        z: 500.0,
        id: 1,
    };
    assert_eq!(o.start(300.0), 500.0, "steht, solange man weit weg ist");
    let s1 = o.start(450.0);
    let s2 = o.start(470.0);
    assert!(s2 < s1 && s1 < 500.0, "faehrt entgegen");
}

fn run(
    m: &mut Mover,
    t: &Track,
    extra: &[Obstacle],
    secs: f32,
    c: impl Fn(f32) -> Controls,
) -> Vec<MoveEvent> {
    let mut ev = Vec::new();
    let mut time = 0.0;
    while time < secs {
        ev.extend(m.step(DT, c(time), t, extra, 12.0, Boost::default()));
        time += DT;
    }
    ev
}

fn blank_track() -> Track {
    // Seed egal: wir laufen nur im freien Startbereich
    Track::new(7)
}

fn obstacle(kind: Kind, z: f32) -> Obstacle {
    Obstacle {
        kind,
        lane: 0,
        z,
        id: 99,
    }
}

#[test]
fn jumping_clears_barriers_and_running_into_them_crashes() {
    let t = blank_track();
    let wall = [obstacle(Kind::Barrier, 30.0)];
    let mut m = Mover::new(0, 0.0);
    let ev = run(&mut m, &t, &wall, 4.0, |_| Controls::default());
    assert!(
        ev.contains(&MoveEvent::Crash(99)),
        "ohne Sprung: Zusammenstoss"
    );
    let mut m = Mover::new(0, 0.0);
    m.speed = 12.0;
    // rechtzeitig vor der Absperrung springen
    let ev = run(&mut m, &t, &wall, 4.0, |time| Controls {
        jump: (time - 28.0 / 12.0).abs() < DT * 0.6,
        ..Default::default()
    });
    assert!(
        !ev.iter().any(|e| matches!(e, MoveEvent::Crash(_))),
        "{ev:?}"
    );
    assert!(m.z > 40.0);
}

#[test]
fn sliding_passes_under_beams() {
    let t = blank_track();
    let beam = [obstacle(Kind::Beam, 30.0)];
    let mut m = Mover::new(0, 0.0);
    m.speed = 12.0;
    let ev = run(&mut m, &t, &beam, 4.0, |time| Controls {
        slide: (time - 27.0 / 12.0).abs() < DT * 0.6,
        ..Default::default()
    });
    assert!(
        !ev.iter().any(|e| matches!(e, MoveEvent::Crash(_))),
        "{ev:?}"
    );
}

#[test]
fn ramps_lead_onto_the_tram_roof() {
    let t = blank_track();
    let obs = [
        obstacle(Kind::Ramp, 20.0),
        Obstacle {
            id: 98,
            ..obstacle(Kind::Train, 20.0 + Kind::Ramp.length())
        },
    ];
    let mut m = Mover::new(0, 0.0);
    m.speed = 12.0;
    let mut top: f32 = 0.0;
    let mut time = 0.0;
    while time < 3.0 {
        m.step(DT, Controls::default(), &t, &obs, 12.0, Boost::default());
        top = top.max(m.y);
        time += DT;
    }
    assert!((top - TRAIN_H).abs() < 0.1, "nicht aufs Dach: {top}");
    assert!(!m.crashed());
}

#[test]
fn side_bump_bounces_back_and_stumbles() {
    let t = blank_track();
    let truck = [Obstacle {
        lane: 1,
        ..obstacle(Kind::Truck, 10.0)
    }];
    let mut m = Mover::new(0, 0.0);
    m.speed = 12.0;
    let ev = run(&mut m, &t, &truck, 1.5, |time| Controls {
        left: (time - 1.0).abs() < DT * 0.6, // links im Bild = Spur +1
        ..Default::default()
    });
    assert!(ev.contains(&MoveEvent::Stumble), "{ev:?}");
    assert_eq!(m.lane, 0);
}

#[test]
fn shield_absorbs_one_crash() {
    let t = blank_track();
    let wall = [obstacle(Kind::Van, 20.0)];
    let mut m = Mover::new(0, 0.0);
    m.speed = 12.0;
    let mut ev = Vec::new();
    for _ in 0..200 {
        ev.extend(m.step(
            DT,
            Controls::default(),
            &t,
            &wall,
            12.0,
            Boost {
                shield: true,
                ..Default::default()
            },
        ));
    }
    assert!(ev.contains(&MoveEvent::Shielded(99)));
    assert!(!ev.iter().any(|e| matches!(e, MoveEvent::Crash(_))));
}

#[test]
fn dog_bites_unless_the_thief_jumps() {
    let d = Dog::new(1, 0, 0.0, 0.0, 10.0);
    let z = d.z(2.0);
    assert!(d.bites(2.0 + DT, DT * 2.0, 0.0, 0.0, z));
    assert!(!d.bites(2.0 + DT, DT * 2.0, 0.0, 1.0, z), "im Sprung");
    assert!(!d.bites(2.0 + DT, DT * 2.0, 2.5, 0.0, z), "andere Spur");
}

#[test]
fn round_runs_countdown_then_ends_when_time_is_up() {
    let mut r = Round::new(1, &entries(2, false), 0);
    assert_eq!(r.phase, Phase::Countdown);
    r.step(COUNTDOWN + DT, &|_| Controls::default());
    assert_eq!(r.phase, Phase::Running);
    // Cop wird festgehalten, damit er nicht faengt
    r.time = ROUND_TIME;
    let ev = r.judge();
    assert!(ev.contains(&RoundEvent::Escaped));
    assert!(matches!(
        r.phase,
        Phase::Over {
            thief_won: true,
            ..
        }
    ));
    assert!(r.player(0).unwrap().points >= 100);
}

#[test]
fn cop_close_to_the_thief_catches_him() {
    let mut r = Round::new(1, &entries(3, false), 0);
    r.step(COUNTDOWN + DT, &|_| Controls::default());
    let tz = r.thief().unwrap().mover.z;
    let cop = r.player_mut(2).unwrap();
    cop.mover = Mover::new(0, tz - 0.5);
    let ev = r.judge();
    assert!(ev.contains(&RoundEvent::Caught { cop: 2 }));
    assert_eq!(r.player(2).unwrap().points, 100);
    assert_eq!(r.player(1).unwrap().points, 30);
}

#[test]
fn abilities_respect_cooldowns_and_place_things_ahead() {
    let mut r = Round::new(5, &entries(2, false), 0);
    r.step(COUNTDOWN + DT, &|_| Controls::default());
    let tz = r.thief().unwrap().mover.z;
    match r.use_ability(1, Ability::Tape) {
        Some(Spawn::Tape(t)) => assert!(t.z > tz + 20.0),
        other => panic!("kein Band: {other:?}"),
    }
    assert!(r.use_ability(1, Ability::Tape).is_none(), "Abklingzeit");
    assert!(
        r.use_ability(0, Ability::Dog).is_none(),
        "Dieb hat keine Faehigkeiten"
    );
    assert!(matches!(
        r.use_ability(1, Ability::Whistle),
        Some(Spawn::Whistle(1))
    ));
}

#[test]
fn roles_rotate_through_all_players() {
    let ids = [0u16, 3, 7, 100];
    let mut t = next_thief(&ids, None);
    let mut seen = vec![t];
    for _ in 0..3 {
        t = next_thief(&ids, Some(t));
        seen.push(t);
    }
    assert_eq!(seen, ids.to_vec());
    assert_eq!(next_thief(&ids, Some(100)), 0);
}

/// Eine ganze Runde nur mit Bots: es passiert etwas, und sie endet ordentlich.
#[test]
fn bot_round_with_eight_players_plays_to_the_end() {
    for seed in [3u64, 11] {
        let mut r = Round::new(seed, &entries(MAX_PLAYERS, true), 4);
        let mut events = Vec::new();
        for _ in 0..((ROUND_TIME + COUNTDOWN + 1.0) / DT) as usize {
            events.extend(r.step(DT, &|_| Controls::default()));
            events.extend(r.judge());
            if matches!(r.phase, Phase::Over { .. }) {
                break;
            }
        }
        assert!(matches!(r.phase, Phase::Over { .. }), "Runde endet nicht");
        let thief = r.thief().unwrap();
        let crashes_dbg: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    RoundEvent::Move(4, MoveEvent::Crash(_))
                        | RoundEvent::Move(4, MoveEvent::Stumble)
                        | RoundEvent::Bitten(4)
                        | RoundEvent::Caught { .. }
                )
            })
            .collect();
        assert!(
            thief.mover.z > 200.0,
            "Dieb kam nicht weit: {} nach {} s, {:?}",
            thief.mover.z,
            r.time,
            crashes_dbg
        );
        assert!(events.iter().any(|e| matches!(e, RoundEvent::Ring(_))));
        assert!(events.iter().any(|e| matches!(e, RoundEvent::Ability(..))));
        let crashes = events
            .iter()
            .filter(|e| matches!(e, RoundEvent::Move(4, MoveEvent::Crash(_))))
            .count();
        assert!(
            crashes < 12,
            "Dieb-Bot faehrt staendig gegen Hindernisse: {crashes}"
        );
        let _ = CHUNK_LEN;
    }
}

#[test]
fn jump_is_higher_than_before_and_springs_reach_the_roof() {
    use super::mover::{GRAVITY, JUMP_V, SUPER_JUMP_V};
    let apex = |v: f32| v * v / (2.0 * GRAVITY);
    assert!(
        apex(JUMP_V) > 1.7 && apex(JUMP_V) < 2.1,
        "Sprung {}",
        apex(JUMP_V)
    );
    assert!(
        apex(SUPER_JUMP_V) > TRAIN_H + 0.3,
        "Sprungfedern reichen nicht aufs Dach"
    );
    // mit Sprungfedern direkt aufs Zugdach, ohne Rampe
    let t = blank_track();
    let train = [obstacle(Kind::Train, 20.0)];
    let mut m = Mover::new(0, 0.0);
    m.speed = 12.0;
    let springs = Boost {
        springs: true,
        ..Default::default()
    };
    let mut top: f32 = 0.0;
    let mut time: f32 = 0.0;
    let mut crashed = false;
    while time < 2.5 {
        let c = Controls {
            jump: (time - 0.9).abs() < DT * 0.6,
            ..Default::default()
        };
        for e in m.step(DT, c, &t, &train, 12.0, springs) {
            crashed |= matches!(e, MoveEvent::Crash(_));
        }
        if m.on_ground {
            top = top.max(m.y);
        }
        time += DT;
    }
    assert!(!crashed);
    assert!(
        (top - TRAIN_H).abs() < 0.1,
        "nicht auf dem Dach gelandet: {top}"
    );
}

#[test]
fn beams_cannot_be_jumped_over() {
    let t = blank_track();
    let beam = [obstacle(Kind::Beam, 30.0)];
    let mut m = Mover::new(0, 0.0);
    m.speed = 12.0;
    // beste Sprungzeit: Scheitel genau am Balken
    let ev = run(&mut m, &t, &beam, 4.0, |time| Controls {
        jump: (time - 29.0 / 12.0).abs() < DT * 0.6,
        ..Default::default()
    });
    assert!(
        ev.iter().any(|e| matches!(e, MoveEvent::Crash(_))),
        "Balken uebersprungen"
    );
}

#[test]
fn bridges_are_open_below_and_walkable_from_the_roof() {
    use super::track::BRIDGE_LOW;
    let t = blank_track();
    // unten durch: kein Zusammenstoss
    let bridge = [obstacle(Kind::Bridge, 20.0)];
    let mut m = Mover::new(0, 0.0);
    m.speed = 12.0;
    let ev = run(&mut m, &t, &bridge, 3.0, |_| Controls::default());
    assert!(!ev.iter().any(|e| matches!(e, MoveEvent::Crash(_))));
    assert!(m.y < 0.1 && m.z > 25.0);
    assert!(BRIDGE_LOW > 1.9 + 0.5, "Kopf passt drunter");
    // oben drauf: Rampe -> Zug -> lange Bruecke, man bleibt oben
    let obs = [
        obstacle(Kind::Ramp, 20.0),
        Obstacle {
            id: 98,
            ..obstacle(Kind::Train, 26.0)
        },
        Obstacle {
            id: 97,
            ..obstacle(Kind::BridgeLong, 39.0)
        },
    ];
    let mut m = Mover::new(0, 0.0);
    m.speed = 12.0;
    let mut on_bridge = false;
    let mut time = 0.0;
    // bis deutlich hinter das Brueckenende (39 + 14 m) laufen
    while time < 6.0 {
        m.step(DT, Controls::default(), &t, &obs, 12.0, Boost::default());
        if m.z > 42.0 && m.z < 52.0 && (m.y - TRAIN_H).abs() < 0.05 {
            on_bridge = true;
        }
        time += DT;
    }
    assert!(on_bridge, "nicht ueber die Bruecke gelaufen");
    assert!(m.y < 0.1, "am Ende wieder unten");
}

#[test]
fn rocket_flies_over_everything_and_leaves_a_ring_trail() {
    use super::round::{sky_rings, ROCKET_TIME};
    let mut r = Round::new(9, &entries(2, false), 0);
    r.step(COUNTDOWN + DT, &|_| Controls::default());
    let thief = r.player_mut(0).unwrap();
    thief.rocket = ROCKET_TIME;
    let z = thief.mover.z;
    r.sky = sky_rings(1, z, 12.0);
    assert!(r.sky.len() > 15);
    let mut rings = 0;
    for _ in 0..(3.0 / DT) as usize {
        for e in r.step(DT, &|_| Controls::default()) {
            if matches!(e, RoundEvent::Ring(0)) {
                rings += 1;
            }
            assert!(
                !matches!(e, RoundEvent::Move(0, MoveEvent::Crash(_))),
                "im Flug angestossen"
            );
        }
    }
    let thief = r.player(0).unwrap();
    assert!(thief.mover.y > 5.0, "fliegt nicht: {}", thief.mover.y);
    assert!(rings > 3, "Luftringe nicht eingesammelt: {rings}");
}

#[test]
fn multiplier_doubles_rings() {
    let mut r = Round::new(4, &entries(2, false), 0);
    r.step(COUNTDOWN + DT, &|_| Controls::default());
    let p = r.player(0).unwrap();
    let (lane, z) = (p.mover.lane, p.mover.z);
    // ein Ring direkt vor dem Dieb, auf Laufhoehe
    r.sky = vec![super::track::Pickup {
        id: 4_000_000_000,
        lane,
        z: z + 0.3,
        y: 0.9,
        power: None,
    }];
    r.player_mut(0).unwrap().multiplier = 10.0;
    r.step(DT, &|_| Controls::default());
    assert_eq!(r.player(0).unwrap().rings, 2, "Ring zaehlt doppelt");
}
