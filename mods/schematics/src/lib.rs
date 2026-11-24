mod blueprint_data;

use blueprint_data::{
    PlacedBlock, BP_1_BLOCKS, BP_2_BLOCKS, BP_3_BLOCKS, BP_4_BLOCKS, BP_5_BLOCKS, BP_6_BLOCKS,
    BP_7_BLOCKS, BP_8_BLOCKS,
};
use mod_api::{
    self, ClientHost, ClientTickPostEvent, Command, CommandEvent, Corner, Flow, Hooks, Identifier,
    InputKeyEvent, KeyAction, KeyConflictContext, KeyRegistration, LightCoords,
    ModelBakingCompletedEvent, ModelInstance, ModelRef, Overlay, RegisterClientCommandsEvent,
    RegisterCommandsEvent, Registrar, RegistrationContext, RenderGuiPostEvent, RenderLevelStage,
    RenderLevelStageEvent, RunModCommandEvent, ServerHost, SettingChangedEvent, SettingSpec,
    StandaloneModelsEvent, Text, WorldDepth, WorldLine, WorldQuad,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

const DEFAULT_ASSET_SOURCE: &str = "https://dev.lain42.top/mc-assets";

struct RemoteSourceConfig {
    active: String,
    token: String,
    sources: Vec<String>,
}

fn source_cfg_path() -> PathBuf {
    let dir = PathBuf::from("horizon-mods/.horizon-mod-data");
    let _ = fs::create_dir_all(&dir);
    dir.join("asset_sources.cfg")
}

fn load_source_cfg() -> RemoteSourceConfig {
    let mut cfg = RemoteSourceConfig {
        active: DEFAULT_ASSET_SOURCE.to_string(),
        token: String::new(),
        sources: vec![DEFAULT_ASSET_SOURCE.to_string()],
    };
    if let Ok(raw) = fs::read_to_string(source_cfg_path()) {
        for line in raw.lines() {
            if let Some(v) = line.strip_prefix("active=") {
                if !v.trim().is_empty() {
                    cfg.active = v.trim().trim_end_matches('/').to_string();
                }
            } else if let Some(v) = line.strip_prefix("token=") {
                cfg.token = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("source=") {
                let u = v.trim().trim_end_matches('/').to_string();
                if !u.is_empty() && !cfg.sources.contains(&u) {
                    cfg.sources.push(u);
                }
            }
        }
    }
    if !cfg.sources.contains(&cfg.active) {
        cfg.sources.push(cfg.active.clone());
    }
    cfg
}

fn save_source_cfg(cfg: &RemoteSourceConfig) {
    let mut out = format!("active={}\ntoken={}\n", cfg.active, cfg.token);
    for s in &cfg.sources {
        out.push_str(&format!("source={s}\n"));
    }
    let _ = fs::write(source_cfg_path(), out);
}

fn http_get_text(url: &str) -> Result<String, String> {
    let out = ProcessCommand::new("curl")
        .args(["-sSL", "--max-time", "10", url])
        .output()
        .map_err(|e| format!("无法调用 curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("网络请求失败 (exit {:?})", out.status.code()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn http_download_file(url: &str, dest: &PathBuf) -> Result<u16, String> {
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let dest_s = dest.to_string_lossy().to_string();
    let out = ProcessCommand::new("curl")
        .args(["-sSL", "--max-time", "20", "-w", "%{http_code}", "-o", &dest_s, url])
        .output()
        .map_err(|e| format!("无法调用 curl: {e}"))?;
    let code_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let code = code_str.parse::<u16>().unwrap_or(0);
    if code != 200 {
        let err_body = fs::read_to_string(dest).unwrap_or_default();
        let _ = fs::remove_file(dest);
        return Err(format!("HTTP {code}: {}", err_body.chars().take(160).collect::<String>()));
    }
    Ok(code)
}

const ID: &str = "schematics";
const KEY_MENU: &str = "menu";
const KEY_NEXT: &str = "next";
const KEY_LAYER_UP: &str = "layer_up";
const KEY_LAYER_DOWN: &str = "layer_down";

struct BlueprintInfo {
    file: &'static str,
    name: &'static str,
    short_name: &'static str,
    category: &'static str,
    size: [i32; 3],
    total_blocks: u32,
    materials: &'static str,
    blocks: &'static [PlacedBlock],
}

const BLUEPRINTS: [BlueprintInfo; 8] = [
    BlueprintInfo {
        file: "01_redstone_clock_lamp.litematic",
        name: "01 红石高频时钟灯 (Redstone Clock & Lamp)",
        short_name: "01 红石高频时钟灯",
        category: "红石设备 (Redstone)",
        size: [5, 2, 5],
        total_blocks: 35,
        materials: "cobblestone:25, repeater:4, redstone_wire:4, redstone_lamp:1, lever:1",
        blocks: BP_1_BLOCKS,
    },
    BlueprintInfo {
        file: "02_piston_door_2x2.litematic",
        name: "02 2x2 自动粘性活塞门 (2x2 Sticky Piston Door)",
        short_name: "02 2x2自动粘性活塞门",
        category: "红石设备 (Redstone)",
        size: [6, 4, 5],
        total_blocks: 64,
        materials: "stone_bricks:30, oak_planks:16, sticky_piston:4, glass:4, pressure_plate:4, torch:2, dust:2",
        blocks: BP_2_BLOCKS,
    },
    BlueprintInfo {
        file: "03_auto_sugarcane_farm.litematic",
        name: "03 侦测器全自动甘蔗机 (Observer Auto Sugarcane Farm)",
        short_name: "03 侦测器自动甘蔗机",
        category: "红石设备 (Redstone)",
        size: [6, 5, 4],
        total_blocks: 88,
        materials: "cobblestone:56, glass:12, hopper:4, water:4, sand:4, piston:4, observer:4, chest:1",
        blocks: BP_3_BLOCKS,
    },
    BlueprintInfo {
        file: "04_super_smelter_4x.litematic",
        name: "04 四联漏斗全自动超级熔炉 (4x Hopper Super Smelter)",
        short_name: "04 四联自动超级熔炉",
        category: "红石设备 (Redstone)",
        size: [6, 4, 3],
        total_blocks: 37,
        materials: "cobblestone:18, hopper:12, furnace:4, chest:3",
        blocks: BP_4_BLOCKS,
    },
    BlueprintInfo {
        file: "05_auto_item_sorter.litematic",
        name: "05 四路全自动物品分类机 (4-Slice Auto Item Sorter)",
        short_name: "05 四路自动物品分类机",
        category: "红石设备 (Redstone)",
        size: [4, 5, 5],
        total_blocks: 76,
        materials: "cobblestone:32, hopper:16, chest:8, redstone_wire:8, torch:4, repeater:4, comparator:4",
        blocks: BP_5_BLOCKS,
    },
    BlueprintInfo {
        file: "06_chinese_pavilion.litematic",
        name: "06 中式飞檐八角凉亭 (Traditional Chinese Flying-Eaves Pavilion)",
        short_name: "06 中式飞檐八角凉亭",
        category: "文化建筑 (Cultural Architecture)",
        size: [9, 8, 9],
        total_blocks: 156,
        materials: "dark_oak_planks:74, stone_bricks:49, red_terracotta:16, oak_slab:6, lantern:5, gold_block:1",
        blocks: BP_6_BLOCKS,
    },
    BlueprintInfo {
        file: "07_cultural_grand_arch.litematic",
        name: "07 东方传统雕梁牌坊 (Grand Cultural Archway Gate)",
        short_name: "07 东方传统雕梁牌坊",
        category: "文化建筑 (Cultural Architecture)",
        size: [11, 8, 5],
        total_blocks: 123,
        materials: "polished_andesite:55, dark_oak_slab:33, red_terracotta:16, dark_oak_log:9, gold_block:7, lantern:3",
        blocks: BP_7_BLOCKS,
    },
    BlueprintInfo {
        file: "08_cultural_library_clocktower.litematic",
        name: "08 文化藏书阁与红石钟楼 (Cultural Library & Redstone Bell Clocktower)",
        short_name: "08 文化藏书阁与红石钟楼",
        category: "文化建筑 (Cultural Architecture)",
        size: [7, 10, 7],
        total_blocks: 194,
        materials: "stone_bricks:91, spruce_planks:25, spruce_slab:25, spruce_log:20, glass:8, bookshelf:6, lectern:1",
        blocks: BP_8_BLOCKS,
    },
];

const LAYER_MODES: [&str; 3] = [
    "All Layers (完整3D投影)",
    "Single Layer (单层切片)",
    "All Below (当前层及以下)",
];
const ROTATIONS: [&str; 4] = ["0° (North)", "90° (East)", "180° (South)", "270° (West)"];
const MIRRORS: [&str; 3] = ["None", "Left-Right (X)", "Front-Back (Z)"];
const GHOST_MODES: [&str; 2] = ["Enabled (3D Hologram + HUD)", "Hidden"];
const BLOCK_CONNECT_MODES: [&str; 2] = [
    "Gap (默认留空间距 0.84x)",
    "Connected (无缝紧密连接 1.0x)",
];

static BP_IDX: AtomicU32 = AtomicU32::new(0);
static LAYER_MODE: AtomicU32 = AtomicU32::new(0);
static ACTIVE_Y: AtomicI32 = AtomicI32::new(0);
static ROT_IDX: AtomicU32 = AtomicU32::new(0);
static MIRROR_IDX: AtomicU32 = AtomicU32::new(0);
static GHOST_ENABLED: AtomicBool = AtomicBool::new(true);
static BLOCK_CONNECTED: AtomicBool = AtomicBool::new(false);
static ANCHOR_SET: AtomicBool = AtomicBool::new(false);
static ANCHOR_X: AtomicI32 = AtomicI32::new(6);
static ANCHOR_Y: AtomicI32 = AtomicI32::new(64);
static ANCHOR_Z: AtomicI32 = AtomicI32::new(6);
static CORRECT_BLOCKS: AtomicU32 = AtomicU32::new(0);
static WRONG_BLOCKS: AtomicU32 = AtomicU32::new(0);

static BLOCK_MODEL_CACHE: OnceLock<Mutex<HashMap<&'static str, ModelRef>>> = OnceLock::new();

fn block_model_cache() -> &'static Mutex<HashMap<&'static str, ModelRef>> {
    BLOCK_MODEL_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Map schematic block IDs to their vanilla Minecraft block model IDs for real textured 3D ghost rendering.
const BLOCK_MODEL_MAP: [(&str, &str); 35] = [
    ("minecraft:bookshelf", "minecraft:block/bookshelf"),
    ("minecraft:chest", "minecraft:block/oak_planks"),
    ("minecraft:chiseled_stone_bricks", "minecraft:block/chiseled_stone_bricks"),
    ("minecraft:cobblestone", "minecraft:block/cobblestone"),
    ("minecraft:comparator", "minecraft:block/comparator"),
    ("minecraft:dark_oak_log", "minecraft:block/dark_oak_log"),
    ("minecraft:dark_oak_planks", "minecraft:block/dark_oak_planks"),
    ("minecraft:dark_oak_slab", "minecraft:block/dark_oak_slab"),
    ("minecraft:furnace", "minecraft:block/furnace"),
    ("minecraft:glass", "minecraft:block/glass"),
    ("minecraft:gold_block", "minecraft:block/gold_block"),
    ("minecraft:hopper", "minecraft:block/hopper"),
    ("minecraft:lantern", "minecraft:block/lantern"),
    ("minecraft:lectern", "minecraft:block/lectern"),
    ("minecraft:lever", "minecraft:block/lever"),
    ("minecraft:oak_fence", "minecraft:block/oak_fence_post"),
    ("minecraft:oak_planks", "minecraft:block/oak_planks"),
    ("minecraft:oak_slab", "minecraft:block/oak_slab"),
    ("minecraft:observer", "minecraft:block/observer"),
    ("minecraft:piston", "minecraft:block/piston"),
    ("minecraft:polished_andesite", "minecraft:block/polished_andesite"),
    ("minecraft:redstone_block", "minecraft:block/redstone_block"),
    ("minecraft:redstone_lamp", "minecraft:block/redstone_lamp"),
    ("minecraft:redstone_torch", "minecraft:block/redstone_torch"),
    ("minecraft:redstone_wire", "minecraft:block/redstone_dust_dot"),
    ("minecraft:red_terracotta", "minecraft:block/red_terracotta"),
    ("minecraft:repeater", "minecraft:block/repeater_1tick"),
    ("minecraft:sand", "minecraft:block/sand"),
    ("minecraft:spruce_fence", "minecraft:block/spruce_fence_post"),
    ("minecraft:spruce_log", "minecraft:block/spruce_log"),
    ("minecraft:spruce_planks", "minecraft:block/spruce_planks"),
    ("minecraft:spruce_slab", "minecraft:block/spruce_slab"),
    ("minecraft:sticky_piston", "minecraft:block/sticky_piston"),
    ("minecraft:stone_bricks", "minecraft:block/stone_bricks"),
    ("minecraft:stone_pressure_plate", "minecraft:block/stone_pressure_plate"),
];

fn is_ignorable_air(host: &dyn ClientHost, ws: mod_api::BlockState) -> bool {
    match host.block_state_string(ws) {
        Some(s) => {
            s.starts_with("minecraft:air")
                || s.starts_with("minecraft:cave_air")
                || s.starts_with("minecraft:void_air")
                || s.starts_with("minecraft:short_grass")
                || s.starts_with("minecraft:tall_grass")
                || s.starts_with("minecraft:grass")
                || s.starts_with("minecraft:fern")
        }
        None => true,
    }
}

fn push_block_wireframe(lines: &mut Vec<WorldLine>, pos: [i32; 3], color: u32, width: f32) {
    let x0 = pos[0] as f64 + 0.01;
    let y0 = pos[1] as f64 + 0.01;
    let z0 = pos[2] as f64 + 0.01;
    let x1 = pos[0] as f64 + 0.99;
    let y1 = pos[1] as f64 + 0.99;
    let z1 = pos[2] as f64 + 0.99;
    let edges = [
        ([x0, y0, z0], [x1, y0, z0]),
        ([x1, y0, z0], [x1, y0, z1]),
        ([x1, y0, z1], [x0, y0, z1]),
        ([x0, y0, z1], [x0, y0, z0]),
        ([x0, y1, z0], [x1, y1, z0]),
        ([x1, y1, z0], [x1, y1, z1]),
        ([x1, y1, z1], [x0, y1, z1]),
        ([x0, y1, z1], [x0, y1, z0]),
        ([x0, y0, z0], [x0, y1, z0]),
        ([x1, y0, z0], [x1, y1, z0]),
        ([x1, y0, z1], [x1, y1, z1]),
        ([x0, y0, z1], [x0, y1, z1]),
    ];
    for (a, b) in edges {
        lines.push(WorldLine::new(a, b, color, width, WorldDepth::Tested));
    }
}

fn write_active_target(bp_idx: usize, l_mode: usize, y_layer: i32, rot: usize, origin: [i32; 3]) {
    let bp = &BLUEPRINTS[bp_idx % BLUEPRINTS.len()];
    let dir = PathBuf::from("schematics");
    let _ = fs::create_dir_all(&dir);
    let payload = format!(
        "{{\n  \"file\": \"{}\",\n  \"name\": \"{}\",\n  \"category\": \"{}\",\n  \"dimensions\": [{}, {}, {}],\n  \"total_blocks\": {},\n  \"correct_blocks\": {},\n  \"wrong_blocks\": {},\n  \"materials\": \"{}\",\n  \"layer_mode\": \"{}\",\n  \"active_layer_y\": {},\n  \"rotation\": \"{}\",\n  \"origin\": [{}, {}, {}]\n}}\n",
        bp.file,
        bp.name,
        bp.category,
        bp.size[0],
        bp.size[1],
        bp.size[2],
        bp.total_blocks,
        CORRECT_BLOCKS.load(Ordering::Relaxed),
        WRONG_BLOCKS.load(Ordering::Relaxed),
        bp.materials,
        LAYER_MODES[l_mode % LAYER_MODES.len()],
        y_layer,
        ROTATIONS[rot % ROTATIONS.len()],
        origin[0],
        origin[1],
        origin[2]
    );
    let _ = fs::write(dir.join("active_target.json"), payload);
}

fn transform_offset(
    dx: i32,
    dy: i32,
    dz: i32,
    size: [i32; 3],
    rot: usize,
    mirror: usize,
) -> [i32; 3] {
    let mut x = dx;
    let mut z = dz;
    if mirror == 1 {
        x = size[0] - 1 - x;
    } else if mirror == 2 {
        z = size[2] - 1 - z;
    }
    match rot % 4 {
        1 => [-z, dy, x],
        2 => [-x, dy, -z],
        3 => [z, dy, -x],
        _ => [x, dy, z],
    }
}

fn block_palette_color(block: &str) -> u32 {
    if block.contains("redstone") || block.contains("repeater") || block.contains("comparator") {
        0xFF2B4A
    } else if block.contains("piston") || block.contains("observer") || block.contains("hopper") {
        0x38BDF8
    } else if block.contains("gold") || block.contains("lantern") || block.contains("lamp") {
        0xFACC15
    } else if block.contains("terracotta") {
        0xEF4444
    } else if block.contains("oak") || block.contains("spruce") || block.contains("bookshelf") || block.contains("chest") || block.contains("lectern") {
        0xD97706
    } else if block.contains("glass") || block.contains("water") {
        0x60A5FA
    } else {
        0x94A3B8
    }
}

fn current_anchor(host: &dyn ClientHost) -> [i32; 3] {
    let player_pos = host
        .local_player()
        .and_then(|p| p.entity().position(host))
        .unwrap_or([0.0, 64.0, 0.0]);
    let px = [
        player_pos[0].floor() as i32 + 3,
        player_pos[1].floor() as i32,
        player_pos[2].floor() as i32 + 3,
    ];
    if !ANCHOR_SET.swap(true, Ordering::Relaxed) {
        ANCHOR_X.store(px[0], Ordering::Relaxed);
        ANCHOR_Y.store(px[1], Ordering::Relaxed);
        ANCHOR_Z.store(px[2], Ordering::Relaxed);
    }
    [
        ANCHOR_X.load(Ordering::Relaxed),
        ANCHOR_Y.load(Ordering::Relaxed),
        ANCHOR_Z.load(Ordering::Relaxed),
    ]
}

fn show_checklist_and_reanchor(host: &mut dyn ClientHost) {
    let reads: &dyn ClientHost = &*host;
    if let Some(pos) = reads.local_player().and_then(|p| p.entity().position(reads)) {
        ANCHOR_X.store(pos[0].floor() as i32 + 3, Ordering::Relaxed);
        ANCHOR_Y.store(pos[1].floor() as i32, Ordering::Relaxed);
        ANCHOR_Z.store(pos[2].floor() as i32 + 3, Ordering::Relaxed);
        ANCHOR_SET.store(true, Ordering::Relaxed);
    }
    let idx = BP_IDX.load(Ordering::Relaxed) as usize % BLUEPRINTS.len();
    let bp = &BLUEPRINTS[idx];
    let lm = LAYER_MODE.load(Ordering::Relaxed) as usize % LAYER_MODES.len();
    let y = ACTIVE_Y.load(Ordering::Relaxed);
    let rot = ROT_IDX.load(Ordering::Relaxed) as usize % ROTATIONS.len();
    let origin = [
        ANCHOR_X.load(Ordering::Relaxed),
        ANCHOR_Y.load(Ordering::Relaxed),
        ANCHOR_Z.load(Ordering::Relaxed),
    ];
    let done = CORRECT_BLOCKS.load(Ordering::Relaxed);
    let wrong = WRONG_BLOCKS.load(Ordering::Relaxed);
    write_active_target(idx, lm, y, rot, origin);
    let _ = host.chat_line(Text::literal(format!(
        "§b[Litematica 3D 投影]§r 当前蓝图 ({}/{}): §e{}§r [{}] | 尺寸: {}x{}x{} (共 {} 块, 已完成 {}/{}, 错误 {})",
        idx + 1,
        BLUEPRINTS.len(),
        bp.name,
        bp.category,
        bp.size[0],
        bp.size[1],
        bp.size[2],
        bp.total_blocks,
        done,
        bp.total_blocks,
        wrong
    )));
    let _ = host.chat_line(Text::literal(format!(
        "§b[Litematica 材料清单]§r {} | 切片: {} (Y={}) | 旋转: {} | 投影锚点: ({}, {}, {})",
        bp.materials,
        LAYER_MODES[lm],
        y,
        ROTATIONS[rot],
        origin[0],
        origin[1],
        origin[2]
    )));
}

fn cycle_next_blueprint(host: &mut dyn ClientHost) {
    let next = (BP_IDX.load(Ordering::Relaxed) as usize + 1) % BLUEPRINTS.len();
    BP_IDX.store(next as u32, Ordering::Relaxed);
    ACTIVE_Y.store(0, Ordering::Relaxed);
    host.set_setting(ID, "blueprint", &next.to_string());
    let bp = &BLUEPRINTS[next];
    let lm = LAYER_MODE.load(Ordering::Relaxed) as usize;
    let rot = ROT_IDX.load(Ordering::Relaxed) as usize;
    let origin = current_anchor(&*host);
    write_active_target(next, lm, 0, rot, origin);
    let _ = host.chat_line(Text::literal(format!(
        "§d[Litematica]§r 已切换至蓝图 ({}/{}): §e{}§r ({}) -> 所需材料: [{}]",
        next + 1,
        BLUEPRINTS.len(),
        bp.name,
        bp.file,
        bp.materials
    )));
}

fn step_layer_up(host: &mut dyn ClientHost) {
    let idx = BP_IDX.load(Ordering::Relaxed) as usize % BLUEPRINTS.len();
    let max_y = BLUEPRINTS[idx].size[1] - 1;
    let cur = ACTIVE_Y.load(Ordering::Relaxed);
    let next_y = if cur >= max_y { 0 } else { cur + 1 };
    ACTIVE_Y.store(next_y, Ordering::Relaxed);
    if LAYER_MODE.load(Ordering::Relaxed) == 0 {
        LAYER_MODE.store(1, Ordering::Relaxed);
        host.set_setting(ID, "layer_mode", "1");
    }
    let lm = LAYER_MODE.load(Ordering::Relaxed) as usize;
    let rot = ROT_IDX.load(Ordering::Relaxed) as usize;
    let origin = current_anchor(&*host);
    write_active_target(idx, lm, next_y, rot, origin);
    let _ = host.chat_line(Text::literal(format!(
        "§b[Litematica 切片]§r {} -> 当前显示层 Y = {} / {} ({})",
        BLUEPRINTS[idx].short_name,
        next_y,
        max_y,
        LAYER_MODES[lm % LAYER_MODES.len()]
    )));
}

fn step_layer_down(host: &mut dyn ClientHost) {
    let idx = BP_IDX.load(Ordering::Relaxed) as usize % BLUEPRINTS.len();
    let max_y = BLUEPRINTS[idx].size[1] - 1;
    let cur = ACTIVE_Y.load(Ordering::Relaxed);
    let next_y = if cur <= 0 { max_y } else { cur - 1 };
    ACTIVE_Y.store(next_y, Ordering::Relaxed);
    let lm = (LAYER_MODE.load(Ordering::Relaxed) as usize + 1) % LAYER_MODES.len();
    LAYER_MODE.store(lm as u32, Ordering::Relaxed);
    host.set_setting(ID, "layer_mode", &lm.to_string());
    let rot = ROT_IDX.load(Ordering::Relaxed) as usize;
    let origin = current_anchor(&*host);
    write_active_target(idx, lm, next_y, rot, origin);
    let _ = host.chat_line(Text::literal(format!(
        "§b[Litematica 切片]§r {} -> 切换模式: {} | 层 Y = {} / {}",
        BLUEPRINTS[idx].short_name,
        LAYER_MODES[lm],
        next_y,
        max_y
    )));
}

#[derive(Default)]
struct SchematicsMod;

#[mod_api::horizon_mod]
impl Hooks for SchematicsMod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let bp_opts = BLUEPRINTS
            .iter()
            .map(|b| format!("{} [{} | {} blocks]", b.name, b.category, b.total_blocks))
            .collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "blueprint",
            "schematics.setting.blueprint",
            bp_opts,
            0,
        ));

        let lm_opts = LAYER_MODES.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "layer_mode",
            "schematics.setting.layer_mode",
            lm_opts,
            0,
        ));

        let rot_opts = ROTATIONS.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "rotation",
            "schematics.setting.rotation",
            rot_opts,
            0,
        ));

        let mir_opts = MIRRORS.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "mirror",
            "schematics.setting.mirror",
            mir_opts,
            0,
        ));

        let gh_opts = GHOST_MODES.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "ghost_render",
            "schematics.setting.ghost_render",
            gh_opts,
            0,
        ));

        let bc_opts = BLOCK_CONNECT_MODES
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "block_connect",
            "schematics.setting.block_connect",
            bc_opts,
            0,
        ));

        r.register_key_mapping(
            KeyRegistration::new(KEY_MENU, "key.keyboard.m")
                .label("key.schematics.menu")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(KEY_NEXT, "key.keyboard.n")
                .label("key.schematics.next")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(KEY_LAYER_UP, "key.keyboard.page.up")
                .label("key.schematics.layer_up")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(KEY_LAYER_DOWN, "key.keyboard.page.down")
                .label("key.schematics.layer_down")
                .context(KeyConflictContext::InGame),
        );

        write_active_target(0, 0, 0, 0, [6, 64, 6]);
    }

    fn register_client_commands(
        &self,
        _: &mut dyn ClientHost,
        event: &mut RegisterClientCommandsEvent<'_>,
    ) {
        event.commands.push(Command::new("schematics"));
        event.commands.push(Command::new("schematics_next"));
        event.commands.push(Command::new("schematics_here"));
        event.commands.push(Command::new("schematics_layer"));
        event.commands.push(Command::new("schematics_connect"));
        event.commands.push(Command::new("schematics_sources"));
        event.commands.push(Command::new("schematics_add_source"));
        event.commands.push(Command::new("schematics_token"));
        event.commands.push(Command::new("schematics_sync"));
        event.commands.push(Command::new("schematics_download"));
    }

    fn command(&self, host: &mut dyn ClientHost, event: &mut CommandEvent<'_>) -> Flow {
        let cmd = event.command.trim();
        if cmd == "schematics" || cmd == "schematics_here" {
            show_checklist_and_reanchor(host);
            return Flow::Cancel;
        } else if cmd == "schematics_next" {
            cycle_next_blueprint(host);
            return Flow::Cancel;
        } else if cmd == "schematics_layer" {
            step_layer_up(host);
            return Flow::Cancel;
        } else if cmd == "schematics_connect" {
            let next = !BLOCK_CONNECTED.load(Ordering::Relaxed);
            BLOCK_CONNECTED.store(next, Ordering::Relaxed);
            host.set_setting(ID, "block_connect", if next { "1" } else { "0" });
            let _ = host.chat_line(Text::literal(format!(
                "§b[Litematica 方块连接]§r 已切换为: §e{}§r",
                BLOCK_CONNECT_MODES[usize::from(next)]
            )));
            return Flow::Cancel;
        } else if cmd == "schematics_sources" {
            let cfg = load_source_cfg();
            let _ = host.chat_line(Text::literal(format!(
                "§b[Litematica 网络源]§r 当前激活源: §e{}§r | 积分Token: §a{}§r | 全部源({})：{}",
                cfg.active,
                if cfg.token.is_empty() { "未绑定 (请用 /schematics_token <lkr_xxx>)" } else { &cfg.token },
                cfg.sources.len(),
                cfg.sources.join(", ")
            )));
            return Flow::Cancel;
        } else if let Some(rest) = cmd.strip_prefix("schematics_add_source ") {
            let url = rest.trim().trim_end_matches('/').to_string();
            if !url.starts_with("http://") && !url.starts_with("https://") {
                let _ = host.chat_line(Text::literal("§c[Litematica 网络源]§r 源地址必须以 http:// 或 https:// 开头"));
                return Flow::Cancel;
            }
            let mut cfg = load_source_cfg();
            if !cfg.sources.contains(&url) {
                cfg.sources.push(url.clone());
            }
            cfg.active = url.clone();
            save_source_cfg(&cfg);
            let _ = host.chat_line(Text::literal(format!("§a[Litematica 网络源]§r 已添加并切换默认源 -> §b{url}§r")));
            return Flow::Cancel;
        } else if let Some(rest) = cmd.strip_prefix("schematics_token ") {
            let tok = rest.trim().to_string();
            let mut cfg = load_source_cfg();
            cfg.token = tok.clone();
            save_source_cfg(&cfg);
            let _ = host.chat_line(Text::literal(format!("§a[Litematica 积分绑定]§r 已保存积分 Token: §e{tok}§r")));
            return Flow::Cancel;
        } else if cmd == "schematics_sync" {
            let cfg = load_source_cfg();
            let url = format!("{}/api/v1/manifest.json?kind=schematic&token={}", cfg.active, cfg.token);
            match http_get_text(&url) {
                Ok(body) => {
                    let slugs: Vec<&str> = body
                        .split("\"slug\":\"")
                        .skip(1)
                        .filter_map(|s| s.split('"').next())
                        .collect();
                    let _ = host.chat_line(Text::literal(format!(
                        "§b[Litematica 云端蓝图]§r 从 §e{}§r 同步到 §a{}§r 份蓝图: {} | 下载: /schematics_download <slug>",
                        cfg.active,
                        slugs.len(),
                        slugs.join(", ")
                    )));
                }
                Err(e) => {
                    let _ = host.chat_line(Text::literal(format!("§c[Litematica 云端同步失败]§r {e}")));
                }
            }
            return Flow::Cancel;
        } else if let Some(rest) = cmd.strip_prefix("schematics_download ") {
            let slug = rest.trim();
            let cfg = load_source_cfg();
            let dl_url = format!("{}/api/v1/assets/schematic/{slug}/download?token={}", cfg.active, cfg.token);
            let dest = PathBuf::from("schematics").join(format!("{slug}.litematic"));
            match http_download_file(&dl_url, &dest) {
                Ok(_) => {
                    let _ = host.chat_line(Text::literal(format!(
                        "§a[Litematica 积分下载成功]§r 已从 §b{}§r 下载蓝图 §e{slug}.litematic§r 至本地 schematics/ 目录！",
                        cfg.active
                    )));
                }
                Err(e) => {
                    let _ = host.chat_line(Text::literal(format!(
                        "§c[Litematica 积分下载被拒]§r {e} | 前往 §b{}§r 登录/签到/爱发电充值积分",
                        cfg.active
                    )));
                }
            }
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(Command::new("schematics_server"));
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if event.command.literals.first().map(String::as_str) == Some("schematics_server") {
            let idx = BP_IDX.load(Ordering::Relaxed) as usize % BLUEPRINTS.len();
            let bp = &BLUEPRINTS[idx];
            for p in host.players().server_players() {
                let _ = p.tell(
                    host,
                    Text::literal(format!(
                        "§b[Litematica Server]§r 服务端 8 张 .litematic 蓝图已同步！当前目标: §e{}§r ({} 块) | 材料: {}",
                        bp.name, bp.total_blocks, bp.materials
                    )),
                );
            }
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, _event: &mut SettingChangedEvent<'_>) {
        let b = host.setting_choice("blueprint").unwrap_or(0) as usize % BLUEPRINTS.len();
        let lm = host.setting_choice("layer_mode").unwrap_or(0) as usize % LAYER_MODES.len();
        let rot = host.setting_choice("rotation").unwrap_or(0) as usize % ROTATIONS.len();
        let mir = host.setting_choice("mirror").unwrap_or(0) as usize % MIRRORS.len();
        let gh = host.setting_choice("ghost_render").unwrap_or(0) == 0;
        let bc = host.setting_choice("block_connect").unwrap_or(0) == 1;
        BP_IDX.store(b as u32, Ordering::Relaxed);
        LAYER_MODE.store(lm as u32, Ordering::Relaxed);
        ROT_IDX.store(rot as u32, Ordering::Relaxed);
        MIRROR_IDX.store(mir as u32, Ordering::Relaxed);
        GHOST_ENABLED.store(gh, Ordering::Relaxed);
        BLOCK_CONNECTED.store(bc, Ordering::Relaxed);
        let origin = current_anchor(&*host);
        write_active_target(b, lm, ACTIVE_Y.load(Ordering::Relaxed), rot, origin);
    }

    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        if event.action != KeyAction::Press || !host.playing() {
            return;
        }
        if event.key == "key.keyboard.m" && !host.is_down(ID, KEY_MENU) {
            show_checklist_and_reanchor(host);
        } else if event.key == "key.keyboard.n" && !host.is_down(ID, KEY_NEXT) {
            cycle_next_blueprint(host);
        } else if (event.key == "key.keyboard.page.up" || event.key == "key.keyboard.page_up")
            && !host.is_down(ID, KEY_LAYER_UP)
        {
            step_layer_up(host);
        } else if (event.key == "key.keyboard.page.down" || event.key == "key.keyboard.page_down")
            && !host.is_down(ID, KEY_LAYER_DOWN)
        {
            step_layer_down(host);
        }
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        static SCHEM_TICK: AtomicU32 = AtomicU32::new(0);
        static LAST_SCHEM_NONCE: AtomicU32 = AtomicU32::new(0);
        let origin = current_anchor(&*host);
        if SCHEM_TICK.fetch_add(1, Ordering::Relaxed) % 10 == 0 {
            if let Ok(raw) = fs::read_to_string("horizon-mods/.horizon-mod-data/schematics/web_switch.txt") {
                let mut parts = raw.split_whitespace();
                if let (Some(b_s), Some(l_s), Some(m_s), Some(g_s), Some(n_s)) =
                    (parts.next(), parts.next(), parts.next(), parts.next(), parts.next())
                {
                    if let (Ok(b), Ok(lm), Ok(mir), Ok(gh), Ok(nonce)) = (
                        b_s.parse::<usize>(),
                        l_s.parse::<usize>(),
                        m_s.parse::<u32>(),
                        g_s.parse::<u32>(),
                        n_s.parse::<u32>(),
                    ) {
                        if nonce != 0 && nonce != LAST_SCHEM_NONCE.load(Ordering::Relaxed) {
                            LAST_SCHEM_NONCE.store(nonce, Ordering::Relaxed);
                            let b_clamped = b % BLUEPRINTS.len();
                            let lm_clamped = lm % LAYER_MODES.len();
                            BP_IDX.store(b_clamped as u32, Ordering::Relaxed);
                            LAYER_MODE.store(lm_clamped as u32, Ordering::Relaxed);
                            MIRROR_IDX.store(mir % MIRRORS.len() as u32, Ordering::Relaxed);
                            GHOST_ENABLED.store(gh != 0, Ordering::Relaxed);
                            write_active_target(
                                b_clamped,
                                lm_clamped,
                                ACTIVE_Y.load(Ordering::Relaxed),
                                ROT_IDX.load(Ordering::Relaxed) as usize,
                                origin,
                            );
                            let _ = host.chat_line(Text::literal(format!(
                                "§a[Lilyco Web GUI]§r 已实时切换 Litematica 投影 -> §e{}",
                                BLUEPRINTS[b_clamped].name
                            )));
                        }
                    }
                }
            }
        }

        while host.consume_click(ID, KEY_MENU) {
            show_checklist_and_reanchor(host);
        }
        while host.consume_click(ID, KEY_NEXT) {
            cycle_next_blueprint(host);
        }
        while host.consume_click(ID, KEY_LAYER_UP) {
            step_layer_up(host);
        }
        while host.consume_click(ID, KEY_LAYER_DOWN) {
            step_layer_down(host);
        }

        // Verify placed blocks in the client world against the active schematic blueprint
        let reads: &dyn ClientHost = &*host;
        if let Some(level) = reads.level(reads.dimension()) {
            let idx = BP_IDX.load(Ordering::Relaxed) as usize % BLUEPRINTS.len();
            let bp = &BLUEPRINTS[idx];
            let rot = ROT_IDX.load(Ordering::Relaxed) as usize;
            let mir = MIRROR_IDX.load(Ordering::Relaxed) as usize;
            let origin = [
                ANCHOR_X.load(Ordering::Relaxed),
                ANCHOR_Y.load(Ordering::Relaxed),
                ANCHOR_Z.load(Ordering::Relaxed),
            ];
            let mut correct = 0u32;
            let mut wrong = 0u32;
            for item in bp.blocks {
                let [ox, oy, oz] =
                    transform_offset(item.dx, item.dy, item.dz, bp.size, rot, mir);
                let pos = [origin[0] + ox, origin[1] + oy, origin[2] + oz];
                if let Some(world_state) = level.block_state(reads, pos) {
                    let target_state = reads.block_state_id(item.block);
                    if target_state.is_some_and(|ts| ts.block() == world_state.block()) {
                        correct += 1;
                    } else if !is_ignorable_air(reads, world_state) {
                        wrong += 1;
                    }
                }
            }
            CORRECT_BLOCKS.store(correct, Ordering::Relaxed);
            WRONG_BLOCKS.store(wrong, Ordering::Relaxed);
        }
    }

    fn standalone_models(
        &self,
        _context: &RegistrationContext,
        event: &mut StandaloneModelsEvent<'_>,
    ) {
        for &(_block_id, model_id) in &BLOCK_MODEL_MAP {
            event.models.push(model_id.into());
        }
    }

    fn model_baking_completed(
        &self,
        host: &mut dyn ClientHost,
        _: &mut ModelBakingCompletedEvent,
    ) {
        let assets = host.render_assets();
        if let Ok(mut map) = block_model_cache().lock() {
            map.clear();
            for &(block_id, model_id) in &BLOCK_MODEL_MAP {
                if let Ok(id) = Identifier::parse(model_id)
                    && let Ok(mref) = assets.model(&id)
                {
                    map.insert(block_id, mref);
                }
            }
        }
    }

    fn render_level_stage(&self, host: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        if event.stage != RenderLevelStage::AfterLevel
            || !host.on(ID)
            || !GHOST_ENABLED.load(Ordering::Relaxed)
        {
            return;
        }
        let idx = BP_IDX.load(Ordering::Relaxed) as usize % BLUEPRINTS.len();
        let bp = &BLUEPRINTS[idx];
        let lm = LAYER_MODE.load(Ordering::Relaxed) as usize % LAYER_MODES.len();
        let active_y = ACTIVE_Y.load(Ordering::Relaxed);
        let rot = ROT_IDX.load(Ordering::Relaxed) as usize;
        let mir = MIRROR_IDX.load(Ordering::Relaxed) as usize;
        let origin = current_anchor(host);

        let mut lines = Vec::with_capacity(512);
        let mut quads = Vec::with_capacity(256);

        // 1. Draw the full 3D schematic bounding box in bright cyan
        let [sx, sy, sz] = bp.size;
        let corners = [
            transform_offset(0, 0, 0, bp.size, rot, mir),
            transform_offset(sx, sy, sz, bp.size, rot, mir),
        ];
        let min_x = (origin[0] + corners[0][0].min(corners[1][0])) as f64;
        let max_x = (origin[0] + corners[0][0].max(corners[1][0])) as f64;
        let min_y = (origin[1] + corners[0][1].min(corners[1][1])) as f64;
        let max_y = (origin[1] + corners[0][1].max(corners[1][1])) as f64;
        let min_z = (origin[2] + corners[0][2].min(corners[1][2])) as f64;
        let max_z = (origin[2] + corners[0][2].max(corners[1][2])) as f64;

        let box_color = 0xFF00_E5FF;
        let box_edges = [
            ([min_x, min_y, min_z], [max_x, min_y, min_z]),
            ([max_x, min_y, min_z], [max_x, min_y, max_z]),
            ([max_x, min_y, max_z], [min_x, min_y, max_z]),
            ([min_x, min_y, max_z], [min_x, min_y, min_z]),
            ([min_x, max_y, min_z], [max_x, max_y, min_z]),
            ([max_x, max_y, min_z], [max_x, max_y, max_z]),
            ([max_x, max_y, max_z], [min_x, max_y, max_z]),
            ([min_x, max_y, max_z], [min_x, max_y, min_z]),
            ([min_x, min_y, min_z], [min_x, max_y, min_z]),
            ([max_x, min_y, min_z], [max_x, max_y, min_z]),
            ([max_x, min_y, max_z], [max_x, max_y, max_z]),
            ([min_x, min_y, max_z], [min_x, max_y, max_z]),
        ];
        for (a, b) in box_edges {
            lines.push(WorldLine::new(a, b, box_color, 2.2, WorldDepth::Tested));
        }

        // 2. Group missing blocks by ModelRef for GPU-instanced REAL TEXTURED Minecraft block rendering!
        let mut batches: HashMap<&'static str, Vec<ModelInstance>> = HashMap::new();
        let connected = BLOCK_CONNECTED.load(Ordering::Relaxed);
        let ghost_scale: f32 = if connected { 1.0 } else { 0.84 };
        let ghost_pad: f64 = (1.0 - ghost_scale as f64) * 0.5;
        let ghost_transform: [f32; 12] = [
            ghost_scale, 0.0, 0.0, 0.0,
            0.0, ghost_scale, 0.0, 0.0,
            0.0, 0.0, ghost_scale, 0.0,
        ];

        let level = host.level(host.dimension());
        let model_map = block_model_cache().lock().ok();

        for item in bp.blocks {
            if lm == 1 && item.dy != active_y {
                continue;
            }
            if lm == 2 && item.dy > active_y {
                continue;
            }
            let [ox, oy, oz] = transform_offset(item.dx, item.dy, item.dz, bp.size, rot, mir);
            let pos = [origin[0] + ox, origin[1] + oy, origin[2] + oz];

            let mut is_correct = false;
            let mut is_wrong = false;
            if let Some(lvl) = level
                && let Some(ws) = lvl.block_state(host, pos)
            {
                if host
                    .block_state_id(item.block)
                    .is_some_and(|ts| ts.block() == ws.block())
                {
                    is_correct = true;
                } else if !is_ignorable_air(host, ws) {
                    is_wrong = true;
                }
            }

            if is_correct {
                // Already placed correctly -> subtle green wireframe check
                if lines.len() < 1800 {
                    push_block_wireframe(&mut lines, pos, 0x8822_C55E, 1.2);
                }
                continue;
            }

            if is_wrong {
                // Wrong block placed -> bright red warning box (still draw target textured block inside!)
                if lines.len() < 1800 {
                    push_block_wireframe(&mut lines, pos, 0xFFFF_2020, 2.5);
                }
            }

            // Missing or wrong block -> render REAL 3D TEXTURED Minecraft block model + hologram outline!
            let has_model = model_map
                .as_ref()
                .is_some_and(|m| m.contains_key(item.block));
            if has_model {
                let inst = ModelInstance::new(
                    [
                        pos[0] as f64 + ghost_pad,
                        pos[1] as f64 + ghost_pad,
                        pos[2] as f64 + ghost_pad,
                    ],
                    ghost_transform,
                    0xFFFF_FFFF,
                    LightCoords::FULL_BRIGHT,
                    Overlay::NONE,
                );
                batches.entry(item.block).or_default().push(inst);
                if !is_wrong && lines.len() < 1800 && (lm == 1 || item.dy == active_y) {
                    push_block_wireframe(&mut lines, pos, 0xAA00_E5FF, 1.3);
                }
            } else {
                // Fallback for translucent fluids (water)
                let argb = 0x5500_0000 | block_palette_color(item.block);
                let x0 = pos[0] as f64 + 0.08;
                let _y0 = pos[1] as f64 + 0.08;
                let z0 = pos[2] as f64 + 0.08;
                let x1 = pos[0] as f64 + 0.92;
                let y1 = pos[1] as f64 + 0.92;
                let z1 = pos[2] as f64 + 0.92;
                if quads.len() < 1200 {
                    quads.push(WorldQuad::new(
                        [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
                        argb,
                        WorldDepth::Tested,
                    ));
                }
            }
        }

        if let Some(map) = model_map {
            for (block_id, instances) in batches {
                if let Some(&mref) = map.get(block_id) {
                    let _ = event.draw.draw_model_instances(mref, &instances);
                }
            }
        }

        let _ = event.draw.lines(&lines);
        let _ = event.draw.quads(&quads);
    }

    fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
        if !host.on(ID) || !GHOST_ENABLED.load(Ordering::Relaxed) {
            return;
        }
        let idx = BP_IDX.load(Ordering::Relaxed) as usize % BLUEPRINTS.len();
        let bp = &BLUEPRINTS[idx];
        let lm = LAYER_MODE.load(Ordering::Relaxed) as usize % LAYER_MODES.len();
        let y_layer = ACTIVE_Y.load(Ordering::Relaxed);
        let rot = ROT_IDX.load(Ordering::Relaxed) as usize % ROTATIONS.len();
        let done = CORRECT_BLOCKS.load(Ordering::Relaxed);
        let wrong = WRONG_BLOCKS.load(Ordering::Relaxed);

        let line1 = format!(
            "[Litematica] {} ({}/{}) [N切换/M锚点]",
            bp.short_name,
            idx + 1,
            BLUEPRINTS.len()
        );
        let line2 = format!(
            "进度: {}/{} (错:{}) | 层:Y={}/{} ({}) | 旋转:{}",
            done,
            bp.total_blocks,
            wrong,
            y_layer,
            bp.size[1] - 1,
            if lm == 0 { "全层" } else if lm == 1 { "单层" } else { "以下" },
            ROTATIONS[rot]
        );
        let mat_preview: String = bp.materials.chars().take(48).collect();
        let line3 = format!("材料: {}", mat_preview);

        let draw = &mut *event.draw;
        let w = draw
            .text_width(&line1)
            .max(draw.text_width(&line2))
            .max(draw.text_width(&line3))
            + 8.0;
        let h = 35.0;
        let (x, y) = event.corners.place(Corner::TopLeft, w, h);
        draw.fill([x, y, w, h], 0x880C_1928);
        draw.fill([x, y, 2.0, h], 0xFF00_E5FF);
        draw.text(&line1, [x + 5.0, y + 2.0], 0xFF38_BDF8, true);
        draw.text(&line2, [x + 5.0, y + 13.0], 0xFFA7_F3D0, true);
        draw.text(&line3, [x + 5.0, y + 24.0], 0xFFFDE047, true);
    }
}
