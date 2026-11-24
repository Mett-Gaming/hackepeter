//! Menue-Bildschirme: Eintraege und was sie tun. Das Zeichnen macht hud.rs.

use crate::settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Main,
    Solo,
    Friends,
    Settings,
    Lobby,
    Playing,
    Paused,
}

/// Was ein Menue-Eintrag ausloest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Go(Screen),
    StartSolo,
    Host,
    Quit,
    Resume,
    LeaveToMenu,
    /// Einstellung `n` aendern (-1 / +1)
    Setting(usize),
    Bots,
    None,
}

fn pct(v: f32) -> String {
    format!("{:.0} %", v * 100.0)
}

fn on(b: bool) -> &'static str {
    if b {
        "An"
    } else {
        "Aus"
    }
}

/// Eintraege und Aktionen eines Bildschirms.
pub fn items(screen: Screen, s: &Settings, launcher: bool) -> Vec<(String, Action)> {
    match screen {
        Screen::Main => vec![
            ("Einzelspieler".into(), Action::Go(Screen::Solo)),
            ("Mit Freunden".into(), Action::Go(Screen::Friends)),
            ("Einstellungen".into(), Action::Go(Screen::Settings)),
            ("Beenden".into(), Action::Quit),
        ],
        Screen::Solo => vec![
            (format!("Gegner: {} Bots", s.bots), Action::Bots),
            ("Los geht's!".into(), Action::StartSolo),
            ("Zurück".into(), Action::Go(Screen::Main)),
        ],
        Screen::Friends => {
            let mut v = Vec::new();
            if launcher {
                v.push(("Spiel hosten".into(), Action::Host));
            }
            v.push(("Zurück".into(), Action::Go(Screen::Main)));
            v
        }
        Screen::Settings => vec![
            (format!("Lautstärke: {}", pct(s.volume)), Action::Setting(0)),
            (format!("Musik: {}", pct(s.music)), Action::Setting(1)),
            (
                format!("Auflösung: {}", pct(s.render_scale)),
                Action::Setting(2),
            ),
            (
                format!("Vollbild: {}", on(s.fullscreen)),
                Action::Setting(3),
            ),
            (format!("Schatten: {}", on(s.shadows)), Action::Setting(4)),
            (
                format!("Moderne Grafik: {}", on(s.modern)),
                Action::Setting(5),
            ),
            (
                format!("Raytracing (ab Neustart): {}", on(s.raytracing)),
                Action::Setting(6),
            ),
            ("Zurück".into(), Action::Go(Screen::Main)),
        ],
        Screen::Paused => vec![
            ("Weiter".into(), Action::Resume),
            ("Zum Hauptmenü".into(), Action::LeaveToMenu),
        ],
        Screen::Lobby | Screen::Playing => Vec::new(),
    }
}

/// Einstellung `n` um eine Stufe aendern.
pub fn change_setting(s: &mut Settings, n: usize, dir: i32) {
    // auf zwei Nachkommastellen runden, damit 0.1-Schritte sauber bei 0 und 1 landen
    let step = |v: f32, d: f32| ((v + d * dir as f32) * 100.0).round() / 100.0;
    match n {
        0 => s.volume = step(s.volume, 0.1).clamp(0.0, 1.0),
        1 => s.music = step(s.music, 0.1).clamp(0.0, 1.0),
        2 => s.render_scale = step(s.render_scale, 0.25).clamp(0.5, 1.5),
        3 => s.fullscreen = !s.fullscreen,
        4 => s.shadows = !s.shadows,
        5 => s.modern = !s.modern,
        6 => s.raytracing = !s.raytracing,
        _ => {}
    }
}

/// Hinweistexte fuer "Mit Freunden".
pub fn friends_text(launcher: bool) -> Vec<&'static str> {
    if launcher {
        vec![
            "Hoste ein Spiel und gib deinen Freunden den Code.",
            "Beitreten: im Mett Launcher bei HACKEPETER auf „Mit Freunden spielen“ und den Code eingeben.",
            "Bis zu 8 Spieler: 1 Dieb, der Rest ist Ordnungsamt. Freie Plätze füllen Bots.",
        ]
    } else {
        vec![
            "Mehrspieler läuft über den Mett Launcher.",
            "Starte HACKEPETER im Launcher über „Mit Freunden spielen“: hosten oder mit Code beitreten.",
            "Kein Account, kein eigener Server nötig.",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_change_in_steps_and_stay_in_range() {
        let mut s = Settings::default();
        for _ in 0..20 {
            change_setting(&mut s, 0, 1);
            change_setting(&mut s, 2, -1);
        }
        assert_eq!(s.volume, 1.0);
        assert_eq!(s.render_scale, 0.5);
        change_setting(&mut s, 3, 1);
        assert!(s.fullscreen);
        assert!(!s.modern && !s.raytracing, "Vorgabe: wie bisher");
        change_setting(&mut s, 5, 1);
        assert!(s.modern);
        assert!(items(Screen::Friends, &s, false)
            .iter()
            .all(|(_, a)| *a != Action::Host));
    }
}
