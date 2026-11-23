//! Eine Spielsitzung: Lobby, Runden nacheinander mit wechselndem Dieb, Abgleich ueber das Netz.
//!
//! Host-Autoritaet: Der Host startet Runden (Seed, Dieb), entscheidet Fang und Rundenende,
//! loest Faehigkeiten aus und schickt 20-mal pro Sekunde den Stand aller Laeufer. Jeder
//! Mitspieler simuliert seine eigene Figur und schickt 30-mal pro Sekunde seinen Zustand.
//! Fremde Figuren werden vorhergesagt und weich nachgezogen. Einzelspieler = Host ohne Netz.

use super::protocol::{Member, Msg, PlayerState};
use crate::game::bots::Brain;
use crate::game::cops::Ability;
use crate::game::mover::Controls;
use crate::game::round::{next_thief, Entry, Phase, Round, RoundEvent};
use crate::game::MAX_PLAYERS;
use mettgine::net::{Delivery, NetEvent, Transport, BROADCAST, HOST};
use std::collections::HashMap;

/// Sendetakt: Mitspieler-Zustand und Host-Stand (s)
const STATE_EVERY: f32 = 1.0 / 30.0;
const SNAPSHOT_EVERY: f32 = 1.0 / 20.0;
/// Pause zwischen zwei Runden (Punkte ansehen, s)
pub const OVER_PAUSE: f32 = 7.0;
/// Bots bekommen Kennungen ab hier
const BOT_BASE: u16 = 100;
/// Wie schnell fremde Figuren auf ihren vorhergesagten Platz gezogen werden (1/s)
const SMOOTH: f32 = 12.0;

const BOT_NAMES: [&str; 8] = [
    "Mettigel",
    "Zwiebelmett",
    "Thüringer",
    "Schweinemett",
    "Mettenden",
    "Hackfleisch",
    "Brötchenbert",
    "Senfsepp",
];

pub struct Session {
    transport: Option<Box<dyn Transport>>,
    pub host: bool,
    pub my_id: u16,
    pub members: Vec<Member>,
    /// Code zum Beitreten (vom Launcher), sobald gehostet wird
    pub code: Option<String>,
    pub round: Option<Round>,
    /// Grund, warum die Sitzung vorbei ist (zurueck ins Menue)
    pub ended: Option<String>,
    /// letzte Fehlermeldung des Netzes (fuer das HUD)
    pub notice: Option<String>,
    /// Nur Host: Lobby mit Bots auf so viele Spieler auffuellen
    pub fill_to: usize,
    last_thief: Option<u16>,
    remote: HashMap<u16, (PlayerState, f32)>,
    clock: f32,
    send_timer: f32,
    /// Zeit seit Rundenende (s)
    pub over_timer: f32,
    over_sent: bool,
    seed_counter: u64,
}

impl Session {
    /// Einzelspieler: Host ohne Netz, mit `bots` Bot-Gegnern.
    pub fn solo(name: &str, bots: usize) -> Self {
        let mut s = Self::new(None, true, name);
        s.fill_to = (bots + 1).min(MAX_PLAYERS);
        s.fill_bots();
        s
    }

    /// Mehrspieler ueber eine Verbindung (Host oder Mitspieler je nach Transport).
    pub fn network(transport: Box<dyn Transport>, name: &str) -> Self {
        let host = transport.is_host();
        Self::new(Some(transport), host, name)
    }

    fn new(transport: Option<Box<dyn Transport>>, host: bool, name: &str) -> Self {
        let members = if host {
            vec![Member {
                id: HOST,
                name: name.into(),
                bot: false,
                score: 0,
            }]
        } else {
            Vec::new()
        };
        Self {
            transport,
            host,
            my_id: if host { HOST } else { u16::MAX },
            members,
            code: None,
            round: None,
            ended: None,
            notice: None,
            fill_to: 1,
            last_thief: None,
            remote: HashMap::new(),
            clock: 0.0,
            send_timer: 0.0,
            over_timer: 0.0,
            over_sent: false,
            seed_counter: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64),
        }
    }

    pub fn humans(&self) -> usize {
        self.members.iter().filter(|m| !m.bot).count()
    }

    /// Nur Host: Bots ergaenzen oder entfernen, bis `fill_to` Spieler da sind.
    pub fn fill_bots(&mut self) {
        if !self.host {
            return;
        }
        let want = self.fill_to.clamp(self.humans(), MAX_PLAYERS);
        while self.members.len() > want {
            match self.members.iter().rposition(|m| m.bot) {
                Some(i) => {
                    self.members.remove(i);
                }
                None => break,
            }
        }
        let mut k = 0;
        while self.members.len() < want {
            let id = BOT_BASE + k;
            k += 1;
            if self.members.iter().any(|m| m.id == id) {
                continue;
            }
            self.members.push(Member {
                id,
                name: format!("Bot {}", BOT_NAMES[k as usize % BOT_NAMES.len()]),
                bot: true,
                score: 0,
            });
        }
        self.broadcast(&Msg::Lobby {
            members: self.members.clone(),
        });
    }

    fn send(&mut self, to: u16, m: &Msg, d: Delivery) {
        if let Some(t) = self.transport.as_mut() {
            t.send(to, &m.encode(), d);
        }
    }

    fn broadcast(&mut self, m: &Msg) {
        if self.host {
            self.send(BROADCAST, m, Delivery::Reliable);
        }
    }

    /// Nur Host: naechste Runde starten (Dieb reihum).
    pub fn start_round(&mut self) {
        if !self.host || self.members.len() < 2 {
            return;
        }
        let ids: Vec<u16> = self.members.iter().map(|m| m.id).collect();
        let thief = next_thief(&ids, self.last_thief);
        self.seed_counter = self
            .seed_counter
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let seed = self.seed_counter;
        self.begin(seed, thief, self.members.clone());
        self.broadcast(&Msg::Start {
            seed,
            thief,
            members: self.members.clone(),
        });
    }

    fn begin(&mut self, seed: u64, thief: u16, members: Vec<Member>) {
        let entries: Vec<Entry> = members
            .iter()
            .map(|m| Entry {
                id: m.id,
                name: m.name.clone(),
                bot: m.bot && self.host,
                local: m.id == self.my_id || (m.bot && self.host),
            })
            .collect();
        self.members = members;
        self.round = Some(Round::new(seed, &entries, thief));
        self.last_thief = Some(thief);
        self.remote.clear();
        self.over_timer = 0.0;
        self.over_sent = false;
    }

    /// Ein Bild: Netz abfragen, Runde rechnen, senden. Liefert Ereignisse fuer Klang und HUD.
    pub fn update(
        &mut self,
        dt: f32,
        controls: Controls,
        ability: Option<Ability>,
    ) -> Vec<RoundEvent> {
        self.clock += dt;
        self.poll();
        let mut ev = Vec::new();
        let me = self.my_id;
        if let Some(r) = self.round.as_mut() {
            ev.extend(r.step(dt, &|id| {
                if id == me {
                    controls
                } else {
                    Controls::default()
                }
            }));
        }
        if let Some(a) = ability {
            self.request_ability(a, &mut ev);
        }
        self.smooth_remote(dt);
        if self.host {
            self.host_tick(dt, &mut ev);
        } else {
            self.client_tick(dt);
        }
        ev
    }

    fn request_ability(&mut self, a: Ability, ev: &mut Vec<RoundEvent>) {
        if self.host {
            let me = self.my_id;
            if let Some(s) = self.round.as_mut().and_then(|r| r.use_ability(me, a)) {
                ev.push(RoundEvent::Ability(me, a));
                self.broadcast(&Msg::Spawn(s));
            }
        } else {
            self.send(HOST, &Msg::UseAbility(a), Delivery::Reliable);
        }
    }

    fn host_tick(&mut self, dt: f32, ev: &mut Vec<RoundEvent>) {
        let Some(r) = self.round.as_mut() else {
            return;
        };
        ev.extend(r.judge());
        let spawns: Vec<_> = r.spawned.drain(..).collect();
        for s in spawns {
            self.broadcast(&Msg::Spawn(s));
        }
        let Some(r) = self.round.as_ref() else {
            return;
        };
        if let Phase::Over { thief_won, catcher } = r.phase {
            if !self.over_sent {
                self.over_sent = true;
                let points: Vec<(u16, i32)> = r.players.iter().map(|p| (p.id, p.points)).collect();
                self.add_points(&points);
                self.broadcast(&Msg::Over {
                    thief_won,
                    catcher,
                    points,
                });
            }
            self.over_timer += dt;
            if self.over_timer > OVER_PAUSE {
                self.start_round();
            }
            return;
        }
        self.send_timer += dt;
        if self.send_timer >= SNAPSHOT_EVERY {
            self.send_timer = 0.0;
            let players = r.players.iter().map(state_of).collect();
            let m = Msg::Snapshot {
                time: r.time,
                players,
            };
            self.send(BROADCAST, &m, Delivery::Unreliable);
        }
    }

    fn client_tick(&mut self, dt: f32) {
        if let Some(Phase::Over { .. }) = self.round.as_ref().map(|r| r.phase) {
            self.over_timer += dt;
        }
        self.send_timer += dt;
        if self.send_timer < STATE_EVERY {
            return;
        }
        self.send_timer = 0.0;
        let me = self.my_id;
        if let Some(p) = self.round.as_ref().and_then(|r| r.player(me)) {
            let m = Msg::State(state_of(p));
            self.send(HOST, &m, Delivery::Unreliable);
        }
    }

    fn add_points(&mut self, points: &[(u16, i32)]) {
        for (id, p) in points {
            if let Some(m) = self.members.iter_mut().find(|m| m.id == *id) {
                m.score += p;
            }
        }
    }

    /// Fremde Figuren auf ihren vorhergesagten Platz ziehen.
    fn smooth_remote(&mut self, dt: f32) {
        let Some(r) = self.round.as_mut() else {
            return;
        };
        let k = (dt * SMOOTH).min(1.0);
        for p in r.players.iter_mut().filter(|p| !p.local) {
            let Some((s, at)) = self.remote.get(&p.id) else {
                continue;
            };
            let age = (self.clock - at).min(0.3);
            let m = &mut p.mover;
            let target_z = s.z + s.speed * age;
            if (target_z - m.z).abs() > 8.0 {
                m.z = target_z; // zu weit weg: springen statt ziehen
            }
            m.z += (target_z - m.z) * k;
            m.x += (s.x - m.x) * k;
            m.y += (s.y - m.y) * k;
            m.lane = s.lane;
            m.speed = s.speed;
            p.shield = s.flags & 1 != 0;
            p.mustard = if s.flags & 2 != 0 { 1.0 } else { 0.0 };
            p.whistle = if s.flags & 8 != 0 { 1.0 } else { 0.0 };
            p.rocket = if s.flags & 16 != 0 { 1.0 } else { 0.0 };
            p.springs = if s.flags & 32 != 0 { 1.0 } else { 0.0 };
            p.multiplier = if s.flags & 64 != 0 { 1.0 } else { 0.0 };
            p.rings = s.rings as u32;
            p.remote_clip = s.clip;
        }
    }

    fn poll(&mut self) {
        let Some(t) = self.transport.as_mut() else {
            return;
        };
        for e in t.poll() {
            match e {
                NetEvent::Hosted { code } => self.code = Some(code),
                NetEvent::Joined { .. } => self.notice = None,
                NetEvent::PeerJoined { peer, name } => self.peer_joined(peer, name),
                NetEvent::PeerLeft { peer } => self.peer_left(peer),
                NetEvent::Message { from, data } => {
                    if let Some(m) = Msg::decode(&data) {
                        self.handle(from, m);
                    }
                }
                NetEvent::Ended { reason } => self.ended = Some(reason),
                NetEvent::Error { message } => self.notice = Some(message),
            }
        }
    }

    fn peer_joined(&mut self, peer: u16, name: String) {
        if !self.host {
            return;
        }
        if self.members.len() >= MAX_PLAYERS {
            // Platz eines Bots freimachen; ist keiner da, ist die Lobby voll
            if let Some(i) = self.members.iter().rposition(|m| m.bot) {
                self.members.remove(i);
            }
        }
        self.members.push(Member {
            id: peer,
            name,
            bot: false,
            score: 0,
        });
        self.send(peer, &Msg::Welcome { id: peer }, Delivery::Reliable);
        if self.round.is_none() {
            self.fill_bots();
        } else {
            self.broadcast(&Msg::Lobby {
                members: self.members.clone(),
            });
        }
    }

    fn peer_left(&mut self, peer: u16) {
        self.members.retain(|m| m.id != peer);
        // laufende Runde: ein Bot uebernimmt die Figur
        if let Some(p) = self.round.as_mut().and_then(|r| r.player_mut(peer)) {
            p.local = true;
            p.bot = Some(Brain::new(peer as u64));
            p.name = format!("{} (Bot)", p.name);
        }
        self.broadcast(&Msg::Lobby {
            members: self.members.clone(),
        });
    }

    fn handle(&mut self, from: u16, m: Msg) {
        match m {
            Msg::Welcome { id } if !self.host => self.my_id = id,
            Msg::Lobby { members } if !self.host => self.members = members,
            Msg::Start {
                seed,
                thief,
                members,
            } if !self.host => self.begin(seed, thief, members),
            Msg::Snapshot { time, players } if !self.host => {
                if let Some(r) = self.round.as_mut() {
                    // Uhr sanft an den Host angleichen
                    if (r.time - time).abs() > 0.5 {
                        r.time = time;
                    } else {
                        r.time += (time - r.time) * 0.1;
                    }
                }
                for s in players {
                    if s.id != self.my_id {
                        self.remote.insert(s.id, (s, self.clock));
                    }
                }
            }
            Msg::State(mut s) if self.host => {
                s.id = from; // niemand spricht fuer andere
                self.remote.insert(from, (s, self.clock));
            }
            Msg::UseAbility(a) if self.host => {
                if let Some(s) = self.round.as_mut().and_then(|r| r.use_ability(from, a)) {
                    self.broadcast(&Msg::Spawn(s));
                }
            }
            Msg::Spawn(s) if !self.host => {
                if let Some(r) = self.round.as_mut() {
                    r.apply_spawn(s);
                }
            }
            Msg::Over {
                thief_won,
                catcher,
                points,
            } if !self.host => {
                if let Some(r) = self.round.as_mut() {
                    r.finish(thief_won, catcher);
                    for (id, p) in &points {
                        if let Some(pl) = r.player_mut(*id) {
                            pl.points = *p;
                        }
                    }
                }
                self.add_points(&points);
                self.over_timer = 0.0;
            }
            _ => {}
        }
    }

    /// Sitzung verlassen (zurueck ins Menue).
    pub fn leave(&mut self) {
        if let Some(t) = self.transport.as_mut() {
            t.leave();
        }
    }
}

fn state_of(p: &crate::game::round::Player) -> PlayerState {
    let m = &p.mover;
    let clip = if p.local {
        m.clip(p.mustard > 0.0 || p.whistle > 0.0) as u8
    } else {
        p.remote_clip
    };
    PlayerState {
        id: p.id,
        x: m.x,
        y: m.y,
        z: m.z,
        speed: m.speed,
        lane: m.lane,
        clip,
        flags: (p.shield as u8)
            | ((p.mustard > 0.0) as u8) << 1
            | ((p.magnet > 0.0) as u8) << 2
            | ((p.whistle > 0.0) as u8) << 3
            | ((p.rocket > 0.0) as u8) << 4
            | ((p.springs > 0.0) as u8) << 5
            | ((p.multiplier > 0.0) as u8) << 6,
        rings: p.rings.min(u16::MAX as u32) as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mettgine::net::loopback::Loopback;

    fn net_session(names: &[&str]) -> Vec<Session> {
        Loopback::network(names)
            .into_iter()
            .zip(names)
            .map(|(t, n)| Session::network(Box::new(t), n))
            .collect()
    }

    fn tick(all: &mut [Session], secs: f32) {
        let dt = 1.0 / 60.0;
        for _ in 0..(secs / dt) as usize {
            for s in all.iter_mut() {
                s.update(dt, Controls::default(), None);
            }
        }
    }

    #[test]
    fn host_and_two_peers_play_a_round_and_swap_the_thief() {
        let mut all = net_session(&["Host", "Anna", "Ben"]);
        tick(&mut all, 0.1);
        assert_eq!(all[1].my_id, 1);
        assert_eq!(all[2].members.len(), 3, "Lobby beim Mitspieler");
        all[0].fill_to = 4; // ein Bot dazu
        all[0].fill_bots();
        all[0].start_round();
        tick(&mut all, 0.2);
        for s in &all {
            let r = s.round.as_ref().expect("Runde laeuft ueberall");
            assert_eq!(r.players.len(), 4);
            assert_eq!(r.track.seed, all[0].round.as_ref().unwrap().track.seed);
        }
        let first_thief = all[0].round.as_ref().unwrap().thief().unwrap().id;
        // Mitspieler laufen: der Host sieht sie vorankommen
        tick(&mut all, crate::game::COUNTDOWN + 3.0);
        let host_view = all[0].round.as_ref().unwrap().player(1).unwrap().mover.z;
        let own_view = all[1].round.as_ref().unwrap().player(1).unwrap().mover.z;
        assert!(
            host_view > 10.0,
            "Host sieht Anna nicht laufen: {host_view}"
        );
        assert!(
            (host_view - own_view).abs() < 3.0,
            "{host_view} vs {own_view}"
        );
        // Faehigkeit eines Mitspielers kommt ueber den Host bei allen an
        let cop = all
            .iter()
            .position(|s| s.my_id != first_thief && !s.host)
            .unwrap();
        // Funk freimachen (Bots koennten ihn gerade belegt haben)
        for s in all.iter_mut() {
            let r = s.round.as_mut().unwrap();
            r.team_lock = 0.0;
            r.tapes.clear();
        }
        all[cop].update(1.0 / 60.0, Controls::default(), Some(Ability::Tape));
        tick(&mut all, 0.1);
        let host_tapes: Vec<u32> = all[0]
            .round
            .as_ref()
            .unwrap()
            .tapes
            .iter()
            .map(|t| t.id)
            .collect();
        assert!(!host_tapes.is_empty(), "Band fehlt beim Host");
        for s in &all {
            let ids: Vec<u32> = s
                .round
                .as_ref()
                .unwrap()
                .tapes
                .iter()
                .map(|t| t.id)
                .collect();
            assert_eq!(ids, host_tapes, "Band fehlt bei Spieler {}", s.my_id);
        }
        // Runde zu Ende bringen
        all[0].round.as_mut().unwrap().time = crate::game::ROUND_TIME;
        tick(&mut all, 0.1);
        for s in &all {
            assert!(matches!(
                s.round.as_ref().unwrap().phase,
                Phase::Over { .. }
            ));
            assert!(s.members.iter().any(|m| m.score > 0), "Punkte fehlen");
        }
        tick(&mut all, OVER_PAUSE + 0.5);
        let second = all[2].round.as_ref().unwrap().thief().unwrap().id;
        assert_ne!(second, first_thief, "Dieb wechselt");
    }

    #[test]
    fn a_leaving_peer_is_replaced_by_a_bot() {
        let mut all = net_session(&["Host", "Anna"]);
        tick(&mut all, 0.1);
        all[0].start_round();
        tick(&mut all, 0.1);
        all[1].leave();
        tick(&mut all, 0.1);
        let r = all[0].round.as_ref().unwrap();
        let p = r.player(1).unwrap();
        assert!(p.local && p.bot.is_some());
        assert_eq!(all[0].members.len(), 1);
    }

    #[test]
    fn solo_fills_up_with_bots() {
        let mut s = Session::solo("Ich", 7);
        assert_eq!(s.members.len(), MAX_PLAYERS);
        s.start_round();
        let r = s.round.as_ref().unwrap();
        assert_eq!(r.players.iter().filter(|p| p.bot.is_some()).count(), 7);
        assert!(r.player(0).unwrap().local);
    }
}
