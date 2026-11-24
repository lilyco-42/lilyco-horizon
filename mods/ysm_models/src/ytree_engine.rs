//! 1:1 Port of `YesSteveModel-Native` (`euler.h` + `pose_stack.h` + `model_state_impl.inc`)
//! + `YesSteveModel/AnimationRegister.java` state machine + 17-slot `.yanim` clip player
//! + hierarchical bone-tree procedural fallback for models with empty locomotion/gesture stubs.

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use mod_api::{BlockEntityDrawContext, ItemDisplayContext, ItemStack, MobRender};

const MODEL_UNIT: f32 = 1.0 / 16.0;
const YTREE_MAGIC: &[u8; 4] = b"YTR1";
const YANIM_MAGIC: &[u8; 4] = b"YAN1";

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeldCat {
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UseKind {
    None = 0,
    Eat = 1,
    Bow = 2,
    Block = 3,
}

impl UseKind {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Eat,
            2 => Self::Bow,
            3 => Self::Block,
            _ => Self::None,
        }
    }
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
    length: f32,
    frame_count: usize,
    tracks: Vec<AnimTrack>,
}

pub struct YTreeModel {
    sprite: &'static str,
    scale: f32,
    bones: Vec<TreeBone>,
    slots: [Option<AnimSlot>; TOTAL_SLOTS],
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
    [-wx * scale, wy * scale, -wz * scale]
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

fn decode_yanim(b: &[u8]) -> [Option<AnimSlot>; TOTAL_SLOTS] {
    let mut slots: [Option<AnimSlot>; TOTAL_SLOTS] = std::array::from_fn(|_| None);
    if b.len() < 5 || &b[0..4] != YANIM_MAGIC {
        return slots;
    }
    let slot_count = b[4] as usize;
    let mut off = 5usize;
    for _ in 0..slot_count {
        let Some(&slot_id) = b.get(off) else { break };
        off += 1;
        let Some(length) = read_f32(b, &mut off) else { break };
        let Some(frame_count) = read_u16(b, &mut off).map(|v| v as usize) else { break };
        let Some(track_count) = read_u16(b, &mut off).map(|v| v as usize) else { break };
        let mut tracks = Vec::with_capacity(track_count);
        for _ in 0..track_count {
            let Some(bone_index) = read_u16(b, &mut off).map(|v| v as usize) else { break };
            let Some(&flags) = b.get(off) else { break };
            off += 1;
            let mut rot = if (flags & FLAG_ROT) != 0 { Vec::with_capacity(frame_count) } else { Vec::new() };
            let mut pos = if (flags & FLAG_POS) != 0 { Vec::with_capacity(frame_count) } else { Vec::new() };
            let mut scale = if (flags & FLAG_SCALE) != 0 { Vec::with_capacity(frame_count) } else { Vec::new() };
            for _ in 0..frame_count {
                if (flags & FLAG_ROT) != 0 {
                    if let Some(v) = read_vec3(b, &mut off) { rot.push(v); }
                }
                if (flags & FLAG_POS) != 0 {
                    if let Some(v) = read_vec3(b, &mut off) { pos.push(v); }
                }
                if (flags & FLAG_SCALE) != 0 {
                    if let Some(v) = read_vec3(b, &mut off) { scale.push(v); }
                }
            }
            tracks.push(AnimTrack { bone_index, flags, rot, pos, scale });
        }
        if (slot_id as usize) < TOTAL_SLOTS {
            slots[slot_id as usize] = Some(AnimSlot { length, frame_count, tracks });
        }
    }
    slots
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
        let role = classify_bone(&lower);
        bones.push(TreeBone {
            role,
            is_core_root,
            is_gui_prop,
            depth,
            subtree_end,
            pivot,
            rest_rotation,
            quads,
        });
    }
    let slots = load_file(slug, "yanim")
        .map(|yb| decode_yanim(&yb))
        .unwrap_or_else(|| std::array::from_fn(|_| None));
    Some(YTreeModel { sprite, scale, bones, slots })
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

fn apply_slot(slot: &AnimSlot, time_sec: f32, attrs: &mut [BoneAttr], additive: bool) {
    if slot.frame_count == 0 {
        return;
    }
    let (f0, f1, frac) = if slot.frame_count == 1 || slot.length <= 0.001 {
        (0, 0, 0.0)
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
            _ => {
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
        }
    }
}

struct BlendCache {
    model_ptr: usize,
    last_time: f32,
    swing_start: f32,
    was_swinging: bool,
    attrs: Vec<BoneAttr>,
}

static LOCAL_BLEND: OnceLock<Mutex<Option<BlendCache>>> = OnceLock::new();

/// Evaluate official `AnimationRegister.java` state machine + `YesSteveModel-Native` `ModelState::Extract`
/// with real `arm.animation.json` held-item / use / swing overlays, riding pose, silky cross-fade blending,
/// and 3D held item rendering directly on the animated hand locator bones!
#[allow(clippy::too_many_arguments)]
pub fn draw_ytree_model(
    draw: &mut dyn BlockEntityDrawContext,
    model: &YTreeModel,
    g_idx: usize,
    player: &MobRender<'_>,
    local: bool,
    riding: bool,
    crouching: bool,
    moving: bool,
    sprinting: bool,
    jumping: bool,
    swinging: bool,
    held_cat: HeldCat,
    use_kind: UseKind,
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
    let is_moving = moving || walk_speed > 0.03;
    let in_air = if local { jumping } else { !player.on_ground };
    let tint = if player.hurt_time > 0 { 0xFF8888 } else { 0xFFFFFF };

    let mut attrs = vec![BoneAttr::default(); model.bones.len()];

    // Layer 0: `pre_parallel` (hides Gui wall, Kuang, ysmGlowLieFeng, hidden weapons/effects!)
    if let Some(ref pre) = model.slots[SLOT_PRE_PARALLEL] {
        apply_slot(pre, time_sec, &mut attrs, false);
    }

    // Layer 1: Primary animation state (official `AnimationRegister.java` priority + `G` extra gestures!)
    let active_gesture =
        g_idx > 0 && !riding && !is_moving && !crouching && !in_air && player.hurt_time == 0;
    let state_name: &'static str;

    if active_gesture {
        let extra_slot = (SLOT_EXTRA_BASE + g_idx).min(SLOT_HOLD_SWORD - 1);
        state_name = match g_idx {
            1 => "extra1/wave",
            2 => "extra2/cheer",
            3 => "extra3/work",
            4 => "extra4/sit",
            5 => "extra5/dance",
            6 => "extra6/bow",
            _ => "extra7/victory",
        };
        if let Some(ref clip) = model.slots[extra_slot] {
            apply_slot(clip, time_sec, &mut attrs, false);
        } else {
            if let Some(ref idle) = model.slots[SLOT_IDLE] {
                apply_slot(idle, time_sec, &mut attrs, false);
            }
            apply_procedural_gesture(&model.bones, &mut attrs, g_idx, time_sec);
        }
    } else {
        let (primary_slot, sname) = if player.hurt_time > 0 {
            (SLOT_ATTACKED, "attacked")
        } else if riding {
            (SLOT_RIDE, "ride")
        } else if in_air {
            (SLOT_JUMP, "jump")
        } else if crouching && is_moving {
            (SLOT_SNEAK, "sneak")
        } else if crouching {
            (SLOT_SNEAKING, "sneaking")
        } else if (sprinting && is_moving) || walk_speed > 0.65 {
            (SLOT_RUN, "run")
        } else if is_moving {
            (SLOT_WALK, "walk")
        } else {
            (SLOT_IDLE, "idle")
        };
        state_name = sname;
        if let Some(ref clip) = model.slots[primary_slot] {
            apply_slot(clip, time_sec, &mut attrs, false);
        } else {
            // Model's `main.animation.json` omitted this slot (e.g. controller-driven walk/sneak):
            // play smooth hierarchical bone-tree locomotion so every model animates naturally!
            apply_procedural_locomotion(&model.bones, &mut attrs, primary_slot, time_sec);
        }
    }

    // Layer 2: `parallel` (official blinking, hair physics, skirt physics)
    if let Some(ref par) = model.slots[SLOT_PARALLEL] {
        apply_slot(par, time_sec, &mut attrs, true);
    }

    // Layer 2.5: Official YSM `arm.animation.json` held-item & item-use overlay!
    // Overrides arm bones with the model's real hold_mainhand:* / use_mainhand:* clip
    // (or clean procedural arm-hold fallback if the model omits that clip).
    if !active_gesture {
        let arm_slot = match use_kind {
            UseKind::Bow => Some(SLOT_USE_BOW),
            UseKind::Eat => Some(SLOT_USE_EAT),
            UseKind::Block => Some(SLOT_HOLD_SHIELD),
            UseKind::None => match held_cat {
                HeldCat::Empty => None,
                HeldCat::Sword => Some(SLOT_HOLD_SWORD),
                HeldCat::Bow => Some(SLOT_HOLD_BOW),
                HeldCat::Crossbow => Some(SLOT_HOLD_CROSSBOW),
                HeldCat::Tool => Some(SLOT_HOLD_TOOL),
                HeldCat::Shield => Some(SLOT_HOLD_SHIELD),
                HeldCat::Fishing => Some(SLOT_HOLD_FISHING),
                HeldCat::Spear => Some(SLOT_HOLD_SPEAR),
                HeldCat::Item => Some(SLOT_HOLD_ITEM),
            },
        };
        if let Some(aslot) = arm_slot {
            if let Some(ref clip) = model.slots[aslot].as_ref().or(model.slots[SLOT_HOLD_ITEM].as_ref()) {
                apply_slot(clip, time_sec, &mut attrs, false);
            } else {
                // Procedural held-item fallback on right arm (or two-handed bow/crossbow)
                for (i, bone) in model.bones.iter().enumerate() {
                    match bone.role {
                        BoneRole::RightArm => {
                            attrs[i].rot[0] = match use_kind {
                                UseKind::Bow => 1.45,
                                UseKind::Eat => 1.35 + (time_sec * 16.0).sin() * 0.15,
                                UseKind::Block => 0.95,
                                UseKind::None => match held_cat {
                                    HeldCat::Bow | HeldCat::Crossbow => 1.25,
                                    _ => 0.45,
                                },
                            };
                            attrs[i].rot[1] = if use_kind == UseKind::Block { 0.45 } else { 0.12 };
                        }
                        BoneRole::LeftArm
                            if use_kind == UseKind::Bow
                                || held_cat == HeldCat::Bow
                                || held_cat == HeldCat::Crossbow =>
                        {
                            attrs[i].rot[0] = 1.40;
                            attrs[i].rot[1] = -0.35;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Layer 3: Head look-at + GUI prop culling + root safety
    for (i, bone) in model.bones.iter().enumerate() {
        if bone.role == BoneRole::Head {
            attrs[i].rot[0] += -player.pitch.clamp(-55.0, 55.0).to_radians();
            attrs[i].rot[1] += player.head_yaw.clamp(-70.0, 70.0).to_radians();
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

    // Layer 4: Smooth cross-fade blending + left-click arm swing overlay (local player)
    if local {
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
                    swing_start: if swinging { time_sec } else { -10.0 },
                    was_swinging: swinging,
                    attrs: attrs.clone(),
                });
            } else if let Some(cache) = guard.as_mut() {
                let dt = (time_sec - cache.last_time).clamp(0.0, 0.1);
                cache.last_time = time_sec;
                if swinging && (!cache.was_swinging || (time_sec - cache.swing_start) > 0.28) {
                    cache.swing_start = time_sec;
                }
                cache.was_swinging = swinging;

                let alpha = (dt * 14.0).clamp(0.12, 1.0);
                for (prev, target) in cache.attrs.iter_mut().zip(attrs.iter_mut()) {
                    // Instant hide/show for zero-scale GUI/effect subtrees so they never flash
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

                // Real YSM `swing:sword` / `sword_attack_01` clip OR additive slash swing (0.28s)
                let swing_age = time_sec - cache.swing_start;
                if (0.0..0.28).contains(&swing_age) {
                    if let Some(ref swing_clip) = model.slots[SLOT_SWING] {
                        apply_slot(swing_clip, swing_age, &mut attrs, false);
                    } else {
                        let p = swing_age / 0.28;
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
        let rx = bone.rest_rotation[0] + attr.rot[0];
        let ry = bone.rest_rotation[1] + attr.rot[1];
        let rz = bone.rest_rotation[2] + attr.rot[2];
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
                let corners = [
                    xform_pt(&m, q.corners[0], model.scale),
                    xform_pt(&m, q.corners[1], model.scale),
                    xform_pt(&m, q.corners[2], model.scale),
                    xform_pt(&m, q.corners[3], model.scale),
                ];
                let _ = draw.quad(model.sprite, corners, q.uv16, tint);
            }
        }
        idx += 1;
    }

    // Render actual 3D held items directly in the character's animated right & left hands!
    let draw_hand_item = |draw: &mut dyn BlockEntityDrawContext,
                          stack: &ItemStack,
                          hand: Option<(u8, Vec3, Mat4)>,
                          ctx: ItemDisplayContext| {
        if let Some((_, pos, m)) = hand {
            // Extract bone Euler angles from upper 3x3 of m (matching 180-deg Y flip)
            let sx = (m[0][0] * m[0][0] + m[0][1] * m[0][1] + m[0][2] * m[0][2]).sqrt().max(1e-4);
            let sy = (m[1][0] * m[1][0] + m[1][1] * m[1][1] + m[1][2] * m[1][2]).sqrt().max(1e-4);
            let sz = (m[2][0] * m[2][0] + m[2][1] * m[2][1] + m[2][2] * m[2][2]).sqrt().max(1e-4);
            let r02 = (m[0][2] / sx).clamp(-1.0, 1.0);
            let pitch_deg = (m[1][2] / sy).atan2(m[2][2] / sz).to_degrees();
            let yaw_deg = (-r02).asin().to_degrees();
            let roll_deg = (m[0][1] / sx).atan2(m[0][0] / sx).to_degrees();

            draw.push();
            draw.translate(pos);
            draw.rotate_about([0.0, 1.0, 0.0], yaw_deg, [0.0; 3]);
            draw.rotate_about([1.0, 0.0, 0.0], -pitch_deg, [0.0; 3]);
            draw.rotate_about([0.0, 0.0, 1.0], -roll_deg, [0.0; 3]);
            draw.scale([0.90, 0.90, 0.90]);
            let _ = draw.item(stack, ctx, None, 0);
            draw.pop();
        }
    };
    if let Some(stack) = main_stack {
        draw_hand_item(draw, stack, right_hand, ItemDisplayContext::ThirdPersonRightHand);
    }
    if let Some(stack) = off_stack {
        draw_hand_item(draw, stack, left_hand, ItemDisplayContext::ThirdPersonLeftHand);
    }

    (true, state_name)
}
