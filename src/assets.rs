//! Modelle des Spiels (.k0m, erzeugt von tools/prepare_assets.py), in den Spieldaten.
//! Index 0 und 1 sind Boxen aus dem Code (Wuerfel, Keil fuer Rampen).

use glam::Vec3;
use mettgine::asset::data;
use mettgine::gpu::{MeshData, MeshVertex};
use std::sync::OnceLock;

/// Reihenfolge = JOBS in tools/prepare_assets.py (ab Index 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum MeshId {
    Cube = 0,
    /// Rampe: Keil, steigt entlang +z von 0 auf 1 an ([-0.5, 0.5] in x/z, 0..1 in y)
    Wedge,
    /// Zuege: Personenwagen, Lok, Schnellzug-Kopf, Containerwagen (je 2.3 x 3.3 x 13 m)
    TrainCarriage,
    TrainFront,
    TrainHighspeed,
    TrainContainer,
    Truck,
    Van,
    PoliceCar,
    TrafficBarrier,
    BarrierLarge,
    ConstructionBarrier,
    Cone,
    StreetLight,
    Hydrant,
    Bench,
    Trashcan,
    BuildingA,
    BuildingB,
    TownHouse,
    MarketStand,
    TrafficLight,
    Dog,
    OnionRings,
    Mustard,
    Magnet,
    Sandwich,
    Bread,
    Rocket,
    Spring,
    Star,
}

pub const MESH_COUNT: usize = MeshId::Star as usize + 1;

const SOURCES: [&str; MESH_COUNT - 2] = [
    "props/train_carriage.k0m",
    "props/train_front.k0m",
    "props/train_highspeed.k0m",
    "props/train_container.k0m",
    "props/truck.k0m",
    "props/van.k0m",
    "props/police_car.k0m",
    "props/traffic_barrier.k0m",
    "props/barrier_large.k0m",
    "props/construction_barrier.k0m",
    "props/cone.k0m",
    "props/street_light.k0m",
    "props/hydrant.k0m",
    "props/bench.k0m",
    "props/trashcan.k0m",
    "props/building_a.k0m",
    "props/building_b.k0m",
    "props/town_house.k0m",
    "props/market_stand.k0m",
    "props/traffic_light.k0m",
    "props/dog.k0m",
    "props/onion_rings.k0m",
    "props/mustard.k0m",
    "props/magnet.k0m",
    "props/sandwich.k0m",
    "props/bread.k0m",
    "props/rocket.k0m",
    "props/spring.k0m",
    "props/star.k0m",
];

/// Alle Modelle, Index = MeshId.
pub fn load_all() -> Vec<MeshData> {
    let mut v = vec![cube(), wedge()];
    v.extend(SOURCES.iter().map(|n| data::load::<MeshData>(n)));
    v
}

/// Groesse (Bounding Box) eines Modells; Modelle stehen auf y = 0, Mitte x/z = 0.
pub fn size(id: MeshId) -> Vec3 {
    static SIZES: OnceLock<Vec<Vec3>> = OnceLock::new();
    SIZES.get_or_init(|| load_all().iter().map(|m| m.max - m.min).collect())[id as usize]
}

fn tri(out: &mut Vec<MeshVertex>, a: Vec3, b: Vec3, c: Vec3) {
    let n = (b - a).cross(c - a).normalize_or_zero();
    for p in [a, b, c] {
        out.push(MeshVertex {
            pos: p.to_array(),
            normal: n.to_array(),
            color: [1.0; 3],
        });
    }
}

fn quad(out: &mut Vec<MeshVertex>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
    tri(out, a, b, c);
    tri(out, a, c, d);
}

/// Normalen eines konvexen Koerpers nach aussen drehen (die Wicklung ist dem Renderer egal).
fn fix_outward(v: &mut [MeshVertex], center: Vec3) {
    for t in v.chunks_exact_mut(3) {
        let mid = (Vec3::from(t[0].pos) + Vec3::from(t[1].pos) + Vec3::from(t[2].pos)) / 3.0;
        let n = Vec3::from(t[0].normal);
        if n.dot(mid - center) < 0.0 {
            for x in t.iter_mut() {
                x.normal = (-n).to_array();
            }
        }
    }
}

/// Einheitswuerfel [-0.5, 0.5]^3 fuer Boxen (Strasse, Gleise, Balken).
fn cube() -> MeshData {
    let mut v = Vec::with_capacity(36);
    let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * 0.5;
    let (l, h) = (-1.0, 1.0);
    quad(&mut v, p(h, l, l), p(h, h, l), p(h, h, h), p(h, l, h));
    quad(&mut v, p(l, l, h), p(l, h, h), p(l, h, l), p(l, l, l));
    quad(&mut v, p(l, h, l), p(l, h, h), p(h, h, h), p(h, h, l));
    quad(&mut v, p(l, l, l), p(h, l, l), p(h, l, h), p(l, l, h));
    quad(&mut v, p(l, l, h), p(h, l, h), p(h, h, h), p(l, h, h));
    quad(&mut v, p(h, l, l), p(l, l, l), p(l, h, l), p(h, h, l));
    fix_outward(&mut v, Vec3::ZERO);
    MeshData {
        vertices: v,
        min: Vec3::splat(-0.5),
        max: Vec3::splat(0.5),
    }
}

/// Keil: Boden 1x1, Hoehe 0 bei z = -0.5 bis 1 bei z = +0.5.
fn wedge() -> MeshData {
    let mut v = Vec::new();
    let (a, b) = (Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 0.0, -0.5));
    let (c, d) = (Vec3::new(0.5, 0.0, 0.5), Vec3::new(-0.5, 0.0, 0.5));
    let (e, f) = (Vec3::new(0.5, 1.0, 0.5), Vec3::new(-0.5, 1.0, 0.5));
    quad(&mut v, a, b, c, d);
    quad(&mut v, a, f, e, b);
    quad(&mut v, d, c, e, f);
    tri(&mut v, b, e, c);
    tri(&mut v, a, d, f);
    fix_outward(&mut v, Vec3::new(0.0, 0.33, 0.17));
    MeshData {
        vertices: v,
        min: Vec3::new(-0.5, 0.0, -0.5),
        max: Vec3::new(0.5, 1.0, 0.5),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_meshes_load_and_fit_the_track() {
        let meshes = load_all();
        assert_eq!(meshes.len(), MESH_COUNT);
        for (i, m) in meshes.iter().enumerate() {
            assert!(!m.vertices.is_empty(), "Mesh {i} leer");
            assert_eq!(m.vertices.len() % 3, 0);
            if i > 0 {
                assert!(m.min.y.abs() < 1e-3, "Mesh {i} schwebt: {}", m.min.y);
            }
        }
        let tram = size(MeshId::TrainCarriage);
        assert!(tram.x < 2.5 && tram.z > 10.0, "Zug passt auf eine Spur");
        assert!(
            size(MeshId::TrafficBarrier).y < 1.0,
            "Absperrung ist ueberspringbar"
        );
        assert!(
            size(MeshId::Dog).z > size(MeshId::Dog).x * 2.5,
            "Dackel ist lang"
        );
    }
}
