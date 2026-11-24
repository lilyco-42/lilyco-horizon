#!/usr/bin/env python3
"""Build high-poly sculpted LYCO (lyco-ip) YSM mesh + texture from high-star m_bf632c3a339fe3b9 base."""
from pathlib import Path
import shutil
import struct
import zipfile

import numpy as np
from PIL import Image


HORIZON_DIR = Path("/home/lilyco/.local/share/horizon")
LAUNCHER_VERSIONS = Path("/home/lilyco/.local/share/horizon-launcher/versions")
SRC_BIN = HORIZON_DIR / "player_models" / "baked_models" / "deepseek_chan.bin"
SRC_PNG = (
    HORIZON_DIR
    / "horizon-native-mods"
    / "ysm_models"
    / "assets"
    / "ysm_models"
    / "textures"
    / "entity"
    / "deepseek_chan.png"
)


def build_group_masks(bin_path: Path, width: int, height: int) -> np.ndarray:
    """Rasterize each quad's UV rectangle into a per-pixel group mask (bitmask of groups 0..5)."""
    data = bin_path.read_bytes()
    off = 0
    counts = []
    for _ in range(6):
        _, _, _, cnt = struct.unpack_from("<fffI", data, off)
        off += 16
        counts.append(cnt)

    mask = np.zeros((height, width), dtype=np.uint8)
    for gid, cnt in enumerate(counts):
        bit = 1 << gid
        for _ in range(cnt):
            vals = struct.unpack_from("<16f", data, off)
            off += 64
            u0, v0, u1, v1 = vals[12], vals[13], vals[14], vals[15]
            x0 = max(0, min(width - 1, int(min(u0, u1) * width)))
            x1 = max(0, min(width, int(max(u0, u1) * width) + 1))
            y0 = max(0, min(height - 1, int(min(v0, v1) * height)))
            y1 = max(0, min(height, int(max(v0, v1) * height) + 1))
            mask[y0:y1, x0:x1] |= bit
    return mask


def rgb_to_hsv_np(rgb: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    r, g, b = rgb[..., 0], rgb[..., 1], rgb[..., 2]
    mx = np.maximum(np.maximum(r, g), b)
    mn = np.minimum(np.minimum(r, g), b)
    df = mx - mn
    h = np.zeros_like(mx)
    nz = df > 1e-5
    r_m = nz & (mx == r)
    g_m = nz & (mx == g)
    b_m = nz & (mx == b)
    h[r_m] = (60.0 * (((g[r_m] - b[r_m]) / df[r_m]) % 6.0))
    h[g_m] = (60.0 * (((b[g_m] - r[g_m]) / df[g_m]) + 2.0))
    h[b_m] = (60.0 * (((r[b_m] - g[b_m]) / df[b_m]) + 4.0))
    s = np.where(mx > 1e-5, df / np.maximum(mx, 1e-5), 0.0)
    return h, s, mx


def reskin_texture(src_png: Path, bin_path: Path) -> Image.Image:
    img = Image.open(src_png).convert("RGBA")
    arr = np.asarray(img, dtype=np.float32)
    h_img, w_img = arr.shape[:2]
    rgb = arr[..., :3] / 255.0
    alpha = arr[..., 3]
    opaque = alpha > 10.0

    hue, sat, val = rgb_to_hsv_np(rgb)
    gmask = build_group_masks(bin_path, w_img, h_img)

    out = rgb.copy()
    is_skin = opaque & (hue <= 48.0) & (sat >= 0.06) & (sat <= 0.50) & (val >= 0.62)
    is_blue = opaque & (hue >= 175.0) & (hue <= 265.0) & (sat >= 0.14)
    is_neutral = opaque & (~is_skin) & (~is_blue)

    head_px = (gmask & (1 << 1)) != 0
    body_px = (gmask & ((1 << 0) | (1 << 2) | (1 << 3))) != 0
    leg_px = (gmask & ((1 << 4) | (1 << 5))) != 0

    # 1. Head / Hair: Obsidian-violet main hair + crimson inner-layer/highlights
    head_blue = is_blue & head_px
    # Bright vivid blue accents on hair -> LYCO crimson inner-layer (#8E1B24 -> #C82834)
    crimson_hair = head_blue & (sat >= 0.52) & (val >= 0.58)
    obsidian_hair = head_blue & (~crimson_hair)

    out[obsidian_hair, 0] = 0.08 + val[obsidian_hair] * 0.14
    out[obsidian_hair, 1] = 0.07 + val[obsidian_hair] * 0.12
    out[obsidian_hair, 2] = 0.11 + val[obsidian_hair] * 0.17

    out[crimson_hair, 0] = 0.48 + val[crimson_hair] * 0.36
    out[crimson_hair, 1] = 0.09 + val[crimson_hair] * 0.10
    out[crimson_hair, 2] = 0.13 + val[crimson_hair] * 0.12

    # 2. Body / Skirt / Arms: Tailored charcoal-black + crimson trim + silver highlights
    body_blue = is_blue & body_px & (~head_px)
    crimson_trim = body_blue & (sat >= 0.50) & (val >= 0.52)
    dark_cloth = body_blue & (~crimson_trim)

    out[dark_cloth, 0] = 0.07 + val[dark_cloth] * 0.13
    out[dark_cloth, 1] = 0.07 + val[dark_cloth] * 0.13
    out[dark_cloth, 2] = 0.10 + val[dark_cloth] * 0.16

    out[crimson_trim, 0] = 0.52 + val[crimson_trim] * 0.34
    out[crimson_trim, 1] = 0.10 + val[crimson_trim] * 0.09
    out[crimson_trim, 2] = 0.14 + val[crimson_trim] * 0.11

    # White/neutral cloth on torso/skirt -> dark tailored charcoal with silver edge contrast
    body_neutral = is_neutral & body_px & (~head_px)
    silver_hi = body_neutral & (val >= 0.92)
    charcoal_panel = body_neutral & (~silver_hi)
    out[charcoal_panel, 0] = 0.10 + val[charcoal_panel] * 0.14
    out[charcoal_panel, 1] = 0.10 + val[charcoal_panel] * 0.14
    out[charcoal_panel, 2] = 0.13 + val[charcoal_panel] * 0.17
    out[silver_hi, 0] = 0.76
    out[silver_hi, 1] = 0.78
    out[silver_hi, 2] = 0.82

    # 3. Legs / Tights / Boots: Translucent dark-grey tights + obsidian boots + crimson soles
    leg_blue = is_blue & leg_px & (~head_px)
    leg_neutral = is_neutral & leg_px & (~head_px)
    out[leg_blue, 0] = 0.48 + val[leg_blue] * 0.30
    out[leg_blue, 1] = 0.10 + val[leg_blue] * 0.08
    out[leg_blue, 2] = 0.14 + val[leg_blue] * 0.10

    out[leg_neutral, 0] = 0.20 + val[leg_neutral] * 0.14
    out[leg_neutral, 1] = 0.19 + val[leg_neutral] * 0.13
    out[leg_neutral, 2] = 0.24 + val[leg_neutral] * 0.16

    # 4. Any remaining unmasked blue pixels -> obsidian-violet
    rem_blue = is_blue & (~head_px) & (~body_px) & (~leg_px)
    out[rem_blue, 0] = 0.09 + val[rem_blue] * 0.14
    out[rem_blue, 1] = 0.08 + val[rem_blue] * 0.12
    out[rem_blue, 2] = 0.12 + val[rem_blue] * 0.16

    # Slightly brighten skin to match LYCO's pale warm complexion (#F3DCD1)
    out[is_skin, 0] = np.clip(out[is_skin, 0] * 1.03, 0.0, 1.0)
    out[is_skin, 1] = np.clip(out[is_skin, 1] * 1.01, 0.0, 1.0)
    out[is_skin, 2] = np.clip(out[is_skin, 2] * 1.02, 0.0, 1.0)

    rgba = np.zeros_like(arr, dtype=np.uint8)
    rgba[..., :3] = np.clip(out * 255.0, 0, 255).astype(np.uint8)
    rgba[..., 3] = np.clip(alpha, 0, 255).astype(np.uint8)
    return Image.fromarray(rgba, mode="RGBA")


def main() -> None:
    reskinned = reskin_texture(SRC_PNG, SRC_BIN)

    # 1. Install sculpted high-poly mesh (3,938 quads) as lyco_ip.bin
    for dst_bin in (
        HORIZON_DIR / "player_models" / "baked_models" / "lyco_ip.bin",
        HORIZON_DIR / "horizon-native-mods" / "ysm_models" / "baked_models" / "lyco_ip.bin",
    ):
        dst_bin.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(SRC_BIN, dst_bin)

    # 2. Install 256x256 LYCO palette texture into mod source & active native builds
    mod_tex = (
        HORIZON_DIR
        / "horizon-native-mods"
        / "ysm_models"
        / "assets"
        / "ysm_models"
        / "textures"
        / "entity"
        / "lyco_ip.png"
    )
    reskinned.save(mod_tex)

    for vdir in LAUNCHER_VERSIONS.glob("native-0a875da-*"):
        target = vdir / "horizon-native-mods" / "ysm_models" / "assets" / "ysm_models" / "textures" / "entity" / "lyco_ip.png"
        if target.parent.exists():
            reskinned.save(target)

    # 3. Repackage ysm_models-2.6.7.hmod so the updated texture is persisted
    d = HORIZON_DIR / "horizon-native-mods" / "ysm_models"
    out_hmod = HORIZON_DIR / "horizon-mods" / "ysm_models-2.6.7.hmod"
    allowed = {"horizon-mod.toml", "icon.png", "Cargo.toml", "Cargo.lock", "src", "assets"}
    with zipfile.ZipFile(out_hmod, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for p in sorted(d.rglob("*")):
            if p.is_file() and p.relative_to(d).parts[0] in allowed:
                zf.write(p, arcname=p.relative_to(d).as_posix())

    print(f"Upgraded lyco_ip to high-star sculpted mesh ({SRC_BIN.stat().st_size} bytes, 3938 quads) + LYCO palette texture ({mod_tex.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
