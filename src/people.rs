//! Figuren des Spiels (Diebe und Ordnungsamt) und ihre Animationen.
//! Erzeugt von tools/prepare_people.py (Quaternius, CC0); Formate und Abspielen: Mettgine.

use mettgine::anim::{AnimSet, ClipId};
use mettgine::asset::data;
use mettgine::gpu::SkinData;
use std::sync::OnceLock;

/// Animationen (Reihenfolge wie CLIPS in tools/prepare_people.py).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
#[repr(u8)]
pub enum Clip {
    #[default]
    Idle = 0,
    Run,
    Sprint,
    JumpStart,
    JumpLoop,
    JumpLand,
    SlideStart,
    SlideLoop,
    SlideExit,
    Roll,
    Hit,
    GetUp,
    Fall,
    Wave,
    Dance,
    Yes,
    No,
    FoldArms,
    Walk,
    Grab,
    Throw,
}

pub const ALL_CLIPS: [Clip; 21] = [
    Clip::Idle,
    Clip::Run,
    Clip::Sprint,
    Clip::JumpStart,
    Clip::JumpLoop,
    Clip::JumpLand,
    Clip::SlideStart,
    Clip::SlideLoop,
    Clip::SlideExit,
    Clip::Roll,
    Clip::Hit,
    Clip::GetUp,
    Clip::Fall,
    Clip::Wave,
    Clip::Dance,
    Clip::Yes,
    Clip::No,
    Clip::FoldArms,
    Clip::Walk,
    Clip::Grab,
    Clip::Throw,
];

impl Clip {
    pub fn from_u8(v: u8) -> Clip {
        ALL_CLIPS.get(v as usize).copied().unwrap_or_default()
    }
}

impl ClipId for Clip {
    fn index(self) -> usize {
        self as usize
    }
    fn locomotion(self) -> bool {
        matches!(self, Clip::Run | Clip::Sprint | Clip::Walk)
    }
    fn looping(self) -> bool {
        matches!(
            self,
            Clip::Idle
                | Clip::JumpLoop
                | Clip::SlideLoop
                | Clip::Wave
                | Clip::Dance
                | Clip::No
                | Clip::FoldArms
        )
    }
}

/// Rolle einer Figurvariante
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Thief,
    Cop,
}

/// Varianten (Reihenfolge = tools/people.json): (Name, weiblich, Rolle); Figur in `people/<Name>.k0s`
pub static PEOPLE: &[(&str, bool, Look)] = &[
    ("t_m_hoodie", false, Look::Thief),
    ("t_m_punk", false, Look::Thief),
    ("t_m_beach", false, Look::Thief),
    ("t_f_punk", true, Look::Thief),
    ("t_f_casual", true, Look::Thief),
    ("o_m_worker", false, Look::Cop),
    ("o_m_jacket", false, Look::Cop),
    ("o_m_casual", false, Look::Cop),
    ("o_f_worker", true, Look::Cop),
    ("o_f_suit", true, Look::Cop),
];

/// Indizes der Varianten einer Rolle.
pub fn variants(look: Look) -> Vec<usize> {
    (0..PEOPLE.len()).filter(|&i| PEOPLE[i].2 == look).collect()
}

/// Variante fuer Spieler `slot` in einer Rolle (verteilt die Figuren reihum).
pub fn variant_for(look: Look, slot: usize) -> usize {
    let v = variants(look);
    v[slot % v.len()]
}

pub fn is_female(variant: usize) -> bool {
    PEOPLE[variant % PEOPLE.len()].1
}

/// Animationen fuer Maenner- bzw. Frauen-Skelett.
pub fn anims(female: bool) -> &'static AnimSet {
    static M: OnceLock<AnimSet> = OnceLock::new();
    static F: OnceLock<AnimSet> = OnceLock::new();
    if female {
        F.get_or_init(|| data::load("people/anim_f.k0a"))
    } else {
        M.get_or_init(|| data::load("people/anim_m.k0a"))
    }
}

/// Alle Figuren, Index = Variante. Laedt auch die Animationen, damit es spaeter nicht ruckelt.
pub fn load_skins() -> Vec<SkinData> {
    let _ = (anims(false), anims(true));
    PEOPLE
        .iter()
        .map(|p| data::load(&format!("people/{}.k0s", p.0)))
        .collect()
}

/// Laufgeschwindigkeit, zu der die Rennanimation in Echtzeit passt (m/s); schnellere Figuren
/// spielen sie entsprechend schneller ab, damit die Fuesse nicht rutschen.
pub fn run_stride(female: bool) -> f32 {
    if female {
        5.2
    } else {
        5.8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Vec3, Vec4};

    #[test]
    fn skins_and_anims_fit_together() {
        let skins = load_skins();
        assert_eq!(skins.len(), PEOPLE.len());
        for female in [false, true] {
            let a = anims(female);
            for c in ALL_CLIPS {
                assert!(a.has_clip(c), "Clip fehlt: {c:?}");
            }
            assert_eq!(a.clip_name(Clip::SlideLoop), "slide_loop");
            assert_eq!(a.clip_name(Clip::Throw), "throw");
        }
        for (v, s) in skins.iter().enumerate() {
            let bones = anims(PEOPLE[v].1).bones() as f32;
            for vx in &s.vertices {
                let w: f32 = vx.weights.iter().sum();
                assert!((w - 1.0).abs() < 1e-3, "{}: Gewichte {w}", PEOPLE[v].0);
                assert!(vx.joints.iter().all(|&j| j >= 0.0 && j < bones));
            }
        }
        assert!(variants(Look::Thief).len() >= 4 && variants(Look::Cop).len() >= 4);
    }

    #[test]
    fn people_are_human_sized_and_sliding_is_low() {
        let skins = load_skins();
        for (v, s) in skins.iter().enumerate() {
            let a = anims(PEOPLE[v].1);
            let height = |clip: Clip, t: f32| {
                let mut m = Vec::new();
                a.sample(clip, t, &mut m);
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for vx in s.vertices.iter().step_by(5) {
                    let p = Vec3::from(vx.pos).extend(1.0);
                    let mut r = Vec4::ZERO;
                    for k in 0..4 {
                        r += m[vx.joints[k] as usize] * p * vx.weights[k];
                    }
                    lo = lo.min(r.y);
                    hi = hi.max(r.y);
                }
                (lo, hi)
            };
            let (lo, hi) = height(Clip::Idle, 0.0);
            assert!(lo.abs() < 0.15, "{} schwebt: {lo}", PEOPLE[v].0);
            assert!(hi > 1.4 && hi < 2.05, "{} ist {hi} m gross", PEOPLE[v].0);
            let (_, slide) = height(Clip::SlideLoop, 0.5);
            assert!(slide < 1.2, "{} rutscht zu hoch: {slide}", PEOPLE[v].0);
        }
    }
}
