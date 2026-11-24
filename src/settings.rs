//! Einstellungen des Spiels in %APPDATA%\.mettlauncher\hackepeter\settings.txt (Datei: mettgine::settings).

use mettgine::settings::{self, KeyValues};

pub const APP: &str = "hackepeter";
const FILE: &str = "settings.txt";

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub volume: f32,
    pub music: f32,
    /// Aufloesung der Szene relativ zum Fenster (0.5 = halbe Pixel, schneller)
    pub render_scale: f32,
    pub fullscreen: bool,
    pub shadows: bool,
    /// moderne Grafik (Materialien, Schattenstufen, Verdeckung, Ueberstrahlen)
    pub modern: bool,
    /// Raytracing in der modernen Grafik (nur wenn die Grafikkarte es kann, ab Neustart)
    pub raytracing: bool,
    /// Name im Einzelspieler (im Mehrspieler kommt er aus dem Launcher)
    pub name: String,
    /// Bot-Gegner im Einzelspieler (1..7)
    pub bots: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            music: 0.6,
            render_scale: 1.0,
            fullscreen: false,
            shadows: true,
            modern: false,
            raytracing: false,
            name: "Hackepeter".into(),
            bots: 3,
        }
    }
}

impl Settings {
    pub fn from_kv(kv: &KeyValues) -> Self {
        let d = Self::default();
        Self {
            volume: kv.f32("volume", d.volume, 0.0, 1.0),
            music: kv.f32("music", d.music, 0.0, 1.0),
            render_scale: kv.f32("render_scale", d.render_scale, 0.5, 1.5),
            fullscreen: kv.bool("fullscreen", d.fullscreen),
            shadows: kv.bool("shadows", d.shadows),
            modern: kv.bool("modern", d.modern),
            raytracing: kv.bool("raytracing", d.raytracing),
            name: kv
                .str("name")
                .filter(|n| !n.is_empty())
                .map(|n| n.chars().take(16).collect())
                .unwrap_or(d.name),
            bots: kv.u32("bots", d.bots, 1, 7),
        }
    }

    pub fn to_kv(&self) -> KeyValues {
        let mut kv = KeyValues::default();
        kv.set("volume", self.volume);
        kv.set("music", self.music);
        kv.set("render_scale", self.render_scale);
        kv.set("fullscreen", self.fullscreen);
        kv.set("shadows", self.shadows);
        kv.set("modern", self.modern);
        kv.set("raytracing", self.raytracing);
        kv.set("name", &self.name);
        kv.set("bots", self.bots);
        kv
    }

    pub fn load() -> Self {
        Self::from_kv(&settings::load(APP, FILE))
    }

    pub fn save(&self) {
        settings::save(APP, FILE, &self.to_kv());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_clamping() {
        let s = Settings {
            volume: 0.3,
            music: 0.2,
            render_scale: 0.75,
            fullscreen: true,
            shadows: false,
            modern: true,
            raytracing: true,
            name: "Mettwurst".into(),
            bots: 5,
        };
        assert_eq!(Settings::from_kv(&s.to_kv()), s);
        let bad = Settings::from_kv(&KeyValues::parse("volume=7\nbots=99\nname="));
        assert_eq!(bad.volume, 1.0);
        assert_eq!(bad.bots, 7);
        assert_eq!(bad.name, "Hackepeter");
    }
}
