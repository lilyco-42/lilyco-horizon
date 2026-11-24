use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{CoreError, CoreResult};
use crate::models::{
    ModEntry, SchematicEntry, SchematicRenderOpts, SchematicSwitchRequest, SchematicTargetInfo,
    SchematicsOverview, YsmActiveState, YsmModelEntry, YsmOverview, YsmSwitchRequest,
};

#[derive(Deserialize)]
struct RawActiveYsm {
    active_persona: Option<String>,
    title: Option<String>,
}

#[derive(Deserialize)]
struct RawActiveTarget {
    file: Option<String>,
    name: Option<String>,
    category: Option<String>,
    total_blocks: Option<u64>,
}

pub struct HorizonStore {
    horizon_dir: PathBuf,
    extra_ysm_dir: PathBuf,
}

impl HorizonStore {
    pub fn new(horizon_dir: PathBuf, extra_ysm_dir: PathBuf) -> Self {
        Self {
            horizon_dir,
            extra_ysm_dir,
        }
    }

    pub fn horizon_dir(&self) -> &Path {
        &self.horizon_dir
    }

    pub fn load_ysm_overview(&self) -> CoreResult<YsmOverview> {
        let models = scan_ysm_dir(&self.horizon_dir.join("player_models"), "horizon")?;
        let desktop_models = scan_ysm_dir(&self.extra_ysm_dir, "desktop")?;
        let active = self.read_ysm_active();
        Ok(YsmOverview {
            active,
            models,
            desktop_models,
        })
    }

    pub fn switch_ysm(&self, req: &YsmSwitchRequest) -> CoreResult<YsmActiveState> {
        let data_dir = self.mod_data_dir("ysm_models");
        ensure_dir(&data_dir)?;
        write_text(&data_dir.join("persona"), &req.persona_index.to_string())?;
        write_text(&data_dir.join("gesture"), &req.gesture_index.to_string())?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| (d.as_millis() % 1_000_000_000) as u32)
            .unwrap_or(1)
            .max(1);
        let switch_line = format!("{} {} {}\n", req.persona_index, req.gesture_index, nonce);
        write_text(&data_dir.join("web_switch.txt"), &switch_line)?;
        if let Some(title) = &req.title {
            let state_path = data_dir.join("active_ysm_state.json");
            let payload = format!(
                "{{\n  \"active_persona\": \"preset_{}\",\n  \"title\": \"{}\",\n  \"multiplayer_sync\": true\n}}\n",
                req.persona_index,
                escape_json(title)
            );
            write_text(&state_path, &payload)?;
        }
        Ok(self.read_ysm_active())
    }

    pub fn import_desktop_ysm(&self, file_name: &str) -> CoreResult<YsmModelEntry> {
        validate_file_name(file_name)?;
        let src = self.extra_ysm_dir.join(file_name);
        if !src.is_file() {
            return Err(CoreError::Validation(format!("model not found: {file_name}")));
        }
        let target_dir = self.horizon_dir.join("player_models");
        ensure_dir(&target_dir)?;
        let target_name = if file_name.ends_with(".zip") {
            file_name.trim_end_matches(".zip").to_string() + ".ysm"
        } else {
            file_name.to_string()
        };
        let dst = target_dir.join(&target_name);
        fs::copy(&src, &dst).map_err(|source| CoreError::Io {
            path: dst.display().to_string(),
            source,
        })?;
        let meta = fs::metadata(&dst).map_err(|source| CoreError::Io {
            path: dst.display().to_string(),
            source,
        })?;
        Ok(YsmModelEntry {
            persona_index: resolve_persona_index(&target_name),
            display_name: strip_ext(&target_name),
            file_name: target_name,
            size_bytes: meta.len(),
        })
    }

    pub fn load_schematics_overview(&self) -> CoreResult<SchematicsOverview> {
        let schem_dir = self.horizon_dir.join("schematics");
        let target = read_active_target(&schem_dir.join("active_target.json"));
        let data_dir = self.mod_data_dir("schematics");
        let active_index = read_u32(&data_dir.join("blueprint"), 0);
        let render_opts = SchematicRenderOpts {
            layer_mode: read_u32(&data_dir.join("layer_mode"), 0),
            mirror: read_u32(&data_dir.join("mirror"), 0) != 0,
            ghost_render: read_u32(&data_dir.join("ghost_render"), 1) != 0,
        };
        let items = scan_schematics(&schem_dir, &target.file)?;
        Ok(SchematicsOverview {
            active_index,
            target,
            render_opts,
            items,
        })
    }

    pub fn switch_schematic(&self, req: &SchematicSwitchRequest) -> CoreResult<SchematicsOverview> {
        validate_file_name(&req.file_name)?;
        let data_dir = self.mod_data_dir("schematics");
        ensure_dir(&data_dir)?;
        write_text(&data_dir.join("blueprint"), &req.index.to_string())?;
        write_text(
            &data_dir.join("layer_mode"),
            &req.render_opts.layer_mode.to_string(),
        )?;
        write_text(
            &data_dir.join("mirror"),
            if req.render_opts.mirror { "1" } else { "0" },
        )?;
        write_text(
            &data_dir.join("ghost_render"),
            if req.render_opts.ghost_render { "1" } else { "0" },
        )?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| (d.as_millis() % 1_000_000_000) as u32)
            .unwrap_or(1)
            .max(1);
        let switch_line = format!(
            "{} {} {} {} {}\n",
            req.index,
            req.render_opts.layer_mode,
            if req.render_opts.mirror { 1 } else { 0 },
            if req.render_opts.ghost_render { 1 } else { 0 },
            nonce
        );
        write_text(&data_dir.join("web_switch.txt"), &switch_line)?;

        let target_path = self.horizon_dir.join("schematics").join("active_target.json");
        let layer_label = match req.render_opts.layer_mode {
            1 => "Single Layer (单层切片)",
            2 => "All Below (当前层及以下)",
            _ => "All Layers (完整3D投影)",
        };
        let payload = format!(
            "{{\n  \"file\": \"{}\",\n  \"name\": \"{}\",\n  \"category\": \"Lilyco Horizon Studio\",\n  \"total_blocks\": 0,\n  \"layer_mode\": \"{}\",\n  \"active_layer_y\": 0\n}}\n",
            escape_json(&req.file_name),
            escape_json(&strip_ext(&req.file_name)),
            layer_label
        );
        write_text(&target_path, &payload)?;
        self.load_schematics_overview()
    }

    pub fn list_mods(&self) -> CoreResult<Vec<ModEntry>> {
        let toggles = read_mod_toggles(&self.horizon_dir.join("horizon-mods.txt"));
        let mods_dir = self.horizon_dir.join("horizon-mods");
        let mut out = Vec::new();
        if let Ok(entries) = fs::read_dir(&mods_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("hmod") {
                    continue;
                }
                let file_name = entry.file_name().to_string_lossy().to_string();
                let id = extract_mod_id(&file_name);
                let enabled = toggles.get(&id).copied().unwrap_or(true);
                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                out.push(ModEntry {
                    id,
                    file_name,
                    enabled,
                    size_bytes,
                });
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    pub fn set_mod_enabled(&self, id: &str, enabled: bool) -> CoreResult<Vec<ModEntry>> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(CoreError::Validation(format!("invalid mod id: {id}")));
        }
        let path = self.horizon_dir.join("horizon-mods.txt");
        let mut toggles = read_mod_toggles(&path);
        toggles.insert(id.to_string(), enabled);
        let mut lines = Vec::new();
        for (k, v) in &toggles {
            lines.push(format!("mod.{k}:{v}"));
        }
        write_text(&path, &(lines.join("\n") + "\n"))?;
        self.list_mods()
    }

    fn mod_data_dir(&self, mod_id: &str) -> PathBuf {
        self.horizon_dir
            .join("horizon-mods")
            .join(".horizon-mod-data")
            .join(mod_id)
    }

    fn read_ysm_active(&self) -> YsmActiveState {
        let data_dir = self.mod_data_dir("ysm_models");
        let persona_index = read_u32(&data_dir.join("persona"), 0);
        let gesture_index = read_u32(&data_dir.join("gesture"), 0);
        let raw = fs::read_to_string(data_dir.join("active_ysm_state.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<RawActiveYsm>(&s).ok());
        YsmActiveState {
            persona_index,
            gesture_index,
            active_persona: raw
                .as_ref()
                .and_then(|r| r.active_persona.clone())
                .unwrap_or_else(|| format!("preset_{persona_index}")),
            title: raw
                .and_then(|r| r.title)
                .unwrap_or_else(|| "Unselected".to_string()),
        }
    }
}

fn scan_ysm_dir(dir: &Path, _source: &str) -> CoreResult<Vec<YsmModelEntry>> {
    let mut items = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(items);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext != "ysm" && ext != "zip" {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        items.push(YsmModelEntry {
            persona_index: resolve_persona_index(&file_name),
            display_name: strip_ext(&file_name),
            file_name,
            size_bytes,
        });
    }
    items.sort_by(|a, b| a.persona_index.cmp(&b.persona_index).then(a.file_name.cmp(&b.file_name)));
    Ok(items)
}

fn resolve_persona_index(file_name: &str) -> u32 {
    let lower = file_name.to_lowercase();
    if lower.contains("橘雪莉") || lower.contains("ojousama") {
        0
    } else if lower.contains("蝴蝶") || lower.contains("butterfly") {
        1
    } else if lower.contains("鲸鱼") || lower.contains("whale") {
        2
    } else if lower.contains("deepseek") {
        3
    } else if lower.contains("凯尔希") || lower.contains("kaltsit") {
        4
    } else if lower.contains("龙娘") || lower.contains("dragon") {
        5
    } else if lower.contains("柴郡") || lower.contains("cheshire") {
        6
    } else if lower.contains("猫宫") || lower.contains("nekomiya") {
        7
    } else if lower.contains("lyco") {
        8
    } else {
        8
    }
}

fn scan_schematics(dir: &Path, active_file: &str) -> CoreResult<Vec<SchematicEntry>> {
    let mut files: Vec<(String, u64)> = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("litematic") {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        files.push((file_name, size_bytes));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files
        .into_iter()
        .enumerate()
        .map(|(idx, (file_name, size_bytes))| SchematicEntry {
            index: idx as u32,
            is_active: file_name == active_file,
            file_name,
            size_bytes,
        })
        .collect())
}

fn read_active_target(path: &Path) -> SchematicTargetInfo {
    let raw = fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<RawActiveTarget>(&s).ok());
    SchematicTargetInfo {
        file: raw.as_ref().and_then(|r| r.file.clone()).unwrap_or_default(),
        name: raw
            .as_ref()
            .and_then(|r| r.name.clone())
            .unwrap_or_else(|| "None".to_string()),
        category: raw
            .as_ref()
            .and_then(|r| r.category.clone())
            .unwrap_or_else(|| "General".to_string()),
        total_blocks: raw.and_then(|r| r.total_blocks).unwrap_or(0),
    }
}

fn read_mod_toggles(path: &Path) -> BTreeMap<String, bool> {
    let mut map = BTreeMap::new();
    let Ok(raw) = fs::read_to_string(path) else {
        return map;
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("mod.") {
            if let Some((id, val)) = rest.split_once(':') {
                map.insert(id.trim().to_string(), val.trim() == "true");
            }
        }
    }
    map
}

fn extract_mod_id(file_name: &str) -> String {
    let stem = file_name.trim_end_matches(".hmod");
    match stem.split_once('-') {
        Some((id, _)) => id.to_string(),
        None => stem.to_string(),
    }
}

fn read_u32(path: &Path, default: u32) -> u32 {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

fn ensure_dir(path: &Path) -> CoreResult<()> {
    fs::create_dir_all(path).map_err(|source| CoreError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn write_text(path: &Path, content: &str) -> CoreResult<()> {
    fs::write(path, content).map_err(|source| CoreError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn validate_file_name(name: &str) -> CoreResult<()> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(CoreError::Validation(format!("unsafe file name: {name}")));
    }
    Ok(())
}

fn strip_ext(name: &str) -> String {
    name.rsplit_once('.').map(|(a, _)| a).unwrap_or(name).to_string()
}

fn escape_json(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}
