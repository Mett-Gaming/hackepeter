# Credits

Spiel, Engine ([Mettgine](https://github.com/Mett-Gaming/mettgine)), Code, Shader, Musik und alle Sounds (synthetisiert): eigenes Projekt.

## 3D-Modelle
Alle Modelle wurden an den Stil angepasst: Polygone reduziert, Texturen durch Durchschnittsfarben ersetzt, Größe angeglichen und teils umgefärbt (`tools/prepare_assets.py`, `tools/prepare_people.py`).

### Figuren
| Modell | Quelle | Autor | Lizenz |
|---|---|---|---|
| Diebe und Ordnungsamt (Teile gemischt, Ordnungsamt in Dienstkleidung umgefärbt) | [Ultimate Modular Men Pack](https://poly.pizza/bundle/ZiH8muWqwQ), [Ultimate Modular Women Pack](https://poly.pizza/bundle/aCBDXDdTNN) | Quaternius | CC0 1.0 |
| Animationen (Rennen, Springen, Rutschen, Stolpern usw., auf die Figuren übertragen) | [Universal Animation Library](https://quaternius.itch.io/universal-animation-library) 1 und [2](https://quaternius.itch.io/universal-animation-library-2) | Quaternius | CC0 1.0 |

### Strecke, Fahrzeuge und Gegenstände (von [poly.pizza](https://poly.pizza), Link: `https://poly.pizza/m/<ID>`)
| Im Spiel | Modell (ID) | Autor | Lizenz |
|---|---|---|---|
| Personenwagen | Locomotive Passenger Carriage (woQPFlzPHM) | Quaternius | CC0 1.0 |
| Lok | Locomotive Front (WY84FHug9s) | Quaternius | CC0 1.0 |
| Schnellzug | High Speed Front (mJZDSqyzlk) | Quaternius | CC0 1.0 |
| Containerwagen | Cargo Train Container (AILdezfcv1) | Quaternius | CC0 1.0 |
| Lastwagen | Truck (cXw6oiFtZ8) | Quaternius | CC0 1.0 |
| Lieferwagen | Van (aT_24cDaW1a) | Poly by Google | **CC-BY 3.0** |
| Streifenwagen | Police Car (BwwnUrWGmV) | Quaternius | CC0 1.0 |
| Absperrgitter | Traffic Barrier (cM3aJPU9NS) | Quaternius | CC0 1.0 |
| Große Absperrung | Barrier Large (gLbBiYwt7l) | Quaternius | CC0 1.0 |
| Baustellenbake | Construction Barrier (4mrO9ueiQr) | Kenney | CC0 1.0 |
| Leitkegel | Traffic Cone (lAx8JytxGD) | Quaternius | CC0 1.0 |
| Straßenlaterne | Street Light (nFwrlcLvM5) | Quaternius | CC0 1.0 |
| Hydrant | Fire Hydrant (DKkMQbEklp) | Quaternius | CC0 1.0 |
| Bank | Bench (jLxjFxFRpw) | Quaternius | CC0 1.0 |
| Mülleimer | Trashcan Small (i7HDuYDLkx) | Quaternius | CC0 1.0 |
| Hochhaus A | Large Building (ppwtREejXg) | Kenney | CC0 1.0 |
| Hochhaus B | Large Building (yKo7F36Qk2) | Kenney | CC0 1.0 |
| Stadthaus | Town House (imVkxz7oZD) | Quaternius | CC0 1.0 |
| Marktstand (Start) | Market Stand (DGIM5HGISb) | Quaternius | CC0 1.0 |
| Ampel | Trafficlight B (miIn9cCbEg) | Kay Lousberg | CC0 1.0 |
| Dackel (in die Länge gezogen) | Shiba Inu (y4wdQpg767) | Quaternius | CC0 1.0 |
| Zwiebelringe | Onion rings (SvFCxPx2kf) | Kay Lousberg | CC0 1.0 |
| Senf-Turbo | Bottle Musterd (itJkZdvRFS) | Kenney | CC0 1.0 |
| Zwiebelmagnet | Magnet (dD2RIIea6WR) | Poly by Google | **CC-BY 3.0** |
| Pfeffer-Rakete | Minimal Cartoon Rocketship (dsjkFYy-rb0) | Gambsmoore | **CC-BY 3.0** |
| Sprungfedern | Spring (vKySckBbyb) | Quaternius | CC0 1.0 |
| Mett-Stern | Star (Tyau1aS2r0) | Quaternius | CC0 1.0 |

### Aus Packs
| Im Spiel | Pack | Autor | Lizenz |
|---|---|---|---|
| Mettbrötchen (`sandwich`), Brötchen-Schild (`bread`) | [Food Kit](https://kenney.nl/assets/food-kit) | Kenney | CC0 1.0 |

Gleise, Bordsteine, Rampen, Brücken und Balken sind einfache Boxen.

### Namensnennung (CC-BY 3.0)
Diese Modelle stehen unter [CC-BY 3.0](https://creativecommons.org/licenses/by/3.0/) und werden beim Weitergeben des Spiels genannt:
- "Van" by Poly by Google, https://poly.pizza/m/aT_24cDaW1a
- "Magnet" by Poly by Google, https://poly.pizza/m/dD2RIIea6WR
- "Minimal Cartoon Rocketship" by Gambsmoore, https://poly.pizza/m/dsjkFYy-rb0

## Schrift
**Archivo Black** von Omnibus-Type, SIL Open Font License 1.1 (`assets/fonts/OFL.txt`, im Paket unter `Lizenzen/`).

## Werkzeuge und Bibliotheken
Konvertierung mit Blender (headless). Rust-Bibliotheken: ash (Vulkan), winit, naga, glam, ab_glyph, rodio, gilrs, iroh (über mett-net) und weitere, siehe `Cargo.lock`.
