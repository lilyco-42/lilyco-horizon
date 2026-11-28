use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

const HOT_DIR: &str = "horizon-mods/.horizon-mod-data/ysm_models/hot";
const HOT_LIST: &str = "horizon-mods/.horizon-mod-data/ysm_models/hot_list.json";
const MOD_IMAGE_SPRITE: &str = "ysm_models:modimage";
const MOD_IMAGE_SIZE: u32 = 1024;

#[derive(Clone)]
pub struct HotModel {
    pub slug: String,
    pub title: String,
    pub bin: Vec<u8>,
    pub tex: Vec<u8>,
    pub tex_w: u32,
    pub tex_h: u32,
}

fn http_download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let out = ProcessCommand::new("curl")
        .args(["-sSL", "--max-time", "25", "-w", "\n%{http_code}", url])
        .output()
        .map_err(|e| format!("curl 失败: {e}"))?;
    let raw = out.stdout;
    let split = raw.iter().rposition(|&b| b == b'\n').ok_or("无 HTTP 码")?;
    let code: u16 = String::from_utf8_lossy(&raw[split + 1..]).trim().parse().unwrap_or(0);
    if code != 200 {
        return Err(format!("HTTP {code}"));
    }
    Ok(raw[..split].to_vec())
}

fn decode_png_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| format!("PNG 头错误: {e}"))?;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("无法计算贴图缓冲大小")?];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("PNG 解码错误: {e}"))?;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for px in buf[..info.buffer_size()].chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        other => return Err(format!("不支持的贴图格式 {:?}，需 RGBA/RGB", other)),
    };
    Ok((info.width, info.height, rgba))
}

pub fn ensure_dir() {
    let _ = fs::create_dir_all(HOT_DIR);
}

pub type CatalogItem = (String, String, String, String); // slug, title, bin_url, tex_url

pub fn load_catalog() -> Vec<CatalogItem> {
    let mut out = Vec::new();
    if let Ok(raw) = fs::read_to_string(HOT_LIST) {
        if let Ok(v) = serde_json::from_str::<Vec<CatalogItem>>(&raw) {
            out = v;
        }
    }
    out
}

pub fn save_catalog(items: &[CatalogItem]) {
    if let Ok(s) = serde_json::to_string(items) {
        let _ = fs::write(HOT_LIST, s);
    }
}

pub fn fetch_manifest(active: &str, token: &str) -> Result<Vec<CatalogItem>, String> {
    let url = format!("{active}/api/v1/manifest.json?kind=ysm&token={token}");
    let out = ProcessCommand::new("curl")
        .args(["-sSL", "--max-time", "15", &url])
        .output()
        .map_err(|e| format!("curl 失败: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("manifest JSON 错误: {e}"))?;
    let mut items = Vec::new();
    if let Some(arr) = value.get("items").and_then(|v| v.as_array()) {
        for it in arr {
            let slug = it.get("slug").and_then(|v| v.as_str()).unwrap_or("");
            let title = it.get("title").and_then(|v| v.as_str()).unwrap_or(slug);
            let bin_url = it.get("download_url").and_then(|v| v.as_str()).unwrap_or("");
            let tex_url = it.get("texture_url").and_then(|v| v.as_str()).unwrap_or("");
            if !slug.is_empty() {
                items.push((slug.to_string(), title.to_string(), bin_url.to_string(), tex_url.to_string()));
            }
        }
    }
    Ok(items)
}

pub fn load_hot(slug: &str) -> Option<HotModel> {
    let bin_path = PathBuf::from(HOT_DIR).join(format!("{slug}.bin"));
    let tex_path = PathBuf::from(HOT_DIR).join(format!("{slug}.png"));
    let bin = fs::read(&bin_path).ok()?;
    let tex = fs::read(&tex_path).ok()?;
    let (w, h, _) = decode_png_rgba(&tex).ok()?;
    if bin.len() < 96 {
        return None;
    }
    let title = load_catalog()
        .into_iter()
        .find(|(s, _, _, _)| s == slug)
        .map(|(_, t, _, _)| t)
        .unwrap_or_else(|| slug.to_string());
    Some(HotModel { slug: slug.to_string(), title, bin, tex, tex_w: w, tex_h: h })
}

pub fn download(active: &str, token: &str, slug: &str) -> Result<HotModel, String> {
    ensure_dir();
    let entry = load_catalog().into_iter().find(|(s, _, _, _)| s == slug);
    let dl = entry.as_ref().and_then(|(_, _, b, _)| if b.is_empty() { None } else { Some(b.clone()) })
        .unwrap_or_else(|| format!("{active}/api/v1/assets/ysm/{slug}/download?token={token}"));
    let tex_url = entry.as_ref().and_then(|(_, _, _, t)| if t.is_empty() { None } else { Some(t.clone()) })
        .unwrap_or_else(|| format!("{active}/api/v1/assets/ysm/{slug}/texture"));
    let bin = http_download_bytes(&dl)?;
    if bin.len() < 96 {
        return Err(format!("模型数据异常 ({} 字节)", bin.len()));
    }
    let tex = http_download_bytes(&tex_url)?;
    let (w, h, _) = decode_png_rgba(&tex)?;
    if w > MOD_IMAGE_SIZE || h > MOD_IMAGE_SIZE {
        return Err(format!("贴图 {w}x{h} 超过 {MOD_IMAGE_SIZE} 上限"));
    }
    fs::write(PathBuf::from(HOT_DIR).join(format!("{slug}.bin")), &bin).map_err(|e| e.to_string())?;
    fs::write(PathBuf::from(HOT_DIR).join(format!("{slug}.png")), &tex).map_err(|e| e.to_string())?;
    let title = load_catalog()
        .into_iter()
        .find(|(s, _, _, _)| s == slug)
        .map(|(_, t, _, _)| t)
        .unwrap_or_else(|| slug.to_string());
    Ok(HotModel { slug: slug.to_string(), title, bin, tex, tex_w: w, tex_h: h })
}

pub fn rgba_for_upload(model: &HotModel) -> Result<(u32, u32, Vec<u8>), String> {
    let (w, h, rgba) = decode_png_rgba(&model.tex)?;
    if w == MOD_IMAGE_SIZE && h == MOD_IMAGE_SIZE {
        return Ok((w, h, rgba));
    }
    // Fill the whole 1024x1024 mod_image slot (nearest-neighbour).
    // draw.quad maps the model's 0..16 UV to the entire sprite rect, so the
    // texture must cover the full slot, not just its top-left corner.
    let size = MOD_IMAGE_SIZE as usize;
    let mut out = vec![0u8; size * size * 4];
    for y in 0..size {
        let sy = (y * h as usize / size).min(h as usize - 1);
        for x in 0..size {
            let sx = (x * w as usize / size).min(w as usize - 1);
            let src = (sy * w as usize + sx) * 4;
            let dst = (y * size + x) * 4;
            out[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
        }
    }
    Ok((MOD_IMAGE_SIZE, MOD_IMAGE_SIZE, out))
}

pub fn sprite_name() -> &'static str {
    MOD_IMAGE_SPRITE
}
