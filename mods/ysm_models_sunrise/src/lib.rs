mod gui;
mod hotload;
mod ytree_engine;

use mod_api::{
    self, ArmPose, BlockEntityDrawContext, CameraShakeEvent, ClientHost, ClientTickPostEvent,
    Command, CommandEvent, Corner, CubeFace, Flow, GetArmPoseEvent, Hand, Hooks, InputKeyEvent,
    ItemStack, KeyAction, KeyConflictContext, KeyRegistration, PlayerLoggedInEvent,
    RegisterClientCommandsEvent, RegisterCommandsEvent, Registrar, RegistrationContext,
    RenderArmEvent, RenderGuiPostEvent, RenderHandEvent, RenderPlayerPreEvent, RunModCommandEvent,
    ScreenClosingEvent, ScreenMouseButtonPressedPreEvent, ScreenMouseScrolledPreEvent,
    ScreenRenderPostEvent, ServerHost, ServerTickEvent, SettingChangedEvent, SettingSpec, Text,
    UseAnimation,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

const DEFAULT_ASSET_SOURCE: &str = "https://dev.lain42.top/mc-assets";

// Hot-loaded models from a remote asset source (dev.lain42.top).
// HOT_SLUG empty means the active persona is one of the built-in PERSONAS.
static HOT_SLUG: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static HOT_UPLOAD_PENDING: AtomicBool = AtomicBool::new(false);
static HOT_CATALOG: std::sync::Mutex<Vec<hotload::CatalogItem>> = std::sync::Mutex::new(Vec::new());

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

pub(crate) const ID: &str = "ysm_models";
const KEY_CYCLE_MODEL: &str = "cycle_model";
const KEY_CYCLE_GESTURE: &str = "cycle_gesture";
const KEY_OPEN_GUI: &str = "open_gui";

pub struct PersonaSpec {
    pub id: &'static str,
    pub model_id: &'static str,
    pub title: &'static str,
    pub short_title: &'static str,
    pub bones_count: u32,
    pub cubes_count: u32,
    pub bones: &'static str,
    pub anims: &'static str,
    pub scale: [f32; 3],
    pub hand_tint_offset: [f32; 3],
    pub primary_rgb: u32,
    pub accent_rgb: u32,
    pub glow_rgb: u32,
}

pub const PERSONAS: [PersonaSpec; 10] = [
    PersonaSpec {
        id: "claude_ojousama",
        model_id: "m_b3d34d442c934147",
        title: "Claude 橙色大小姐 [橘雪莉 96骨骼/968立方体]",
        short_title: "Claude·橘雪莉大小姐(96骨)",
        bones_count: 96,
        cubes_count: 968,
        bones: "Root,AllBody,UpBody,Head,hair_back,flower_hairpin,layered_skirt,rightArm,leftArm",
        anims: "idle,walk,swing_hand,ojousama_bow,read_book",
        scale: [0.95, 0.95, 0.95],
        hand_tint_offset: [0.02, -0.01, 0.0],
        primary_rgb: 0xFF8C24,
        accent_rgb: 0xFFF2D6,
        glow_rgb: 0xFFD700,
    },
    PersonaSpec {
        id: "claude_butterfly",
        model_id: "m_f719671952a0d236",
        title: "Claude 蝴蝶晚礼服大小姐 [557骨骼/4237立方体]",
        short_title: "Claude·蝴蝶晚礼服(557骨)",
        bones_count: 557,
        cubes_count: 4237,
        bones: "NAllRoot,MAllBody,ButterFly,RightWingsA,LeftWingsA,evening_gown,ribbon_crown",
        anims: "idle,walk,butterfly_flutter,waltz_spin,curtsy",
        scale: [0.96, 0.96, 0.96],
        hand_tint_offset: [0.02, 0.0, 0.01],
        primary_rgb: 0xFFA53B,
        accent_rgb: 0x5E3BFF,
        glow_rgb: 0xFFE484,
    },
    PersonaSpec {
        id: "deepseek_whale",
        model_id: "m_91686caca5dd11f4",
        title: "DeepSeek 蓝色鲸鱼娘 [DS鲸鱼娘 208骨骼/1730立方体]",
        short_title: "DeepSeek·DS鲸鱼娘(208骨)",
        bones_count: 208,
        cubes_count: 1730,
        bones: "Root,MAllBody,AllBody,whale_fins,ahoge,whale_tail,ALLbowl,maid_skirt",
        anims: "idle,walk,whale_tail_wag,slacking_eat_rice,water_float",
        scale: [0.94, 0.94, 0.94],
        hand_tint_offset: [-0.01, -0.02, 0.02],
        primary_rgb: 0x2684FF,
        accent_rgb: 0xE6F4FF,
        glow_rgb: 0x00E5FF,
    },
    PersonaSpec {
        id: "deepseek_chan",
        model_id: "m_bf632c3a339fe3b9",
        title: "DeepSeek 蓝白女仆DeepSeek酱 [173骨骼/694立方体]",
        short_title: "DeepSeek·DeepSeek酱(173骨)",
        bones_count: 173,
        cubes_count: 694,
        bones: "Root,MAllBody,UpperBody,MoreUpperBody,Skirt,Ribbon3,whale_beret,twintails",
        anims: "idle,walk,blink_random,wall_touch,maid_wave",
        scale: [0.94, 0.94, 0.94],
        hand_tint_offset: [-0.01, -0.01, 0.01],
        primary_rgb: 0x3B9DFF,
        accent_rgb: 0xFFFFFF,
        glow_rgb: 0x66FFFF,
    },
    PersonaSpec {
        id: "chatgpt_kaltsit",
        model_id: "m_0116b7cdf77b9b18",
        title: "ChatGPT 优雅御姐前辈 [凯尔希 325骨骼/1750立方体]",
        short_title: "ChatGPT·凯尔希御姐(325骨)",
        bones_count: 325,
        cubes_count: 1750,
        bones: "Root,Body_size,allcrystal,Mcrystal,ysmGlow_crystal_A,lynx_ears,medical_coat",
        anims: "idle,walk,crystal_halo_spin,senpai_nod,tactical_command",
        scale: [0.98, 0.98, 0.98],
        hand_tint_offset: [0.0, 0.01, -0.01],
        primary_rgb: 0x18B87A,
        accent_rgb: 0xE8F5EE,
        glow_rgb: 0x39FF14,
    },
    PersonaSpec {
        id: "chatgpt_dragon",
        model_id: "m_07df0a8cbf64ca89",
        title: "ChatGPT 安德莉亚末影龙娘 [474骨骼/4081立方体]",
        short_title: "ChatGPT·末影龙娘(474骨)",
        bones_count: 474,
        cubes_count: 4081,
        bones: "ALL_root,dragon_root,dragon_horns,dragon_wings,dragon_tail,ysmGlow_claw",
        anims: "idle,walk,dragon_wing_spread,ender_breath,tail_sweep",
        scale: [0.99, 0.99, 0.99],
        hand_tint_offset: [0.0, 0.02, -0.01],
        primary_rgb: 0x8F2BFF,
        accent_rgb: 0x22222A,
        glow_rgb: 0xE040FB,
    },
    PersonaSpec {
        id: "gemini_cheshire",
        model_id: "m_bdf204f41312e775",
        title: "Gemini 星光哈基米猫娘 [柴郡猫娘 134骨骼/804立方体]",
        short_title: "Gemini·柴郡猫娘(134骨)",
        bones_count: 134,
        cubes_count: 804,
        bones: "Root,MAllBody,UpBody,cat_ears,star_hairpin,Mtail,tail_A,tail_B,ribbon_bell",
        anims: "idle,walk,cat_ear_twitch,star_dance,cheshire_grin",
        scale: [0.93, 0.93, 0.93],
        hand_tint_offset: [0.01, 0.01, 0.02],
        primary_rgb: 0xA855F7,
        accent_rgb: 0xFFD966,
        glow_rgb: 0xFF66CC,
    },
    PersonaSpec {
        id: "gemini_nekomiya",
        model_id: "m_d7d102c40716f521",
        title: "Gemini 猫宫又奈双尾猫娘 [227骨骼/1806立方体]",
        short_title: "Gemini·猫宫又奈(227骨)",
        bones_count: 227,
        cubes_count: 1806,
        bones: "MRoot,MAllBody,AllHead,neko_ears,dual_tails_L,dual_tails_R,tech_jacket",
        anims: "idle,walk,dual_tail_wave,neko_pounce,victory_pose",
        scale: [0.93, 0.93, 0.93],
        hand_tint_offset: [0.01, 0.01, 0.01],
        primary_rgb: 0xFF4D94,
        accent_rgb: 0x38BDF8,
        glow_rgb: 0xFDE047,
    },
    PersonaSpec {
        id: "lyco_ip",
        model_id: "lilyco_ip_v1",
        title: "LYCO 莉可 [lyco-ip 原创3D角色]",
        short_title: "LYCO·莉可(原创)",
        bones_count: 14,
        cubes_count: 126,
        bones: "Root,Body,Head,Hair,RightArm,LeftArm,RightLeg,LeftLeg",
        anims: "idle,walk,wave,cheer",
        scale: [0.95, 0.95, 0.95],
        hand_tint_offset: [0.0, 0.0, 0.0],
        primary_rgb: 0x38BDF8,
        accent_rgb: 0xF8FAFC,
        glow_rgb: 0x7DD3FC,
    },
    PersonaSpec {
        id: "yuqian",
        model_id: "yuqian_3d_v1",
        title: "透明兔女郎雨倩 [136骨骼/8742立方体]",
        short_title: "雨倩·透明兔女郎",
        bones_count: 136,
        cubes_count: 8742,
        bones: "Root,AllBody,Head,Hair,RightArm,LeftArm,RightLeg,LeftLeg",
        anims: "idle,walk,extra0,dance",
        scale: [0.98, 0.98, 0.98],
        hand_tint_offset: [0.0, 0.0, 0.0],
        primary_rgb: 0xF472B6,
        accent_rgb: 0xFFFFFF,
        glow_rgb: 0xFBCFE8,
    },
];

pub const GESTURES: [&str; 9] = [
    "idle (默认呼吸+发丝裙摆物理摆动)",
    "wave (挥手致意)",
    "cheer (欢呼跳跃)",
    "work_redstone (红石精密施工姿态)",
    "sit (原地休息/摸鱼吃大米)",
    "dance (角色专属星光律动舞)",
    "bow (大小姐优雅提裙礼)",
    "victory (屠龙凯旋招牌动作)",
    "mace_twirl (重锤/武器旋风甩动 [3.8.0彩蛋])",
];

const REACH_CHOICES: [&str; 4] = ["16 blocks", "32 blocks", "64 blocks (Default)", "128 blocks"];
const BOOL_CHOICES: [&str; 2] = ["Enabled", "Disabled"];

pub(crate) static PERSONA_IDX: AtomicU32 = AtomicU32::new(0);
pub(crate) static GESTURE_IDX: AtomicU32 = AtomicU32::new(0);
static ARMOR_AUTO_HIDE: AtomicU32 = AtomicU32::new(0);
static FIRST_PERSON_ARM: AtomicU32 = AtomicU32::new(0);
static HUD_OVERLAY: AtomicU32 = AtomicU32::new(1);
static TICK_COUNTER: AtomicU32 = AtomicU32::new(0);
static LAST_WEB_NONCE: AtomicU32 = AtomicU32::new(0);
static IS_CROUCHING: AtomicBool = AtomicBool::new(false);
static DOUBLE_TAP_SPRINT: AtomicBool = AtomicBool::new(false);
static LAST_W_PRESS_TICK: AtomicU32 = AtomicU32::new(0);
static LIVE_STATE_IDX: AtomicU32 = AtomicU32::new(0);
static LOCAL_HELD_CAT: AtomicU32 = AtomicU32::new(0);
static LOCAL_USE_KIND: AtomicU32 = AtomicU32::new(0);
static LOCAL_HAND_SWING: AtomicBool = AtomicBool::new(false);
static LOCAL_OFF_SWING: AtomicBool = AtomicBool::new(false);
static LOCAL_RIDING: AtomicBool = AtomicBool::new(false);
static LOCAL_IN_BOAT: AtomicBool = AtomicBool::new(false);
static LOCAL_SLEEPING_SRV: AtomicBool = AtomicBool::new(false);
static LOCAL_IN_WATER: AtomicBool = AtomicBool::new(false);
static LOCAL_SWIMMING: AtomicBool = AtomicBool::new(false);
static LOCAL_WATER_DEPTH_MM: AtomicU32 = AtomicU32::new(0);
static LOCAL_EYE_IN_WATER: AtomicBool = AtomicBool::new(false);
static LOCAL_EYE_HEIGHT_MM: AtomicU32 = AtomicU32::new(1620);
static LOCAL_CREATIVE_FLYING: AtomicBool = AtomicBool::new(false);
static LOCAL_CLIMBING: AtomicBool = AtomicBool::new(false);
static LOCAL_VEL_Y_MM: AtomicI32 = AtomicI32::new(0);
static LOCAL_BLOCKING: AtomicBool = AtomicBool::new(false);
pub(crate) static NAMED_ANIM_IDX: AtomicU32 = AtomicU32::new(u32::MAX);
pub(crate) static NAMED_ANIM_LABEL: OnceLock<Mutex<String>> = OnceLock::new();

#[derive(Clone)]
struct EntityHeld {
    main_cat: ytree_engine::HeldCat,
    off_cat: ytree_engine::HeldCat,
    main_use: ytree_engine::UseKind,
    off_use: ytree_engine::UseKind,
    main_stack: Option<ItemStack>,
    off_stack: Option<ItemStack>,
    tick: u32,
}

static ENTITY_HELD: OnceLock<Mutex<HashMap<u32, EntityHeld>>> = OnceLock::new();
static LOCAL_ENTITY_HELD: OnceLock<Mutex<Option<EntityHeld>>> = OnceLock::new();

fn item_id_segment(id: &str) -> &str {
    let s = id.rsplit_once(':').map(|(_, tail)| tail).unwrap_or(id);
    s.rsplit('/').next().unwrap_or(s)
}

fn classify_item_id(id: &str) -> ytree_engine::HeldCat {
    let name = item_id_segment(id);
    let l = name.to_ascii_lowercase();
    if l.is_empty() || l == "air" {
        return ytree_engine::HeldCat::Empty;
    }
    if l == "bow" || l.ends_with("_bow") || l.starts_with("bow_") {
        ytree_engine::HeldCat::Bow
    } else if l == "crossbow" || l.ends_with("_crossbow") || l.starts_with("crossbow_") {
        ytree_engine::HeldCat::Crossbow
    } else if l.contains("sword") || l.contains("katana") || l.contains("blade") {
        ytree_engine::HeldCat::Sword
    } else if l.contains("shield") {
        ytree_engine::HeldCat::Shield
    } else if l.contains("fishing_rod") {
        ytree_engine::HeldCat::Fishing
    } else if l.contains("trident") || l.contains("mace") || l.contains("spear") {
        ytree_engine::HeldCat::Spear
    } else if l.contains("pickaxe") || l.contains("axe") || l.contains("shovel") || l.contains("hoe") {
        ytree_engine::HeldCat::Tool
    } else {
        ytree_engine::HeldCat::Item
    }
}

fn map_use_anim(anim: UseAnimation) -> ytree_engine::UseKind {
    match anim {
        UseAnimation::Eat => ytree_engine::UseKind::Eat,
        UseAnimation::Drink => ytree_engine::UseKind::Drink,
        UseAnimation::Bow => ytree_engine::UseKind::Bow,
        UseAnimation::Crossbow => ytree_engine::UseKind::Crossbow,
        UseAnimation::Trident | UseAnimation::Spear => ytree_engine::UseKind::Spear,
        UseAnimation::Block => ytree_engine::UseKind::Block,
        UseAnimation::Spyglass => ytree_engine::UseKind::Spyglass,
        UseAnimation::TootHorn => ytree_engine::UseKind::Horn,
        UseAnimation::Brush => ytree_engine::UseKind::Brush,
        _ => ytree_engine::UseKind::None,
    }
}

fn mapping_down(host: &dyn ClientHost, name: &str) -> bool {
    host.mapping(name)
        .ok()
        .flatten()
        .is_some_and(|m| m.down)
}

const LIVE_STATE_NAMES: [&str; 27] = [
    "idle",
    "walk",
    "run",
    "sneak",
    "sneaking",
    "jump",
    "fly",
    "elytra_fly",
    "ride",
    "boat",
    "swim",
    "swim_stand",
    "ladder_up",
    "ladder_down",
    "ladder_stillness",
    "sleep",
    "attacked",
    "death",
    "custom",
    "extra1/wave",
    "extra2/cheer",
    "extra3/work",
    "extra4/sit",
    "extra5/dance",
    "extra6/bow",
    "extra7/victory",
    "extra8/mace_twirl",
];

fn state_name_to_idx(name: &str) -> u32 {
    LIVE_STATE_NAMES
        .iter()
        .position(|&s| s == name)
        .unwrap_or(0) as u32
}

pub(crate) fn sync_state_to_disk(p_idx: usize, g_idx: usize) {
    let p = &PERSONAS[p_idx % PERSONAS.len()];
    let g = GESTURES[g_idx % GESTURES.len()];
    let dir = PathBuf::from("horizon-mods/.horizon-mod-data/ysm_models");
    let _ = fs::create_dir_all(&dir);
    let json = format!(
        "{{\n  \"active_persona\": \"{}\",\n  \"alltheysm_id\": \"{}\",\n  \"title\": \"{}\",\n  \"bones_count\": {},\n  \"cubes_count\": {},\n  \"bones\": \"{}\",\n  \"animations\": \"{}\",\n  \"active_gesture\": \"{}\",\n  \"scale\": [{:.2}, {:.2}, {:.2}],\n  \"fallback_skins\": {{\"Claude\": \"zuri (claude.png)\", \"DeepSeek\": \"sunny (deepseek.png)\", \"ChatGPT\": \"noor (chatgpt.png)\", \"Gemini\": \"kai (gemini.png)\"}},\n  \"multiplayer_sync\": true\n}}\n",
        p.id, p.model_id, p.title, p.bones_count, p.cubes_count, p.bones, p.anims, g, p.scale[0], p.scale[1], p.scale[2]
    );
    let _ = fs::write(dir.join("active_ysm_state.json"), json);
}

fn resolve_player_persona(name: &str, local: bool) -> usize {
    if local {
        return PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
    }
    let lower = name.to_ascii_lowercase();
    if lower.contains("claude") {
        0
    } else if lower.contains("deepseek") {
        2
    } else if lower.contains("chatgpt") {
        4
    } else if lower.contains("gemini") {
        6
    } else {
        (name.bytes().map(usize::from).sum::<usize>()) % PERSONAS.len()
    }
}

fn cycle_model(host: &mut dyn ClientHost) {
    let next = (PERSONA_IDX.load(Ordering::Relaxed) as usize + 1) % PERSONAS.len();
    PERSONA_IDX.store(next as u32, Ordering::Relaxed);
    NAMED_ANIM_IDX.store(u32::MAX, Ordering::Relaxed);
    host.set_setting(ID, "persona", &next.to_string());
    let g = GESTURE_IDX.load(Ordering::Relaxed) as usize;
    sync_state_to_disk(next, g);
    let p = &PERSONAS[next];
    let _ = host.chat_line(Text::literal(format!(
        "§6[YSM 3D]§r 已切换 3D 角色模型 ({}/{}): §e{}§r [ID: {} | {}骨骼/{}立方体]",
        next + 1,
        PERSONAS.len(),
        p.title,
        p.model_id,
        p.bones_count,
        p.cubes_count
    )));
}

fn cycle_gesture(host: &mut dyn ClientHost) {
    NAMED_ANIM_IDX.store(u32::MAX, Ordering::Relaxed);
    let next_g = (GESTURE_IDX.load(Ordering::Relaxed) as usize + 1) % GESTURES.len();
    GESTURE_IDX.store(next_g as u32, Ordering::Relaxed);
    host.set_setting(ID, "gesture", &next_g.to_string());
    let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
    sync_state_to_disk(p_idx, next_g);
    let p = &PERSONAS[p_idx];
    let _ = host.chat_line(Text::literal(format!(
        "§b[YSM 3D]§r 播放 3D 骨骼动作 ({}/{}): §a{}§r -> §f{}",
        next_g + 1,
        GESTURES.len(),
        p.short_title,
        GESTURES[next_g]
    )));
}

struct BakedQuad {
    corners: [[f32; 3]; 4],
    uv16: [f32; 4],
}

struct BakedGroup {
    pivot: [f32; 3],
    quads: Vec<BakedQuad>,
}

struct BakedModel {
    sprite: &'static str,
    groups: [BakedGroup; 6],
}

pub(crate) const MODEL_SPECS: [(&str, &str); 10] = [
    ("claude_ojousama", "ysm_models:entity/claude_ojousama"),
    ("claude_butterfly", "ysm_models:entity/claude_butterfly"),
    ("deepseek_whale", "ysm_models:entity/deepseek_whale"),
    ("deepseek_chan", "ysm_models:entity/deepseek_chan"),
    ("chatgpt_kaltsit", "ysm_models:entity/chatgpt_kaltsit"),
    ("chatgpt_dragon", "ysm_models:entity/chatgpt_dragon"),
    ("gemini_cheshire", "ysm_models:entity/gemini_cheshire"),
    ("gemini_nekomiya", "ysm_models:entity/gemini_nekomiya"),
    ("lyco_ip", "ysm_models:entity/lyco_ip"),
    ("yuqian", "ysm_models:entity/yuqian"),
];

static BAKED_MODELS: OnceLock<Vec<BakedModel>> = OnceLock::new();

fn load_baked_bytes(slug: &str) -> Option<Vec<u8>> {
    let fname = format!("{slug}.bin");
    let mut candidates = vec![
        PathBuf::from("horizon-native-mods/ysm_models/baked_models").join(&fname),
        PathBuf::from("horizon-mods/.horizon-mod-data/ysm_models/baked_models").join(&fname),
        PathBuf::from("player_models/baked_models").join(&fname),
    ];
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(
            PathBuf::from(home)
                .join(".local/share/horizon/horizon-native-mods/ysm_models/baked_models")
                .join(&fname),
        );
    }
    for p in candidates {
        if let Ok(bytes) = fs::read(&p) {
            if bytes.len() >= 96 {
                return Some(bytes);
            }
        }
    }
    None
}

fn read_f32(bytes: &[u8], off: &mut usize) -> f32 {
    let v = f32::from_le_bytes([bytes[*off], bytes[*off + 1], bytes[*off + 2], bytes[*off + 3]]);
    *off += 4;
    v
}

fn read_u32(bytes: &[u8], off: &mut usize) -> u32 {
    let v = u32::from_le_bytes([bytes[*off], bytes[*off + 1], bytes[*off + 2], bytes[*off + 3]]);
    *off += 4;
    v
}

fn parse_baked_model(sprite: &'static str, bytes: &[u8]) -> BakedModel {
    let mut off = 0usize;
    let mut pivots = [[0.0f32; 3]; 6];
    let mut counts = [0usize; 6];
    for i in 0..6 {
        pivots[i] = [read_f32(bytes, &mut off), read_f32(bytes, &mut off), read_f32(bytes, &mut off)];
        counts[i] = read_u32(bytes, &mut off) as usize;
    }
    let groups = std::array::from_fn(|i| {
        let cnt = counts[i];
        let mut quads = Vec::with_capacity(cnt);
        for _ in 0..cnt {
            if off + 64 > bytes.len() {
                break;
            }
            let c0 = [read_f32(bytes, &mut off), read_f32(bytes, &mut off), read_f32(bytes, &mut off)];
            let c1 = [read_f32(bytes, &mut off), read_f32(bytes, &mut off), read_f32(bytes, &mut off)];
            let c2 = [read_f32(bytes, &mut off), read_f32(bytes, &mut off), read_f32(bytes, &mut off)];
            let c3 = [read_f32(bytes, &mut off), read_f32(bytes, &mut off), read_f32(bytes, &mut off)];
            let ua = read_f32(bytes, &mut off) * 16.0;
            let va = read_f32(bytes, &mut off) * 16.0;
            let ub = read_f32(bytes, &mut off) * 16.0;
            let vb = read_f32(bytes, &mut off) * 16.0;
            quads.push(BakedQuad {
                corners: [c0, c1, c2, c3],
                uv16: [ua, va, ub, vb],
            });
        }
        BakedGroup {
            pivot: pivots[i],
            quads,
        }
    });
    BakedModel { sprite, groups }
}

fn empty_baked_model(sprite: &'static str) -> BakedModel {
    BakedModel {
        sprite,
        groups: std::array::from_fn(|_| BakedGroup {
            pivot: [0.0; 3],
            quads: Vec::new(),
        }),
    }
}

fn baked_models() -> &'static [BakedModel] {
    BAKED_MODELS.get_or_init(|| {
        MODEL_SPECS
            .iter()
            .map(|(slug, sprite)| {
                if let Some(bytes) = load_baked_bytes(slug) {
                    parse_baked_model(sprite, &bytes)
                } else {
                    empty_baked_model(sprite)
                }
            })
            .collect()
    })
}

/// Draw the real baked Bedrock 3D YSM character mesh + anime texture in `render_player_pre`.
/// Returns `true` if the texture sprite exists and the real 3D model was drawn (so vanilla Steve/Alex should be cancelled).
/// Returns `false` if the sprite is missing, leaving vanilla player + user's 4 fallback skins visible.
fn draw_ysm_real_model(
    draw: &mut dyn BlockEntityDrawContext,
    p_idx: usize,
    g_idx: usize,
    player: &mod_api::MobRender<'_>,
    crouching: bool,
) -> bool {
    let models = baked_models();
    let builtin = &models[p_idx % models.len()];

    // A hot-loaded model overrides the built-in one: parsed on the fly from its
    // downloaded .bin and textured through the single mod_image sprite slot.
    let hot_model = {
        let slug = HOT_SLUG.lock().ok().map(|g| g.clone()).unwrap_or_default();
        if slug.is_empty() {
            None
        } else {
            hotload::load_hot(&slug).map(|hm| parse_baked_model(hotload::sprite_name(), &hm.bin))
        }
    };
    let model: &BakedModel = hot_model.as_ref().unwrap_or(builtin);

    // Probe the sprite with the first available quad before cancelling vanilla
    let first_quad = model.groups.iter().find_map(|g| g.quads.first());
    if let Some(q) = first_quad {
        draw.push();
        draw.scale([0.0, 0.0, 0.0]);
        let sprite_ok = draw.quad(model.sprite, q.corners, q.uv16, 0xFFFFFF);
        draw.pop();
        if !sprite_ok {
            return false;
        }
    } else {
        return false;
    }

    let head_yaw = player.head_yaw;
    let head_pitch = player.pitch;
    let walk_pos = player.walk_position;
    let walk_speed = player.walk_speed.min(1.0);
    let on_ground = player.on_ground;
    let tint = if player.hurt_time > 0 { 0xFF8888 } else { 0xFFFFFF };

    let t = (draw.animation_time() as f32) * 0.10;
    let breath = t.sin();
    let crouch_damp = if crouching { 0.65 } else { 1.0 };
    let swing = (walk_pos * 0.6662).cos() * walk_speed * 42.0 * crouch_damp;
    let walk_bob = if on_ground {
        (walk_pos * 1.3324).sin().abs() * walk_speed * 0.045
    } else {
        0.0
    };
    let hip_y = ((model.groups[4].pivot[1] + model.groups[5].pivot[1]) * 0.5).clamp(0.55, 1.25);

    draw.push();
    // Root motion: walk bob + crouch drop + whole-body gestures
    if walk_bob > 0.0001 {
        draw.translate([0.0, walk_bob, 0.0]);
    }
    if crouching {
        draw.translate([0.0, -0.15, 0.0]);
    }
    match g_idx {
        2 => draw.translate([0.0, (t * 2.8).sin().abs() * 0.16, 0.0]),
        4 => draw.translate([0.0, -0.35, 0.0]),
        5 => {
            draw.translate([0.0, (t * 2.0).sin().abs() * 0.06, 0.0]);
            draw.rotate_about([0.0, 1.0, 0.0], (t * 40.0) % 360.0, [0.0, hip_y, 0.0]);
        }
        _ => {}
    }

    // Hierarchical UpperBody (Body + Head + Arms) pivots cleanly at hip_y
    draw.push();
    let upper_lean = if crouching {
        24.0
    } else if g_idx == 6 {
        18.0 + breath * 2.0
    } else {
        walk_speed * 4.5
    };
    if upper_lean.abs() > 0.01 {
        draw.rotate_about([1.0, 0.0, 0.0], upper_lean, [0.0, hip_y, 0.0]);
    }
    if walk_speed > 0.05 {
        draw.rotate_about([0.0, 1.0, 0.0], swing * 0.14, [0.0, hip_y, 0.0]);
    }

    // Group 0: Body / Torso / Skirt / Wings / Tails
    for q in &model.groups[0].quads {
        draw.quad(model.sprite, q.corners, q.uv16, tint);
    }

    // Group 1: Head (follows player head_yaw & pitch, compensates crouch lean)
    {
        let grp = &model.groups[1];
        draw.push();
        let comp_pitch = if crouching { -16.0 } else { 0.0 };
        draw.rotate_about([0.0, 1.0, 0.0], -head_yaw.clamp(-70.0, 70.0), grp.pivot);
        draw.rotate_about(
            [1.0, 0.0, 0.0],
            (head_pitch + comp_pitch).clamp(-55.0, 55.0),
            grp.pivot,
        );
        for q in &grp.quads {
            draw.quad(model.sprite, q.corners, q.uv16, tint);
        }
        draw.pop();
    }

    // Group 2: RightArm
    {
        let grp = &model.groups[2];
        draw.push();
        let air_lift = if !on_ground { -18.0 } else { 0.0 };
        let crouch_arm = if crouching { -14.0 } else { 0.0 };
        let arm_x = match g_idx {
            1 => -135.0 + (t * 3.5).sin() * 20.0,
            2 | 7 => -145.0 + breath * 10.0,
            3 => -55.0 + (t * 2.5).sin() * 12.0,
            _ => -swing * 0.85 + breath * 2.5 + air_lift + crouch_arm,
        };
        let arm_z = if g_idx == 1 {
            -22.0 + (t * 3.5).cos() * 14.0
        } else {
            -breath * 2.0 - if !on_ground { 10.0 } else { 0.0 }
        };
        draw.rotate_about([1.0, 0.0, 0.0], arm_x, grp.pivot);
        draw.rotate_about([0.0, 0.0, 1.0], arm_z, grp.pivot);
        for q in &grp.quads {
            draw.quad(model.sprite, q.corners, q.uv16, tint);
        }
        draw.pop();
    }

    // Group 3: LeftArm
    {
        let grp = &model.groups[3];
        draw.push();
        let air_lift = if !on_ground { -18.0 } else { 0.0 };
        let crouch_arm = if crouching { -14.0 } else { 0.0 };
        let arm_x = match g_idx {
            2 | 7 => -145.0 - breath * 10.0,
            3 => -45.0 - (t * 2.5).sin() * 10.0,
            _ => swing * 0.85 - breath * 2.5 + air_lift + crouch_arm,
        };
        let arm_z = breath * 2.0 + if !on_ground { 10.0 } else { 0.0 };
        draw.rotate_about([1.0, 0.0, 0.0], arm_x, grp.pivot);
        draw.rotate_about([0.0, 0.0, 1.0], arm_z, grp.pivot);
        for q in &grp.quads {
            draw.quad(model.sprite, q.corners, q.uv16, tint);
        }
        draw.pop();
    }

    // End UpperBody hierarchy
    draw.pop();

    // Group 4: RightLeg
    {
        let grp = &model.groups[4];
        draw.push();
        let leg_x = if g_idx == 4 {
            -75.0
        } else if !on_ground {
            16.0
        } else if crouching {
            swing * 0.7 - 12.0
        } else {
            swing
        };
        draw.rotate_about([1.0, 0.0, 0.0], leg_x, grp.pivot);
        for q in &grp.quads {
            draw.quad(model.sprite, q.corners, q.uv16, tint);
        }
        draw.pop();
    }

    // Group 5: LeftLeg
    {
        let grp = &model.groups[5];
        draw.push();
        let leg_x = if g_idx == 4 {
            -75.0
        } else if !on_ground {
            -14.0
        } else if crouching {
            -swing * 0.7 - 12.0
        } else {
            -swing
        };
        draw.rotate_about([1.0, 0.0, 0.0], leg_x, grp.pivot);
        for q in &grp.quads {
            draw.quad(model.sprite, q.corners, q.uv16, tint);
        }
        draw.pop();
    }

    draw.pop();
    true
}

#[derive(Default)]
struct YsmModelsMod;

#[mod_api::sunrise_mod]
impl Hooks for YsmModelsMod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let p_opts = PERSONAS.iter().map(|p| p.title.to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "persona",
            "ysm_models.setting.persona",
            p_opts,
            0,
        ));

        let g_opts = GESTURES.iter().map(|g| (*g).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "gesture",
            "ysm_models.setting.gesture",
            g_opts,
            0,
        ));

        let r_opts = REACH_CHOICES.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "reach",
            "ysm_models.setting.reach",
            r_opts,
            2,
        ));

        let b_opts = BOOL_CHOICES.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "armor_auto_hide",
            "ysm_models.setting.armor_auto_hide",
            b_opts.clone(),
            0,
        ));
        let _ = r.register_setting(SettingSpec::choice(
            "first_person_arm",
            "ysm_models.setting.first_person_arm",
            b_opts.clone(),
            0,
        ));
        let _ = r.register_setting(SettingSpec::choice(
            "hud_overlay",
            "ysm_models.setting.hud_overlay",
            b_opts.clone(),
            1,
        ));
        let _ = r.register_setting(SettingSpec::choice(
            "multiplayer_sync",
            "ysm_models.setting.multiplayer_sync",
            b_opts,
            0,
        ));

        r.register_key_mapping(
            KeyRegistration::new(KEY_CYCLE_MODEL, "key.keyboard.y")
                .label("key.ysm_models.cycle_model")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(KEY_CYCLE_GESTURE, "key.keyboard.g")
                .label("key.ysm_models.cycle_gesture")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(KEY_OPEN_GUI, "key.keyboard.o")
                .label("key.ysm_models.open_gui")
                .context(KeyConflictContext::InGame),
        );
        sync_state_to_disk(0, 0);
    }

    fn register_client_commands(
        &self,
        _: &mut dyn ClientHost,
        event: &mut RegisterClientCommandsEvent<'_>,
    ) {
        event.commands.push(Command::new("ysm"));
        event.commands.push(Command::new("ysm_next"));
        event.commands.push(Command::new("ysm_gesture"));
        event.commands.push(Command::new("ysm_play"));
        event.commands.push(Command::new("ysm_action"));
        event.commands.push(Command::new("ysm_anims"));
        event.commands.push(Command::new("ysm_list"));
        event.commands.push(Command::new("ysm_stop"));
        event.commands.push(Command::new("ysm_gui"));
        event.commands.push(Command::new("ysm_menu"));
        event.commands.push(Command::new("ysm_sources"));
        event.commands.push(Command::new("ysm_add_source"));
        event.commands.push(Command::new("ysm_token"));
        event.commands.push(Command::new("ysm_sync"));
        event.commands.push(Command::new("ysm_download"));
        event.commands.push(Command::new("ysm_hotlist"));
        event.commands.push(Command::new("ysm_fetch"));
        event.commands.push(Command::new("ysm_hot"));
        event.commands.push(Command::new("mc_hub"));
        event.commands.push(Command::new("mc_sync_all"));
    }

    fn command(&self, host: &mut dyn ClientHost, event: &mut CommandEvent<'_>) -> Flow {
        let cmd = event.command.trim();
        if cmd == "ysm" || cmd == "ysm status" {
            let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
            let g_idx = GESTURE_IDX.load(Ordering::Relaxed) as usize % GESTURES.len();
            let p = &PERSONAS[p_idx];
            let cfg = load_source_cfg();
            let _ = host.chat_line(Text::literal(format!(
                "§6[YSM 3D 状态]§r 当前模型: §e{}§r ({}骨骼/{}立方体) | 动作: §a{}§r | 素材源: §b{}§r | 按 §bY§r 切换模型, 按 §bG§r 切换动作",
                p.title, p.bones_count, p.cubes_count, GESTURES[g_idx], cfg.active
            )));
            return Flow::Cancel;
        } else if cmd == "ysm_next" || cmd == "ysm next" || cmd == "ysm model" {
            cycle_model(host);
            return Flow::Cancel;
        } else if cmd == "ysm_gesture" || cmd == "ysm gesture" {
            cycle_gesture(host);
            return Flow::Cancel;
        } else if cmd == "ysm_gui" || cmd == "ysm_menu" {
            let _ = gui::open_ysm_gui(host);
            return Flow::Cancel;
        } else if cmd == "ysm_stop" {
            NAMED_ANIM_IDX.store(u32::MAX, Ordering::Relaxed);
            GESTURE_IDX.store(0, Ordering::Relaxed);
            let _ = host.chat_line(Text::literal("§b[YSM 3D]§r 已恢复自动动作状态机 (idle/walk/run/sneak/ride)"));
            return Flow::Cancel;
        } else if cmd == "ysm_anims" || cmd == "ysm_list" || cmd.starts_with("ysm_anims ") || cmd.starts_with("ysm_list ") {
            let filter = cmd
                .strip_prefix("ysm_anims")
                .or_else(|| cmd.strip_prefix("ysm_list"))
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
            let ytrees = ytree_engine::get_ytree_models(&MODEL_SPECS);
            if let Some(Some(ytree)) = ytrees.get(p_idx % ytrees.len()) {
                let all = ytree.animation_names();
                let matched: Vec<String> = all
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| filter.is_empty() || n.to_ascii_lowercase().contains(&filter))
                    .take(40)
                    .map(|(i, n)| format!("{}:{}", i + 1, n))
                    .collect();
                let _ = host.chat_line(Text::literal(format!(
                    "§6[YSM 动画库]§r §e{}§r 共加载 §a{}§r 个动画 (匹配显示 {} 个): §f{}§r | 播放: §b/ysm_play <动画名或序号>§r | 停止: §b/ysm_stop§r",
                    PERSONAS[p_idx].short_title,
                    all.len(),
                    matched.len(),
                    matched.join(", ")
                )));
            }
            return Flow::Cancel;
        } else if let Some(rest) = cmd.strip_prefix("ysm_play ").or_else(|| cmd.strip_prefix("ysm_action ")) {
            let q = rest.trim();
            let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
            if q == "0" || q.eq_ignore_ascii_case("stop") || q.eq_ignore_ascii_case("auto") {
                NAMED_ANIM_IDX.store(u32::MAX, Ordering::Relaxed);
                GESTURE_IDX.store(0, Ordering::Relaxed);
                let _ = host.chat_line(Text::literal("§b[YSM 3D]§r 已恢复自动动作状态机"));
                return Flow::Cancel;
            }
            let ytrees = ytree_engine::get_ytree_models(&MODEL_SPECS);
            if let Some(Some(ytree)) = ytrees.get(p_idx % ytrees.len()) {
                if let Some((idx, name)) = ytree.find_animation(q) {
                    NAMED_ANIM_IDX.store(idx as u32, Ordering::Relaxed);
                    if let Ok(mut s) = NAMED_ANIM_LABEL.get_or_init(|| Mutex::new(String::new())).lock() {
                        *s = name.to_string();
                    }
                    let _ = host.chat_line(Text::literal(format!(
                        "§b[YSM 动画播放]§r §e{}§r -> §a#{}: {}§r (输入 §b/ysm_stop§r 恢复自动)",
                        PERSONAS[p_idx].short_title,
                        idx + 1,
                        name
                    )));
                    return Flow::Cancel;
                }
            }
            if let Ok(idx) = q.parse::<usize>() {
                NAMED_ANIM_IDX.store(u32::MAX, Ordering::Relaxed);
                let g = idx % GESTURES.len();
                GESTURE_IDX.store(g as u32, Ordering::Relaxed);
                host.set_setting(ID, "gesture", &g.to_string());
                sync_state_to_disk(p_idx, g);
                let _ = host.chat_line(Text::literal(format!(
                    "§b[YSM 3D]§r 切换动作 ({}/{}): §a{}§r -> §f{}",
                    g + 1,
                    GESTURES.len(),
                    PERSONAS[p_idx].short_title,
                    GESTURES[g]
                )));
            } else {
                let _ = host.chat_line(Text::literal(format!(
                    "§c[YSM 动画]§r 未找到匹配 \"{q}\" 的动画，请先输入 §b/ysm_anims§r 查看当前模型全部动画"
                )));
            }
            return Flow::Cancel;
        } else if cmd == "ysm_sources" {
            let cfg = load_source_cfg();
            let _ = host.chat_line(Text::literal(format!(
                "§6[YSM 网络源]§r 当前激活源: §b{}§r | 积分Token: §a{}§r | 全部源({})：{}",
                cfg.active,
                if cfg.token.is_empty() { "未绑定 (请用 /ysm_token <lkr_xxx>)" } else { &cfg.token },
                cfg.sources.len(),
                cfg.sources.join(", ")
            )));
            return Flow::Cancel;
        } else if let Some(rest) = cmd.strip_prefix("ysm_add_source ") {
            let url = rest.trim().trim_end_matches('/').to_string();
            if !url.starts_with("http://") && !url.starts_with("https://") {
                let _ = host.chat_line(Text::literal("§c[YSM 网络源]§r 源地址必须以 http:// 或 https:// 开头"));
                return Flow::Cancel;
            }
            let mut cfg = load_source_cfg();
            if !cfg.sources.contains(&url) {
                cfg.sources.push(url.clone());
            }
            cfg.active = url.clone();
            save_source_cfg(&cfg);
            let _ = host.chat_line(Text::literal(format!("§a[YSM 网络源]§r 已添加并切换默认源 -> §b{url}§r")));
            return Flow::Cancel;
        } else if let Some(rest) = cmd.strip_prefix("ysm_token ") {
            let tok = rest.trim().to_string();
            let mut cfg = load_source_cfg();
            cfg.token = tok.clone();
            save_source_cfg(&cfg);
            let _ = host.chat_line(Text::literal(format!("§a[YSM 积分绑定]§r 已保存积分 Token: §e{tok}§r")));
            return Flow::Cancel;
        } else if cmd == "ysm_sync" {
            let cfg = load_source_cfg();
            match hotload::fetch_manifest(&cfg.active, &cfg.token) {
                Ok(items) => {
                    hotload::save_catalog(&items);
                    if let Ok(mut c) = HOT_CATALOG.lock() { *c = items.clone(); }
                    let preview: Vec<String> = items.iter().take(12).map(|(s, _, _, _)| s.clone()).collect();
                    let _ = host.chat_line(Text::literal(format!(
                        "§6[YSM 云端目录]§r 从 §b{}§r 同步到 §e{}§r 款 3D 模型: {} ... | 下载: /ysm_fetch <slug> | 列表: /ysm_hotlist",
                        cfg.active, items.len(), preview.join(", ")
                    )));
                }
                Err(e) => {
                    let _ = host.chat_line(Text::literal(format!("§c[YSM 云端同步失败]§r {e}")));
                }
            }
            return Flow::Cancel;
        } else if cmd == "ysm_hotlist" {
            let items = hotload::load_catalog();
            let hot_slug = HOT_SLUG.lock().ok().map(|g| g.clone()).unwrap_or_default();
            let preview: Vec<String> = items.iter().take(20).map(|(s, t, _, _)| format!("{} ({})", s, t)).collect();
            let _ = host.chat_line(Text::literal(format!(
                "§6[YSM 热加载目录]§r 共 §e{}§r 款 | 当前热模型: §a{}§r\n{}",
                items.len(), if hot_slug.is_empty() { "无(内置角色)" } else { &hot_slug },
                preview.join("\n")
            )));
            return Flow::Cancel;
        } else if let Some(rest) = cmd.strip_prefix("ysm_fetch ") {
            let slug = rest.trim().trim_start_matches("ysm:").to_string();
            let cfg = load_source_cfg();
            match hotload::download(&cfg.active, &cfg.token, &slug) {
                Ok(hm) => {
                    let _ = host.chat_line(Text::literal(format!(
                        "§a[YSM 热下载成功]§r §e{slug}§r ({}) 已就绪，输入 §b/ysm_hot {slug}§r 立即换装",
                        hm.title
                    )));
                }
                Err(e) => {
                    let _ = host.chat_line(Text::literal(format!("§c[YSM 热下载失败]§r {slug}: {e}")));
                }
            }
            return Flow::Cancel;
        } else if let Some(rest) = cmd.strip_prefix("ysm_hot ") {
            let slug = rest.trim().trim_start_matches("ysm:").to_string();
            if slug.eq_ignore_ascii_case("off") || slug == "0" {
                if let Ok(mut g) = HOT_SLUG.lock() { g.clear(); }
                HOT_UPLOAD_PENDING.store(false, Ordering::Relaxed);
                let _ = host.chat_line(Text::literal("§b[YSM 热加载]§r 已切回内置角色"));
            } else {
                match hotload::load_hot(&slug) {
                    Some(_) => {
                        if let Ok(mut g) = HOT_SLUG.lock() { *g = slug.clone(); }
                        HOT_UPLOAD_PENDING.store(true, Ordering::Relaxed);
                        let _ = host.chat_line(Text::literal(format!(
                            "§a[YSM 热加载]§r 已即时切换 -> §e{slug}§r (贴图将在下一帧上传)"
                        )));
                    }
                    None => {
                        let _ = host.chat_line(Text::literal(format!("§c[YSM 热加载]§r 未下载 {slug}，先 /ysm_fetch {slug}")));
                    }
                }
            }
            return Flow::Cancel;
        } else if cmd == "mc_hub" {
            let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
            let g_idx = GESTURE_IDX.load(Ordering::Relaxed) as usize % GESTURES.len();
            let p = &PERSONAS[p_idx];
            let cfg = load_source_cfg();
            let schem_active = fs::read_to_string("schematics/active_target.json")
                .ok()
                .and_then(|s| s.split("\"name\": \"").nth(1).and_then(|r| r.split('"').next()).map(String::from))
                .unwrap_or_else(|| "未锚定蓝图 (按 M 锚定)".to_string());
            let _ = host.chat_line(Text::literal(format!(
                "§d[MC Horizon 三模组协同总控]§r\n  §6• YSM 3D§r: {} | 动作: {}\n  §b• Litematica§r: {}\n  §c• Replay 云录制§r: R 开关 / B 打点 (夸克自动同步)\n  §a• 统一云源§r: {} | 积分Token: {}",
                p.short_title,
                GESTURES[g_idx],
                schem_active,
                cfg.active,
                if cfg.token.is_empty() { "未绑定 (用 /ysm_token 一次绑定全模组共享)" } else { "已绑定 (全模组共享)" }
            )));
            return Flow::Cancel;
        } else if cmd == "mc_sync_all" {
            let cfg = load_source_cfg();
            let ysm_url = format!("{}/api/v1/manifest.json?kind=ysm&limit=8&token={}", cfg.active, cfg.token);
            let sch_url = format!("{}/api/v1/manifest.json?kind=schematic&limit=8&token={}", cfg.active, cfg.token);
            let y_ok = http_get_text(&ysm_url).is_ok();
            let s_ok = http_get_text(&sch_url).is_ok();
            let _ = host.chat_line(Text::literal(format!(
                "§d[MC Horizon 协同同步]§r 源: §b{}§r | YSM 目录: {} | 投影目录: {} | 详情请用 /ysm_sync 或 /schematics_sync",
                cfg.active,
                if y_ok { "§a在线 (1208款)§r" } else { "§c离线§r" },
                if s_ok { "§a在线 (1058份)§r" } else { "§c离线§r" }
            )));
            return Flow::Cancel;
        } else if let Some(rest) = cmd.strip_prefix("ysm_download ") {
            let slug = rest.trim();
            let cfg = load_source_cfg();
            let dl_url = format!("{}/api/v1/assets/ysm/{slug}/download?token={}", cfg.active, cfg.token);
            let dest = PathBuf::from("horizon-native-mods/ysm_models/baked_models").join(format!("{slug}.bin"));
            match http_download_file(&dl_url, &dest) {
                Ok(_) => {
                    let tex_url = format!("{}/api/v1/assets/ysm/{slug}/texture", cfg.active);
                    let tex_dest = PathBuf::from("horizon-native-mods/ysm_models/assets/ysm_models/textures/entity").join(format!("{slug}.png"));
                    let _ = http_download_file(&tex_url, &tex_dest);
                    let _ = host.chat_line(Text::literal(format!(
                        "§a[YSM 积分下载成功]§r 已从 §b{}§r 下载模型 §e{slug}§r（含贴图）至本地！重启客户端即可加载最新网格。",
                        cfg.active
                    )));
                }
                Err(e) => {
                    let _ = host.chat_line(Text::literal(format!(
                        "§c[YSM 积分下载被拒]§r {e} | 前往 §b{}§r 登录/签到/爱发电充值积分",
                        cfg.active
                    )));
                }
            }
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(Command::new("ysm_server"));
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if event.command.literals.first().map(String::as_str) == Some("ysm_server") {
            for p in host.players().server_players() {
                let _ = p.tell(
                    host,
                    Text::literal(
                        "§6[YSM 3D Server]§r 服务端 8 款 alltheysm.st 3D 角色模型与 4 款保底皮肤 (Claude/DeepSeek/ChatGPT/Gemini) 已同步就绪！",
                    ),
                );
            }
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn player_logged_in(&self, _host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        let idx = resolve_player_persona(&event.name, false);
        sync_state_to_disk(idx, 0);
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, _event: &mut SettingChangedEvent<'_>) {
        let p_idx = host.setting_choice("persona").unwrap_or(0) as usize % PERSONAS.len();
        let g_idx = host.setting_choice("gesture").unwrap_or(0) as usize % GESTURES.len();
        PERSONA_IDX.store(p_idx as u32, Ordering::Relaxed);
        GESTURE_IDX.store(g_idx as u32, Ordering::Relaxed);
        ARMOR_AUTO_HIDE.store(host.setting_choice("armor_auto_hide").unwrap_or(0), Ordering::Relaxed);
        FIRST_PERSON_ARM.store(host.setting_choice("first_person_arm").unwrap_or(0), Ordering::Relaxed);
        HUD_OVERLAY.store(host.setting_choice("hud_overlay").unwrap_or(0), Ordering::Relaxed);
        sync_state_to_disk(p_idx, g_idx);
    }

    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        if !host.playing() {
            IS_CROUCHING.store(false, Ordering::Relaxed);
            DOUBLE_TAP_SPRINT.store(false, Ordering::Relaxed);
            return;
        }
        if event.key == "key.keyboard.left.shift" || event.key == "key.keyboard.right.shift" {
            IS_CROUCHING.store(event.action != KeyAction::Release, Ordering::Relaxed);
        }
        if event.key == "key.keyboard.w" {
            if event.action == KeyAction::Press {
                let now = TICK_COUNTER.load(Ordering::Relaxed);
                let prev = LAST_W_PRESS_TICK.swap(now, Ordering::Relaxed);
                if prev > 0 && now.saturating_sub(prev) <= 8 {
                    DOUBLE_TAP_SPRINT.store(true, Ordering::Relaxed);
                }
            } else if event.action == KeyAction::Release {
                DOUBLE_TAP_SPRINT.store(false, Ordering::Relaxed);
            }
        }
        if event.action != KeyAction::Press {
            return;
        }
        // Handle Y/G in input_key ONLY if consume_click hasn't already drained it, or for horizon-ctl direct key taps
        if event.key == "key.keyboard.f9" {
            let next = if HUD_OVERLAY.load(Ordering::Relaxed) == 0 { 1 } else { 0 };
            HUD_OVERLAY.store(next, Ordering::Relaxed);
            let _ = host.chat_line(Text::literal(if next == 0 {
                "§b[Lilyco HUD]§r 调试信息已显示 §7(按 F9 再次隐藏)".to_string()
            } else {
                "§b[Lilyco HUD]§r 调试信息已隐藏 §7(按 F9 重新显示)".to_string()
            }));
        } else if event.key == "key.keyboard.y" && !host.is_down(ID, KEY_CYCLE_MODEL) {
            cycle_model(host);
        } else if event.key == "key.keyboard.g" && !host.is_down(ID, KEY_CYCLE_GESTURE) {
            cycle_gesture(host);
        } else if event.key == "key.keyboard.o" && !host.is_down(ID, KEY_OPEN_GUI) {
            let _ = gui::open_ysm_gui(host);
        }
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        // Upload the hot-loaded model's texture into the shared mod_image slot once,
        // right after /ysm_hot marks it pending. Render then uses "horizon/modimage".
        if HOT_UPLOAD_PENDING.swap(false, Ordering::Relaxed) {
            let slug = HOT_SLUG.lock().ok().map(|g| g.clone()).unwrap_or_default();
            if !slug.is_empty() {
                match hotload::load_hot(&slug).ok_or_else(|| "模型未就绪".to_string()).and_then(|hm| hotload::rgba_for_upload(&hm)) {
                    Ok((w, h, rgba)) => {
                        let id = mod_api::Identifier::parse(hotload::sprite_name())
                            .map_err(|e| format!("{e:?}"));
                        let res = id.and_then(|id| {
                            host.create_dynamic_texture(&id, w, h, &rgba)
                                .map_err(|e| format!("{e:?}"))
                        });
                        let _ = host.chat_line(Text::literal(match &res {
                            Ok(()) => format!("§b[YSM 热贴图]§r {w}x{h} 上传成功"),
                            Err(e) => format!("§c[YSM 热贴图失败]§r {e}"),
                        }));
                    }
                    Err(e) => {
                        let _ = host.chat_line(Text::literal(format!("§c[YSM 热贴图失败]§r {e}")));
                    }
                }
            }
        }
        let tick = TICK_COUNTER.fetch_add(1, Ordering::Relaxed);
        // Sample 100% client-predicted movement/posture state (works in both singleplayer & multiplayer!)
        if let Some(player) = host.local_player() {
            let entity = player.entity();
            let living = host.living(entity);

            let depth_mm = entity
                .fluid_heights(host)
                .and_then(|heights| {
                    heights
                        .into_iter()
                        .find(|(k, _)| k.contains("water"))
                        .map(|(_, h)| (h.max(0.0) * 1000.0) as u32)
                })
                .unwrap_or(0);
            LOCAL_WATER_DEPTH_MM.store(depth_mm, Ordering::Relaxed);
            let eye_water = entity
                .eye_fluid(host)
                .flatten()
                .is_some_and(|f| f.contains("water"));
            LOCAL_EYE_IN_WATER.store(eye_water, Ordering::Relaxed);

            // Eye height (mm): standing ~1620, crouching ~1270, swimming/elytra ~400, sleeping ~200
            let eye_h_mm = entity
                .eye_height(host)
                .map(|h| (h.max(0.0) * 1000.0) as u32)
                .unwrap_or(1620);
            LOCAL_EYE_HEIGHT_MM.store(eye_h_mm, Ordering::Relaxed);

            let flying = player.abilities(host).is_some_and(|a| a.flying);
            LOCAL_CREATIVE_FLYING.store(flying, Ordering::Relaxed);

            let climbing = living.and_then(|l| l.is_climbing(host)).unwrap_or(false);
            LOCAL_CLIMBING.store(climbing, Ordering::Relaxed);

            let vy_mm = entity
                .velocity(host)
                .map(|v| (v[1] * 1000.0).clamp(-10000.0, 10000.0) as i32)
                .unwrap_or(0);
            LOCAL_VEL_Y_MM.store(vy_mm, Ordering::Relaxed);

            let blocking = living.and_then(|l| l.is_blocking(host)).unwrap_or(false);
            LOCAL_BLOCKING.store(blocking, Ordering::Relaxed);
        } else {
            LOCAL_WATER_DEPTH_MM.store(0, Ordering::Relaxed);
            LOCAL_EYE_IN_WATER.store(false, Ordering::Relaxed);
            LOCAL_EYE_HEIGHT_MM.store(1620, Ordering::Relaxed);
            LOCAL_CREATIVE_FLYING.store(false, Ordering::Relaxed);
            LOCAL_CLIMBING.store(false, Ordering::Relaxed);
            LOCAL_VEL_Y_MM.store(0, Ordering::Relaxed);
            LOCAL_BLOCKING.store(false, Ordering::Relaxed);
        }
        if tick % 10 == 0 {
            if let Ok(raw) = fs::read_to_string("horizon-mods/.horizon-mod-data/ysm_models/web_switch.txt") {
                let mut parts = raw.split_whitespace();
                if let (Some(p_s), Some(g_s), Some(n_s)) = (parts.next(), parts.next(), parts.next()) {
                    if let (Ok(p_idx), Ok(g_idx), Ok(nonce)) = (p_s.parse::<usize>(), g_s.parse::<usize>(), n_s.parse::<u32>()) {
                        if nonce != 0 && nonce != LAST_WEB_NONCE.load(Ordering::Relaxed) {
                            LAST_WEB_NONCE.store(nonce, Ordering::Relaxed);
                            // A hot-loaded remote model takes priority over the web GUI's
                            // built-in persona switch; ignore web switches while it is active.
                            if HOT_SLUG.lock().ok().map(|g| !g.is_empty()).unwrap_or(false) {
                                return;
                            }
                            let p_clamped = p_idx % PERSONAS.len();
                            let g_clamped = g_idx % GESTURES.len();
                            PERSONA_IDX.store(p_clamped as u32, Ordering::Relaxed);
                            GESTURE_IDX.store(g_clamped as u32, Ordering::Relaxed);
                            sync_state_to_disk(p_clamped, g_clamped);
                            let _ = host.chat_line(Text::literal(format!(
                                "§b[Lilyco Web GUI]§r 已实时切换 YSM 3D 模型 -> §e{}",
                                PERSONAS[p_clamped].title
                            )));
                        }
                    }
                }
            }
        }
        while host.consume_click(ID, KEY_CYCLE_MODEL) {
            cycle_model(host);
        }
        while host.consume_click(ID, KEY_CYCLE_GESTURE) {
            cycle_gesture(host);
        }
        while host.consume_click(ID, KEY_OPEN_GUI) {
            let _ = gui::open_ysm_gui(host);
        }
    }

    fn get_arm_pose(
        &self,
        _host: &dyn ClientHost,
        event: &mut GetArmPoseEvent<'_>,
    ) -> Option<ArmPose> {
        let now = TICK_COUNTER.load(Ordering::Relaxed);
        let main_cat = event
            .humanoid
            .main_hand
            .map(|s| {
                mod_api::__private::item_key_id(s.item())
                    .map(classify_item_id)
                    .unwrap_or(ytree_engine::HeldCat::Item)
            })
            .unwrap_or(ytree_engine::HeldCat::Empty);
        let off_cat = event
            .humanoid
            .off_hand
            .map(|s| {
                mod_api::__private::item_key_id(s.item())
                    .map(classify_item_id)
                    .unwrap_or(ytree_engine::HeldCat::Item)
            })
            .unwrap_or(ytree_engine::HeldCat::Empty);
        let (main_use, off_use) = match event.humanoid.use_hand {
            Some(Hand::Main) => (map_use_anim(event.humanoid.use_animation), ytree_engine::UseKind::None),
            Some(Hand::Off) => (ytree_engine::UseKind::None, map_use_anim(event.humanoid.use_animation)),
            None => (ytree_engine::UseKind::None, ytree_engine::UseKind::None),
        };
        let entry = EntityHeld {
            main_cat,
            off_cat,
            main_use,
            off_use,
            main_stack: event.humanoid.main_hand.cloned(),
            off_stack: event.humanoid.off_hand.cloned(),
            tick: now,
        };
        let map = ENTITY_HELD.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut guard) = map.lock() {
            guard.insert(event.entity_id, entry.clone());
        }
        if let Ok(mut local_guard) = LOCAL_ENTITY_HELD.get_or_init(|| Mutex::new(None)).lock() {
            *local_guard = Some(entry);
        }
        None
    }

    fn server_tick(&self, host: &mut dyn ServerHost, _: &mut ServerTickEvent) {
        // Optional singleplayer/integrated-server boost (pure client works without this!)
        let mut any_riding = false;
        let mut any_boat = false;
        let mut any_sleeping = false;
        let mut any_in_water = false;
        let mut any_swimming = false;
        for p in host.players().server_players() {
            let ent = p.entity();
            if ent.is_passenger(host).unwrap_or(false) {
                any_riding = true;
                if let Some(veh) = ent.vehicle(host) {
                    if veh.name(host).is_some_and(|n| {
                        let l = n.to_ascii_lowercase();
                        l.contains("boat") || l.contains("raft")
                    }) {
                        any_boat = true;
                    }
                }
            }
            if ent.is_in_water(host).unwrap_or(false) {
                any_in_water = true;
            }
            match ent.pose(host) {
                Some("swimming") => any_swimming = true,
                Some("sleeping") => any_sleeping = true,
                _ => {}
            }
        }
        LOCAL_RIDING.store(any_riding, Ordering::Relaxed);
        LOCAL_IN_BOAT.store(any_boat, Ordering::Relaxed);
        LOCAL_SLEEPING_SRV.store(any_sleeping, Ordering::Relaxed);
        LOCAL_IN_WATER.store(any_in_water, Ordering::Relaxed);
        LOCAL_SWIMMING.store(any_swimming, Ordering::Relaxed);
    }

    fn render_player_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut RenderPlayerPreEvent<'_>,
    ) -> Flow {
        if !host.on(ID) {
            return Flow::Continue;
        }
        let p_idx = resolve_player_persona(event.who.name, event.who.local);
        let g_idx = GESTURE_IDX.load(Ordering::Relaxed) as usize % GESTURES.len();
        let riding = event.who.local && LOCAL_RIDING.load(Ordering::Relaxed);
        let in_boat = event.who.local && LOCAL_IN_BOAT.load(Ordering::Relaxed);
        let crouching = event.who.local
            && (mapping_down(host, "key.sneak")
                || host.is_key_down("key.keyboard.left.shift")
                || host.is_key_down("key.keyboard.right.shift")
                || IS_CROUCHING.load(Ordering::Relaxed));
        let moving = event.who.local
            && (mapping_down(host, "key.forward")
                || mapping_down(host, "key.back")
                || mapping_down(host, "key.left")
                || mapping_down(host, "key.right")
                || host.is_key_down("key.keyboard.w")
                || host.is_key_down("key.keyboard.s")
                || host.is_key_down("key.keyboard.a")
                || host.is_key_down("key.keyboard.d"));
        let sprinting = event.who.local
            && (mapping_down(host, "key.sprint")
                || host.is_key_down("key.keyboard.left.control")
                || DOUBLE_TAP_SPRINT.load(Ordering::Relaxed));
        let jumping = event.who.local
            && (mapping_down(host, "key.jump") || host.is_key_down("key.keyboard.space"));
        let water_depth = (LOCAL_WATER_DEPTH_MM.load(Ordering::Relaxed) as f32) * 0.001;
        let eye_in_water = LOCAL_EYE_IN_WATER.load(Ordering::Relaxed);
        let eye_height = (LOCAL_EYE_HEIGHT_MM.load(Ordering::Relaxed) as f32) * 0.001;
        let creative_flying = event.who.local && LOCAL_CREATIVE_FLYING.load(Ordering::Relaxed);
        let climbing = event.who.local && LOCAL_CLIMBING.load(Ordering::Relaxed) && !creative_flying && !riding;
        let climb_vy = (LOCAL_VEL_Y_MM.load(Ordering::Relaxed) as f32) * 0.001;
        let server_in_water = LOCAL_IN_WATER.load(Ordering::Relaxed);
        let server_swimming = LOCAL_SWIMMING.load(Ordering::Relaxed);
        let sleeping = event.who.local
            && (LOCAL_SLEEPING_SRV.load(Ordering::Relaxed) || eye_height <= 0.30);
        let in_water = event.who.local
            && (server_swimming
                || eye_in_water
                || water_depth > 0.35
                || (server_in_water && !event.player.on_ground));
        let swimming = event.who.local
            && (server_swimming
                || eye_in_water
                || (in_water && (moving || eye_height < 0.75)));
        let elytra_flying = event.who.local
            && !event.player.on_ground
            && !creative_flying
            && !in_water
            && !sleeping
            && !riding
            && eye_height < 0.75;
        let mut main_swinging = event.who.local
            && (mapping_down(host, "key.attack")
                || host.is_key_down("key.mouse.left")
                || LOCAL_HAND_SWING.swap(false, Ordering::Relaxed));
        let off_swinging = event.who.local && LOCAL_OFF_SWING.swap(false, Ordering::Relaxed);

        // Resolve mainhand + offhand held items & item-use state from `get_arm_pose`.
        let now_tick = TICK_COUNTER.load(Ordering::Relaxed);
        let held_opt = ENTITY_HELD
            .get()
            .and_then(|m| m.lock().ok())
            .and_then(|g| g.get(&event.player.id).cloned())
            .filter(|h| now_tick.saturating_sub(h.tick) <= 4)
            .or_else(|| {
                if event.who.local {
                    LOCAL_ENTITY_HELD
                        .get()
                        .and_then(|m| m.lock().ok())
                        .and_then(|g| g.clone())
                        .filter(|h| now_tick.saturating_sub(h.tick) <= 4)
                } else {
                    None
                }
            });
        let (mut main_cat, mut off_cat, mut main_use, mut off_use, main_stack, off_stack) = held_opt
            .map(|h| (h.main_cat, h.off_cat, h.main_use, h.off_use, h.main_stack, h.off_stack))
            .unwrap_or((
                ytree_engine::HeldCat::Empty,
                ytree_engine::HeldCat::Empty,
                ytree_engine::UseKind::None,
                ytree_engine::UseKind::None,
                None,
                None,
            ));
        // yuqian has no idle bow/crossbow hold animation (only a firing pose);
        // suppress bow/crossbow use/held states so the character never gets stuck aiming.
        {
            let cur = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
            if cur < PERSONAS.len() && PERSONAS[cur].id == "yuqian" {
                if main_use == ytree_engine::UseKind::Bow || main_use == ytree_engine::UseKind::Crossbow {
                    main_use = ytree_engine::UseKind::None;
                }
                if off_use == ytree_engine::UseKind::Bow || off_use == ytree_engine::UseKind::Crossbow {
                    off_use = ytree_engine::UseKind::None;
                }
                if main_cat == ytree_engine::HeldCat::Bow || main_cat == ytree_engine::HeldCat::Crossbow {
                    main_cat = ytree_engine::HeldCat::Item;
                }
                if off_cat == ytree_engine::HeldCat::Bow || off_cat == ytree_engine::HeldCat::Crossbow {
                    off_cat = ytree_engine::HeldCat::Item;
                }
            }
        }
        if event.who.local {
            let right_down = mapping_down(host, "key.use") || host.is_key_down("key.mouse.right");
            if main_cat == ytree_engine::HeldCat::Fishing && right_down {
                main_swinging = true;
            }
            if LOCAL_BLOCKING.load(Ordering::Relaxed)
                && main_use != ytree_engine::UseKind::Block
                && off_use != ytree_engine::UseKind::Block
            {
                if off_cat == ytree_engine::HeldCat::Shield {
                    off_use = ytree_engine::UseKind::Block;
                } else if main_cat == ytree_engine::HeldCat::Shield {
                    main_use = ytree_engine::UseKind::Block;
                }
            }
            if main_use == ytree_engine::UseKind::None && off_use == ytree_engine::UseKind::None {
                if let Some(iu) = host.local_item_use() {
                    let is_off = iu.hand == Hand::Off;
                    let cat = if is_off { off_cat } else { main_cat };
                    let kind = match cat {
                        ytree_engine::HeldCat::Bow => ytree_engine::UseKind::Bow,
                        ytree_engine::HeldCat::Crossbow => ytree_engine::UseKind::Crossbow,
                        ytree_engine::HeldCat::Shield => ytree_engine::UseKind::Block,
                        ytree_engine::HeldCat::Spear => ytree_engine::UseKind::Spear,
                        _ => ytree_engine::UseKind::Eat,
                    };
                    if is_off {
                        off_use = kind;
                    } else {
                        main_use = kind;
                    }
                } else if right_down {
                    match main_cat {
                        ytree_engine::HeldCat::Bow => main_use = ytree_engine::UseKind::Bow,
                        ytree_engine::HeldCat::Crossbow => main_use = ytree_engine::UseKind::Crossbow,
                        ytree_engine::HeldCat::Shield => main_use = ytree_engine::UseKind::Block,
                        ytree_engine::HeldCat::Spear => main_use = ytree_engine::UseKind::Spear,
                        _ if off_cat == ytree_engine::HeldCat::Shield => {
                            off_use = ytree_engine::UseKind::Block;
                        }
                        _ => {}
                    }
                }
            }
            // 空手或普通物品绝不带 use 姿势；无条件清除：老世界存档会残留 use 状态，
            // 使 local_item_use() 持续为 Some，条件清理会漏掉，导致空手卡在拉弓姿势。
            if matches!(
                main_cat,
                ytree_engine::HeldCat::Empty | ytree_engine::HeldCat::Item
            ) {
                main_use = ytree_engine::UseKind::None;
            }
            if matches!(
                off_cat,
                ytree_engine::HeldCat::Empty | ytree_engine::HeldCat::Item
            ) {
                off_use = ytree_engine::UseKind::None;
            }
            LOCAL_HELD_CAT.store(main_cat as u32, Ordering::Relaxed);
            LOCAL_USE_KIND.store(
                if main_use != ytree_engine::UseKind::None {
                    main_use as u32
                } else {
                    off_use as u32
                },
                Ordering::Relaxed,
            );
        }

        let anim_state = ytree_engine::PlayerAnimState {
            local: event.who.local,
            riding,
            in_boat,
            sleeping,
            elytra_flying,
            creative_flying,
            climbing,
            climb_vy,
            in_water,
            swimming,
            crouching,
            moving,
            sprinting,
            jumping,
            main_swinging,
            off_swinging,
            main_cat,
            off_cat,
            main_use,
            off_use,
        };

        let ytrees = ytree_engine::get_ytree_models(&MODEL_SPECS);
        let named_raw = NAMED_ANIM_IDX.load(Ordering::Relaxed);
        let named_idx = if event.who.local && named_raw != u32::MAX {
            Some(named_raw as usize)
        } else {
            None
        };
        let hot_active = HOT_SLUG.lock().ok().map(|g| !g.is_empty()).unwrap_or(false);
        let drew = if !hot_active && let Some(Some(ytree)) = ytrees.get(p_idx % ytrees.len()) {
            let (ok, state) = ytree_engine::draw_ytree_model(
                event.draw,
                ytree,
                g_idx,
                named_idx,
                event.player,
                &anim_state,
                main_stack.as_ref(),
                off_stack.as_ref(),
            );
            if event.who.local {
                LIVE_STATE_IDX.store(state_name_to_idx(state), Ordering::Relaxed);
            }
            ok
        } else {
            draw_ysm_real_model(event.draw, p_idx, g_idx, event.player, crouching)
        };
        if drew {
            event.cancelled = true;
        }
        Flow::Continue
    }

    fn render_arm(&self, host: &dyn ClientHost, event: &mut RenderArmEvent<'_>) -> Flow {
        if !host.on(ID) || FIRST_PERSON_ARM.load(Ordering::Relaxed) != 0 {
            return Flow::Continue;
        }
        let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
        let p = &PERSONAS[p_idx];
        let face_wool = [Some(CubeFace::new("block/white_concrete")); 6];
        let face_glow = [Some(CubeFace::new("block/sea_lantern")); 6];
        // Draw custom 3D YSM cuff & character wristband ribbon around the first-person arm
        event.draw.push();
        event.draw.cube([-3.5, 1.5, -2.5], [1.5, 4.5, 2.5], face_wool, p.primary_rgb);
        event.draw.cube([-3.7, 3.8, -2.7], [1.7, 4.8, 2.7], face_glow, p.accent_rgb);
        event.draw.pop();
        Flow::Continue
    }

    fn render_hand(&self, _host: &dyn ClientHost, event: &mut RenderHandEvent<'_, '_>) -> Flow {
        if event.render.hand == Hand::Main {
            if event.render.swing > 0.02 {
                LOCAL_HAND_SWING.store(true, Ordering::Relaxed);
            }
            let cat = event
                .render
                .stack
                .and_then(|s| mod_api::__private::item_key_id(s.item()))
                .map(classify_item_id)
                .unwrap_or(ytree_engine::HeldCat::Empty);
            LOCAL_HELD_CAT.store(cat as u32, Ordering::Relaxed);
        } else if event.render.hand == Hand::Off && event.render.swing > 0.02 {
            LOCAL_OFF_SWING.store(true, Ordering::Relaxed);
        }
        if FIRST_PERSON_ARM.load(Ordering::Relaxed) == 0 {
            let p = &PERSONAS[PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len()];
            let t = (TICK_COUNTER.load(Ordering::Relaxed) as f32) * 0.08;
            let breath = t.sin() * 0.008;
            event.render.translation[0] += p.hand_tint_offset[0];
            event.render.translation[1] += p.hand_tint_offset[1] + breath;
            event.render.translation[2] += p.hand_tint_offset[2];
        }
        Flow::Continue
    }

    fn camera_shake(&self, _: &dyn ClientHost, event: &mut CameraShakeEvent<'_>) {
        let g = GESTURE_IDX.load(Ordering::Relaxed) as usize % GESTURES.len();
        if g == 2 {
            let t = (TICK_COUNTER.load(Ordering::Relaxed) as f32) * 0.35;
            event.shake.offset[1] += t.sin().abs() * 0.03;
        }
    }

    fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
        if !host.on(ID) || HUD_OVERLAY.load(Ordering::Relaxed) != 0 {
            return;
        }
        let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
        let g_idx = GESTURE_IDX.load(Ordering::Relaxed) as usize % GESTURES.len();
        let p = &PERSONAS[p_idx];
        let live_state = LIVE_STATE_NAMES
            .get(LIVE_STATE_IDX.load(Ordering::Relaxed) as usize)
            .copied()
            .unwrap_or("idle");
        let held_cat_hud = ytree_engine::HeldCat::from_u8(LOCAL_HELD_CAT.load(Ordering::Relaxed) as u8);
        let use_kind_hud = LOCAL_USE_KIND.load(Ordering::Relaxed);
        let held_label = match use_kind_hud {
            1 => "进食",
            2 => "持弓(拉弓)",
            3 => "盾牌(格挡)",
            4 => "饮用",
            5 => "望远镜",
            6 => "长兵",
            7 => "持弩",
            8 => "刷子",
            9 => "羊角号",
            _ => held_cat_hud.label(),
        };
        let named_raw = NAMED_ANIM_IDX.load(Ordering::Relaxed);
        let custom_label = if named_raw != u32::MAX {
            NAMED_ANIM_LABEL
                .get()
                .and_then(|m| m.lock().ok())
                .map(|s| s.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let line1 = format!(
            "[YSM 3D] {} ({}骨/{}块) [O预览 / Y切换]",
            p.short_title, p.bones_count, p.cubes_count
        );
        let line2 = if !custom_label.is_empty() {
            format!("播放动画: {} | 手持: {} [/ysm_stop]", custom_label, held_label)
        } else {
            format!("实时: {} | 手持: {} | 姿势: {} [G切换]", live_state, held_label, GESTURES[g_idx])
        };
        let draw = &mut *event.draw;
        let w = draw.text_width(&line1).max(draw.text_width(&line2)) + 8.0;
        let h = 24.0;
        let (x, y) = event.corners.place(Corner::TopLeft, w, h);
        draw.fill([x, y, w, h], 0x8810_141E);
        draw.fill([x, y, 2.0, h], 0xFF00_0000 | p.primary_rgb);
        draw.text(&line1, [x + 5.0, y + 2.0], 0xFFFF_D700, true);
        draw.text(&line2, [x + 5.0, y + 13.0], 0xFFE0_F2FE, true);
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        gui::render_ysm_gui(host, event);
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        let _ = gui::handle_mouse_click(host, event);
        Flow::Continue
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        let _ = gui::handle_mouse_scroll(host, event);
        Flow::Continue
    }

    fn screen_closing(&self, host: &dyn ClientHost, _event: &mut ScreenClosingEvent<'_>) {
        gui::on_screen_closing(host);
    }
}
