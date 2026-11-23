"""Konvertiert die Requisiten (Fahrzeuge, Absperrungen, Haeuser, Sammelsachen) in das einfache
HACKEPETER-Meshformat (.k0m).

Aufruf (headless, beruehrt keine offene Blender-Sitzung):
    blender -b --factory-startup -P tools/prepare_assets.py -- <hackepeter-Ordner>
    (HP_ONLY=tram,dog fuer einzelne Jobs)

"Grob an unseren Stil anpassen" heisst hier:
  * Polygone reduzieren (Decimate), flache Normalen (Low-Poly-Look)
  * Texturen weg, stattdessen eine Durchschnittsfarbe pro Material
  * Boden bei y = 0, Mitte auf x/z = 0, laengste Seite entlang z (Laufrichtung)
  * Groesse: `height` (einheitlich), `size` (laengste Kante) oder `fit` = (Breite, Hoehe, Laenge)
    exakt (nicht einheitlich skaliert, damit Kollision und Modell genau passen)

Format .k0m (little endian):
    b"K0M1", u32 vertex_count, f32[3] bounds_min, f32[3] bounds_max,
    vertex_count * (f32[3] pos, f32[3] normal, f32[3] color)   -- nicht indiziert, Dreiecke

Quellen und Lizenzen: siehe CREDITS.md (Dateinamen in assets_src/downloads).
"""

import bpy
import math
import os
import struct
import sys

from mathutils import Matrix, Vector

ROOT = sys.argv[sys.argv.index("--") + 1]
DL = os.path.join(ROOT, "assets_src", "downloads")
OUT = os.path.join(ROOT, "assets", "props")
os.makedirs(OUT, exist_ok=True)


def src(name):
    return os.path.join(DL, name + ".glb")


# Reihenfolge = engine::assets::MeshId (ab Index 1, Index 0 ist der Einheitswuerfel)
JOBS = [
    # Zuege (Quaternius Train Pack, CC0): Personenwagen, Lok, Schnellzug-Kopf, Containerwagen
    dict(name="train_carriage", src=src("train_carriage"), fit=(2.3, 3.3, 13.0), long_z=True, tris=3000),
    dict(name="train_front", src=src("train_front"), fit=(2.3, 3.3, 13.0), long_z=True, tris=3000),
    dict(name="train_highspeed", src=src("train_highspeed"), fit=(2.3, 3.3, 13.0), long_z=True, tris=3000),
    dict(name="train_container", src=src("train_container"), fit=(2.3, 3.3, 13.0), long_z=True, tris=2500),
    dict(name="truck", src=src("truck"), fit=(2.2, 2.7, 5.6), long_z=True, tris=2500),
    dict(name="van", src=src("van"), fit=(2.1, 2.2, 4.8), long_z=True, tris=2000),
    dict(name="police_car", src=src("police_car"), fit=(2.0, 1.6, 4.4), long_z=True, tris=2000,
         colors={}),
    dict(name="traffic_barrier", src=src("traffic_barrier"), fit=(2.2, 0.9, 0.45), long_z=False, tris=800),
    dict(name="barrier_large", src=src("barrier_large"), fit=(2.3, 1.0, 0.7), long_z=False, tris=1200),
    dict(name="construction_barrier", src=src("construction_barrier"), fit=(2.2, 1.0, 0.45), long_z=False, tris=600),
    dict(name="cone", src=src("cone"), height=0.7, tris=300),
    dict(name="street_light", src=src("street_light"), height=5.5, tris=500),
    dict(name="hydrant", src=src("hydrant"), height=0.8, tris=500),
    dict(name="bench", src=src("bench"), size=1.8, tris=600),
    dict(name="trashcan", src=src("trashcan"), height=1.0, tris=400),
    dict(name="building_a", src=src("building_a"), height=14.0, tris=2500),
    dict(name="building_b", src=src("building_b"), height=11.0, tris=2500),
    dict(name="town_house", src=src("town_house"), height=9.0, tris=3000),
    dict(name="market_stand", src=src("market_stand"), height=3.0, tris=1500),
    dict(name="traffic_light", src=src("traffic_light"), height=4.0, tris=600),
    # Der "Dackel": Hund (Quaternius, CC0) in die Laenge gezogen
    dict(name="dog", src=src("dog"), fit=(0.32, 0.5, 1.15), long_z=True, tris=1200),
    dict(name="onion_rings", src=src("onion_rings"), size=0.55, tris=600),
    dict(name="mustard", src=src("mustard"), height=0.7, tris=400),
    dict(name="magnet", src=src("magnet"), size=0.6, tris=600),
    # Mettbroetchen (Kenney Food Kit "sandwich") und Broetchen-Schild ("bread")
    dict(name="sandwich", src=src("sandwich"), size=0.5, tris=600),
    dict(name="bread", src=src("bread"), size=0.6, tris=600),
    # Neue Power-ups: Rakete (Gambsmoore, CC-BY 3.0), Feder (Quaternius, CC0), Stern (Quaternius, CC0)
    dict(name="rocket", src=src("rocket"), height=1.0, tris=900),
    dict(name="spring", src=src("spring"), height=0.55, tris=600),
    dict(name="star", src=src("star"), size=0.7, tris=300),
]
def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def import_file(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    return [o for o in bpy.data.objects if o not in before]


def average_image_color(img):
    try:
        n = len(img.pixels)
        if n == 0:
            return None
        px = [0.0] * n
        img.pixels.foreach_get(px)
        step = max(4, (n // 4 // 4096) * 4)  # hoechstens ~4096 Stichproben
        r = g = b = 0.0
        c = 0
        for i in range(0, n - 3, step):
            r += px[i]
            g += px[i + 1]
            b += px[i + 2]
            c += 1
        return (r / c, g / c, b / c) if c else None
    except Exception as e:  # noqa: BLE001
        print("  Bildfehler", img.name, e)
        return None


def material_color(mat, overrides):
    if mat is None:
        return (0.7, 0.7, 0.7)
    if mat.name in overrides:
        return overrides[mat.name]
    col = (0.7, 0.7, 0.7)
    if mat.use_nodes:
        bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
        if bsdf is not None:
            inp = bsdf.inputs[0]  # Base Color
            col = tuple(inp.default_value[:3])
            if inp.is_linked:
                node = inp.links[0].from_node
                # durch Mix/Multiply-Knoten zum Bild laufen
                seen = 0
                while node is not None and node.type != "TEX_IMAGE" and seen < 6:
                    nxt = None
                    for i in node.inputs:
                        if i.is_linked:
                            nxt = i.links[0].from_node
                            break
                    node = nxt
                    seen += 1
                if node is not None and node.type == "TEX_IMAGE" and node.image is not None:
                    avg = average_image_color(node.image)
                    if avg:
                        col = avg
    # linear -> sRGB-artig (unser Renderer arbeitet in Anzeigewerten)
    return tuple(max(0.0, min(1.0, c)) ** (1 / 2.2) for c in col)


def apply_pose(objs, job):
    action_name = job.get("action")
    if not action_name:
        return
    if "pose_src" in job:
        import_file(job["pose_src"])  # bringt die Actions mit
        # die Hilfs-Objekte der Library nicht exportieren
    action = bpy.data.actions.get(action_name)
    arm = next((o for o in objs if o.type == "ARMATURE"), None)
    if action is None or arm is None:
        print("  WARNUNG: Pose nicht moeglich", action_name, arm)
        return
    if arm.animation_data is None:
        arm.animation_data_create()
    arm.animation_data.action = action
    # Blender 4.4+/5.x: Action-Slots
    if hasattr(arm.animation_data, "action_slot") and getattr(action, "slots", None):
        try:
            arm.animation_data.action_slot = action.slots[0]
        except Exception as e:  # noqa: BLE001
            print("  Slot-Hinweis:", e)
    bpy.context.scene.frame_set(job.get("frame", 1))


def collect(objs, job):
    depsgraph = bpy.context.evaluated_depsgraph_get()
    wanted = job.get("mesh")
    meshes = [o for o in objs if o.type == "MESH" and (wanted is None or o.name == wanted)
              and not o.name.startswith("Icosphere")]
    if not meshes:
        raise RuntimeError(f"keine Meshes fuer {job['name']}")
    total = sum(len(o.data.polygons) * 2 for o in meshes)
    ratio = min(1.0, job.get("tris", 100000) / max(1, total))
    for o in meshes:
        if ratio < 0.98:
            m = o.modifiers.new("dec", "DECIMATE")
            m.ratio = ratio
    depsgraph = bpy.context.evaluated_depsgraph_get()
    depsgraph.update()

    # Blender (Z oben, -Y vorne) -> Engine (Y oben, +Z vorne)
    conv = Matrix(((1, 0, 0, 0), (0, 0, 1, 0), (0, -1, 0, 0), (0, 0, 0, 1)))
    verts = []
    for o in meshes:
        ev = o.evaluated_get(depsgraph)
        me = ev.to_mesh()
        me.calc_loop_triangles()
        mw = conv @ o.matrix_world
        nm = mw.to_3x3().inverted_safe().transposed()
        cols = [material_color(m, job.get("colors", {})) for m in me.materials] or [(0.7, 0.7, 0.7)]
        for tri in me.loop_triangles:
            c = cols[min(tri.material_index, len(cols) - 1)]
            n = (nm @ tri.normal).normalized()
            for vi in tri.vertices:
                p = mw @ me.vertices[vi].co
                verts.append((p.x, p.y, p.z, n.x, n.y, n.z, c[0], c[1], c[2]))
        ev.to_mesh_clear()
    return verts


def normalize(verts, job):
    xs = [v[0] for v in verts]
    zs = [v[2] for v in verts]
    # laengste horizontale Seite auf z drehen (Fahrzeuge fahren entlang der Strecke)
    if "long_z" in job:
        wide = (max(xs) - min(xs)) > (max(zs) - min(zs))
        if wide == job["long_z"]:
            verts = [(-v[2], v[1], v[0], -v[5], v[4], v[3], v[6], v[7], v[8]) for v in verts]
    xs = [v[0] for v in verts]
    ys = [v[1] for v in verts]
    zs = [v[2] for v in verts]
    mn = Vector((min(xs), min(ys), min(zs)))
    mx = Vector((max(xs), max(ys), max(zs)))
    dim = mx - mn
    if "fit" in job:
        w, h, l = job["fit"]
        s = Vector((w / max(dim.x, 1e-6), h / max(dim.y, 1e-6), l / max(dim.z, 1e-6)))
    elif "height" in job:
        s = Vector([job["height"] / dim.y] * 3)
    else:
        s = Vector([job["size"] / max(dim)] * 3)
    cx, cz = (mn.x + mx.x) / 2, (mn.z + mx.z) / 2
    yaw = math.radians(job.get("yaw", 0.0))
    cy, sy = math.cos(yaw), math.sin(yaw)
    out = []
    for v in verts:
        x, y, z = (v[0] - cx) * s.x, (v[1] - mn.y) * s.y, (v[2] - cz) * s.z
        # Normalen bei ungleicher Skalierung mit dem Kehrwert skalieren
        nx, ny, nz = v[3] / s.x, v[4] / s.y, v[5] / s.z
        ln = math.sqrt(nx * nx + ny * ny + nz * nz) or 1.0
        nx, ny, nz = nx / ln, ny / ln, nz / ln
        x, z = cy * x + sy * z, -sy * x + cy * z
        nx, nz = cy * nx + sy * nz, -sy * nx + cy * nz
        out.append((x, y, z, nx, ny, nz, v[6], v[7], v[8]))
    return out


def write(path, verts):
    mn = [min(v[i] for v in verts) for i in range(3)]
    mx = [max(v[i] for v in verts) for i in range(3)]
    with open(path, "wb") as f:
        f.write(b"K0M1")
        f.write(struct.pack("<I", len(verts)))
        f.write(struct.pack("<6f", *mn, *mx))
        for v in verts:
            f.write(struct.pack("<9f", *v))
    print(f"  -> {os.path.basename(path)}: {len(verts) // 3} Dreiecke, "
          f"Groesse {mx[0]-mn[0]:.2f} x {mx[1]-mn[1]:.2f} x {mx[2]-mn[2]:.2f} m")


ONLY = os.environ.get("HP_ONLY")
for job in JOBS:
    if ONLY and job["name"] not in ONLY.split(","):
        continue
    print("==", job["name"])
    reset()
    objs = import_file(job["src"])
    apply_pose(objs, job)
    verts = normalize(collect(objs, job), job)
    write(os.path.join(OUT, job["name"] + ".k0m"), verts)

print("FERTIG")
