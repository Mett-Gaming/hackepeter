//! Tastenbelegung: Aktionen des Spiels auf Tastatur, Controller und Stick (mettgine::input).

use mettgine::input::{Bind, Pad, Stick};
use winit::keyboard::KeyCode as K;

pub const LEFT: &[Bind] = &[
    Bind::Key(K::KeyA),
    Bind::Key(K::ArrowLeft),
    Bind::Pad(Pad::Left),
    Bind::Stick(Stick::Left),
];
pub const RIGHT: &[Bind] = &[
    Bind::Key(K::KeyD),
    Bind::Key(K::ArrowRight),
    Bind::Pad(Pad::Right),
    Bind::Stick(Stick::Right),
];
pub const UP: &[Bind] = &[
    Bind::Key(K::KeyW),
    Bind::Key(K::ArrowUp),
    Bind::Pad(Pad::Up),
    Bind::Stick(Stick::Up),
];
pub const DOWN: &[Bind] = &[
    Bind::Key(K::KeyS),
    Bind::Key(K::ArrowDown),
    Bind::Pad(Pad::Down),
    Bind::Stick(Stick::Down),
];
pub const JUMP: &[Bind] = &[
    Bind::Key(K::KeyW),
    Bind::Key(K::ArrowUp),
    Bind::Key(K::Space),
    Bind::Pad(Pad::South),
    Bind::Pad(Pad::Up),
    Bind::Stick(Stick::Up),
];
pub const SLIDE: &[Bind] = &[
    Bind::Key(K::KeyS),
    Bind::Key(K::ArrowDown),
    Bind::Key(K::ControlLeft),
    Bind::Key(K::ShiftLeft),
    Bind::Pad(Pad::East),
    Bind::Pad(Pad::Down),
    Bind::Stick(Stick::Down),
];
/// Faehigkeiten der Cops: Absperrband, Dackel, Trillerpfeife
pub const ABILITY: [&[Bind]; 3] = [
    &[
        Bind::Key(K::Digit1),
        Bind::Key(K::KeyQ),
        Bind::Pad(Pad::West),
    ],
    &[
        Bind::Key(K::Digit2),
        Bind::Key(K::KeyE),
        Bind::Pad(Pad::North),
    ],
    &[
        Bind::Key(K::Digit3),
        Bind::Key(K::KeyR),
        Bind::Pad(Pad::RightBumper),
    ],
];
pub const CONFIRM: &[Bind] = &[
    Bind::Key(K::Enter),
    Bind::Key(K::NumpadEnter),
    Bind::Key(K::Space),
    Bind::Pad(Pad::South),
];
pub const BACK: &[Bind] = &[Bind::Key(K::Escape), Bind::Pad(Pad::East)];
pub const PAUSE: &[Bind] = &[Bind::Key(K::Escape), Bind::Pad(Pad::Start)];
pub const FULLSCREEN: &[Bind] = &[Bind::Key(K::F11)];
