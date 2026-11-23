//! Klaenge des Spiels, beim Start im Code erzeugt (Bausteine: mettgine::audio::synth).
//! Dazu eine kurze Musikschleife ("Mett-Groove", 120 BPM).

use mettgine::audio::synth::{self, Wave};
use mettgine::audio::Sound;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sfx {
    Jump,
    Land,
    Slide,
    Ring,
    Power,
    Crash,
    Stumble,
    Bark,
    Whistle,
    Tape,
    Beep,
    Go,
    Caught,
    Escaped,
    Blip,
    Select,
}

pub const ALL_SFX: [Sfx; 16] = [
    Sfx::Jump,
    Sfx::Land,
    Sfx::Slide,
    Sfx::Ring,
    Sfx::Power,
    Sfx::Crash,
    Sfx::Stumble,
    Sfx::Bark,
    Sfx::Whistle,
    Sfx::Tape,
    Sfx::Beep,
    Sfx::Go,
    Sfx::Caught,
    Sfx::Escaped,
    Sfx::Blip,
    Sfx::Select,
];

fn env(mut s: Vec<f32>, attack: f32, decay: f32, gain: f32) -> Vec<f32> {
    synth::envelope(&mut s, attack, decay);
    s.iter_mut().for_each(|x| *x *= gain);
    s
}

fn notes(seq: &[(f32, f32)], wave: Wave, step: f32, decay: f32) -> Vec<f32> {
    let mut out = Vec::new();
    for (i, &(f, len)) in seq.iter().enumerate() {
        let n = env(synth::tone(wave, f, f, len), 0.005, decay, 0.6);
        synth::mix_into(&mut out, &n, i as f32 * step, 1.0);
    }
    out
}

/// Einen Effekt bauen.
pub fn make(s: Sfx) -> Sound {
    let mut v = match s {
        Sfx::Jump => env(
            synth::tone(Wave::Square, 280.0, 620.0, 0.16),
            0.004,
            0.08,
            0.35,
        ),
        Sfx::Land => {
            let mut n = synth::noise(0.12, 3);
            synth::lowpass(&mut n, 0.08);
            env(n, 0.002, 0.04, 1.0)
        }
        Sfx::Slide => {
            let mut n = synth::noise(0.4, 5);
            synth::lowpass(&mut n, 0.25);
            env(n, 0.05, 0.18, 0.5)
        }
        Sfx::Ring => notes(&[(1320.0, 0.08), (1760.0, 0.12)], Wave::Sine, 0.06, 0.06),
        Sfx::Power => notes(
            &[(523.0, 0.1), (659.0, 0.1), (784.0, 0.1), (1047.0, 0.2)],
            Wave::Triangle,
            0.07,
            0.1,
        ),
        Sfx::Crash => {
            let mut n = synth::noise(0.5, 9);
            synth::lowpass(&mut n, 0.15);
            let mut v = env(n, 0.002, 0.15, 1.0);
            let thud = env(synth::tone(Wave::Sine, 120.0, 40.0, 0.4), 0.002, 0.12, 0.9);
            synth::mix_into(&mut v, &thud, 0.0, 1.0);
            v
        }
        Sfx::Stumble => env(synth::tone(Wave::Saw, 300.0, 150.0, 0.2), 0.004, 0.08, 0.3),
        Sfx::Bark => {
            let mut v = Vec::new();
            for k in 0..2 {
                let mut b = synth::tone(Wave::Saw, 520.0, 300.0, 0.12);
                synth::lowpass(&mut b, 0.35);
                synth::mix_into(&mut v, &env(b, 0.003, 0.05, 0.8), k as f32 * 0.18, 1.0);
            }
            v
        }
        Sfx::Whistle => {
            // Trillerpfeife: hoher Ton mit schnellem Flattern
            let mut t = synth::tone(Wave::Sine, 2900.0, 2700.0, 0.6);
            for (i, x) in t.iter_mut().enumerate() {
                let f =
                    (i as f32 / mettgine::audio::RATE as f32 * 38.0 * std::f32::consts::TAU).sin();
                *x *= 0.6 + 0.4 * f;
            }
            env(t, 0.01, 0.4, 0.35)
        }
        Sfx::Tape => env(
            synth::tone(Wave::Square, 900.0, 600.0, 0.25),
            0.003,
            0.1,
            0.25,
        ),
        Sfx::Beep => env(
            synth::tone(Wave::Square, 660.0, 660.0, 0.15),
            0.003,
            0.08,
            0.3,
        ),
        Sfx::Go => env(
            synth::tone(Wave::Square, 1320.0, 1320.0, 0.35),
            0.003,
            0.2,
            0.3,
        ),
        Sfx::Caught => notes(
            &[(392.0, 0.2), (330.0, 0.2), (262.0, 0.5)],
            Wave::Square,
            0.18,
            0.2,
        ),
        Sfx::Escaped => notes(
            &[(523.0, 0.12), (659.0, 0.12), (784.0, 0.12), (1047.0, 0.5)],
            Wave::Square,
            0.11,
            0.25,
        ),
        Sfx::Blip => env(
            synth::tone(Wave::Square, 880.0, 880.0, 0.05),
            0.002,
            0.03,
            0.2,
        ),
        Sfx::Select => notes(&[(660.0, 0.06), (990.0, 0.1)], Wave::Square, 0.05, 0.05),
    };
    synth::normalize(&mut v, 0.9);
    Sound::new(v)
}

/// Musikschleife: 8 Takte, Kick, Hi-Hat, Bass und eine einfache Melodie in a-Moll.
pub fn music() -> Sound {
    let beat = 0.5; // 120 BPM
    let bars = 8;
    let mut out = Vec::new();
    let kick = env(synth::tone(Wave::Sine, 150.0, 45.0, 0.25), 0.002, 0.08, 0.9);
    let mut hat = synth::noise(0.05, 11);
    synth::envelope(&mut hat, 0.001, 0.015);
    let bass_roots = [110.0, 110.0, 87.3, 98.0, 110.0, 110.0, 130.8, 98.0];
    let melody = [
        440.0, 0.0, 523.3, 440.0, 659.3, 0.0, 587.3, 523.3, 440.0, 0.0, 392.0, 440.0, 523.3, 0.0,
        493.9, 440.0,
    ];
    for bar in 0..bars {
        let t0 = bar as f32 * 4.0 * beat;
        for b in 0..4 {
            let t = t0 + b as f32 * beat;
            synth::mix_into(&mut out, &kick, t, 0.8);
            synth::mix_into(&mut out, &hat, t + beat * 0.5, 0.25);
            let root = bass_roots[bar];
            let note = if b % 2 == 0 { root } else { root * 1.5 };
            let mut bass = synth::tone(Wave::Saw, note, note, beat * 0.9);
            synth::lowpass(&mut bass, 0.08);
            synth::mix_into(&mut out, &env(bass, 0.005, 0.25, 0.7), t, 0.6);
        }
        for k in 0..8 {
            let f = melody[(bar % 2) * 8 + k];
            if f > 0.0 {
                let n = env(
                    synth::tone(Wave::Triangle, f, f, beat * 0.45),
                    0.005,
                    0.12,
                    0.5,
                );
                synth::mix_into(&mut out, &n, t0 + k as f32 * beat * 0.5, 0.35);
            }
        }
    }
    // exakt auf Schleifenlaenge schneiden
    out.resize(synth::len(bars as f32 * 4.0 * beat), 0.0);
    synth::normalize(&mut out, 0.7);
    Sound::new(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sounds_are_short_and_not_silent() {
        for s in ALL_SFX {
            let snd = make(s);
            assert!(snd.duration() > 0.03 && snd.duration() < 2.0, "{s:?}");
            assert!(snd.0.iter().any(|x| x.abs() > 0.1), "{s:?} still");
            assert!(snd.0.iter().all(|x| x.abs() <= 1.0), "{s:?} uebersteuert");
        }
        let m = music();
        assert!((m.duration() - 16.0).abs() < 0.01);
    }
}
