//! Aus dem Rundenzustand die Bildliste bauen: Strasse, Gleise, Haeuser, Hindernisse,
//! Sammelsachen, Dackel, Figuren, Kamera und Licht. Nur Darstellung, keine Spielregeln.

use crate::assets::{size, MeshId};
use crate::game::round::{Player, Round};
use crate::game::track::{Kind, Obstacle, Power, CHUNK_LEN, ROAD_HALF};
use crate::game::{lane_x, Role, LANE_W, START_GAP};
use crate::people::{anims, is_female, run_stride, Clip};
use glam::{Mat4, Quat, Vec3, Vec4};
use mettgine::anim::{Animator, PoseFx};
use mettgine::render::{perspective, Clean, Lighting, MeshDraw, Post, SkinDraw, Sun, View};
use std::collections::HashMap;

/// Sichtweite nach vorn und hinten (Meter)
const VIEW_AHEAD: f32 = 170.0;
const VIEW_BEHIND: f32 = 25.0;
/// Bildwinkel (Radiant), nahe und ferne Grenze (Meter) der Kamera
pub const FOV_Y: f32 = 65.0 * std::f32::consts::PI / 180.0;
pub const NEAR: f32 = 0.1;
pub const FAR: f32 = 400.0;
/// Breite des Gehwegs (Meter)
const SIDEWALK: f32 = 4.0;

/// Bonbonfarben fuer Haeuser, Zuege und Fahrzeuge (Farbmultiplikator auf die Modellfarben)
const CANDY: [Vec3; 8] = [
    Vec3::new(1.0, 0.45, 0.55),
    Vec3::new(0.45, 0.8, 1.0),
    Vec3::new(1.0, 0.85, 0.3),
    Vec3::new(0.5, 1.0, 0.6),
    Vec3::new(1.0, 0.6, 0.3),
    Vec3::new(0.75, 0.55, 1.0),
    Vec3::new(0.35, 0.95, 0.9),
    Vec3::new(1.0, 0.5, 0.85),
];

fn candy(n: u64) -> Vec3 {
    CANDY[(hash(n, 99) % CANDY.len() as u64) as usize]
}

/// Alles, was ein Bild braucht.
#[derive(Default)]
pub struct Frame {
    pub meshes: Vec<MeshDraw>,
    pub skins: Vec<SkinDraw>,
    pub bones: Vec<Mat4>,
}

/// Kamera hinter dem eigenen Spieler, weich nachgefuehrt.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 3.5, START_GAP - 7.0),
            target: Vec3::new(0.0, 1.3, START_GAP + 6.0),
        }
    }
}

impl Camera {
    pub fn follow(&mut self, p: Vec3, dt: f32) {
        let want_eye = Vec3::new(p.x * 0.7, p.y * 0.6 + 3.4, p.z - 7.2);
        let want_target = Vec3::new(p.x * 0.85, p.y * 0.7 + 1.3, p.z + 6.0);
        let k = (dt * 8.0).min(1.0);
        // z sofort, sonst verschwindet der Laeufer bei hohem Tempo aus dem Bild
        self.eye = Vec3::new(
            self.eye.x + (want_eye.x - self.eye.x) * k,
            self.eye.y + (want_eye.y - self.eye.y) * k,
            want_eye.z,
        );
        self.target = Vec3::new(
            self.target.x + (want_target.x - self.target.x) * k,
            self.target.y + (want_target.y - self.target.y) * k,
            want_target.z,
        );
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        perspective(FOV_Y, aspect, NEAR, FAR) * Mat4::look_at_rh(self.eye, self.target, Vec3::Y)
    }
}

/// Animationen der Figuren zwischen den Bildern merken.
#[derive(Default)]
pub struct Animators {
    map: HashMap<u16, (Animator<Clip>, f32)>,
}

fn hash(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_add(0xD1B5_4A32_D192_ED03);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

fn hf(a: u64, b: u64) -> f32 {
    (hash(a, b) % 10_000) as f32 / 10_000.0
}

fn boxed(center: Vec3, scale: Vec3, color: Vec3) -> MeshDraw {
    MeshDraw::new(
        MeshId::Cube as usize,
        Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, center),
    )
    .tint(color)
}

fn placed(id: MeshId, pos: Vec3, yaw: f32) -> MeshDraw {
    MeshDraw::new(
        id as usize,
        Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), pos),
    )
}

/// Strasse, Gleise, Gehwege und Stadt fuer den sichtbaren Bereich.
fn street(f: &mut Frame, seed: u64, z0: f32, z1: f32) {
    let seg = 20.0;
    let start = (z0 / seg).floor() as i32;
    let end = (z1 / seg).ceil() as i32;
    let road_w = ROAD_HALF * 2.0;
    for s in start..end {
        let zc = (s as f32 + 0.5) * seg;
        f.meshes.push(boxed(
            Vec3::new(0.0, -0.05, zc),
            Vec3::new(road_w, 0.1, seg + 0.02),
            Vec3::new(0.34, 0.31, 0.42),
        ));
        for side in [-1.0f32, 1.0] {
            let x = side * (ROAD_HALF + SIDEWALK * 0.5);
            f.meshes.push(boxed(
                Vec3::new(x, 0.1, zc),
                Vec3::new(SIDEWALK, 0.2, seg + 0.02),
                Vec3::new(0.98, 0.8, 0.62),
            ));
            f.meshes.push(boxed(
                Vec3::new(side * (ROAD_HALF + 0.08), 0.12, zc),
                Vec3::new(0.16, 0.24, seg + 0.02),
                Vec3::new(1.0, 0.82, 0.2),
            ));
        }
        // Gleise: zwei Schienen je Spur, Schwellen
        for lane in -1..=1i8 {
            let lx = lane_x(lane);
            for rail in [-0.72f32, 0.72] {
                f.meshes.push(
                    boxed(
                        Vec3::new(lx + rail, 0.04, zc),
                        Vec3::new(0.09, 0.08, seg + 0.02),
                        Vec3::new(0.9, 0.9, 0.98),
                    )
                    .no_shadow(),
                );
            }
            for k in 0..10 {
                let z = s as f32 * seg + k as f32 * 2.0 + 1.0;
                f.meshes.push(
                    boxed(
                        Vec3::new(lx, 0.015, z),
                        Vec3::new(1.9, 0.03, 0.28),
                        Vec3::new(0.62, 0.38, 0.22),
                    )
                    .no_shadow(),
                );
            }
        }
    }
    city(f, seed, z0, z1);
}

/// Haeuser und Strassenmoebel, pro 60-m-Abschnitt aus dem Seed.
fn city(f: &mut Frame, seed: u64, z0: f32, z1: f32) {
    let a = (z0 / CHUNK_LEN).floor() as i64 - 1;
    let b = (z1 / CHUNK_LEN).ceil() as i64;
    let houses = [MeshId::BuildingA, MeshId::BuildingB, MeshId::TownHouse];
    let tints = CANDY;
    for c in a.max(-1)..=b {
        for (si, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            let key = hash(seed, (c as u64) << 1 | si as u64);
            let mut z = c as f32 * CHUNK_LEN;
            let mut k = 0u64;
            while z < (c + 1) as f32 * CHUNK_LEN {
                let id = houses[(hash(key, k) % 3) as usize];
                let sz = size(id);
                // Laengsseite zur Strasse: Modell um 90 Grad drehen
                let (along, depth) = (sz.x, sz.z);
                let x = side * (ROAD_HALF + SIDEWALK + 0.5 + depth * 0.5);
                let pos = Vec3::new(x, 0.0, z + along * 0.5);
                let yaw = if side > 0.0 {
                    -std::f32::consts::FRAC_PI_2
                } else {
                    std::f32::consts::FRAC_PI_2
                };
                f.meshes.push(
                    placed(id, pos, yaw)
                        .tint(tints[(hash(key, k + 7) % CANDY.len() as u64) as usize] * 1.25),
                );
                z += along + 1.0 + hf(key, k + 3) * 3.0;
                k += 1;
            }
            // Strassenmoebel auf dem Gehweg
            let walk_x = side * (ROAD_HALF + 1.0);
            for n in 0..3 {
                let z = c as f32 * CHUNK_LEN + n as f32 * 20.0 + 4.0;
                let yaw = if side > 0.0 {
                    std::f32::consts::PI
                } else {
                    0.0
                };
                f.meshes.push(placed(
                    MeshId::StreetLight,
                    Vec3::new(walk_x, 0.2, z),
                    yaw + std::f32::consts::FRAC_PI_2 * side,
                ));
                let extra = match hash(key, 100 + n) % 5 {
                    0 => Some(MeshId::Bench),
                    1 => Some(MeshId::Hydrant),
                    2 => Some(MeshId::Trashcan),
                    3 => Some(MeshId::TrafficLight),
                    _ => Some(MeshId::Cone),
                };
                if let Some(id) = extra {
                    let ex = side * (ROAD_HALF + 2.2);
                    f.meshes.push(placed(id, Vec3::new(ex, 0.2, z + 8.0), yaw));
                }
            }
        }
    }
    // Der Mettbroetchen-Stand am Start (hier beginnt die Flucht)
    if z0 < START_GAP + 20.0 {
        f.meshes.push(placed(
            MeshId::MarketStand,
            Vec3::new(ROAD_HALF + 2.0, 0.2, START_GAP + 2.0),
            -std::f32::consts::FRAC_PI_2,
        ));
        f.meshes.push(
            placed(
                MeshId::Sandwich,
                Vec3::new(ROAD_HALF + 1.2, 1.25, START_GAP + 2.0),
                0.0,
            )
            .glow(0.2),
        );
    }
}

fn obstacle(f: &mut Frame, o: &Obstacle, all: &[Obstacle], observer_z: f32) {
    let len = o.kind.length();
    let x = lane_x(o.lane);
    let zc = o.start(observer_z) + len * 0.5;
    let pos = Vec3::new(x, 0.0, zc);
    match o.kind {
        // Stehende Zuege: Personen- oder Containerwagen in Bonbonfarben
        Kind::Train => {
            let id = if o.id % 3 == 0 {
                MeshId::TrainContainer
            } else {
                MeshId::TrainCarriage
            };
            f.meshes
                .push(placed(id, pos, 0.0).tint(candy(o.id as u64) * 1.3));
        }
        // Entgegenkommende Zuege: Lok oder Schnellzug, mit Scheinwerfern
        Kind::MovingTrain => {
            let id = if o.id % 2 == 0 {
                MeshId::TrainFront
            } else {
                MeshId::TrainHighspeed
            };
            f.meshes
                .push(placed(id, pos, std::f32::consts::PI).tint(candy(o.id as u64 + 5) * 1.3));
            for dx in [-0.7f32, 0.7] {
                f.meshes.push(
                    boxed(
                        Vec3::new(x + dx, 0.9, zc - len * 0.5 - 0.05),
                        Vec3::new(0.3, 0.2, 0.05),
                        Vec3::new(1.0, 0.95, 0.7),
                    )
                    .glow(2.5)
                    .no_shadow(),
                );
            }
        }
        Kind::Truck => f
            .meshes
            .push(placed(MeshId::Truck, pos, 0.0).tint(candy(o.id as u64 + 1) * 1.2)),
        Kind::Van => f
            .meshes
            .push(placed(MeshId::Van, pos, 0.0).tint(candy(o.id as u64 + 2) * 1.2)),
        Kind::PoliceCar => f.meshes.push(placed(MeshId::PoliceCar, pos, 0.0)),
        Kind::Bridge | Kind::BridgeLong => bridge(f, o, all, pos, len),
        Kind::Barrier => f.meshes.push(placed(MeshId::TrafficBarrier, pos, 0.0)),
        Kind::BarrierLarge => f.meshes.push(placed(MeshId::BarrierLarge, pos, 0.0)),
        // Absperrband blinkt als Warnung
        Kind::Tape => f
            .meshes
            .push(placed(MeshId::ConstructionBarrier, pos, 0.0).blink(0.6)),
        Kind::Ramp => f.meshes.push(
            MeshDraw::new(
                MeshId::Wedge as usize,
                Mat4::from_scale_rotation_translation(
                    Vec3::new(2.3, crate::game::track::TRAIN_H, len),
                    Quat::IDENTITY,
                    pos,
                ),
            )
            .tint(Vec3::new(1.0, 0.75, 0.25)),
        ),
        Kind::Beam => {
            use crate::game::track::{BEAM_HIGH, BEAM_LOW};
            for dx in [-1.05f32, 1.05] {
                f.meshes.push(boxed(
                    Vec3::new(x + dx, BEAM_HIGH * 0.5, zc),
                    Vec3::new(0.14, BEAM_HIGH, 0.14),
                    Vec3::new(0.85, 0.85, 0.85),
                ));
            }
            // rot-weiss gestreifter Balken
            for k in 0..6 {
                let red = k % 2 == 0;
                let c = if red {
                    Vec3::new(0.85, 0.12, 0.1)
                } else {
                    Vec3::new(0.95, 0.95, 0.95)
                };
                f.meshes.push(boxed(
                    Vec3::new(
                        x - 0.875 + k as f32 * 0.35,
                        (BEAM_LOW + BEAM_HIGH) * 0.5,
                        zc,
                    ),
                    Vec3::new(0.35, BEAM_HIGH - BEAM_LOW, 0.2),
                    c,
                ));
            }
        }
    }
}

/// Fussgaengerbruecke aus Stahlteilen: Deck auf Zughoehe, Gelaender und Pfeiler nur an den
/// Aussenkanten (Nachbarspuren mit Bruecke an derselben Stelle bilden ein durchgehendes Deck).
fn bridge(f: &mut Frame, o: &Obstacle, all: &[Obstacle], pos: Vec3, len: f32) {
    use crate::game::track::{BRIDGE_LOW, TRAIN_H};
    let color = candy(o.id as u64 / 8 + 3);
    let thick = TRAIN_H - BRIDGE_LOW;
    f.meshes.push(boxed(
        Vec3::new(pos.x, BRIDGE_LOW + thick * 0.5, pos.z),
        Vec3::new(LANE_W + 0.02, thick, len),
        color,
    ));
    let has = |lane: i8| {
        all.iter()
            .any(|b| b.kind.overhead() && b.lane == lane && (b.z - o.z).abs() < 0.5)
    };
    for side in [-1i8, 1] {
        if has(o.lane + side) {
            continue;
        }
        let edge = pos.x + side as f32 * LANE_W * 0.5;
        // aussen an der Strasse stehen die Pfeiler auf dem Gehweg
        let px = if (o.lane + side).abs() > 1 {
            side as f32 * (ROAD_HALF + 0.4)
        } else {
            edge
        };
        f.meshes.push(boxed(
            Vec3::new(edge, TRAIN_H + 0.5, pos.z),
            Vec3::new(0.08, 1.0, len),
            Vec3::new(1.0, 1.0, 1.0),
        ));
        for dz in [-0.45f32, 0.45] {
            f.meshes.push(boxed(
                Vec3::new(px, BRIDGE_LOW * 0.5, pos.z + dz * (len - 0.4)),
                Vec3::new(0.3, BRIDGE_LOW, 0.3),
                color * 0.8,
            ));
        }
    }
}

fn pickups(f: &mut Frame, round: &Round, z0: f32, z1: f32, taken: Option<&Player>, time: f32) {
    let sky = round.sky.iter().copied().filter(|p| p.z >= z0 && p.z <= z1);
    for p in round.track.pickups_in(z0, z1).into_iter().chain(sky) {
        if taken.is_some_and(|t| t.taken.contains(&p.id)) {
            continue;
        }
        let bob = (time * 3.0 + p.z).sin() * 0.08;
        let pos = Vec3::new(lane_x(p.lane), p.y - 0.45 + bob, p.z);
        let spin = Quat::from_rotation_y(time * 2.5 + p.z);
        let d = match p.power {
            // Zwiebelring hochkant, dreht sich
            None => MeshDraw::new(
                MeshId::OnionRings as usize,
                Mat4::from_rotation_translation(
                    spin * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                    pos + Vec3::Y * 0.2,
                ),
            )
            .tint(Vec3::new(1.3, 1.0, 0.4))
            .glow(0.45)
            .no_shadow(),
            Some(pw) => {
                let (id, tint) = match pw {
                    Power::Mustard => (MeshId::Mustard, Vec3::ONE),
                    Power::Magnet => (MeshId::Magnet, Vec3::new(1.3, 0.4, 0.4)),
                    Power::Shield => (MeshId::Bread, Vec3::ONE),
                    Power::Rocket => (MeshId::Rocket, Vec3::new(1.3, 0.5, 0.3)),
                    Power::Springs => (MeshId::Spring, Vec3::new(0.5, 1.2, 0.6)),
                    Power::Multiplier => (MeshId::Star, Vec3::new(1.4, 1.1, 0.2)),
                };
                // leuchtender Sockel, damit man Power-ups von weitem sieht
                f.meshes.push(
                    boxed(
                        pos - Vec3::Y * 0.35,
                        Vec3::new(0.7, 0.04, 0.7),
                        Vec3::new(1.0, 0.9, 0.3),
                    )
                    .glow(1.2)
                    .no_shadow(),
                );
                MeshDraw::new(id as usize, Mat4::from_rotation_translation(spin, pos))
                    .tint(tint)
                    .glow(0.6)
            }
        };
        f.meshes.push(d);
    }
}

impl Animators {
    /// Figur eines Spielers abtasten und in die Bildliste legen.
    fn add(&mut self, f: &mut Frame, p: &Player, time: f32, dt: f32) {
        let m = &p.mover;
        let clip = if p.local {
            m.clip(p.mustard > 0.0 || p.whistle > 0.0)
        } else {
            Clip::from_u8(p.remote_clip)
        };
        let female = is_female(p.variant);
        let (anim, run_t) = self.map.entry(p.id).or_insert((Animator::default(), 0.0));
        // Laufclips an die Strecke koppeln, damit die Fuesse nicht rutschen
        *run_t += dt * (m.speed / run_stride(female)).max(0.2);
        let t = if matches!(clip, Clip::Run | Clip::Sprint) {
            *run_t
        } else {
            time
        };
        anim.update(clip, t, dt, 0.12);
        let fx = PoseFx {
            blend: anim.blend(),
            ..Default::default()
        };
        let first_bone = f.bones.len() as u32;
        anims(female).pose(anim.clip, anim.clip_time(), &fx, &mut f.bones);
        let glow = if p.shield { 0.35 } else { 0.0 };
        let tint = match p.role {
            Role::Thief => Vec4::new(1.0, 1.0, 1.0, glow),
            Role::Cop => Vec4::new(1.0, 1.0, 1.0, glow),
        };
        f.skins.push(SkinDraw {
            skin: p.variant,
            model: Mat4::from_translation(Vec3::new(m.x, m.y, m.z)),
            tint,
            first_bone,
            blink: if p.mustard > 0.0 || p.whistle > 0.0 {
                0.25
            } else {
                0.0
            },
            ..Default::default()
        });
        // Pfeffer-Rakete auf dem Ruecken, mit Flamme
        if p.rocket > 0.0 {
            let back = Vec3::new(m.x, m.y + 0.75, m.z - 0.3);
            f.meshes
                .push(placed(MeshId::Rocket, back, 0.0).tint(Vec3::new(1.3, 0.5, 0.3)));
            let flicker = 0.8 + 0.2 * (time * 40.0).sin();
            f.meshes.push(
                boxed(
                    back - Vec3::Y * 0.25,
                    Vec3::new(0.25, 0.45 * flicker, 0.25),
                    Vec3::new(1.0, 0.6, 0.1),
                )
                .glow(3.0)
                .no_shadow(),
            );
        }
    }
}

/// Bildliste fuer den Spieler `me` bauen.
pub fn build(round: &Round, me: u16, anims: &mut Animators, time: f32, dt: f32) -> (Frame, Vec3) {
    let mut f = Frame::default();
    let focus = round
        .player(me)
        .or_else(|| round.thief())
        .map_or(Vec3::new(0.0, 0.0, START_GAP), |p| {
            Vec3::new(p.mover.x, p.mover.y, p.mover.z)
        });
    let (z0, z1) = (focus.z - VIEW_BEHIND, focus.z + VIEW_AHEAD);
    street(&mut f, round.track.seed, z0, z1);
    let all = round.track.obstacles_in(z0, z1, focus.z);
    for o in &all {
        obstacle(&mut f, o, &all, focus.z);
    }
    for t in &round.tapes {
        obstacle(&mut f, &t.obstacle(), &all, focus.z);
    }
    // Ringe: was der Dieb (bei uns bekannt) schon hat, fehlt
    pickups(&mut f, round, z0, z1, round.thief(), time);
    for d in round.dogs.iter().filter(|d| d.alive(round.time)) {
        let z = d.z(round.time);
        let hop = ((round.time - d.t0) * 14.0).sin().abs() * 0.12;
        f.meshes.push(
            placed(MeshId::Dog, Vec3::new(d.x(), hop, z), 0.0).tint(Vec3::new(0.75, 0.45, 0.25)),
        );
    }
    for p in &round.players {
        if (p.mover.z - focus.z).abs() < VIEW_AHEAD {
            anims.add(&mut f, p, time, dt);
        }
    }
    let _ = LANE_W;
    (f, focus)
}

/// Kamera, Licht und Nachbearbeitung eines Bildes.
pub struct Look {
    pub view: View,
    pub lighting: Lighting,
    pub post: Post,
}

/// Licht und Himmel: sonniger Nachmittag in der Stadt.
pub fn look(cam: &Camera, aspect: f32, focus: Vec3, shadows: bool) -> Look {
    let sun = Sun {
        dir: Vec3::new(0.45, 0.85, -0.3),
        color: Vec3::new(1.15, 1.02, 0.85),
        shadow_center: focus + Vec3::new(0.0, 0.0, 14.0),
        shadow_radius: 32.0,
        shadow_strength: if shadows { 0.7 } else { 0.0 },
        // knallblauer Himmel, warmer Horizont, helles Umgebungslicht: bunt statt grau
        sky_top: Vec3::new(0.15, 0.48, 1.0),
        horizon: Vec3::new(1.0, 0.86, 0.74),
        fog_density: 0.0035,
        ground: Vec3::new(0.7, 0.55, 0.5),
    };
    Look {
        view: View {
            view_proj: cam.view_proj(aspect),
            pos: cam.eye,
        },
        lighting: Lighting::Sun(sun),
        post: Post::Clean(Clean {
            fade: 0.0,
            red: 0.0,
            saturation: 1.4,
        }),
    }
}
