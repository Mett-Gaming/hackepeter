//! Menues und HUD (Zeichenflaeche: mettgine::canvas, virtuelle Hoehe 720).

use crate::game::cops::{Ability, ABILITIES};
use crate::game::round::{Phase, Round, TEAM_LOCK};
use crate::game::{Role, ROUND_TIME};
use crate::net::session::{Session, OVER_PAUSE};
use glam::{Mat4, Vec3};
use mettgine::canvas::{alpha, Align, Canvas, Color, BLACK, WHITE};

/// Markenfarben: Mett-Rosa, Senfgelb, Ordnungsamt-Blau
pub const METT: Color = [0.93, 0.42, 0.45, 1.0];
pub const MUSTARD: Color = [0.98, 0.78, 0.15, 1.0];
pub const COP_BLUE: Color = [0.35, 0.55, 0.95, 1.0];
pub const DIM: Color = [0.75, 0.73, 0.7, 1.0];
pub const RED: Color = [0.95, 0.3, 0.25, 1.0];
const PANEL: Color = [0.06, 0.05, 0.06, 0.72];

pub fn role_color(r: Role) -> Color {
    match r {
        Role::Thief => METT,
        Role::Cop => COP_BLUE,
    }
}

/// Titel mit Untertitel oben in der Mitte.
fn title(c: &mut Canvas) {
    let cx = c.w * 0.5;
    c.text_s(cx, 70.0, "HACKEPETER", 96.0, METT, Align::Center);
    c.text_s(
        cx,
        172.0,
        "Die Mettbrötchen-Flucht",
        30.0,
        MUSTARD,
        Align::Center,
    );
}

/// Eintraege eines Menues; Rueckgabe: Rechtecke der Eintraege (fuer die Maus).
pub fn menu(c: &mut Canvas, items: &[String], sel: usize, footer: &str) -> Vec<[f32; 4]> {
    c.rect(0.0, 0.0, c.w, c.h, [0.0, 0.0, 0.0, 0.45]);
    title(c);
    let rects = menu_at(c, items, sel, 290.0);
    c.text_s(c.w * 0.5, c.h - 50.0, footer, 20.0, DIM, Align::Center);
    rects
}

/// Nur die Eintraege, erster bei Hoehe `y0`.
pub fn menu_at(c: &mut Canvas, items: &[String], sel: usize, y0: f32) -> Vec<[f32; 4]> {
    let cx = c.w * 0.5;
    let mut rects = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let y = y0 + i as f32 * 56.0;
        let w = c.text_width(it, 34.0) + 60.0;
        let r = [cx - w * 0.5, y - 8.0, w, 50.0];
        if i == sel {
            c.rect(r[0], r[1], r[2], r[3], alpha(MUSTARD, 0.9));
            c.text(cx, y, it, 34.0, BLACK, Align::Center);
        } else {
            c.text_s(cx, y, it, 34.0, WHITE, Align::Center);
        }
        rects.push(r);
    }
    rects
}

/// Freier Hinweistext unter dem Titel (z. B. "Mit Freunden").
pub fn info(c: &mut Canvas, lines: &[&str]) {
    c.rect(0.0, 0.0, c.w, c.h, [0.0, 0.0, 0.0, 0.55]);
    title(c);
    let cx = c.w * 0.5;
    let mut y = 270.0;
    for l in lines {
        for w in c.wrap(l, 26.0, c.w * 0.7) {
            c.text_s(cx, y, &w, 26.0, WHITE, Align::Center);
            y += 36.0;
        }
        y += 10.0;
    }
}

/// Lobby: Code, Spieler, Bots, Start.
pub fn lobby(c: &mut Canvas, s: &Session) {
    c.rect(0.0, 0.0, c.w, c.h, [0.0, 0.0, 0.0, 0.55]);
    title(c);
    let cx = c.w * 0.5;
    let code = s
        .code
        .as_deref()
        .map(|k| format!("Code für Freunde: {k}"))
        .unwrap_or_else(|| {
            if s.host {
                "Code wird erstellt ...".into()
            } else {
                "Verbinde mit dem Host ...".into()
            }
        });
    c.text_s(cx, 230.0, &code, 26.0, MUSTARD, Align::Center);
    let x = cx - 260.0;
    c.rect(
        x - 20.0,
        280.0,
        560.0,
        50.0 + 44.0 * s.members.len() as f32,
        PANEL,
    );
    c.text(
        x,
        292.0,
        &format!("Spieler ({}/8)", s.members.len()),
        24.0,
        DIM,
        Align::Left,
    );
    for (i, m) in s.members.iter().enumerate() {
        let y = 330.0 + i as f32 * 44.0;
        let me = m.id == s.my_id;
        let col = if m.bot {
            DIM
        } else if me {
            MUSTARD
        } else {
            WHITE
        };
        c.text(x, y, &m.name, 28.0, col, Align::Left);
        c.text(
            x + 520.0,
            y,
            &format!("{} P", m.score),
            24.0,
            DIM,
            Align::Right,
        );
    }
    let hint = if s.host {
        "Enter: Runde starten   Links/Rechts: Bots auffüllen   Esc: verlassen"
    } else {
        "Warte, bis der Host die Runde startet   Esc: verlassen"
    };
    c.text_s(cx, c.h - 50.0, hint, 20.0, DIM, Align::Center);
    if let Some(n) = &s.notice {
        c.text_s(cx, c.h - 90.0, n, 20.0, RED, Align::Center);
    }
}

/// Weltpunkt auf die UI-Flaeche (None = hinter der Kamera).
fn project(vp: &Mat4, p: Vec3, c: &Canvas) -> Option<(f32, f32)> {
    let clip = *vp * p.extend(1.0);
    if clip.w <= 0.1 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(((ndc.x * 0.5 + 0.5) * c.w, (ndc.y * 0.5 + 0.5) * c.h))
}

fn clock(t: f32) -> String {
    let t = t.max(0.0).ceil() as i32;
    format!("{}:{:02}", t / 60, t % 60)
}

/// HUD waehrend einer Runde.
pub fn ingame(c: &mut Canvas, s: &Session, vp: &Mat4, pad: bool) {
    let Some(r) = s.round.as_ref() else {
        return;
    };
    let cx = c.w * 0.5;
    name_tags(c, r, s.my_id, vp);
    let me = r.player(s.my_id);
    // Zeit und Vorsprung
    let left = ROUND_TIME - r.time.max(0.0);
    c.rect(cx - 120.0, 14.0, 240.0, 64.0, PANEL);
    c.text(
        cx,
        20.0,
        &clock(left),
        44.0,
        if left < 15.0 { RED } else { WHITE },
        Align::Center,
    );
    let gap = r.closest_gap();
    if gap < 1e6 {
        let w = 360.0;
        let v = (gap / 60.0).clamp(0.0, 1.0);
        c.bar(cx - w * 0.5, 86.0, w, 10.0, v, METT, [0.2, 0.2, 0.3, 0.8]);
        c.text_s(
            cx,
            100.0,
            &format!("Vorsprung {:.0} m", gap.max(0.0)),
            20.0,
            DIM,
            Align::Center,
        );
    }
    if let Some(p) = me {
        match p.role {
            Role::Thief => thief_panel(c, p),
            Role::Cop => cop_panel(c, p, r, pad),
        }
        // Rollen-Hinweis waehrend Countdown und kurz danach
        if r.time < 2.5 {
            let (txt, col) = match p.role {
                Role::Thief => ("DU HAST DAS METTBRÖTCHEN! LAUF!", METT),
                Role::Cop => ("ORDNUNGSAMT! SCHNAPP DEN DIEB!", COP_BLUE),
            };
            c.text_s(cx, 170.0, txt, 40.0, col, Align::Center);
        }
    }
    if r.phase == Phase::Countdown {
        let n = (-r.time).ceil() as i32;
        c.text_s(
            cx,
            c.h * 0.38,
            &n.max(1).to_string(),
            140.0,
            MUSTARD,
            Align::Center,
        );
    } else if r.time < 0.8 {
        c.text_s(cx, c.h * 0.38, "LOS!", 140.0, MUSTARD, Align::Center);
    }
    if let Phase::Over { thief_won, catcher } = r.phase {
        round_over(c, s, r, thief_won, catcher);
    }
    if let Some(n) = &s.notice {
        c.text_s(cx, c.h - 40.0, n, 18.0, RED, Align::Center);
    }
}

fn name_tags(c: &mut Canvas, r: &Round, me: u16, vp: &Mat4) {
    for p in &r.players {
        if p.id == me {
            continue;
        }
        let m = &p.mover;
        if let Some((x, y)) = project(vp, Vec3::new(m.x, m.y + 2.15, m.z), c) {
            let col = role_color(p.role);
            c.text_s(x, y - 24.0, &p.name, 20.0, col, Align::Center);
        }
    }
}

fn thief_panel(c: &mut Canvas, p: &crate::game::round::Player) {
    c.rect(20.0, 20.0, 300.0, 200.0, PANEL);
    c.text(
        36.0,
        28.0,
        &format!("Zwiebelringe: {}", p.rings),
        26.0,
        MUSTARD,
        Align::Left,
    );
    let mut y = 66.0;
    for (on, label) in [
        (
            p.mustard > 0.0,
            format!("Senf-Turbo {:.0}s", p.mustard.ceil()),
        ),
        (
            p.magnet > 0.0,
            format!("Zwiebelmagnet {:.0}s", p.magnet.ceil()),
        ),
        (p.shield, "Brötchen-Schild".to_string()),
        (
            p.rocket > 0.0,
            format!("Pfeffer-Rakete {:.0}s", p.rocket.ceil()),
        ),
        (
            p.springs > 0.0,
            format!("Sprungfedern {:.0}s", p.springs.ceil()),
        ),
        (
            p.multiplier > 0.0,
            format!("Mett-Stern x2 {:.0}s", p.multiplier.ceil()),
        ),
    ] {
        if on {
            c.text(36.0, y, &label, 22.0, WHITE, Align::Left);
            y += 26.0;
        }
    }
}

fn cop_panel(c: &mut Canvas, p: &crate::game::round::Player, r: &Round, pad: bool) {
    let keys = if pad {
        ["X", "Y", "RB"]
    } else {
        ["1", "2", "3"]
    };
    let w = 210.0;
    let x0 = c.w * 0.5 - (w * 3.0 + 20.0) * 0.5;
    let y = c.h - 110.0;
    for (i, a) in ABILITIES.iter().enumerate() {
        let x = x0 + i as f32 * (w + 10.0);
        c.rect(x, y, w, 80.0, PANEL);
        let locked = *a != Ability::Whistle && r.team_lock > 0.0;
        let ready = p.cooldowns.ready(*a) && !locked;
        let col = if ready { WHITE } else { DIM };
        c.text(
            x + 12.0,
            y + 8.0,
            &format!("[{}] {}", keys[i], a.name()),
            22.0,
            col,
            Align::Left,
        );
        let v = if locked {
            1.0 - r.team_lock / TEAM_LOCK
        } else {
            1.0 - p.cooldowns.fraction(*a)
        };
        c.bar(
            x + 12.0,
            y + 50.0,
            w - 24.0,
            12.0,
            v,
            if ready { MUSTARD } else { COP_BLUE },
            [0.15, 0.15, 0.2, 0.9],
        );
    }
    if r.team_lock > 0.0 {
        c.text_s(
            c.w * 0.5,
            y - 30.0,
            "Funk belegt ...",
            20.0,
            DIM,
            Align::Center,
        );
    }
}

fn round_over(c: &mut Canvas, s: &Session, r: &Round, thief_won: bool, catcher: Option<u16>) {
    let cx = c.w * 0.5;
    c.rect(0.0, 0.0, c.w, c.h, [0.0, 0.0, 0.0, 0.5]);
    let thief = r.thief().map_or("Der Dieb".to_string(), |t| t.name.clone());
    let (head, sub) = if thief_won {
        (
            "ENTKOMMEN!".to_string(),
            format!("{thief} hat das Mettbrötchen in Sicherheit gebracht"),
        )
    } else {
        let who = catcher
            .and_then(|id| r.player(id))
            .map_or("Das Ordnungsamt".to_string(), |p| p.name.clone());
        (
            "GESCHNAPPT!".to_string(),
            format!("{who} hat {thief} erwischt"),
        )
    };
    c.text_s(
        cx,
        120.0,
        &head,
        90.0,
        if thief_won { METT } else { COP_BLUE },
        Align::Center,
    );
    c.text_s(cx, 225.0, &sub, 26.0, WHITE, Align::Center);
    let x = cx - 300.0;
    let mut rows: Vec<_> = s.members.iter().collect();
    rows.sort_by_key(|m| -m.score);
    c.rect(
        x - 20.0,
        280.0,
        640.0,
        40.0 + 40.0 * rows.len() as f32,
        PANEL,
    );
    for (i, m) in rows.iter().enumerate() {
        let y = 296.0 + i as f32 * 40.0;
        let gained = r.player(m.id).map_or(0, |p| p.points);
        let col = if m.id == s.my_id { MUSTARD } else { WHITE };
        c.text(
            x,
            y,
            &format!("{}. {}", i + 1, m.name),
            26.0,
            col,
            Align::Left,
        );
        c.text(x + 470.0, y, &format!("+{gained}"), 24.0, DIM, Align::Right);
        c.text(
            x + 600.0,
            y,
            &format!("{} P", m.score),
            26.0,
            col,
            Align::Right,
        );
    }
    let left = (OVER_PAUSE - s.over_timer).max(0.0).ceil();
    c.text_s(
        cx,
        c.h - 60.0,
        &format!("Nächste Runde in {left:.0} s – neuer Dieb!"),
        22.0,
        DIM,
        Align::Center,
    );
}
