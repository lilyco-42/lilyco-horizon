//! 1:1 Port of `YesSteveModel-Native` (`euler.h` + `pose_stack.h` + `model_state_impl.inc`)
//! + `YesSteveModel/AnimationRegister.java` state machine + 17-slot `.yanim` clip player
//! + hierarchical bone-tree procedural fallback for models with empty locomotion/gesture stubs.

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use mod_api::{BlockEntityDrawContext, ItemDisplayContext, ItemStack, MobRender};

const MODEL_UNIT: f32 = 1.0 / 16.0;
const YTREE_MAGIC: &[u8; 4] = b"YTR1";
const YANIM_MAGIC_V1: &[u8; 4] = b"YAN1";
const YANIM_MAGIC_V2: &[u8; 4] = b"YAN2";

const LOOP_ONCE: u8 = 0;
const LOOP_REPEAT: u8 = 1;
const LOOP_HOLD_LAST: u8 = 2;

const SLOT_PRE_PARALLEL: usize = 0;
const SLOT_IDLE: usize = 1;
const SLOT_WALK: usize = 2;
const SLOT_RUN: usize = 3;
const SLOT_SNEAK: usize = 4;
const SLOT_SNEAKING: usize = 5;
const SLOT_JUMP: usize = 6;
const SLOT_ATTACKED: usize = 7;
const SLOT_PARALLEL: usize = 8;
const SLOT_EXTRA_BASE: usize = 9;
const SLOT_HOLD_SWORD: usize = 17;
const SLOT_HOLD_BOW: usize = 18;
const SLOT_HOLD_CROSSBOW: usize = 19;
const SLOT_HOLD_TOOL: usize = 20;
const SLOT_HOLD_SHIELD: usize = 21;
const SLOT_HOLD_FISHING: usize = 22;
const SLOT_HOLD_SPEAR: usize = 23;
const SLOT_HOLD_ITEM: usize = 24;
const SLOT_SWING: usize = 25;
const SLOT_USE_BOW: usize = 26;
const SLOT_USE_EAT: usize = 27;
const SLOT_RIDE: usize = 28;
const SLOT_SWIM: usize = 29;
const TOTAL_SLOTS: usize = 30;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum HeldCat {
    #[default]
    Empty = 0,
    Sword = 1,
    Bow = 2,
    Crossbow = 3,
    Tool = 4,
    Shield = 5,
    Fishing = 6,
    Spear = 7,
    Item = 8,
}

impl HeldCat {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Sword,
            2 => Self::Bow,
            3 => Self::Crossbow,
            4 => Self::Tool,
            5 => Self::Shield,
            6 => Self::Fishing,
            7 => Self::Spear,
            8 => Self::Item,
            _ => Self::Empty,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "空手",
            Self::Sword => "持剑",
            Self::Bow => "持弓",
            Self::Crossbow => "持弩",
            Self::Tool => "持工具",
            Self::Shield => "持盾",
            Self::Fishing => "持鱼竿",
            Self::Spear => "持长兵",
            Self::Item => "持物",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum UseKind {
    #[default]
    None = 0,
    Eat = 1,
    Bow = 2,
    Block = 3,
    Drink = 4,
    Spyglass = 5,
    Spear = 6,
    Crossbow = 7,
    Brush = 8,
    Horn = 9,
}

impl UseKind {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Eat,
            2 => Self::Bow,
            3 => Self::Block,
            4 => Self::Drink,
            5 => Self::Spyglass,
            6 => Self::Spear,
            7 => Self::Crossbow,
            8 => Self::Brush,
            9 => Self::Horn,
            _ => Self::None,
        }
    }
}

/// Complete client-predicted player animation state (no server required; server boosts optional).
#[derive(Clone, Copy, Default)]
pub struct PlayerAnimState {
    pub local: bool,
    pub riding: bool,
    pub in_boat: bool,
    pub sleeping: bool,
    pub elytra_flying: bool,
    pub creative_flying: bool,
    pub climbing: bool,
    pub climb_vy: f32,
    pub in_water: bool,
    pub swimming: bool,
    pub crouching: bool,
    pub moving: bool,
    pub sprinting: bool,
    pub jumping: bool,
    pub main_swinging: bool,
    pub off_swinging: bool,
    pub main_cat: HeldCat,
    pub off_cat: HeldCat,
    pub main_use: UseKind,
    pub off_use: UseKind,
}

const FLAG_ROT: u8 = 1;
const FLAG_POS: u8 = 2;
const FLAG_SCALE: u8 = 4;

type Mat4 = [[f32; 4]; 4];
type Vec3 = [f32; 3];

const MAT4_ID: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoneRole {
    None,
    Root,
    UpperBody,
    Head,
    LeftArm,
    RightArm,
    LeftForeArm,
    RightForeArm,
    LeftHand,
    RightHand,
    LeftLeg,
    RightLeg,
    LeftLowerLeg,
    RightLowerLeg,
    Skirt,
    Hair,
    Wing,
    Tail,
}

fn classify_bone(lower: &str) -> BoneRole {
    if lower == "head" {
        return BoneRole::Head;
    }
    if lower == "root" || lower == "mallbody" || lower == "allbody" || lower == "body_size" {
        return BoneRole::Root;
    }
    if lower == "upperbody" || lower == "upbody" || lower == "body" || lower == "moreupperbody" {
        return BoneRole::UpperBody;
    }
    if lower == "righthandlocator" || lower == "righthand" || lower == "right_hand" || lower == "rightitem" {
        return BoneRole::RightHand;
    }
    if lower == "lefthandlocator" || lower == "lefthand" || lower == "left_hand" || lower == "leftitem" {
        return BoneRole::LeftHand;
    }
    if lower == "leftforearm" || lower == "left_forearm" || lower == "leftarm_b" || lower == "leftlowerarm" {
        return BoneRole::LeftForeArm;
    }
    if lower == "rightforearm" || lower == "right_forearm" || lower == "rightarm_b" || lower == "rightlowerarm" {
        return BoneRole::RightForeArm;
    }
    if lower == "leftarm" || lower == "left_arm" || lower == "leftarm_a" || lower == "arm_l" {
        return BoneRole::LeftArm;
    }
    if lower == "rightarm" || lower == "right_arm" || lower == "rightarm_a" || lower == "arm_r" {
        return BoneRole::RightArm;
    }
    if lower == "leftlowerleg" || lower == "left_lower_leg" || lower == "leftleg_b" {
        return BoneRole::LeftLowerLeg;
    }
    if lower == "rightlowerleg" || lower == "right_lower_leg" || lower == "rightleg_b" {
        return BoneRole::RightLowerLeg;
    }
    if lower == "leftleg" || lower == "left_leg" || lower == "leftleg_a" || lower == "leg_l" {
        return BoneRole::LeftLeg;
    }
    if lower == "rightleg" || lower == "right_leg" || lower == "rightleg_a" || lower == "leg_r" {
        return BoneRole::RightLeg;
    }
    if lower.contains("skirt") {
        return BoneRole::Skirt;
    }
    if lower.contains("wing") {
        return BoneRole::Wing;
    }
    if lower.contains("tail") && !lower.contains("twintail") {
        return BoneRole::Tail;
    }
    if lower.contains("hair") || lower.contains("twintail") || lower.contains("ahoge") {
        return BoneRole::Hair;
    }
    BoneRole::None
}

struct TreeQuad {
    corners: [Vec3; 4],
    uv16: [f32; 4],
}

struct TreeBone {
    role: BoneRole,
    is_core_root: bool,
    is_gui_prop: bool,
    is_locator: bool,
    depth: usize,
    subtree_end: usize,
    pivot: Vec3,
    rest_rotation: Vec3,
    quads: Vec<TreeQuad>,
}

struct AnimTrack {
    bone_index: usize,
    flags: u8,
    rot: Vec<Vec3>,
    pos: Vec<Vec3>,
    scale: Vec<Vec3>,
}

struct AnimSlot {
    loop_mode: u8,
    length: f32,
    frame_count: usize,
    has_rot_motion: bool,
    tracks: Vec<AnimTrack>,
}

#[derive(Default)]
struct NamedLookup {
    death: Option<usize>,
    sleep: Option<usize>,
    boat: Option<usize>,
    elytra_fly: Option<usize>,
    fly: Option<usize>,
    swim_stand: Option<usize>,
    ladder_up: Option<usize>,
    ladder_down: Option<usize>,
    ladder_stillness: Option<usize>,
    climb: Option<usize>,
    climbing: Option<usize>,
    use_drink: Option<usize>,
    use_spyglass: Option<usize>,
    use_spear: Option<usize>,
    use_crossbow: Option<usize>,
    use_brush: Option<usize>,
    use_horn: Option<usize>,
    use_shield: Option<usize>,
    hold_off_shield: Option<usize>,
    hold_off_sword: Option<usize>,
    hold_off_bow: Option<usize>,
    hold_off_crossbow: Option<usize>,
    hold_off_tool: Option<usize>,
    hold_off_fishing: Option<usize>,
    hold_off_spear: Option<usize>,
    use_off_shield: Option<usize>,
    use_off_eat: Option<usize>,
    use_off_drink: Option<usize>,
    use_off_bow: Option<usize>,
    use_off_crossbow: Option<usize>,
    use_off_spear: Option<usize>,
    use_off_spyglass: Option<usize>,
    swing_offhand: Option<usize>,
}

impl NamedLookup {
    fn from_named(named: &[(String, AnimSlot)]) -> Self {
        let find = |keys: &[&str]| -> Option<usize> {
            for k in keys {
                if let Some(idx) = named.iter().position(|(n, _)| n == k) {
                    return Some(idx);
                }
            }
            None
        };
        Self {
            death: find(&["death", "die"]),
            sleep: find(&["sleep"]),
            boat: find(&["boat"]),
            elytra_fly: find(&["elytra_fly", "elytra"]),
            fly: find(&["fly"]),
            swim_stand: find(&["swim_stand"]),
            ladder_up: find(&["ladder_up", "climb"]),
            ladder_down: find(&["ladder_down", "climb"]),
            ladder_stillness: find(&["ladder_stillness", "climbing", "climb"]),
            climb: find(&["climb"]),
            climbing: find(&["climbing", "climb"]),
            use_drink: find(&["use_mainhand:drink", "use_mainhand:eat"]),
            use_spyglass: find(&["use_mainhand:spyglass"]),
            use_spear: find(&["use_mainhand:spear", "use_mainhand:lance"]),
            use_crossbow: find(&["use_mainhand:crossbow", "use_mainhand:bow"]),
            use_brush: find(&["use_mainhand$minecraft:brush", "use_mainhand"]),
            use_horn: find(&["use_mainhand$minecraft:goat_horn", "use_mainhand:drink"]),
            use_shield: find(&["use_mainhand:shield", "hold_mainhand:shield"]),
            hold_off_shield: find(&["hold_offhand:shield"]),
            hold_off_sword: find(&["hold_offhand:sword"]),
            hold_off_bow: find(&["hold_offhand:bow"]),
            hold_off_crossbow: find(&["hold_offhand:crossbow"]),
            hold_off_tool: find(&["hold_offhand:pickaxe", "hold_offhand:axe", "hold_offhand:shovel", "hold_offhand:hoe"]),
            hold_off_fishing: find(&["hold_offhand:fishing"]),
            hold_off_spear: find(&["hold_offhand:spear", "hold_offhand:lance"]),
            use_off_shield: find(&["use_offhand:shield", "hold_offhand:shield"]),
            use_off_eat: find(&["use_offhand:eat"]),
            use_off_drink: find(&["use_offhand:drink", "use_offhand:eat"]),
            use_off_bow: find(&["use_offhand:bow"]),
            use_off_crossbow: find(&["use_offhand:crossbow", "use_offhand:bow"]),
            use_off_spear: find(&["use_offhand:spear", "use_offhand:lance"]),
            use_off_spyglass: find(&["use_offhand:spyglass"]),
            swing_offhand: find(&["swing_offhand"]),
        }
    }
}

pub struct YTreeModel {
    sprite: &'static str,
    scale: f32,
    bones: Vec<TreeBone>,
    slots: [Option<AnimSlot>; TOTAL_SLOTS],
    named: Vec<(String, AnimSlot)>,
    lookup: NamedLookup,
}

impl YTreeModel {
    pub fn animation_names(&self) -> Vec<&str> {
        self.named.iter().map(|(n, _)| n.as_str()).collect()
    }

    pub fn find_animation(&self, query: &str) -> Option<(usize, &str)> {
        let q = query.trim();
        if q.is_empty() {
            return None;
        }
        if let Ok(idx) = q.parse::<usize>() {
            if idx >= 1 && idx <= self.named.len() {
                return Some((idx - 1, self.named[idx - 1].0.as_str()));
            }
        }
        let ql = q.to_ascii_lowercase();
        if let Some((i, (n, _))) = self.named.iter().enumerate().find(|(_, (n, _))| n.eq_ignore_ascii_case(&ql)) {
            return Some((i, n.as_str()));
        }
        self.named
            .iter()
            .enumerate()
            .find(|(_, (n, _))| n.to_ascii_lowercase().contains(&ql))
            .map(|(i, (n, _))| (i, n.as_str()))
    }
}

#[derive(Clone, Copy)]
struct BoneAttr {
    rot: Vec3,
    pos: Vec3,
    scale: Vec3,
}

impl Default for BoneAttr {
    fn default() -> Self {
        Self {
            rot: [0.0; 3],
            pos: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

fn euler_zyx(a: Vec3) -> Mat4 {
    let (sx, cx) = a[0].sin_cos();
    let (sy, cy) = a[1].sin_cos();
    let (sz, cz) = a[2].sin_cos();
    let (czsx, czcx, szsx, szcx) = (cz * sx, cz * cx, sz * sx, sz * cx);
    [
        [cy * cz, cy * sz, -sy, 0.0],
        [czsx * sy - szcx, szsx * sy + czcx, cy * sx, 0.0],
        [czcx * sy + szsx, szcx * sy - czsx, cy * cx, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mat4_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut o = [[0.0f32; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            o[c][r] = a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
        }
    }
    o
}

fn mat4_translate(m: &mut Mat4, x: f32, y: f32, z: f32) {
    if x == 0.0 && y == 0.0 && z == 0.0 {
        return;
    }
    for r in 0..4 {
        m[3][r] += m[0][r] * x + m[1][r] * y + m[2][r] * z;
    }
}

fn mat4_scale(m: &mut Mat4, sx: f32, sy: f32, sz: f32) {
    if sx == 1.0 && sy == 1.0 && sz == 1.0 {
        return;
    }
    let s = [sx, sy, sz];
    for c in 0..3 {
        for r in 0..4 {
            m[c][r] *= s[c];
        }
    }
}

fn xform_pt(m: &Mat4, p: Vec3, scale: f32) -> Vec3 {
    let wx = m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0];
    let wy = m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1];
    let wz = m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2];
    [wx * scale, wy * scale, -wz * scale]
}

fn read_f32(b: &[u8], off: &mut usize) -> Option<f32> {
    let s = b.get(*off..*off + 4)?;
    *off += 4;
    Some(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u16(b: &[u8], off: &mut usize) -> Option<u16> {
    let s = b.get(*off..*off + 2)?;
    *off += 2;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn read_vec3(b: &[u8], off: &mut usize) -> Option<Vec3> {
    Some([read_f32(b, off)?, read_f32(b, off)?, read_f32(b, off)?])
}

fn load_file(slug: &str, ext: &str) -> Option<Vec<u8>> {
    let fname = format!("{slug}.{ext}");
    let mut paths = vec![PathBuf::from("player_models/ytree_models").join(&fname)];
    if let Ok(home) = std::env::var("HOME") {
        paths.push(
            PathBuf::from(home)
                .join(".local/share/horizon/player_models/ytree_models")
                .join(&fname),
        );
    }
    for p in paths {
        if let Ok(bytes) = fs::read(&p) {
            if bytes.len() >= 8 {
                return Some(bytes);
            }
        }
    }
    None
}

fn decode_anim_payload(b: &[u8], off: &mut usize, loop_mode: u8) -> Option<AnimSlot> {
    let length = read_f32(b, off)?;
    let frame_count = read_u16(b, off)? as usize;
    let track_count = read_u16(b, off)? as usize;
    let mut tracks = Vec::with_capacity(track_count);
    let mut max_rot_delta = 0.0f32;
    for _ in 0..track_count {
        let bone_index = read_u16(b, off)? as usize;
        let flags = *b.get(*off)?;
        *off += 1;
        let mut rot: Vec<Vec3> = if (flags & FLAG_ROT) != 0 { Vec::with_capacity(frame_count) } else { Vec::new() };
        let mut pos: Vec<Vec3> = if (flags & FLAG_POS) != 0 { Vec::with_capacity(frame_count) } else { Vec::new() };
        let mut scale: Vec<Vec3> = if (flags & FLAG_SCALE) != 0 { Vec::with_capacity(frame_count) } else { Vec::new() };
        for _ in 0..frame_count {
            if (flags & FLAG_ROT) != 0 {
                if let Some(v) = read_vec3(b, off) {
                    if let Some(first) = rot.first() {
                        max_rot_delta = max_rot_delta
                            .max((v[0] - first[0]).abs())
                            .max((v[1] - first[1]).abs())
                            .max((v[2] - first[2]).abs());
                    }
                    rot.push(v);
                }
            }
            if (flags & FLAG_POS) != 0 {
                if let Some(v) = read_vec3(b, off) { pos.push(v); }
            }
            if (flags & FLAG_SCALE) != 0 {
                if let Some(v) = read_vec3(b, off) { scale.push(v); }
            }
        }
        tracks.push(AnimTrack { bone_index, flags, rot, pos, scale });
    }
    Some(AnimSlot {
        loop_mode,
        length,
        frame_count,
        has_rot_motion: max_rot_delta > 0.05,
        tracks,
    })
}

fn decode_yanim(b: &[u8]) -> ([Option<AnimSlot>; TOTAL_SLOTS], Vec<(String, AnimSlot)>) {
    let mut slots: [Option<AnimSlot>; TOTAL_SLOTS] = std::array::from_fn(|_| None);
    let mut named: Vec<(String, AnimSlot)> = Vec::new();
    if b.len() < 5 {
        return (slots, named);
    }
    let is_v2 = &b[0..4] == YANIM_MAGIC_V2;
    if !is_v2 && &b[0..4] != YANIM_MAGIC_V1 {
        return (slots, named);
    }
    let slot_count = b[4] as usize;
    let mut off = 5usize;
    for _ in 0..slot_count {
        let Some(&slot_id) = b.get(off) else { break };
        off += 1;
        let loop_mode = if is_v2 {
            let Some(&lm) = b.get(off) else { break };
            off += 1;
            lm
        } else {
            LOOP_REPEAT
        };
        let Some(slot) = decode_anim_payload(b, &mut off, loop_mode) else { break };
        if (slot_id as usize) < TOTAL_SLOTS {
            slots[slot_id as usize] = Some(slot);
        }
    }
    if is_v2 {
        if let Some(named_count) = read_u16(b, &mut off).map(|v| v as usize) {
            named.reserve(named_count);
            for _ in 0..named_count {
                let Some(&nlen) = b.get(off) else { break };
                off += 1;
                let Some(name) = b.get(off..off + nlen as usize).and_then(|s| std::str::from_utf8(s).ok()) else { break };
                off += nlen as usize;
                let Some(&loop_mode) = b.get(off) else { break };
                off += 1;
                let Some(slot) = decode_anim_payload(b, &mut off, loop_mode) else { break };
                named.push((name.to_string(), slot));
            }
        }
    }
    (slots, named)
}

fn decode_ytree(slug: &str, sprite: &'static str, b: &[u8]) -> Option<YTreeModel> {
    if b.len() < 10 || &b[0..4] != YTREE_MAGIC {
        return None;
    }
    let mut off = 4usize;
    let count = read_u16(b, &mut off)? as usize;
    let scale = read_f32(b, &mut off)?;
    let mut bones = Vec::with_capacity(count);
    for _ in 0..count {
        let nlen = *b.get(off)? as usize;
        off += 1;
        let name = std::str::from_utf8(b.get(off..off + nlen)?).ok()?;
        off += nlen;
        off += 2; // parent_index
        let depth = read_u16(b, &mut off)? as usize;
        let subtree_end = read_u16(b, &mut off)? as usize;
        let pivot = read_vec3(b, &mut off)?;
        let rest_rotation = read_vec3(b, &mut off)?;
        let qcnt = u32::from_le_bytes([*b.get(off)?, *b.get(off + 1)?, *b.get(off + 2)?, *b.get(off + 3)?]) as usize;
        off += 4;
        let mut quads = Vec::with_capacity(qcnt);
        for _ in 0..qcnt {
            let c0 = read_vec3(b, &mut off)?;
            let c1 = read_vec3(b, &mut off)?;
            let c2 = read_vec3(b, &mut off)?;
            let c3 = read_vec3(b, &mut off)?;
            let uv16 = [read_f32(b, &mut off)?, read_f32(b, &mut off)?, read_f32(b, &mut off)?, read_f32(b, &mut off)?];
            quads.push(TreeQuad { corners: [c0, c1, c2, c3], uv16 });
        }
        let lower = name.to_ascii_lowercase();
        let is_gui_prop = lower == "gui"
            || lower == "kuang"
            || lower.contains("liefeng")
            || lower.contains("guangzhu")
            || lower.contains("jiguang");
        let is_core_root = lower == "root"
            || lower == "all"
            || lower == "mallbody"
            || lower == "allbody"
            || lower == "upperbody"
            || lower == "head";
        let is_locator = lower.ends_with("locator");
        let role = classify_bone(&lower);
        bones.push(TreeBone {
            role,
            is_core_root,
            is_gui_prop,
            is_locator,
            depth,
            subtree_end,
            pivot,
            rest_rotation,
            quads,
        });
    }
    let (slots, named) = load_file(slug, "yanim")
        .map(|yb| decode_yanim(&yb))
        .unwrap_or_else(|| (std::array::from_fn(|_| None), Vec::new()));
    let lookup = NamedLookup::from_named(&named);
    Some(YTreeModel { sprite, scale, bones, slots, named, lookup })
}

static YTREE_CACHE: OnceLock<Vec<Option<YTreeModel>>> = OnceLock::new();

pub fn get_ytree_models(specs: &[(&'static str, &'static str)]) -> &'static [Option<YTreeModel>] {
    YTREE_CACHE.get_or_init(|| {
        specs
            .iter()
            .map(|(slug, sprite)| load_file(slug, "ytree").and_then(|b| decode_ytree(slug, sprite, &b)))
            .collect()
    })
}

fn lerp3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn apply_slot(slot: &AnimSlot, time_sec: f32, attrs: &mut [BoneAttr], additive: bool, hold_last: bool) {
    if slot.frame_count == 0 {
        return;
    }
    let (f0, f1, frac) = if slot.frame_count == 1 || slot.length <= 0.001 {
        (0, 0, 0.0)
    } else if hold_last {
        let local_t = time_sec.clamp(0.0, slot.length);
        let pos = (local_t / slot.length) * (slot.frame_count - 1) as f32;
        let i0 = (pos.floor() as usize).min(slot.frame_count - 1);
        let i1 = (i0 + 1).min(slot.frame_count - 1);
        (i0, i1, pos - i0 as f32)
    } else {
        let local_t = time_sec.rem_euclid(slot.length);
        let pos = (local_t / slot.length) * (slot.frame_count - 1) as f32;
        let i0 = (pos.floor() as usize).min(slot.frame_count - 1);
        let i1 = (i0 + 1).min(slot.frame_count - 1);
        (i0, i1, pos - i0 as f32)
    };
    for tr in &slot.tracks {
        let Some(attr) = attrs.get_mut(tr.bone_index) else { continue };
        if (tr.flags & FLAG_ROT) != 0 && tr.rot.len() > f1 {
            let v = lerp3(tr.rot[f0], tr.rot[f1], frac);
            if additive {
                attr.rot[0] += v[0];
                attr.rot[1] += v[1];
                attr.rot[2] += v[2];
            } else {
                attr.rot = v;
            }
        }
        if (tr.flags & FLAG_POS) != 0 && tr.pos.len() > f1 {
            let v = lerp3(tr.pos[f0], tr.pos[f1], frac);
            if additive {
                attr.pos[0] += v[0];
                attr.pos[1] += v[1];
                attr.pos[2] += v[2];
            } else {
                attr.pos = v;
            }
        }
        if (tr.flags & FLAG_SCALE) != 0 && tr.scale.len() > f1 {
            let v = lerp3(tr.scale[f0], tr.scale[f1], frac);
            if additive {
                attr.scale[0] *= v[0];
                attr.scale[1] *= v[1];
                attr.scale[2] *= v[2];
            } else {
                attr.scale = v;
            }
        }
    }
}

/// Smooth hierarchical bone-tree procedural locomotion fallback for models whose
/// `main.animation.json` omits `walk` / `run` / `sneak` / `sneaking` / `jump` / `idle`.
fn apply_procedural_locomotion(bones: &[TreeBone], attrs: &mut [BoneAttr], slot: usize, t: f32) {
    let breath = (t * 2.2).sin();
    let walk_s = (t * 7.5).sin();
    let walk_c = (t * 7.5).cos();
    let run_s = (t * 11.5).sin();
    let run_c = (t * 11.5).cos();

    for (i, b) in bones.iter().enumerate() {
        let a = &mut attrs[i];
        match slot {
            SLOT_WALK => match b.role {
                BoneRole::Root => a.pos[1] += walk_c.abs() * 0.45,
                BoneRole::UpperBody => {
                    a.rot[0] -= 0.08;
                    a.rot[1] += walk_s * 0.08;
                }
                BoneRole::LeftArm => a.rot[0] += walk_s * 0.55,
                BoneRole::RightArm => a.rot[0] -= walk_s * 0.55,
                BoneRole::LeftForeArm => a.rot[0] += 0.15 + walk_s.max(0.0) * 0.2,
                BoneRole::RightForeArm => a.rot[0] += 0.15 + (-walk_s).max(0.0) * 0.2,
                BoneRole::LeftLeg => a.rot[0] -= walk_s * 0.65,
                BoneRole::RightLeg => a.rot[0] += walk_s * 0.65,
                BoneRole::LeftLowerLeg => a.rot[0] -= walk_s.max(0.0) * 0.45,
                BoneRole::RightLowerLeg => a.rot[0] -= (-walk_s).max(0.0) * 0.45,
                BoneRole::Skirt | BoneRole::Hair => a.rot[0] += 0.10 + walk_c * 0.06,
                BoneRole::Wing => a.rot[1] += walk_s * 0.18,
                BoneRole::Tail => a.rot[1] += walk_s * 0.25,
                _ => {}
            },
            SLOT_RUN => match b.role {
                BoneRole::Root => a.pos[1] += run_c.abs() * 0.85,
                BoneRole::UpperBody => {
                    a.rot[0] -= 0.24;
                    a.rot[1] += run_s * 0.14;
                }
                BoneRole::LeftArm => a.rot[0] += run_s * 0.95,
                BoneRole::RightArm => a.rot[0] -= run_s * 0.95,
                BoneRole::LeftForeArm => a.rot[0] += 0.45,
                BoneRole::RightForeArm => a.rot[0] += 0.45,
                BoneRole::LeftLeg => a.rot[0] -= run_s * 1.05,
                BoneRole::RightLeg => a.rot[0] += run_s * 1.05,
                BoneRole::LeftLowerLeg => a.rot[0] -= run_s.max(0.0) * 0.75,
                BoneRole::RightLowerLeg => a.rot[0] -= (-run_s).max(0.0) * 0.75,
                BoneRole::Skirt | BoneRole::Hair => a.rot[0] += 0.22 + run_c * 0.10,
                BoneRole::Wing => a.rot[1] += run_s * 0.30,
                BoneRole::Tail => a.rot[1] += run_s * 0.35,
                _ => {}
            },
            SLOT_SNEAK | SLOT_SNEAKING => {
                let sw = if slot == SLOT_SNEAK { (t * 4.5).sin() } else { 0.0 };
                match b.role {
                    BoneRole::Root => a.pos[1] -= 3.2,
                    BoneRole::UpperBody => a.rot[0] -= 0.42,
                    BoneRole::Head => a.rot[0] += 0.32,
                    BoneRole::LeftArm => {
                        a.rot[0] += 0.25 + sw * 0.30;
                        a.rot[2] -= 0.12;
                    }
                    BoneRole::RightArm => {
                        a.rot[0] += 0.25 - sw * 0.30;
                        a.rot[2] += 0.12;
                    }
                    BoneRole::LeftLeg => a.rot[0] += 0.45 - sw * 0.35,
                    BoneRole::RightLeg => a.rot[0] += 0.45 + sw * 0.35,
                    BoneRole::LeftLowerLeg => a.rot[0] -= 0.75,
                    BoneRole::RightLowerLeg => a.rot[0] -= 0.75,
                    _ => {}
                }
            }
            SLOT_RIDE => match b.role {
                BoneRole::Root => a.pos[1] -= 5.5,
                BoneRole::UpperBody => a.rot[0] -= 0.08,
                BoneRole::LeftLeg => {
                    a.rot[0] += 1.32;
                    a.rot[1] -= 0.25;
                    a.rot[2] -= 0.18;
                }
                BoneRole::RightLeg => {
                    a.rot[0] += 1.32;
                    a.rot[1] += 0.25;
                    a.rot[2] += 0.18;
                }
                BoneRole::LeftLowerLeg | BoneRole::RightLowerLeg => a.rot[0] -= 0.55,
                BoneRole::LeftArm => a.rot[0] += 0.52,
                BoneRole::RightArm => a.rot[0] += 0.52,
                _ => {}
            },
            SLOT_JUMP => match b.role {
                BoneRole::LeftArm => {
                    a.rot[0] += 0.45;
                    a.rot[2] -= 0.40;
                }
                BoneRole::RightArm => {
                    a.rot[0] += 0.45;
                    a.rot[2] += 0.40;
                }
                BoneRole::LeftLeg => a.rot[0] += 0.38,
                BoneRole::RightLeg => a.rot[0] -= 0.25,
                BoneRole::LeftLowerLeg => a.rot[0] -= 0.65,
                BoneRole::Skirt | BoneRole::Hair => a.rot[0] += 0.20,
                _ => {}
            },
            SLOT_SWIM => {
                let stroke = (t * 5.2).sin();
                let kick = (t * 7.8).sin();
                match b.role {
                    BoneRole::Root => {
                        a.rot[0] -= 1.25;
                        a.pos[1] += (t * 2.6).sin() * 0.4;
                    }
                    BoneRole::Head => a.rot[0] += 0.55,
                    BoneRole::LeftArm => {
                        a.rot[0] += 1.65 + stroke * 0.85;
                        a.rot[2] -= 0.45 + stroke.abs() * 0.25;
                    }
                    BoneRole::RightArm => {
                        a.rot[0] += 1.65 - stroke * 0.85;
                        a.rot[2] += 0.45 + stroke.abs() * 0.25;
                    }
                    BoneRole::LeftLeg => a.rot[0] += kick * 0.55,
                    BoneRole::RightLeg => a.rot[0] -= kick * 0.55,
                    BoneRole::LeftLowerLeg => a.rot[0] -= 0.25 + kick.max(0.0) * 0.35,
                    BoneRole::RightLowerLeg => a.rot[0] -= 0.25 + (-kick).max(0.0) * 0.35,
                    _ => {}
                }
            }
            _ => match b.role {
                BoneRole::UpperBody => a.rot[0] += breath * 0.025,
                BoneRole::LeftArm => a.rot[2] -= 0.06 + breath * 0.03,
                BoneRole::RightArm => a.rot[2] += 0.06 + breath * 0.03,
                BoneRole::Hair | BoneRole::Skirt => a.rot[0] += breath * 0.03,
                BoneRole::Wing => a.rot[1] += breath * 0.12,
                BoneRole::Tail => a.rot[1] += (t * 2.6).sin() * 0.22,
                _ => {}
            },
        }
    }
}

/// Expressive hierarchical bone-tree procedural gestures for `G` key (`g_idx = 1..=7`)
/// when a model does not ship `extra.animation.json`.
fn apply_procedural_gesture(bones: &[TreeBone], attrs: &mut [BoneAttr], g_idx: usize, t: f32) {
    let w = (t * 6.5).sin();
    let slow = (t * 3.0).sin();
    for (i, b) in bones.iter().enumerate() {
        let a = &mut attrs[i];
        match g_idx {
            1 => {
                // wave (挥手致意)
                match b.role {
                    BoneRole::RightArm => {
                        a.rot[0] += 2.35;
                        a.rot[2] += 0.35 + w * 0.35;
                    }
                    BoneRole::RightForeArm => a.rot[2] += w * 0.45,
                    BoneRole::Head => a.rot[2] += 0.12 + w * 0.05,
                    BoneRole::UpperBody => a.rot[2] -= 0.08,
                    _ => {}
                }
            }
            2 => {
                // cheer (欢呼跳跃)
                let hop = (t * 7.0).sin().abs();
                match b.role {
                    BoneRole::Root => a.pos[1] += hop * 2.4,
                    BoneRole::LeftArm => {
                        a.rot[0] += 2.55;
                        a.rot[2] -= 0.35 + w * 0.18;
                    }
                    BoneRole::RightArm => {
                        a.rot[0] += 2.55;
                        a.rot[2] += 0.35 + w * 0.18;
                    }
                    BoneRole::Head => a.rot[0] += 0.12,
                    _ => {}
                }
            }
            3 => {
                // work_redstone (红石精密施工姿态)
                match b.role {
                    BoneRole::UpperBody => a.rot[0] -= 0.22,
                    BoneRole::LeftArm => a.rot[0] += 0.95 + w * 0.25,
                    BoneRole::RightArm => a.rot[0] += 0.95 - w * 0.25,
                    BoneRole::LeftForeArm | BoneRole::RightForeArm => a.rot[0] += 0.45,
                    _ => {}
                }
            }
            4 => {
                // sit (原地休息/摸鱼)
                match b.role {
                    BoneRole::Root => a.pos[1] -= 7.5,
                    BoneRole::LeftLeg => {
                        a.rot[0] += 1.35;
                        a.rot[2] -= 0.12;
                    }
                    BoneRole::RightLeg => {
                        a.rot[0] += 1.35;
                        a.rot[2] += 0.12;
                    }
                    BoneRole::LeftArm => a.rot[0] += 0.35,
                    BoneRole::RightArm => a.rot[0] += 0.35,
                    BoneRole::Head => a.rot[2] += slow * 0.08,
                    _ => {}
                }
            }
            5 => {
                // dance (星光律动舞)
                let beat = (t * 5.2).sin();
                let beat2 = (t * 2.6).sin();
                match b.role {
                    BoneRole::Root => {
                        a.pos[1] += beat.abs() * 1.1;
                        a.rot[1] += beat2 * 0.35;
                    }
                    BoneRole::UpperBody => a.rot[2] += beat * 0.14,
                    BoneRole::Head => a.rot[2] -= beat * 0.18,
                    BoneRole::LeftArm => {
                        a.rot[0] += 1.2 + beat * 0.6;
                        a.rot[2] -= 0.65 + beat2 * 0.35;
                    }
                    BoneRole::RightArm => {
                        a.rot[0] += 1.2 - beat * 0.6;
                        a.rot[2] += 0.65 - beat2 * 0.35;
                    }
                    BoneRole::LeftLeg => a.rot[0] += beat * 0.25,
                    BoneRole::RightLeg => a.rot[0] -= beat * 0.25,
                    BoneRole::Skirt | BoneRole::Hair => a.rot[2] += beat * 0.15,
                    _ => {}
                }
            }
            6 => {
                // bow (大小姐优雅提裙礼)
                match b.role {
                    BoneRole::Root => a.pos[1] -= 1.6,
                    BoneRole::UpperBody => a.rot[0] -= 0.55,
                    BoneRole::Head => a.rot[0] -= 0.15,
                    BoneRole::LeftArm => {
                        a.rot[0] += 0.25;
                        a.rot[2] -= 0.55;
                    }
                    BoneRole::RightArm => {
                        a.rot[0] += 0.25;
                        a.rot[2] += 0.55;
                    }
                    BoneRole::LeftLeg => a.rot[0] += 0.22,
                    BoneRole::RightLeg => a.rot[0] -= 0.28,
                    _ => {}
                }
            }
            7 => {
                // victory (屠龙凯旋招牌动作)
                match b.role {
                    BoneRole::UpperBody => a.rot[1] += 0.20,
                    BoneRole::RightArm => {
                        a.rot[0] += 2.65;
                        a.rot[2] += 0.25;
                    }
                    BoneRole::LeftArm => {
                        a.rot[0] -= 0.20;
                        a.rot[2] -= 0.45;
                    }
                    BoneRole::Head => a.rot[1] -= 0.18,
                    _ => {}
                }
            }
            _ => {
                // mace_twirl (重锤/武器旋风甩动 [3.8.0彩蛋])
                let flourish = (t * 7.5).sin();
                match b.role {
                    BoneRole::UpperBody => {
                        a.rot[0] -= 0.08;
                        a.rot[1] -= 0.22 + flourish * 0.06;
                    }
                    BoneRole::Head => a.rot[1] += 0.20,
                    BoneRole::RightArm => {
                        a.rot[0] += 0.95 + flourish * 0.18;
                        a.rot[1] += 0.20;
                        a.rot[2] += 0.35 + flourish * 0.12;
                    }
                    BoneRole::RightForeArm => {
                        a.rot[0] += 0.35 + flourish * 0.25;
                    }
                    BoneRole::LeftArm => {
                        a.rot[0] -= 0.15;
                        a.rot[2] -= 0.42;
                    }
                    BoneRole::LeftLeg => a.rot[0] += 0.18,
                    BoneRole::RightLeg => a.rot[0] -= 0.18,
                    _ => {}
                }
            }
        }
    }
}

struct BlendCache {
    model_ptr: usize,
    last_time: f32,
    swing_start: f32,
    was_swinging: bool,
    off_swing_start: f32,
    was_off_swinging: bool,
    last_arm_key: Option<u32>,
    arm_slot_start: f32,
    last_off_key: Option<u32>,
    off_slot_start: f32,
    attrs: Vec<BoneAttr>,
}

static LOCAL_BLEND: OnceLock<Mutex<Option<BlendCache>>> = OnceLock::new();

fn named_clip(model: &YTreeModel, idx: Option<usize>) -> Option<&AnimSlot> {
    idx.and_then(|i| model.named.get(i)).map(|(_, c)| c)
}

/// Evaluate official `AnimationRegister.java` state machine + `YesSteveModel-Native` `ModelState::Extract`
/// with full client-side posture coverage (`death`, `sleep`, `boat`, `ride`, `elytra_fly`, `swim`, `swim_stand`,
/// `ladder_up/down/stillness`, `fly`, `jump`, `sneak/sneaking`, `run`, `walk`, `idle`),
/// mainhand & offhand `arm.animation.json` hold / use / swing overlays, silky cross-fade blending,
/// and 3D held item rendering directly on the animated hand locator bones!
#[allow(clippy::too_many_arguments)]
pub fn draw_ytree_model(
    draw: &mut dyn BlockEntityDrawContext,
    model: &YTreeModel,
    g_idx: usize,
    named_anim_idx: Option<usize>,
    player: &MobRender<'_>,
    st: &PlayerAnimState,
    main_stack: Option<&ItemStack>,
    off_stack: Option<&ItemStack>,
) -> (bool, &'static str) {
    let first_q = model.bones.iter().find_map(|b| b.quads.first());
    if let Some(q) = first_q {
        draw.push();
        draw.scale([0.0, 0.0, 0.0]);
        let ok = draw.quad(model.sprite, q.corners, q.uv16, 0xFFFFFF);
        draw.pop();
        if !ok {
            return (false, "none");
        }
    } else {
        return (false, "none");
    }

    let time_sec = (draw.animation_time() as f32) * 0.05;
    let walk_speed = player.walk_speed.min(1.0);
    let is_moving = st.moving || walk_speed > 0.03;
    let in_air = if st.local { st.jumping } else { !player.on_ground };
    let is_dead = player.death_time > 0;
    let tint = if player.hurt_time > 0 || is_dead { 0xFF8888 } else { 0xFFFFFF };

    let mut attrs = vec![BoneAttr::default(); model.bones.len()];

    // Layer 0: `pre_parallel` (hides Gui wall, Kuang, ysmGlowLieFeng, hidden weapons/effects!)
    if let Some(ref pre) = model.slots[SLOT_PRE_PARALLEL] {
        apply_slot(pre, time_sec, &mut attrs, false, true);
    }

    // Layer 1: Primary animation state (custom `/ysm_play <name>` > `G` extra gesture > official YSM priority!)
    let active_named = named_anim_idx.and_then(|i| model.named.get(i));
    let active_gesture = active_named.is_none()
        && g_idx > 0
        && !is_dead
        && !st.sleeping
        && !st.riding
        && !st.elytra_flying
        && !st.creative_flying
        && !st.climbing
        && !st.in_water
        && !st.swimming
        && !is_moving
        && !st.crouching
        && !in_air
        && player.hurt_time == 0;
    let state_name: &'static str;
    let suppress_head_look: bool;
    let suppress_arm_overlay: bool;

    if let Some((_, clip)) = active_named {
        state_name = "custom";
        suppress_head_look = false;
        suppress_arm_overlay = true;
        let hold_last = clip.loop_mode == LOOP_HOLD_LAST;
        apply_slot(clip, time_sec, &mut attrs, false, hold_last);
    } else if active_gesture {
        let extra_slot = (SLOT_EXTRA_BASE + g_idx).min(SLOT_HOLD_SWORD - 1);
        state_name = match g_idx {
            1 => "extra1/wave",
            2 => "extra2/cheer",
            3 => "extra3/work",
            4 => "extra4/sit",
            5 => "extra5/dance",
            6 => "extra6/bow",
            7 => "extra7/victory",
            _ => "extra8/mace_twirl",
        };
        suppress_head_look = false;
        suppress_arm_overlay = true;
        if g_idx <= 7 {
            if let Some(ref clip) = model.slots[extra_slot] {
                apply_slot(clip, time_sec, &mut attrs, false, false);
            } else {
                if let Some(ref idle) = model.slots[SLOT_IDLE] {
                    apply_slot(idle, time_sec, &mut attrs, false, false);
                }
                apply_procedural_gesture(&model.bones, &mut attrs, g_idx, time_sec);
            }
        } else {
            if let Some(ref idle) = model.slots[SLOT_IDLE] {
                apply_slot(idle, time_sec, &mut attrs, false, false);
            }
            apply_procedural_gesture(&model.bones, &mut attrs, g_idx, time_sec);
        }
    } else if is_dead {
        state_name = "death";
        suppress_head_look = true;
        suppress_arm_overlay = true;
        if let Some(clip) = named_clip(model, model.lookup.death) {
            let death_t = (player.death_time as f32) * 0.05;
            apply_slot(clip, death_t, &mut attrs, false, true);
        } else {
            // Procedural fall-over death fallback
            let p = ((player.death_time as f32) / 20.0).clamp(0.0, 1.0);
            for (i, b) in model.bones.iter().enumerate() {
                if b.role == BoneRole::Root {
                    attrs[i].rot[2] += p * 1.52;
                    attrs[i].pos[1] -= p * 10.0;
                }
            }
        }
    } else if player.hurt_time > 0 {
        state_name = "attacked";
        suppress_head_look = false;
        suppress_arm_overlay = false;
        if let Some(ref clip) = model.slots[SLOT_ATTACKED] {
            apply_slot(clip, time_sec, &mut attrs, false, false);
        } else {
            apply_procedural_locomotion(&model.bones, &mut attrs, SLOT_ATTACKED, time_sec);
        }
    } else if st.sleeping {
        state_name = "sleep";
        suppress_head_look = true;
        suppress_arm_overlay = true;
        if let Some(clip) = named_clip(model, model.lookup.sleep) {
            apply_slot(clip, time_sec, &mut attrs, false, false);
        } else {
            for (i, b) in model.bones.iter().enumerate() {
                if b.role == BoneRole::Root {
                    attrs[i].rot[0] += 1.52;
                    attrs[i].pos[1] -= 8.0;
                }
            }
        }
    } else if st.riding && st.in_boat {
        state_name = "boat";
        suppress_head_look = false;
        suppress_arm_overlay = false;
        if let Some(clip) = named_clip(model, model.lookup.boat).or(model.slots[SLOT_RIDE].as_ref()) {
            apply_slot(clip, time_sec, &mut attrs, false, clip.loop_mode == LOOP_HOLD_LAST);
        } else {
            apply_procedural_locomotion(&model.bones, &mut attrs, SLOT_RIDE, time_sec);
        }
    } else if st.riding {
        state_name = "ride";
        suppress_head_look = false;
        suppress_arm_overlay = false;
        if let Some(ref clip) = model.slots[SLOT_RIDE] {
            apply_slot(clip, time_sec, &mut attrs, false, false);
        } else {
            apply_procedural_locomotion(&model.bones, &mut attrs, SLOT_RIDE, time_sec);
        }
    } else if st.elytra_flying {
        state_name = "elytra_fly";
        suppress_head_look = false;
        suppress_arm_overlay = false;
        if let Some(clip) = named_clip(model, model.lookup.elytra_fly) {
            apply_slot(clip, time_sec, &mut attrs, false, false);
        } else {
            for (i, b) in model.bones.iter().enumerate() {
                match b.role {
                    BoneRole::Root => attrs[i].rot[0] -= 1.35,
                    BoneRole::Head => attrs[i].rot[0] += 0.55,
                    BoneRole::LeftArm => attrs[i].rot[2] -= 0.85,
                    BoneRole::RightArm => attrs[i].rot[2] += 0.85,
                    _ => {}
                }
            }
        }
    } else if st.swimming {
        state_name = "swim";
        suppress_head_look = false;
        suppress_arm_overlay = false;
        if let Some(ref clip) = model.slots[SLOT_SWIM] {
            apply_slot(clip, time_sec, &mut attrs, false, false);
        } else {
            apply_procedural_locomotion(&model.bones, &mut attrs, SLOT_SWIM, time_sec);
        }
    } else if st.in_water {
        state_name = "swim_stand";
        suppress_head_look = false;
        suppress_arm_overlay = false;
        if let Some(clip) = named_clip(model, model.lookup.swim_stand).or(model.slots[SLOT_SWIM].as_ref()) {
            apply_slot(clip, time_sec, &mut attrs, false, false);
        } else {
            apply_procedural_locomotion(&model.bones, &mut attrs, SLOT_SWIM, time_sec);
        }
    } else if st.climbing {
        suppress_head_look = false;
        suppress_arm_overlay = false;
        let (sname, clip_idx) = if st.climb_vy > 0.03 {
            ("ladder_up", model.lookup.ladder_up.or(model.lookup.climb))
        } else if st.climb_vy < -0.03 {
            ("ladder_down", model.lookup.ladder_down.or(model.lookup.climb))
        } else {
            ("ladder_stillness", model.lookup.ladder_stillness.or(model.lookup.climbing).or(model.lookup.climb))
        };
        state_name = sname;
        if let Some(clip) = named_clip(model, clip_idx) {
            apply_slot(clip, time_sec, &mut attrs, false, false);
        } else {
            let cyc = if st.climb_vy.abs() > 0.03 { (time_sec * 8.0).sin() } else { 0.0 };
            for (i, b) in model.bones.iter().enumerate() {
                match b.role {
                    BoneRole::LeftArm => attrs[i].rot[0] += 1.45 + cyc * 0.55,
                    BoneRole::RightArm => attrs[i].rot[0] += 1.45 - cyc * 0.55,
                    BoneRole::LeftLeg => attrs[i].rot[0] += 0.35 - cyc * 0.45,
                    BoneRole::RightLeg => attrs[i].rot[0] += 0.35 + cyc * 0.45,
                    _ => {}
                }
            }
        }
    } else if st.creative_flying {
        state_name = "fly";
        suppress_head_look = false;
        suppress_arm_overlay = false;
        if let Some(clip) = named_clip(model, model.lookup.fly) {
            apply_slot(clip, time_sec, &mut attrs, false, false);
        } else {
            let bob = (time_sec * 3.0).sin();
            for (i, b) in model.bones.iter().enumerate() {
                match b.role {
                    BoneRole::Root => {
                        attrs[i].pos[1] += 1.2 + bob * 0.6;
                        if is_moving {
                            attrs[i].rot[0] -= 0.35;
                        }
                    }
                    BoneRole::LeftArm => attrs[i].rot[2] -= 0.35 + bob * 0.08,
                    BoneRole::RightArm => attrs[i].rot[2] += 0.35 + bob * 0.08,
                    BoneRole::LeftLeg => attrs[i].rot[0] += 0.25,
                    BoneRole::RightLeg => attrs[i].rot[0] -= 0.15,
                    _ => {}
                }
            }
        }
    } else {
        suppress_head_look = false;
        suppress_arm_overlay = false;
        let (primary_slot, sname) = if in_air {
            (SLOT_JUMP, "jump")
        } else if st.crouching && is_moving {
            (SLOT_SNEAK, "sneak")
        } else if st.crouching {
            (SLOT_SNEAKING, "sneaking")
        } else if (st.sprinting && is_moving) || (!st.local && walk_speed > 0.96) {
            (SLOT_RUN, "run")
        } else if is_moving {
            (SLOT_WALK, "walk")
        } else {
            (SLOT_IDLE, "idle")
        };
        state_name = sname;
        if let Some(ref clip) = model.slots[primary_slot] {
            apply_slot(clip, time_sec, &mut attrs, false, false);
        } else {
            apply_procedural_locomotion(&model.bones, &mut attrs, primary_slot, time_sec);
        }
    }

    // Layer 2: `parallel` (official blinking, hair physics, skirt physics)
    if !is_dead {
        if let Some(ref par) = model.slots[SLOT_PARALLEL] {
            apply_slot(par, time_sec, &mut attrs, true, false);
        }
    }

    // Layer 2.5a: Official YSM Mainhand `ConditionalHold` + `ConditionalUse` (`arm.animation.json`)!
    enum ArmClip<'a> {
        Slot(usize, &'a AnimSlot, bool),
        Named(u32, &'a AnimSlot, bool),
        Procedural(u32),
    }

    let main_clip = if suppress_arm_overlay {
        None
    } else {
        match st.main_use {
            UseKind::Bow => model.slots[SLOT_USE_BOW].as_ref().map(|c| ArmClip::Slot(SLOT_USE_BOW, c, true)).or(Some(ArmClip::Procedural(101))),
            UseKind::Crossbow => named_clip(model, model.lookup.use_crossbow).or(model.slots[SLOT_USE_BOW].as_ref()).map(|c| ArmClip::Named(201, c, true)).or(Some(ArmClip::Procedural(101))),
            UseKind::Eat => model.slots[SLOT_USE_EAT].as_ref().map(|c| ArmClip::Slot(SLOT_USE_EAT, c, false)).or(Some(ArmClip::Procedural(102))),
            UseKind::Drink => named_clip(model, model.lookup.use_drink).or(model.slots[SLOT_USE_EAT].as_ref()).map(|c| ArmClip::Named(202, c, false)).or(Some(ArmClip::Procedural(102))),
            UseKind::Block => named_clip(model, model.lookup.use_shield).or(model.slots[SLOT_HOLD_SHIELD].as_ref()).map(|c| ArmClip::Named(203, c, true)).or(Some(ArmClip::Procedural(103))),
            UseKind::Spyglass => named_clip(model, model.lookup.use_spyglass).map(|c| ArmClip::Named(204, c, true)).or(Some(ArmClip::Procedural(104))),
            UseKind::Spear => named_clip(model, model.lookup.use_spear).or(model.slots[SLOT_HOLD_SPEAR].as_ref()).map(|c| ArmClip::Named(205, c, true)).or(Some(ArmClip::Procedural(105))),
            UseKind::Brush => named_clip(model, model.lookup.use_brush).map(|c| ArmClip::Named(206, c, false)).or(Some(ArmClip::Procedural(106))),
            UseKind::Horn => named_clip(model, model.lookup.use_horn).map(|c| ArmClip::Named(207, c, true)).or(Some(ArmClip::Procedural(104))),
            UseKind::None => {
                let s = match st.main_cat {
                    HeldCat::Empty | HeldCat::Item => None,
                    HeldCat::Sword => Some((SLOT_HOLD_SWORD, true)),
                    HeldCat::Bow => Some((SLOT_HOLD_BOW, true)),
                    HeldCat::Crossbow => Some((SLOT_HOLD_CROSSBOW, true)),
                    HeldCat::Tool => Some((SLOT_HOLD_TOOL, true)),
                    HeldCat::Shield => Some((SLOT_HOLD_SHIELD, true)),
                    HeldCat::Fishing => Some((SLOT_HOLD_FISHING, false)),
                    HeldCat::Spear => Some((SLOT_HOLD_SPEAR, true)),
                };
                s.map(|(slot_id, hold)| {
                    if let Some(ref c) = model.slots[slot_id] {
                        ArmClip::Slot(slot_id, c, hold)
                    } else {
                        ArmClip::Procedural(slot_id as u32)
                    }
                })
            }
        }
    };

    let main_arm_key = match main_clip {
        Some(ArmClip::Slot(s, _, _)) => Some(s as u32),
        Some(ArmClip::Named(k, _, _)) => Some(k),
        Some(ArmClip::Procedural(k)) => Some(k),
        None => None,
    };

    if let Some(ac) = main_clip {
        match ac {
            ArmClip::Slot(_, clip, hold_last) | ArmClip::Named(_, clip, hold_last) => {
                let arm_time = if st.local && hold_last {
                    let lock = LOCAL_BLEND.get_or_init(|| Mutex::new(None));
                    lock.lock()
                        .ok()
                        .and_then(|mut g| {
                            if let Some(c) = g.as_mut() {
                                if c.last_arm_key != main_arm_key {
                                    c.last_arm_key = main_arm_key;
                                    c.arm_slot_start = time_sec;
                                }
                                Some((time_sec - c.arm_slot_start).max(0.0))
                            } else {
                                None
                            }
                        })
                        .unwrap_or(10.0)
                } else if hold_last {
                    10.0
                } else {
                    time_sec
                };
                let sample_t = if hold_last { arm_time * 2.5 } else { arm_time };
                apply_slot(clip, sample_t, &mut attrs, false, hold_last);
            }
            ArmClip::Procedural(_) => {
                for (i, bone) in model.bones.iter().enumerate() {
                    match bone.role {
                        BoneRole::RightArm => {
                            attrs[i].rot[0] = match st.main_use {
                                UseKind::Bow | UseKind::Crossbow => 1.45,
                                UseKind::Eat | UseKind::Drink => 1.35 + (time_sec * 16.0).sin() * 0.15,
                                UseKind::Block => 0.95,
                                UseKind::Spyglass | UseKind::Horn => 1.75,
                                UseKind::Spear => 2.55,
                                UseKind::Brush => 1.05 + (time_sec * 10.0).sin() * 0.25,
                                UseKind::None => match st.main_cat {
                                    HeldCat::Bow | HeldCat::Crossbow => 1.25,
                                    HeldCat::Fishing => 0.78 + (time_sec * 2.4).sin() * 0.06,
                                    _ => 0.45,
                                },
                            };
                            attrs[i].rot[1] = if st.main_use == UseKind::Block { 0.45 } else { 0.10 };
                        }
                        BoneRole::RightForeArm if st.main_cat == HeldCat::Fishing => {
                            attrs[i].rot[0] = 0.25;
                        }
                        BoneRole::LeftArm
                            if st.main_use == UseKind::Bow
                                || st.main_use == UseKind::Crossbow
                                || st.main_cat == HeldCat::Bow
                                || st.main_cat == HeldCat::Crossbow =>
                        {
                            attrs[i].rot[0] = 1.40;
                            attrs[i].rot[1] = -0.35;
                        }
                        _ => {}
                    }
                }
            }
        }
    } else if st.local {
        if let Some(lock) = LOCAL_BLEND.get() {
            if let Ok(mut g) = lock.lock() {
                if let Some(c) = g.as_mut() {
                    c.last_arm_key = None;
                }
            }
        }
    }

    // Layer 2.5b: Official YSM Offhand `hold_offhand:*` + `use_offhand:*`!
    let off_clip: Option<(u32, Option<&AnimSlot>, bool)> = if suppress_arm_overlay {
        None
    } else {
        match st.off_use {
            UseKind::Block => Some((301, named_clip(model, model.lookup.use_off_shield), true)),
            UseKind::Eat => Some((302, named_clip(model, model.lookup.use_off_eat), false)),
            UseKind::Drink => Some((303, named_clip(model, model.lookup.use_off_drink), false)),
            UseKind::Bow => Some((304, named_clip(model, model.lookup.use_off_bow), true)),
            UseKind::Crossbow => Some((305, named_clip(model, model.lookup.use_off_crossbow), true)),
            UseKind::Spear => Some((306, named_clip(model, model.lookup.use_off_spear), true)),
            UseKind::Spyglass | UseKind::Horn => Some((307, named_clip(model, model.lookup.use_off_spyglass), true)),
            UseKind::Brush => Some((308, None, false)),
            UseKind::None => match st.off_cat {
                HeldCat::Empty | HeldCat::Item => None,
                HeldCat::Shield => Some((311, named_clip(model, model.lookup.hold_off_shield), true)),
                HeldCat::Sword => Some((312, named_clip(model, model.lookup.hold_off_sword), true)),
                HeldCat::Bow => Some((313, named_clip(model, model.lookup.hold_off_bow), true)),
                HeldCat::Crossbow => Some((314, named_clip(model, model.lookup.hold_off_crossbow), true)),
                HeldCat::Tool => Some((315, named_clip(model, model.lookup.hold_off_tool), true)),
                HeldCat::Fishing => Some((316, named_clip(model, model.lookup.hold_off_fishing), false)),
                HeldCat::Spear => Some((317, named_clip(model, model.lookup.hold_off_spear), true)),
            },
        }
    };

    let off_arm_key = off_clip.map(|(k, _, _)| k);
    if let Some((_, maybe_clip, hold_last)) = off_clip {
        if let Some(clip) = maybe_clip {
            let off_time = if st.local && hold_last {
                let lock = LOCAL_BLEND.get_or_init(|| Mutex::new(None));
                lock.lock()
                    .ok()
                    .and_then(|mut g| {
                        if let Some(c) = g.as_mut() {
                            if c.last_off_key != off_arm_key {
                                c.last_off_key = off_arm_key;
                                c.off_slot_start = time_sec;
                            }
                            Some((time_sec - c.off_slot_start).max(0.0))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(10.0)
            } else if hold_last {
                10.0
            } else {
                time_sec
            };
            let sample_t = if hold_last { off_time * 2.5 } else { off_time };
            apply_slot(clip, sample_t, &mut attrs, false, hold_last);
        } else {
            for (i, bone) in model.bones.iter().enumerate() {
                if bone.role == BoneRole::LeftArm {
                    attrs[i].rot[0] = match st.off_use {
                        UseKind::Block => 0.95,
                        UseKind::Eat | UseKind::Drink => 1.35 + (time_sec * 16.0).sin() * 0.15,
                        UseKind::Spyglass | UseKind::Horn => 1.75,
                        UseKind::Bow | UseKind::Crossbow => 1.45,
                        UseKind::Spear => 2.55,
                        UseKind::Brush => 1.05 + (time_sec * 10.0).sin() * 0.25,
                        UseKind::None => 0.45,
                    };
                    attrs[i].rot[1] = if st.off_use == UseKind::Block { -0.45 } else { -0.10 };
                }
            }
        }
    } else if st.local {
        if let Some(lock) = LOCAL_BLEND.get() {
            if let Ok(mut g) = lock.lock() {
                if let Some(c) = g.as_mut() {
                    c.last_off_key = None;
                }
            }
        }
    }

    // Layer 3: Head look-at + GUI prop culling + root safety
    for (i, bone) in model.bones.iter().enumerate() {
        if bone.role == BoneRole::Head && !suppress_head_look {
            attrs[i].rot[0] += -player.pitch.clamp(-55.0, 55.0).to_radians();
            attrs[i].rot[1] += -player.head_yaw.clamp(-70.0, 70.0).to_radians();
        }
        if bone.is_gui_prop {
            attrs[i].scale = [0.0, 0.0, 0.0];
        } else if bone.is_core_root
            && (attrs[i].scale[0].abs() < 1e-3
                || attrs[i].scale[1].abs() < 1e-3
                || attrs[i].scale[2].abs() < 1e-3)
        {
            attrs[i].scale = [1.0, 1.0, 1.0];
        }
    }

    // Layer 4: Smooth cross-fade blending + mainhand & offhand swing overlays (local player)
    if st.local {
        let model_ptr = model as *const YTreeModel as usize;
        let lock = LOCAL_BLEND.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = lock.lock() {
            let need_reset = match guard.as_ref() {
                Some(c) => {
                    c.model_ptr != model_ptr
                        || c.attrs.len() != attrs.len()
                        || (time_sec - c.last_time).abs() > 0.6
                }
                None => true,
            };
            if need_reset {
                *guard = Some(BlendCache {
                    model_ptr,
                    last_time: time_sec,
                    swing_start: if st.main_swinging { time_sec } else { -10.0 },
                    was_swinging: st.main_swinging,
                    off_swing_start: if st.off_swinging { time_sec } else { -10.0 },
                    was_off_swinging: st.off_swinging,
                    last_arm_key: main_arm_key,
                    arm_slot_start: time_sec,
                    last_off_key: off_arm_key,
                    off_slot_start: time_sec,
                    attrs: attrs.clone(),
                });
            } else if let Some(cache) = guard.as_mut() {
                let dt = (time_sec - cache.last_time).clamp(0.0, 0.1);
                cache.last_time = time_sec;
                if st.main_swinging && (!cache.was_swinging || (time_sec - cache.swing_start) > 0.36) {
                    cache.swing_start = time_sec;
                }
                cache.was_swinging = st.main_swinging;
                if st.off_swinging && (!cache.was_off_swinging || (time_sec - cache.off_swing_start) > 0.36) {
                    cache.off_swing_start = time_sec;
                }
                cache.was_off_swinging = st.off_swinging;

                let alpha = (dt * 14.0).clamp(0.12, 1.0);
                for (prev, target) in cache.attrs.iter_mut().zip(attrs.iter_mut()) {
                    let target_hidden = target.scale[0].abs() < 1e-3
                        || target.scale[1].abs() < 1e-3
                        || target.scale[2].abs() < 1e-3;
                    let prev_hidden = prev.scale[0].abs() < 1e-3
                        || prev.scale[1].abs() < 1e-3
                        || prev.scale[2].abs() < 1e-3;
                    if target_hidden || prev_hidden {
                        *prev = *target;
                    } else {
                        prev.rot = lerp3(prev.rot, target.rot, alpha);
                        prev.pos = lerp3(prev.pos, target.pos, alpha);
                        prev.scale = lerp3(prev.scale, target.scale, alpha);
                        *target = *prev;
                    }
                }

                // Mainhand swing overlay (0.36s)
                let swing_age = time_sec - cache.swing_start;
                if !suppress_arm_overlay && (0.0..0.36).contains(&swing_age) {
                    if let Some(ref swing_clip) = model.slots[SLOT_SWING].as_ref().filter(|c| c.has_rot_motion) {
                        apply_slot(swing_clip, swing_age, &mut attrs, false, true);
                    } else {
                        let p = swing_age / 0.36;
                        let swing = (p * std::f32::consts::PI).sin();
                        for (i, bone) in model.bones.iter().enumerate() {
                            match bone.role {
                                BoneRole::RightArm => {
                                    attrs[i].rot[0] += swing * 1.65;
                                    attrs[i].rot[1] -= swing * 0.45;
                                }
                                BoneRole::RightForeArm => {
                                    attrs[i].rot[0] += swing * 0.55;
                                }
                                BoneRole::UpperBody => {
                                    attrs[i].rot[1] -= swing * 0.25;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Offhand swing overlay (0.36s)
                let off_swing_age = time_sec - cache.off_swing_start;
                if !suppress_arm_overlay && (0.0..0.36).contains(&off_swing_age) {
                    if let Some(swing_clip) = named_clip(model, model.lookup.swing_offhand).filter(|c| c.has_rot_motion) {
                        apply_slot(swing_clip, off_swing_age, &mut attrs, false, true);
                    } else {
                        let p = off_swing_age / 0.36;
                        let swing = (p * std::f32::consts::PI).sin();
                        for (i, bone) in model.bones.iter().enumerate() {
                            match bone.role {
                                BoneRole::LeftArm => {
                                    attrs[i].rot[0] += swing * 1.65;
                                    attrs[i].rot[1] += swing * 0.45;
                                }
                                BoneRole::LeftForeArm => {
                                    attrs[i].rot[0] += swing * 0.55;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    // Exact `YesSteveModel-Native/model_state_impl.inc` DFS extraction (`scale == 0` skips subtree!)
    let mut stack = vec![MAT4_ID; model.bones.len() + 2];
    let mut right_hand: Option<(u8, Vec3, Mat4)> = None;
    let mut left_hand: Option<(u8, Vec3, Mat4)> = None;
    let mut idx = 0usize;
    while idx < model.bones.len() {
        let bone = &model.bones[idx];
        let attr = &attrs[idx];

        if attr.scale[0].abs() < 1e-4 || attr.scale[1].abs() < 1e-4 || attr.scale[2].abs() < 1e-4 {
            idx = bone.subtree_end;
            continue;
        }

        let mut m = stack[bone.depth];
        mat4_translate(
            &mut m,
            (bone.pivot[0] - attr.pos[0]) * MODEL_UNIT,
            (bone.pivot[1] + attr.pos[1]) * MODEL_UNIT,
            (bone.pivot[2] + attr.pos[2]) * MODEL_UNIT,
        );
        let locator_rot_corrupt = bone.is_locator
            && ((attr.rot[0] - attr.pos[0]).abs() < 0.02
                && (attr.rot[1] - attr.pos[1]).abs() < 0.02
                && (attr.rot[2] - attr.pos[2]).abs() < 0.02
                || ((attr.rot[0] - attr.rot[1]).abs() < 0.01
                    && (attr.rot[1] - attr.rot[2]).abs() < 0.01
                    && attr.rot[0].abs() > 0.5)
                || attr.rot[0].abs() > 3.2
                || attr.rot[1].abs() > 3.2
                || attr.rot[2].abs() > 3.2);
        let rx = if locator_rot_corrupt { bone.rest_rotation[0] } else { bone.rest_rotation[0] + attr.rot[0] };
        let ry = if locator_rot_corrupt { bone.rest_rotation[1] } else { bone.rest_rotation[1] + attr.rot[1] };
        let rz = if locator_rot_corrupt { bone.rest_rotation[2] } else { bone.rest_rotation[2] + attr.rot[2] };
        if rx != 0.0 || ry != 0.0 || rz != 0.0 {
            let rot = euler_zyx([rx, ry, rz]);
            m = mat4_mul(&m, &rot);
        }
        mat4_scale(&mut m, attr.scale[0], attr.scale[1], attr.scale[2]);
        mat4_translate(
            &mut m,
            -bone.pivot[0] * MODEL_UNIT,
            -bone.pivot[1] * MODEL_UNIT,
            -bone.pivot[2] * MODEL_UNIT,
        );
        stack[bone.depth + 1] = m;

        // Track hand locator world position & orientation for 3D held-item rendering
        match bone.role {
            BoneRole::RightHand => {
                let p = xform_pt(&m, [bone.pivot[0] * MODEL_UNIT, bone.pivot[1] * MODEL_UNIT, bone.pivot[2] * MODEL_UNIT], model.scale);
                right_hand = Some((3, p, m));
            }
            BoneRole::RightForeArm if right_hand.map_or(true, |(prio, _, _)| prio < 2) => {
                let p = xform_pt(&m, [bone.pivot[0] * MODEL_UNIT, (bone.pivot[1] - 5.0) * MODEL_UNIT, bone.pivot[2] * MODEL_UNIT], model.scale);
                right_hand = Some((2, p, m));
            }
            BoneRole::RightArm if right_hand.is_none() => {
                let p = xform_pt(&m, [bone.pivot[0] * MODEL_UNIT, (bone.pivot[1] - 9.0) * MODEL_UNIT, bone.pivot[2] * MODEL_UNIT], model.scale);
                right_hand = Some((1, p, m));
            }
            BoneRole::LeftHand => {
                let p = xform_pt(&m, [bone.pivot[0] * MODEL_UNIT, bone.pivot[1] * MODEL_UNIT, bone.pivot[2] * MODEL_UNIT], model.scale);
                left_hand = Some((3, p, m));
            }
            BoneRole::LeftForeArm if left_hand.map_or(true, |(prio, _, _)| prio < 2) => {
                let p = xform_pt(&m, [bone.pivot[0] * MODEL_UNIT, (bone.pivot[1] - 5.0) * MODEL_UNIT, bone.pivot[2] * MODEL_UNIT], model.scale);
                left_hand = Some((2, p, m));
            }
            BoneRole::LeftArm if left_hand.is_none() => {
                let p = xform_pt(&m, [bone.pivot[0] * MODEL_UNIT, (bone.pivot[1] - 9.0) * MODEL_UNIT, bone.pivot[2] * MODEL_UNIT], model.scale);
                left_hand = Some((1, p, m));
            }
            _ => {}
        }

        if !bone.quads.is_empty() {
            for q in &bone.quads {
                // Reverse winding [1, 0, 3, 2] with mirrored U [u1, v0, u0, v1] so each vertex keeps its exact UV
                // while converting left-handed Bedrock coordinates to right-handed Horizon draw space.
                let corners = [
                    xform_pt(&m, q.corners[1], model.scale),
                    xform_pt(&m, q.corners[0], model.scale),
                    xform_pt(&m, q.corners[3], model.scale),
                    xform_pt(&m, q.corners[2], model.scale),
                ];
                let uv16 = [q.uv16[2], q.uv16[1], q.uv16[0], q.uv16[3]];
                let _ = draw.quad(model.sprite, corners, uv16, tint);
            }
        }
        idx += 1;
    }

    // Render actual 3D held items directly in the character's animated right (main) & left (off) hands!
    let draw_hand_item = |draw: &mut dyn BlockEntityDrawContext,
                          stack: &ItemStack,
                          hand: Option<(u8, Vec3, Mat4)>,
                          ctx: ItemDisplayContext,
                          is_left: bool| {
        if let Some((_, pos, m)) = hand {
            // Column lengths of upper 3x3 of column-major Bedrock bone matrix m (`m[col][row]`)
            let c0 = (m[0][0] * m[0][0] + m[0][1] * m[0][1] + m[0][2] * m[0][2]).sqrt().max(1e-4);
            let c1 = (m[1][0] * m[1][0] + m[1][1] * m[1][1] + m[1][2] * m[1][2]).sqrt().max(1e-4);
            let c2 = (m[2][0] * m[2][0] + m[2][1] * m[2][1] + m[2][2] * m[2][2]).sqrt().max(1e-4);

            // Conjugate R_bed by F = diag(1, 1, -1) (Z mirror used in xform_pt)
            // to get exact proper rotation R_draw in Horizon's right-handed Y-up, +Z-forward draw space:
            let r02 = -m[2][0] / c2;
            let r10 = m[0][1] / c0;
            let r11 = m[1][1] / c1;
            let r12 = -m[2][1] / c2;
            let r22 = m[2][2] / c2;

            // Exact decomposition of R_draw = Ry(yaw) * Rx(pitch) * Rz(roll)
            let pitch_deg = (-r12).clamp(-1.0, 1.0).asin().to_degrees();
            let yaw_deg = r02.atan2(r22).to_degrees();
            let roll_deg = r10.atan2(r11).to_degrees();

            draw.push();
            draw.translate(pos);
            draw.rotate_about([0.0, 1.0, 0.0], yaw_deg, [0.0; 3]);
            draw.rotate_about([1.0, 0.0, 0.0], pitch_deg, [0.0; 3]);
            draw.rotate_about([0.0, 0.0, 1.0], roll_deg, [0.0; 3]);
            // Exact match to vanilla `base_transform` (scale(-1,-1,1)) * `hand_base` (Rx(-90°) * Ry(180°))
            // expressed in our +Y-up, +Z-forward draw space: Ry(180°) * Rx(-90°) maps (x,y,z) -> (-x, +z, +y),
            // so item +X -> -X (char right), item +Y -> +Z (char forward!), item +Z -> +Y (char up!).
            draw.rotate_about([0.0, 1.0, 0.0], 180.0, [0.0; 3]);
            draw.rotate_about([1.0, 0.0, 0.0], -90.0, [0.0; 3]);
            // Optional 3.8.0 Mace/Weapon Twirl flourish (`G` -> extra8/mace_twirl)!
            if !is_left && g_idx == 8 {
                let spin_deg = (time_sec * 420.0) % 360.0;
                draw.rotate_about([1.0, 0.0, 0.0], spin_deg, [0.0; 3]);
            }
            draw.translate([if is_left { -0.02 } else { 0.02 }, 0.02, -0.04]);
            draw.scale([0.85, 0.85, 0.85]);
            let _ = draw.item(stack, ctx, None, 0);
            draw.pop();
        }
    };
    if let Some(stack) = main_stack {
        draw_hand_item(draw, stack, right_hand, ItemDisplayContext::ThirdPersonRightHand, false);
    }
    if let Some(stack) = off_stack {
        draw_hand_item(draw, stack, left_hand, ItemDisplayContext::ThirdPersonLeftHand, true);
    }

    (true, state_name)
}
