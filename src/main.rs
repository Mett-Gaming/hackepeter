//! HACKEPETER - Die Mettbroetchen-Flucht.
//!
//! Endless-Runner fuer bis zu 8 Spieler: ein Dieb mit Mettbroetchen, der Rest jagt ihn als
//! Ordnungsamt. Engine: Mettgine (Submodule `mettgine/`). Mehrspieler ueber den Mett Launcher.

// Release-Build ohne Konsolenfenster
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod controls;
mod game;
mod hud;
mod menus;
mod net;
mod people;
mod scene;
mod settings;
mod sounds;

use app::{App, Running};
use mettgine::canvas::Canvas;
use mettgine::text::FontAtlas;
use mettgine::{crash, crash_window};
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::{Window, WindowId},
};

/// Version aus dem Git-Tag (build.rs), fuer Absturzberichte und Menue
pub const VERSION: &str = env!("HACKEPETER_VERSION");
const TITLE: &str = "HACKEPETER";
static FONT: &[u8] = include_bytes!("../assets/fonts/ArchivoBlack-Regular.ttf");

struct Handler {
    app: App,
    last_frame: Instant,
}

impl ApplicationHandler for Handler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.running.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(TITLE)
            .with_inner_size(LogicalSize::new(1280.0, 720.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                crash::fatal(
                    "Fenster",
                    &format!("Fenster konnte nicht erstellt werden: {e}"),
                    "Das Spielfenster konnte nicht geöffnet werden.",
                );
                event_loop.exit();
                return;
            }
        };
        let meshes = assets::load_all();
        let skins = people::load_skins();
        let f = &self.app.canvas.font;
        let renderer = match mettgine::scene::SceneRenderer::new(
            &window,
            &meshes,
            &skins,
            (&f.pixels, f.width, f.height),
            self.app.settings.render_scale,
        ) {
            Ok(r) => r,
            Err(e) => {
                crash::fatal(
                    "Vulkan",
                    &format!("Vulkan-Initialisierung fehlgeschlagen: {e}"),
                    "Die Grafik konnte nicht gestartet werden. HACKEPETER braucht eine \
                     Grafikkarte mit Vulkan 1.3. Bitte den Grafiktreiber aktualisieren.",
                );
                event_loop.exit();
                return;
            }
        };
        crash::context("GPU", renderer.gpu_name());
        crash::log(&format!("GPU: {}", renderer.gpu_name()));
        self.app.running = Some(Running { window, renderer });
        self.app.apply_settings();
        self.last_frame = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.app.running {
                    r.renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::Focused(false) => self.app.focus_lost(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(r) = &self.app.running {
                    let s = r.window.inner_size();
                    let x = position.x as f32 / s.width.max(1) as f32 * r.renderer.ui_width();
                    let y = position.y as f32 / s.height.max(1) as f32 * mettgine::scene::UI_HEIGHT;
                    self.app.mouse = (x, y);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.app.clicked = true,
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } => self.app.input.key(code, state == ElementState::Pressed),
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32().min(0.05);
                self.last_frame = now;
                self.app.frame(dt);
                self.app.input.begin_frame();
                if self.app.quit {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(r) = &self.app.running {
            r.window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    // Absturzfenster: eigener Prozess nach einem Absturz
    if let Some(c) = crash_window::parse_args(&args) {
        crash_window::run(&c, TITLE);
        return Ok(());
    }
    crash::init(settings::APP, TITLE, VERSION);
    let font = FontAtlas::bake(FONT)?;
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut handler = Handler {
        app: App::new(Canvas::new(font)),
        last_frame: Instant::now(),
    };
    event_loop.run_app(&mut handler)?;
    if let Some(s) = handler.app.running.take() {
        drop(s);
    }
    Ok(())
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
