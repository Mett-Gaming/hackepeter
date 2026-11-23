# HACKEPETER – Die Mettbrötchen-Flucht

Einer hat das letzte Mettbrötchen geklaut und rennt. Der Rest ist das Ordnungsamt und will es zurück. Ein Endless-Runner auf drei Spuren, **asymmetrisch für 1 bis 8 Leute**: ein Dieb gegen bis zu sieben Cops, freie Plätze füllen Bots auf. Die Rollen wechseln jede Runde.

Läuft auf der eigenen Rust/Vulkan-Engine [Mettgine](https://github.com/Mett-Gaming/mettgine) (Git-Submodule in `mettgine/`). Die Shader sind in WGSL geschrieben und werden beim Bauen per naga zu SPIR-V kompiliert, ein Vulkan SDK ist nicht nötig. Musik und Sounds werden beim Start synthetisiert. Mehrspieler läuft über den [Mett Launcher](https://github.com/Mett-Gaming/mett-launcher) (Peer-to-Peer, ohne Account und eigenen Server).

- Credits und Lizenzen der Modelle: [CREDITS.md](CREDITS.md)
- Spieler-Anleitung im Paket: [packaging/LIESMICH.txt](packaging/LIESMICH.txt)

## Bauen und starten
```powershell
git clone --recurse-submodules https://github.com/Mett-Gaming/hackepeter
cargo run --release
cargo test --release
```

## So spielst du
**Dieb:** 120 Sekunden überleben. Absperrungen überspringen, unter Balken durchrutschen, Zügen und Autos ausweichen. Über Rampen geht es auf die Zugdächer und von dort auf Brücken; unter Brücken kann man auch durchlaufen. Crashes bringen dich ins Stolpern, die Cops holen auf.

**Power-ups** (leuchtende gelbe Sockel): Senf-Turbo (schneller), Zwiebelmagnet (zieht Ringe an), Brötchen-Schild (fängt einen Crash ab), Pfeffer-Rakete (Flug über alles, oben Ringe), Sprungfedern (viel höher springen), Mett-Stern (doppelte Ringpunkte).

**Ordnungsamt:** etwas schneller als der Dieb, eigene Crashes werfen zurück. Gefangen ist der Dieb, wenn ein Cop auf gut einen Meter herankommt. Fähigkeiten: **Absperrband** (sperrt eine Spur vor dem Dieb), **Dackel** (rennt zum Dieb, der muss springen), **Trillerpfeife** (kurzer Sprint). Nach Band oder Dackel hat das Team kurz Funkpause.

**Punkte:** Dieb 1 je Sekunde, 1 je 5 Ringe, +50 bei Flucht. Cops 100 für den Fang, 30 für die anderen.

## Steuerung
| Taste | Aktion |
|---|---|
| A/D, Pfeile links/rechts | Spur wechseln |
| W, Pfeil hoch, Leertaste | Springen |
| S, Pfeil runter, Strg, Shift | Rutschen (in der Luft: schnell runter) |
| 1/2/3 oder Q/E/R | Fähigkeiten (Cops) |
| Esc | Pause / zurück |
| F11 | Vollbild |

Controller: A springen, B rutschen, X/Y/RB Fähigkeiten, Steuerkreuz oder Stick lenken.

## Entwickeln
- Spiellogik in `src/game/` (Strecke per Seed, Bewegung, Cops, Bots, Runde), Netz in `src/net/` (Protokoll, Sitzung), Darstellung in `src/scene.rs` und `src/hud.rs`.
- Mehrspieler lokal testen: eine Instanz mit `HACKEPETER_DEV_HOST=7777`, weitere mit `HACKEPETER_DEV_JOIN=127.0.0.1:7777`.
- Assets: Original-Packs liegen in `assets_src/` (nicht im Repository, Quellen in `CREDITS.md`). Konvertiert wird mit Blender headless:
  ```powershell
  blender -b -P tools/prepare_assets.py
  blender -b -P tools/prepare_people.py
  ```
- Einstellungen und Logs: `%APPDATA%\.mettlauncher\hackepeter`.
- Arbeitsregeln für Agenten: [AGENTS.md](AGENTS.md).
