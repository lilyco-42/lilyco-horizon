#!/usr/bin/env python3
"""Bake a YSM .zip (main.json + texture.png) into Horizon ysm_models baked .bin + entity .png."""
import json
import math
from pathlib import Path
import struct
import sys
import zipfile


def rot_xyz(pt: list[float], pivot: list[float], rot_deg: list[float]) -> list[float]:
    x, y, z = pt[0] - pivot[0], pt[1] - pivot[1], pt[2] - pivot[2]
    # Bedrock / Blockbench rotation convention: -X, -Y, +Z
    rx, ry, rz = -math.radians(rot_deg[0]), -math.radians(rot_deg[1]), math.radians(rot_deg[2])
    if rx:
        cy, sy = math.cos(rx), math.sin(rx)
        y, z = y * cy - z * sy, y * sy + z * cy
    if ry:
        cy, sy = math.cos(ry), math.sin(ry)
        x, z = x * cy + z * sy, -x * sy + z * cy
    if rz:
        cy, sy = math.cos(rz), math.sin(rz)
        x, y = x * cy - y * sy, x * sy + y * cy
    return [x + pivot[0], y + pivot[1], z + pivot[2]]


def classify_group(bone_name: str, ancestors: list[str]) -> int:
    chain = [bone_name.lower()] + [a.lower() for a in ancestors]
    for name in chain:
        if "head" in name or "hair" in name or "ear" in name or "hat" in name or "face" in name:
            return 1
        if "rightarm" in name or "armright" in name or "r_arm" in name or "arm_r" in name:
            return 2
        if "leftarm" in name or "armleft" in name or "l_arm" in name or "arm_l" in name:
            return 3
        if "rightleg" in name or "legright" in name or "r_leg" in name or "leg_r" in name:
            return 4
        if "leftleg" in name or "legleft" in name or "l_leg" in name or "leg_l" in name:
            return 5
    return 0


def box_uv_faces(u: float, v: float, sx: float, sy: float, sz: float, tw: float, th: float) -> dict[str, tuple[float, float, float, float]]:
    return {
        "north": ((u + sz) / tw, (v + sz) / th, (u + sz + sx) / tw, (v + sz + sy) / th),
        "south": ((u + sz + sx + sz) / tw, (v + sz) / th, (u + sz + sx + sz + sx) / tw, (v + sz + sy) / th),
        "west": (u / tw, (v + sz) / th, (u + sz) / tw, (v + sz + sy) / th),
        "east": ((u + sz + sx) / tw, (v + sz) / th, (u + sz + sx + sz) / tw, (v + sz + sy) / th),
        "up": ((u + sz) / tw, v / th, (u + sz + sx) / tw, (v + sz) / th),
        "down": ((u + sz + sx) / tw, (v + sz) / th, (u + sz + sx + sx) / tw, v / th),
    }


def bake_zip(zip_path: Path, slug: str, horizon_dir: Path) -> None:
    with zipfile.ZipFile(zip_path, "r") as zf:
        names = zf.namelist()
        main_name = next((n for n in names if n.endswith("main.json")), None)
        tex_name = next((n for n in names if n.endswith(".png") and "icon" not in n.lower()), None)
        if not main_name or not tex_name:
            raise SystemExit(f"Missing main.json or texture .png in {zip_path}")
        geo_data = json.loads(zf.read(main_name).decode("utf-8"))
        tex_bytes = zf.read(tex_name)

    geo = geo_data["minecraft:geometry"][0]
    desc = geo.get("description", {})
    tw = float(desc.get("texture_width", 64))
    th = float(desc.get("texture_height", 64))
    bones = geo.get("bones", [])
    by_name = {b["name"]: b for b in bones}

    def get_ancestors(name: str) -> list[str]:
        res = []
        cur = by_name.get(name, {}).get("parent")
        while cur and cur in by_name and len(res) < 16:
            res.append(cur)
            cur = by_name[cur].get("parent")
        return res

    def transform_chain(pt: list[float], name: str) -> list[float]:
        chain = [name] + get_ancestors(name)
        p = list(pt)
        for bname in chain:
            b = by_name[bname]
            rot = b.get("rotation")
            if rot and any(rot):
                p = rot_xyz(p, b.get("pivot", [0.0, 0.0, 0.0]), rot)
        return p

    pivots = [[0.0, 0.0, 0.0] for _ in range(6)]
    for b in bones:
        n = b["name"].lower()
        raw_pv = b.get("pivot", [0.0, 0.0, 0.0])
        # 180 deg Y-rotation to match Horizon forward (+Z) & RightArm (+X)
        pv = [-raw_pv[0] / 16.0, raw_pv[1] / 16.0, -raw_pv[2] / 16.0]
        if n == "head":
            pivots[1] = pv
        elif n == "rightarm":
            pivots[2] = pv
        elif n == "leftarm":
            pivots[3] = pv
        elif n == "rightleg":
            pivots[4] = pv
        elif n == "leftleg":
            pivots[5] = pv

    groups: list[list[tuple[list[list[float]], tuple[float, float, float, float]]]] = [[] for _ in range(6)]
    for b in bones:
        bname = b["name"]
        gid = classify_group(bname, get_ancestors(bname))
        for cube in b.get("cubes", []):
            ox, oy, oz = cube["origin"]
            sx, sy, sz = cube["size"]
            inf = float(cube.get("inflate", 0.0))
            x0, y0, z0 = ox - inf, oy - inf, oz - inf
            x1, y1, z1 = ox + sx + inf, oy + sy + inf, oz + sz + inf

            raw_uv = cube.get("uv", [0, 0])
            if isinstance(raw_uv, list):
                fuv = box_uv_faces(float(raw_uv[0]), float(raw_uv[1]), sx, sy, sz, tw, th)
            else:
                fuv = {}
                for fname, fd in raw_uv.items():
                    u0, v0 = fd["uv"]
                    us, vs = fd["uv_size"]
                    fuv[fname] = (u0 / tw, v0 / th, (u0 + us) / tw, (v0 + vs) / th)

            faces = {
                "south": [[x0, y1, z1], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1]],
                "north": [[x1, y1, z0], [x1, y0, z0], [x0, y0, z0], [x0, y1, z0]],
                "west": [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]],
                "east": [[x1, y1, z1], [x1, y0, z1], [x1, y0, z0], [x1, y1, z0]],
                "up": [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
                "down": [[x1, y0, z0], [x1, y0, z1], [x0, y0, z1], [x0, y0, z0]],
            }
            c_rot = cube.get("rotation")
            c_piv = cube.get("pivot", [0.0, 0.0, 0.0])
            for fname, verts in faces.items():
                uv = fuv.get(fname)
                if not uv:
                    continue
                out_v = []
                for v in verts:
                    p = rot_xyz(v, c_piv, c_rot) if (c_rot and any(c_rot)) else list(v)
                    p = transform_chain(p, bname)
                    # 180 deg Y-rotation: Bedrock faces -Z, Horizon player faces +Z
                    out_v.append([-p[0] / 16.0, p[1] / 16.0, -p[2] / 16.0])
                groups[gid].append((out_v, uv))

    # Anatomical proportion calibration for overly-thin/tall models (e.g., lyco_ip)
    if slug == "lyco_ip":
        # Re-anchor pivots to natural 1.86m anime-human proportions
        sy_all = 0.87
        old_pivs = [list(p) for p in pivots]
        pivots[1] = [0.0, old_pivs[1][1] * sy_all, 0.0]
        pivots[2] = [old_pivs[2][0] * 1.18, old_pivs[2][1] * sy_all, 0.0]
        pivots[3] = [old_pivs[3][0] * 1.18, old_pivs[3][1] * sy_all, 0.0]
        pivots[4] = [old_pivs[4][0] * 1.22, old_pivs[4][1] * sy_all, 0.0]
        pivots[5] = [old_pivs[5][0] * 1.22, old_pivs[5][1] * sy_all, 0.0]
        # (sx, sy, sz) around each group's own joint pivot
        g_scales = [
            (1.24, sy_all, 1.22),  # g0: Body & Skirt (fuller torso/skirt)
            (0.84, 0.82, 0.84),    # g1: Head & Hair (balanced head ratio)
            (1.38, 0.82, 1.42),    # g2: RightArm (natural sleeve/arm thickness)
            (1.38, 0.82, 1.42),    # g3: LeftArm
            (1.32, sy_all, 1.24),  # g4: RightLeg (natural leg thickness)
            (1.32, sy_all, 1.24),  # g5: LeftLeg
        ]
        for i in range(6):
            op = old_pivs[i]
            np_ = pivots[i]
            sx, sy, sz = g_scales[i]
            calibrated = []
            for verts, uv in groups[i]:
                nv = [
                    [
                        np_[0] + (v[0] - op[0]) * sx,
                        np_[1] + (v[1] - op[1]) * sy,
                        np_[2] + (v[2] - op[2]) * sz,
                    ]
                    for v in verts
                ]
                calibrated.append((nv, uv))
            groups[i] = calibrated

    buf = bytearray()
    for i in range(6):
        buf.extend(struct.pack("<fffI", pivots[i][0], pivots[i][1], pivots[i][2], len(groups[i])))
    for i in range(6):
        for verts, uv in groups[i]:
            for v in verts:
                buf.extend(struct.pack("<fff", v[0], v[1], v[2]))
            buf.extend(struct.pack("<ffff", uv[0], uv[1], uv[2], uv[3]))

    bin_targets = [
        horizon_dir / "horizon-native-mods" / "ysm_models" / "baked_models" / f"{slug}.bin",
        horizon_dir / "player_models" / "baked_models" / f"{slug}.bin",
    ]
    for dst in bin_targets:
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_bytes(buf)

    tex_dst = (
        horizon_dir
        / "horizon-native-mods"
        / "ysm_models"
        / "assets"
        / "ysm_models"
        / "textures"
        / "entity"
        / f"{slug}.png"
    )
    tex_dst.parent.mkdir(parents=True, exist_ok=True)
    tex_dst.write_bytes(tex_bytes)

    total_quads = sum(len(g) for g in groups)
    print(f"Baked {slug}: {total_quads} quads ({len(buf)} bytes) + texture ({len(tex_bytes)} bytes)")


if __name__ == "__main__":
    zip_arg = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/home/lilyco/桌面/LYCO_YSM模型/lyco_ip.zip")
    slug_arg = sys.argv[2] if len(sys.argv) > 2 else "lyco_ip"
    bake_zip(zip_arg, slug_arg, Path("/home/lilyco/.local/share/horizon"))
