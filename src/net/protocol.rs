//! Nachrichten zwischen Host und Mitspielern (kompaktes Binaerformat, erstes Byte = Version,
//! zweites = Art). Unzuverlaessige Nachrichten bleiben unter 1 KB (8 Spieler a 20 Byte).

use crate::game::cops::{Ability, Dog, Tape};
use crate::game::round::Spawn;

/// Version des Protokolls; andere Versionen werden ignoriert
pub const VERSION: u8 = 1;

/// Ein Mitglied der Lobby.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub id: u16,
    pub name: String,
    pub bot: bool,
    /// Punkte ueber alle Runden
    pub score: i32,
}

/// Zustand eines Laeufers, wie er uebers Netz geht.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PlayerState {
    pub id: u16,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub speed: f32,
    pub lane: i8,
    /// Animation (people::Clip als Zahl)
    pub clip: u8,
    /// Bit 0 = Schild, 1 = Senf-Turbo, 2 = Magnet, 3 = Pfeife, 4 = Rakete, 5 = Sprungfedern,
    /// 6 = Mett-Stern
    pub flags: u8,
    pub rings: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    /// Host -> Mitspieler: deine Kennung
    Welcome { id: u16 },
    /// Host -> alle: wer ist dabei
    Lobby { members: Vec<Member> },
    /// Host -> alle: neue Runde
    Start {
        seed: u64,
        thief: u16,
        members: Vec<Member>,
    },
    /// Host -> alle (unzuverlaessig): Rundenzeit und alle Laeufer
    Snapshot {
        time: f32,
        players: Vec<PlayerState>,
    },
    /// Mitspieler -> Host (unzuverlaessig): eigener Zustand
    State(PlayerState),
    /// Mitspieler -> Host: Faehigkeit ausloesen
    UseAbility(Ability),
    /// Host -> alle: Faehigkeit hat etwas erzeugt
    Spawn(Spawn),
    /// Host -> alle: Runde vorbei, Punkte dieser Runde je Spieler
    Over {
        thief_won: bool,
        catcher: Option<u16>,
        points: Vec<(u16, i32)>,
    },
}

#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        let b = &s.as_bytes()[..s.len().min(255)];
        self.u8(b.len() as u8);
        self.0.extend_from_slice(b);
    }
    fn members(&mut self, m: &[Member]) {
        self.u8(m.len() as u8);
        for x in m {
            self.u16(x.id);
            self.str(&x.name);
            self.u8(x.bot as u8);
            self.i32(x.score);
        }
    }
    fn state(&mut self, s: &PlayerState) {
        self.u16(s.id);
        self.f32(s.x);
        self.f32(s.y);
        self.f32(s.z);
        // Tempo in Zentimetern pro Sekunde reicht
        self.u16((s.speed.clamp(0.0, 600.0) * 100.0) as u16);
        self.u8(s.lane as u8);
        self.u8(s.clip);
        self.u8(s.flags);
        self.u16(s.rings);
    }
}

struct R<'a>(&'a [u8], usize);

impl R<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.0.get(self.1..self.1 + n)?;
        self.1 += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        let v = f32::from_le_bytes(self.take(4)?.try_into().ok()?);
        v.is_finite().then_some(v)
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn members(&mut self) -> Option<Vec<Member>> {
        let n = self.u8()? as usize;
        (0..n)
            .map(|_| {
                Some(Member {
                    id: self.u16()?,
                    name: self.str()?,
                    bot: self.u8()? != 0,
                    score: self.i32()?,
                })
            })
            .collect()
    }
    fn state(&mut self) -> Option<PlayerState> {
        Some(PlayerState {
            id: self.u16()?,
            x: self.f32()?,
            y: self.f32()?,
            z: self.f32()?,
            speed: self.u16()? as f32 / 100.0,
            lane: self.u8()? as i8,
            clip: self.u8()?,
            flags: self.u8()?,
            rings: self.u16()?,
        })
    }
}

fn opt_u16(w: &mut W, v: Option<u16>) {
    w.u16(v.map_or(u16::MAX, |x| x));
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W::default();
        w.u8(VERSION);
        match self {
            Msg::Welcome { id } => {
                w.u8(1);
                w.u16(*id);
            }
            Msg::Lobby { members } => {
                w.u8(2);
                w.members(members);
            }
            Msg::Start {
                seed,
                thief,
                members,
            } => {
                w.u8(3);
                w.u64(*seed);
                w.u16(*thief);
                w.members(members);
            }
            Msg::Snapshot { time, players } => {
                w.u8(4);
                w.f32(*time);
                w.u8(players.len() as u8);
                players.iter().for_each(|p| w.state(p));
            }
            Msg::State(s) => {
                w.u8(5);
                w.state(s);
            }
            Msg::UseAbility(a) => {
                w.u8(6);
                w.u8(a.index() as u8);
            }
            Msg::Spawn(s) => {
                w.u8(7);
                match s {
                    Spawn::Tape(t) => {
                        w.u8(0);
                        w.u32(t.id);
                        w.u8(t.lane as u8);
                        w.f32(t.z);
                        w.f32(t.t0);
                    }
                    Spawn::Dog(d) => {
                        w.u8(1);
                        w.u32(d.id);
                        w.u8(d.lane as u8);
                        w.f32(d.z0);
                        w.f32(d.t0);
                        w.f32(d.speed);
                    }
                    Spawn::Whistle(cop) => {
                        w.u8(2);
                        w.u16(*cop);
                    }
                }
            }
            Msg::Over {
                thief_won,
                catcher,
                points,
            } => {
                w.u8(8);
                w.u8(*thief_won as u8);
                opt_u16(&mut w, *catcher);
                w.u8(points.len() as u8);
                for (id, p) in points {
                    w.u16(*id);
                    w.i32(*p);
                }
            }
        }
        w.0
    }

    /// Liest eine Nachricht; None bei fremder Version oder kaputten Daten.
    pub fn decode(b: &[u8]) -> Option<Msg> {
        let mut r = R(b, 0);
        if r.u8()? != VERSION {
            return None;
        }
        Some(match r.u8()? {
            1 => Msg::Welcome { id: r.u16()? },
            2 => Msg::Lobby {
                members: r.members()?,
            },
            3 => Msg::Start {
                seed: r.u64()?,
                thief: r.u16()?,
                members: r.members()?,
            },
            4 => {
                let time = r.f32()?;
                let n = r.u8()? as usize;
                let players = (0..n).map(|_| r.state()).collect::<Option<_>>()?;
                Msg::Snapshot { time, players }
            }
            5 => Msg::State(r.state()?),
            6 => Msg::UseAbility(Ability::from_index(r.u8()?)?),
            7 => Msg::Spawn(match r.u8()? {
                0 => Spawn::Tape(Tape {
                    id: r.u32()?,
                    lane: r.u8()? as i8,
                    z: r.f32()?,
                    t0: r.f32()?,
                }),
                1 => Spawn::Dog(Dog {
                    id: r.u32()?,
                    lane: r.u8()? as i8,
                    z0: r.f32()?,
                    t0: r.f32()?,
                    speed: r.f32()?,
                    done: false,
                }),
                2 => Spawn::Whistle(r.u16()?),
                _ => return None,
            }),
            8 => {
                let thief_won = r.u8()? != 0;
                let c = r.u16()?;
                let n = r.u8()? as usize;
                let points = (0..n)
                    .map(|_| Some((r.u16()?, r.i32()?)))
                    .collect::<Option<_>>()?;
                Msg::Over {
                    thief_won,
                    catcher: (c != u16::MAX).then_some(c),
                    points,
                }
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(id: u16) -> PlayerState {
        PlayerState {
            id,
            x: -2.5,
            y: 3.3,
            z: 1234.5,
            speed: 14.25,
            lane: -1,
            clip: 7,
            flags: 5,
            rings: 321,
        }
    }

    #[test]
    fn every_message_survives_the_roundtrip() {
        let members = vec![
            Member {
                id: 0,
                name: "Hösti".into(),
                bot: false,
                score: 120,
            },
            Member {
                id: 101,
                name: "Bot Brötchen".into(),
                bot: true,
                score: -3,
            },
        ];
        let all = [
            Msg::Welcome { id: 3 },
            Msg::Lobby {
                members: members.clone(),
            },
            Msg::Start {
                seed: u64::MAX - 7,
                thief: 101,
                members,
            },
            Msg::Snapshot {
                time: 42.5,
                players: (0..8).map(st).collect(),
            },
            Msg::State(st(2)),
            Msg::UseAbility(Ability::Dog),
            Msg::Spawn(Spawn::Tape(Tape {
                id: 9,
                lane: 1,
                z: 400.0,
                t0: 12.0,
            })),
            Msg::Spawn(Spawn::Dog(Dog::new(10, -1, 300.0, 15.0, 14.0))),
            Msg::Spawn(Spawn::Whistle(4)),
            Msg::Over {
                thief_won: false,
                catcher: Some(4),
                points: vec![(0, 30), (4, 100)],
            },
            Msg::Over {
                thief_won: true,
                catcher: None,
                points: vec![],
            },
        ];
        for m in all {
            assert_eq!(Msg::decode(&m.encode()), Some(m.clone()), "{m:?}");
        }
    }

    #[test]
    fn snapshots_fit_into_one_datagram_and_garbage_is_rejected() {
        let snap = Msg::Snapshot {
            time: 1.0,
            players: (0..8).map(st).collect(),
        }
        .encode();
        assert!(
            snap.len() < mettgine::net::MAX_UNRELIABLE,
            "{} Byte",
            snap.len()
        );
        assert_eq!(Msg::decode(&[]), None);
        assert_eq!(Msg::decode(&[VERSION + 1, 1, 0, 0]), None);
        assert_eq!(Msg::decode(&snap[..snap.len() - 3]), None, "abgeschnitten");
    }
}
