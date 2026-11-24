//! Moderne Grafik (Einstellung): dieselbe Szene mit Materialien, Schattenstufen, Verdeckung,
//! Ueberstrahlen und auf Wunsch Raytracing.

use crate::scene::{self, Camera};
use glam::{Mat4, Vec3};
use mettgine::render::{self, *};

/// So weit werfen Dinge Schatten (Meter)
const SHADOW_DISTANCE: f32 = 110.0;
/// Sonne und Himmel sind Anzeigewerte; so hell werden sie in der Beleuchtung
const SUN_STRENGTH: f32 = 2.0;

/// Anzeigefarbe als Lichtfarbe.
fn linear(c: Vec3) -> Vec3 {
    c.max(Vec3::ZERO).powf(2.2)
}

/// Szene zum Blick des Spiels.
pub fn scene(
    cam: &Camera,
    aspect: f32,
    look: &scene::Look,
    shadows: bool,
    rays: bool,
    dt: f32,
) -> SceneInput<'static> {
    let camera = render::Camera {
        pos: cam.eye,
        view: Mat4::look_at_rh(cam.eye, cam.target, Vec3::Y),
        fov_y: scene::FOV_Y,
        aspect,
        near: scene::NEAR,
        far: scene::FAR,
    };
    let mut s = SceneInput {
        plain_roughness: 0.6,
        dt,
        features: Features {
            tonemap: Tonemap::Aces,
            // TAA zoege bei schnellen Zuegen Schlieren
            anti_alias: AntiAlias::Fxaa,
            ssao: Some(Ssao::default()),
            bloom: Some(Bloom::default()),
            sun_shadow_size: if shadows { 2048 } else { 0 },
            ray_tracing: RayTracing {
                sun_shadows: shadows && rays,
                occlusion: rays,
                reflections: rays,
                indirect: rays,
            },
            ..Features::default()
        },
        ..SceneInput::new(camera)
    };
    if let Lighting::Sun(sun) = &look.lighting {
        s.sun = Some(SunLight {
            dir: sun.dir,
            color: linear(sun.color) * SUN_STRENGTH,
            shadow_distance: SHADOW_DISTANCE,
        });
        s.sky = Sky::Gradient {
            zenith: linear(sun.sky_top),
            horizon: linear(sun.horizon),
            ground: linear(sun.ground),
        };
        s.features.fog = Some(HeightFog {
            color: linear(sun.horizon),
            density: sun.fog_density * 0.6,
            falloff: 0.0,
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_camera_sees_the_same_picture_as_the_simple_one() {
        let cam = Camera::default();
        let look = scene::look(&cam, 1.6, Vec3::ZERO, true);
        let s = scene(&cam, 1.6, &look, true, false, 0.016);
        assert!(s.camera.view_proj().abs_diff_eq(look.view.view_proj, 1e-4));
        assert!(s.sun.is_some() && s.features.sun_shadow_size > 0);
        let dark = scene(&cam, 1.6, &look, false, true, 0.016);
        assert!(dark.features.sun_shadow_size == 0 && !dark.features.ray_tracing.sun_shadows);
    }
}
