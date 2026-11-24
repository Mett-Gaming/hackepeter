//! HACKEPETER - Die Mettbroetchen-Flucht.
//!
//! Endless-Runner fuer bis zu 8 Spieler: ein Dieb mit Mettbroetchen, der Rest jagt ihn als
//! Ordnungsamt. Engine: Mettgine (Submodule `mettgine/`). Mehrspieler ueber den Mett Launcher.

// Release-Build ohne Konsolenfenster
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod achievements;
mod app;
mod assets;
mod controls;
mod game;
mod hud;
mod menus;
mod modern;
mod net;
mod people;
mod scene;
mod settings;
mod sounds;

use app::App;
use mettgine::app::AppConfig;
use mettgine::canvas::Canvas;
use mettgine::text::FontAtlas;

/// Version aus dem Git-Tag (build.rs), fuer Absturzberichte und Menue
pub const VERSION: &str = env!("HACKEPETER_VERSION");
const TITLE: &str = "HACKEPETER";
static FONT: &[u8] = include_bytes!("../assets/fonts/ArchivoBlack-Regular.ttf");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig {
        max_dt: 0.05,
        ..AppConfig::new(settings::APP, TITLE, VERSION)
    };
    mettgine::app::run(config, || {
        let font = FontAtlas::bake(FONT)?;
        Ok(App::new(Canvas::new(font)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_font_bakes_with_umlauts() {
        let a = FontAtlas::bake(FONT).unwrap();
        for c in ['A', 'z', '0', 'ä', 'Ö', 'ß', '!'] {
            let g = a.glyph(c);
            assert!(g.size[0] > 0.0 && g.advance > 0.0, "Zeichen fehlt: {c}");
        }
        assert!(a.width("Mett", 20.0) > a.width("M", 20.0));
    }
}
