//! Spielregeln ohne Grafik und Netzwerk: Strecke, Bewegung, Ordnungsamt-Faehigkeiten, Bots,
//! Runde. Alles deterministisch (Seed), damit alle Spieler dieselbe Strecke laufen und die
//! Regeln ohne Fenster getestet werden koennen.
//!
//! Koordinaten: x = seitlich (Spuren bei -2.5, 0, 2.5), y = hoch, z = Laufrichtung (Meter).

pub mod bots;
pub mod cops;
pub mod mover;
pub mod round;
pub mod track;

#[cfg(test)]
mod tests;

/// Abstand der Spuren (Meter)
pub const LANE_W: f32 = 2.5;
/// Anzahl Spuren
pub const LANES: i8 = 3;
/// Rundenlaenge (Sekunden): so lange muss der Dieb durchhalten
pub const ROUND_TIME: f32 = 120.0;
/// Countdown vor dem Start (Sekunden)
pub const COUNTDOWN: f32 = 3.0;
/// Fang-Abstand Cop -> Dieb (Meter)
pub const CATCH_DIST: f32 = 1.3;
/// Vorsprung des Diebs beim Start (Meter)
pub const START_GAP: f32 = 22.0;
/// Hoechstzahl Spieler (1 Dieb + 7 Cops)
pub const MAX_PLAYERS: usize = 8;

/// x-Position einer Spur (-1, 0, 1).
pub fn lane_x(lane: i8) -> f32 {
    lane.clamp(-1, 1) as f32 * LANE_W
}

/// Rolle eines Spielers in einer Runde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Mettbroetchen-Dieb
    Thief,
    /// Ordnungsamt
    Cop,
}

/// Grundtempo der Runde zur Rundenzeit `t` (m/s): steigt in 100 s von 11 auf 17.
pub fn base_speed(t: f32) -> f32 {
    11.0 + 6.0 * (t / 100.0).clamp(0.0, 1.0)
}

/// Tempo-Faktor der Cops: etwas schneller als der Dieb, mit mehr Cops etwas weniger
/// (sonst waere es bei 7 Cops zu leicht).
pub fn cop_speed_factor(cops: usize) -> f32 {
    1.02 - 0.003 * cops.saturating_sub(2) as f32
}
