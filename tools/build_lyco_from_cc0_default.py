#!/usr/bin/env python3
"""Build LYCO (lyco-ip) YSM model derived from the official CC0 YSM 2.6.5 builtin/default template."""
import json
from pathlib import Path
import zipfile

import numpy as np
from PIL import Image

YSM_JAR = Path(
    "/home/lilyco/.local/share/PrismLauncher/instances/MC-AI-YSM-Litematica-1.21.1/.minecraft/mods/ysm-2.6.5-fabric+mc1.21.1-release.jar"
)
PRISM_OUT = Path(
    "/home/lilyco/.local/share/PrismLauncher/instances/MC-AI-YSM-Litematica-1.21.1/.minecraft/config/yes_steve_model/custom/lyco_ip.zip"
)
DESKTOP_OUT = Path("/home/lilyco/桌面/LYCO_YSM模型/lyco_ip_cc0_v2.zip")
PREFIX = "assets/yes_steve_model/builtin/default/"


def collect_bone_uv_masks(geo_data: dict, w: int, h: int) -> dict[str, np.ndarray]:
    """Rasterize cube UVs (both box-UV and per-face UV) into per-bone boolean masks."""
    geo = geo_data["minecraft:geometry"][0]
    tw = float(geo.get("description", {}).get("texture_width", w))
    th = float(geo.get("description", {}).get("texture_height", h))
    masks: dict[str, np.ndarray] = {}

    for bone in geo.get("bones", []):
        bname = bone["name"]
        m = np.zeros((h, w), dtype=bool)
        for cube in bone.get("cubes", []):
            raw_uv = cube.get("uv")
            sx, sy, sz = cube["size"]
            if isinstance(raw_uv, list):
                u, v = float(raw_uv[0]), float(raw_uv[1])
                rects = [
                    (u, v + sz, sz, sy),
                    (u + sz, v + sz, sx, sy),
                    (u + sz + sx, v + sz, sz, sy),
                    (u + sz + sx + sz, v + sz, sx, sy),
                    (u + sz, v, sx, sz),
                    (u + sz + sx, v, sx, sz),
                ]
            elif isinstance(raw_uv, dict):
                rects = []
                for fd in raw_uv.values():
                    u0, v0 = fd["uv"]
                    us, vs = fd["uv_size"]
                    rects.append((float(u0), float(v0), float(us), float(vs)))
            else:
                continue

            for rx, ry, rw, rh in rects:
                x0 = max(0, min(w - 1, int(min(rx, rx + rw) * w / tw)))
                x1 = max(0, min(w, int(max(rx, rx + rw) * w / tw) + 1))
                y0 = max(0, min(h - 1, int(min(ry, ry + rh) * h / th)))
                y1 = max(0, min(h, int(max(ry, ry + rh) * h / th) + 1))
                m[y0:y1, x0:x1] = True
        if m.any():
            masks[bname] = m
    return masks


def luminance(rgb: np.ndarray) -> np.ndarray:
    return 0.299 * rgb[..., 0] + 0.587 * rgb[..., 1] + 0.114 * rgb[..., 2]


def recolor_ramp(
    out: np.ndarray,
    mask: np.ndarray,
    lum: np.ndarray,
    dark: tuple[float, float, float],
    mid: tuple[float, float, float],
    light: tuple[float, float, float],
) -> None:
    if not mask.any():
        return
    l = lum[mask]
    lo, hi = float(np.percentile(l, 5)), float(np.percentile(l, 95))
    t = np.clip((l - lo) / max(hi - lo, 0.08), 0.0, 1.0)
    c = np.zeros((t.shape[0], 3), dtype=np.float32)
    low_m = t < 0.5
    hi_m = ~low_m
    t_lo = t[low_m] * 2.0
    t_hi = (t[hi_m] - 0.5) * 2.0
    for ch in range(3):
        c[low_m, ch] = dark[ch] * (1.0 - t_lo) + mid[ch] * t_lo
        c[hi_m, ch] = mid[ch] * (1.0 - t_hi) + light[ch] * t_hi
    out[mask] = c


def paint_lyco_texture(src_bytes: bytes, geo_data: dict, crimson_boost: bool = False) -> bytes:
    from io import BytesIO

    img = Image.open(BytesIO(src_bytes)).convert("RGBA")
    arr = np.asarray(img, dtype=np.float32)
    h, w = arr.shape[:2]
    rgb = arr[..., :3] / 255.0
    alpha = arr[..., 3]
    opaque = alpha > 10.0
    lum = luminance(rgb)

    # Skin detector to never overwrite pale warm skin on face/arms/legs
    mx = np.max(rgb, axis=-1)
    mn = np.min(rgb, axis=-1)
    sat = np.where(mx > 1e-5, (mx - mn) / np.maximum(mx, 1e-5), 0.0)
    is_skin = opaque & (rgb[..., 0] > rgb[..., 1]) & (rgb[..., 1] >= rgb[..., 2] * 0.92) & (sat <= 0.42) & (mx >= 0.65)

    masks = collect_bone_uv_masks(geo_data, w, h)
    out = rgb.copy()

    def union(names: list[str]) -> np.ndarray:
        m = np.zeros((h, w), dtype=bool)
        for n in names:
            if n in masks:
                m |= masks[n]
        return m & opaque

    # Strict 60-30-10 3-Color Palette (Clean, unified blocks — zero fragmented noise)
    # Color 1 (60%): Pure Obsidian Charcoal (#16151D -> #23212D -> #363344)
    # Color 2 (30%): Clean Ivory White (#E6E3DF -> #F4F2EE -> #FFFFFF) + Pale Skin
    # Color 3 (10%): Restrained Crimson Accent (#8C1A24 -> #B82530 -> #D43B46) ONLY on Ribbon & Collar

    # 1. All Hair (Unified pure obsidian — smooth high-contrast anime sheen, NO patchy splits)
    hair_all = union(["Hair", "front_h1", "corner_h1", "hl_side", "hl_m", "RightEyebrow", "LeftEyebrow"]) & (~is_skin)
    recolor_ramp(out, hair_all, lum, (0.08, 0.08, 0.11), (0.14, 0.13, 0.18), (0.26, 0.25, 0.32))
    if crimson_boost:
        # Variant 2: subtle crimson inner long-hair tips only (clean gradient, not noisy)
        inner_hair = union(["hl_m"]) & (~is_skin)
        recolor_ramp(out, inner_hair, lum, (0.30, 0.06, 0.09), (0.50, 0.10, 0.14), (0.68, 0.15, 0.19))

    # 2. Focal Accent ONLY: Ribbon + Collar -> Crisp Crimson (10% visual anchor)
    accent = union(["Ribbon", "Collar"]) & (~is_skin)
    recolor_ramp(out, accent, lum, (0.45, 0.08, 0.11), (0.70, 0.14, 0.19), (0.85, 0.23, 0.28))

    # 3. Eyes -> Clean Cool Obsidian-Grey (matches obsidian hair family, not a 4th hue)
    pupils = union(["RightPupil", "LeftPupil"])
    recolor_ramp(out, pupils, lum, (0.14, 0.14, 0.18), (0.48, 0.47, 0.54), (0.94, 0.94, 0.96))

    # 4. Outfit (Jacket, Sleeves, Skirt, Boots) -> Clean 2-tone Obsidian + Ivory White (preserves original crisp white trim/shirt!)
    outfit = union([
        "UpperBody", "Breast", "LeftArm", "RightArm", "LeftForeArm", "RightForeArm",
        "Skirt", "RightSkirt", "LeftSkirt", "FrontSkirt", "BackSkirt",
        "LeftFoot", "RightFoot",
    ]) & (~is_skin)
    # Keep originally bright white shirt/cuffs/lace as clean Ivory White; unify all colored cloth into Obsidian
    is_ivory = outfit & (lum >= 0.78) & (sat <= 0.18)
    is_obsidian_cloth = outfit & (~is_ivory)
    recolor_ramp(out, is_obsidian_cloth, lum, (0.07, 0.07, 0.10), (0.13, 0.13, 0.17), (0.23, 0.22, 0.29))
    recolor_ramp(out, is_ivory, lum, (0.84, 0.83, 0.82), (0.93, 0.92, 0.91), (0.99, 0.99, 0.99))

    # 5. Legs -> Clean Dark Obsidian-Grey tights (same hue family as obsidian)
    legs = union(["LeftLeg", "RightLeg", "LeftLowerLeg", "RightLowerLeg"]) & (~is_skin)
    recolor_ramp(out, legs, lum, (0.14, 0.14, 0.18), (0.22, 0.21, 0.26), (0.32, 0.31, 0.37))

    # Clean pale skin
    out[is_skin, 0] = np.clip(out[is_skin, 0] * 1.02, 0.0, 1.0)
    out[is_skin, 1] = np.clip(out[is_skin, 1] * 1.01, 0.0, 1.0)
    out[is_skin, 2] = np.clip(out[is_skin, 2] * 1.02, 0.0, 1.0)

    rgba = np.zeros_like(arr, dtype=np.uint8)
    rgba[..., :3] = np.clip(out * 255.0, 0, 255).astype(np.uint8)
    rgba[..., 3] = np.clip(alpha, 0, 255).astype(np.uint8)
    buf = BytesIO()
    Image.fromarray(rgba, mode="RGBA").save(buf, format="PNG")
    return buf.getvalue()


def main() -> None:
    with zipfile.ZipFile(YSM_JAR, "r") as zin:
        geo_data = json.loads(zin.read(PREFIX + "models/main.json").decode("utf-8"))
        default_tex = zin.read(PREFIX + "textures/default.png")
        ysm_meta = json.loads(zin.read(PREFIX + "ysm.json").decode("utf-8"))

        lyco_classic = paint_lyco_texture(default_tex, geo_data, crimson_boost=False)
        lyco_crimson = paint_lyco_texture(default_tex, geo_data, crimson_boost=True)

        # Update metadata for LYCO (CC0 derivative)
        ysm_meta["metadata"]["name"] = "LYCO 莉可 (lyco-ip)"
        ysm_meta["metadata"]["tips"] = "LYCO 莉可 · 基于 YSM 官方 CC0 64骨精雕骨架与完整动画包定制"
        ysm_meta["metadata"]["license"] = {"type": "CC 0"}

        zh_lang = {
            "metadata.name": "LYCO 莉可 (lyco-ip)",
            "metadata.tips": "LYCO 莉可 · 黑曜红衬 · 官方 CC0 全动画骨架",
            "texture.default": "黑曜绯红 (经典 LYCO)",
            "texture.blue": "绯红挑染 (红衬加强版)",
        }
        en_lang = {
            "metadata.name": "LYCO (lyco-ip)",
            "metadata.tips": "LYCO Obsidian & Crimson · Built on Official CC0 64-Bone YSM Rig",
            "texture.default": "Obsidian Crimson (Classic LYCO)",
            "texture.blue": "Crimson Inner-Layer Variant",
        }

        for out_zip in (PRISM_OUT, DESKTOP_OUT):
            out_zip.parent.mkdir(parents=True, exist_ok=True)
            with zipfile.ZipFile(out_zip, "w", compression=zipfile.ZIP_DEFLATED) as zout:
                for info in zin.infolist():
                    if not info.filename.startswith(PREFIX) or info.is_dir():
                        continue
                    rel = info.filename[len(PREFIX) :]
                    arc = f"lyco_ip/{rel}"
                    if rel == "textures/default.png":
                        zout.writestr(arc, lyco_classic)
                    elif rel == "textures/blue.png":
                        zout.writestr(arc, lyco_crimson)
                    elif rel == "ysm.json":
                        zout.writestr(arc, json.dumps(ysm_meta, indent=2, ensure_ascii=False).encode("utf-8"))
                    elif rel == "lang/zh_cn.json":
                        zout.writestr(arc, json.dumps(zh_lang, indent=2, ensure_ascii=False).encode("utf-8"))
                    elif rel == "lang/en_us.json":
                        zout.writestr(arc, json.dumps(en_lang, indent=2, ensure_ascii=False).encode("utf-8"))
                    else:
                        zout.writestr(arc, zin.read(info.filename))
            print(f"Built {out_zip} ({out_zip.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
