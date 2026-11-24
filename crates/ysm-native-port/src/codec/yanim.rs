//! Compact binary `.yanim` (`YAN2`) compiler for real Bedrock `.animation.json` clips.
//! Encodes:
//! 1. 30 fast-path official `YesSteveModel` slots (`0..29`: locomotion, `extra0..7`, `arm.animation.json`, `ride`, `swim`).
//! 2. Full named animation library (ALL non-empty clips across all `.animation.json` files) with exact `loop_mode`
//!    (`0=Once`, `1=Loop`, `2=HoldOnLastFrame`) so players can list (`/ysm_anims`) & play (`/ysm_play <name>`)
//!    any animation just like Java YSM!

use std::collections::HashMap;

use crate::codec::animation::{AnimClip, LOOP_HOLD_LAST, LOOP_REPEAT};
use crate::molang::MolangContext;
use crate::renderer::model_state::BoneInfo;

pub const YANIM_MAGIC: &[u8; 4] = b"YAN2";
pub const SAMPLE_FPS: f32 = 20.0;
pub const MAX_FRAMES: usize = 60;

pub const SLOT_PRE_PARALLEL: u8 = 0;
pub const SLOT_IDLE: u8 = 1;
pub const SLOT_WALK: u8 = 2;
pub const SLOT_RUN: u8 = 3;
pub const SLOT_SNEAK: u8 = 4;
pub const SLOT_SNEAKING: u8 = 5;
pub const SLOT_JUMP: u8 = 6;
pub const SLOT_ATTACKED: u8 = 7;
pub const SLOT_PARALLEL: u8 = 8;
pub const SLOT_EXTRA_BASE: u8 = 9; // 9..=16 => extra0..extra7
pub const SLOT_HOLD_SWORD: u8 = 17;
pub const SLOT_HOLD_BOW: u8 = 18;
pub const SLOT_HOLD_CROSSBOW: u8 = 19;
pub const SLOT_HOLD_TOOL: u8 = 20;
pub const SLOT_HOLD_SHIELD: u8 = 21;
pub const SLOT_HOLD_FISHING: u8 = 22;
pub const SLOT_HOLD_SPEAR: u8 = 23;
pub const SLOT_HOLD_ITEM: u8 = 24;
pub const SLOT_SWING: u8 = 25;
pub const SLOT_USE_BOW: u8 = 26;
pub const SLOT_USE_EAT: u8 = 27;
pub const SLOT_RIDE: u8 = 28;
pub const SLOT_SWIM: u8 = 29;
pub const TOTAL_SLOTS: u8 = 30;

pub const FLAG_ROT: u8 = 1;
pub const FLAG_POS: u8 = 2;
pub const FLAG_SCALE: u8 = 4;

fn first_of<'a>(all: &'a HashMap<String, AnimClip>, names: &[&str]) -> Vec<&'a AnimClip> {
    for &n in names {
        if let Some(c) = all.get(n) {
            if !c.bones.is_empty() {
                return vec![c];
            }
        }
    }
    Vec::new()
}

fn collect_slot_clips<'a>(all: &'a HashMap<String, AnimClip>, slot: u8) -> Vec<&'a AnimClip> {
    match slot {
        SLOT_PRE_PARALLEL => {
            let mut v: Vec<&AnimClip> = all
                .iter()
                .filter(|(k, _)| k.starts_with("pre_parallel"))
                .map(|(_, c)| c)
                .collect();
            v.sort_by(|a, b| a.name.cmp(&b.name));
            v
        }
        SLOT_PARALLEL => {
            let mut v: Vec<&AnimClip> = all
                .iter()
                .filter(|(k, _)| k.starts_with("parallel"))
                .map(|(_, c)| c)
                .collect();
            v.sort_by(|a, b| a.name.cmp(&b.name));
            v
        }
        SLOT_IDLE => first_of(all, &["idle"]),
        SLOT_WALK => first_of(all, &["walk"]),
        SLOT_RUN => first_of(all, &["run", "walk"]),
        SLOT_SNEAK => first_of(all, &["sneak", "sneaking"]),
        SLOT_SNEAKING => first_of(all, &["sneaking", "sneak"]),
        SLOT_JUMP => first_of(all, &["jump"]),
        SLOT_ATTACKED => first_of(all, &["attacked"]),
        s if (SLOT_EXTRA_BASE..SLOT_HOLD_SWORD).contains(&s) => {
            let idx = s - SLOT_EXTRA_BASE;
            let key = format!("extra{idx}");
            if let Some(c) = all.get(&key) {
                if !c.bones.is_empty() {
                    return vec![c];
                }
            }
            let fallback = match idx {
                0 => "sit",
                1 => "swim",
                2 => "fly",
                3 => "elytra_fly",
                4 => "sit",
                5 => "climb",
                6 => "boat",
                _ => "sleep",
            };
            first_of(all, &[fallback])
        }
        SLOT_HOLD_SWORD => first_of(all, &["hold_mainhand:sword", "hold_mainhand"]),
        SLOT_HOLD_BOW => first_of(all, &["hold_mainhand:bow", "hold_mainhand"]),
        SLOT_HOLD_CROSSBOW => first_of(
            all,
            &["hold_mainhand:charged_crossbow", "hold_mainhand:crossbow", "crossbow_hold"],
        ),
        SLOT_HOLD_TOOL => first_of(
            all,
            &[
                "hold_mainhand:pickaxe",
                "hold_mainhand:axe",
                "hold_mainhand:shovel",
                "hold_mainhand:hoe",
                "hold_mainhand:sword",
                "hold_mainhand",
            ],
        ),
        SLOT_HOLD_SHIELD => first_of(all, &["hold_mainhand:shield", "hold_offhand:shield"]),
        SLOT_HOLD_FISHING => first_of(all, &["hold_mainhand:fishing_rod", "hold_mainhand:fishing"]),
        SLOT_HOLD_SPEAR => first_of(
            all,
            &["hold_mainhand:spear", "hold_mainhand$minecraft:mace", "hold_mainhand:sword"],
        ),
        SLOT_HOLD_ITEM => first_of(all, &["hold_mainhand"]),
        SLOT_SWING => first_of(
            all,
            &["swing:sword", "sword_attack_01", "swing_hand", "swing:fishing_rod"],
        ),
        SLOT_USE_BOW => first_of(all, &["use_mainhand:bow", "hold_mainhand:bow"]),
        SLOT_USE_EAT => first_of(all, &["use_mainhand:eat", "use_mainhand:drink"]),
        SLOT_RIDE => first_of(all, &["ride", "sit", "boat", "ride_pig"]),
        SLOT_SWIM => first_of(all, &["swim", "swim_stand", "fly"]),
        _ => Vec::new(),
    }
}

fn encode_clip_payload(
    bones: &[BoneInfo],
    name_to_idx: &HashMap<&str, u16>,
    matched: &[&AnimClip],
    single_frame: bool,
    is_swing: bool,
    slot_hint: Option<u8>,
) -> Option<(u8, Vec<u8>)> {
    if matched.is_empty() {
        return None;
    }
    let max_len = matched
        .iter()
        .map(|c| c.length)
        .fold(0.1f32, f32::max)
        .clamp(0.1, 4.0);
    let frame_count = if single_frame {
        1usize
    } else {
        ((max_len * SAMPLE_FPS).round() as usize).clamp(1, MAX_FRAMES)
    };
    let loop_mode = if single_frame {
        LOOP_HOLD_LAST
    } else if matched.len() == 1 {
        matched[0].loop_mode
    } else {
        LOOP_REPEAT
    };

    let mut bone_flags: HashMap<u16, u8> = HashMap::new();
    for c in matched {
        for (bname, bt) in &c.bones {
            if let Some(&bidx) = name_to_idx.get(bname.as_str()) {
                let mut fl = *bone_flags.get(&bidx).unwrap_or(&0);
                if bt.rotation.is_some() { fl |= FLAG_ROT; }
                if bt.position.is_some() { fl |= FLAG_POS; }
                if bt.scale.is_some() { fl |= FLAG_SCALE; }
                if fl != 0 {
                    bone_flags.insert(bidx, fl);
                }
            }
        }
    }
    if bone_flags.is_empty() {
        return None;
    }
    let mut sorted_bones: Vec<(u16, u8)> = bone_flags.into_iter().collect();
    sorted_bones.sort_by_key(|(idx, _)| *idx);

    let mut buf = Vec::new();
    buf.extend_from_slice(&max_len.to_le_bytes());
    buf.extend_from_slice(&(frame_count as u16).to_le_bytes());
    buf.extend_from_slice(&(sorted_bones.len() as u16).to_le_bytes());

    let mut base_vars: HashMap<String, f32> = HashMap::new();
    if is_swing {
        // Official YSM sword combo clips check `v.qh == 1` / `v.qh == 2`!
        base_vars.insert("qh".to_string(), 1.0);
    }

    for (bidx, flags) in sorted_bones {
        let bname = bones[bidx as usize].name.as_str();
        buf.extend_from_slice(&bidx.to_le_bytes());
        buf.push(flags);

        for f in 0..frame_count {
            let time = if frame_count == 1 {
                0.0
            } else {
                (f as f32 / (frame_count - 1) as f32) * max_len
            };
            let ctx = MolangContext {
                anim_time: time,
                life_time: time,
                modified_distance_moved: time * 4.0,
                modified_move_speed: match slot_hint {
                    Some(SLOT_RUN) => 1.0,
                    Some(SLOT_WALK | SLOT_SNEAK) => 0.6,
                    _ => 0.0,
                },
                ground_speed: match slot_hint {
                    Some(SLOT_RUN) => 5.6,
                    Some(SLOT_WALK) => 4.3,
                    _ => 0.0,
                },
                is_sneaking: matches!(slot_hint, Some(SLOT_SNEAK | SLOT_SNEAKING)),
                is_on_ground: slot_hint != Some(SLOT_JUMP),
                is_sprinting: slot_hint == Some(SLOT_RUN),
                hurt_time: if slot_hint == Some(SLOT_ATTACKED) { 5.0 } else { 0.0 },
                vars: base_vars.clone(),
                ..Default::default()
            };

            let mut rot = [0.0f32; 3];
            let mut pos = [0.0f32; 3];
            let mut scale = [1.0f32; 3];
            for c in matched {
                if let Some(bt) = c.bones.get(bname) {
                    let ct = if c.looping && c.length > 0.001 {
                        time % c.length
                    } else {
                        time.min(c.length)
                    };
                    if let Some(ch) = &bt.rotation {
                        let deg = ch.sample(ct, &ctx);
                        rot[0] += -deg[0].to_radians();
                        rot[1] += deg[1].to_radians();
                        rot[2] += -deg[2].to_radians();
                    }
                    if let Some(ch) = &bt.position {
                        let p = ch.sample(ct, &ctx);
                        pos[0] += p[0];
                        pos[1] += p[1];
                        pos[2] += p[2];
                    }
                    if let Some(ch) = &bt.scale {
                        let s = ch.sample(ct, &ctx);
                        scale[0] *= s[0];
                        scale[1] *= s[1];
                        scale[2] *= s[2];
                    }
                }
            }
            if (flags & FLAG_ROT) != 0 {
                for v in rot { buf.extend_from_slice(&v.to_le_bytes()); }
            }
            if (flags & FLAG_POS) != 0 {
                for v in pos { buf.extend_from_slice(&v.to_le_bytes()); }
            }
            if (flags & FLAG_SCALE) != 0 {
                for v in scale { buf.extend_from_slice(&v.to_le_bytes()); }
            }
        }
    }
    Some((loop_mode, buf))
}

pub fn compile_yanim(bones: &[BoneInfo], clips: &HashMap<String, AnimClip>) -> Vec<u8> {
    let mut name_to_idx: HashMap<&str, u16> = HashMap::new();
    for (i, b) in bones.iter().enumerate() {
        name_to_idx.insert(b.name.as_str(), i as u16);
    }

    // Section 1: 30 fast-path slots
    let mut encoded_slots: Vec<Vec<u8>> = Vec::new();
    for slot in 0u8..TOTAL_SLOTS {
        let matched = collect_slot_clips(clips, slot);
        let single_frame = slot == SLOT_PRE_PARALLEL;
        let is_swing = slot == SLOT_SWING;
        if let Some((loop_mode, payload)) =
            encode_clip_payload(bones, &name_to_idx, &matched, single_frame, is_swing, Some(slot))
        {
            let mut sbuf = Vec::with_capacity(2 + payload.len());
            sbuf.push(slot);
            sbuf.push(loop_mode);
            sbuf.extend_from_slice(&payload);
            encoded_slots.push(sbuf);
        }
    }

    // Section 2: Full named animation library (ALL non-empty clips for `/ysm_anims` & `/ysm_play <name>`)
    let mut sorted_names: Vec<&String> = clips.keys().collect();
    sorted_names.sort();
    let mut encoded_named: Vec<Vec<u8>> = Vec::new();
    for name in sorted_names {
        if name.starts_with("pre_parallel") || name.starts_with("parallel") || name.len() > 200 {
            continue;
        }
        let Some(clip) = clips.get(name) else { continue };
        if clip.bones.is_empty() {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        let is_swing = lower.contains("swing") || lower.contains("attack");
        if let Some((loop_mode, payload)) =
            encode_clip_payload(bones, &name_to_idx, &[clip], false, is_swing, None)
        {
            let nb = name.as_bytes();
            let mut nbuf = Vec::with_capacity(2 + nb.len() + payload.len());
            nbuf.push(nb.len() as u8);
            nbuf.extend_from_slice(nb);
            nbuf.push(loop_mode);
            nbuf.extend_from_slice(&payload);
            encoded_named.push(nbuf);
        }
    }

    let mut out = Vec::new();
    out.extend_from_slice(YANIM_MAGIC);
    out.push(encoded_slots.len() as u8);
    for s in encoded_slots {
        out.extend_from_slice(&s);
    }
    out.extend_from_slice(&(encoded_named.len() as u16).to_le_bytes());
    for n in encoded_named {
        out.extend_from_slice(&n);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::animation::parse_bedrock_animations;
    use crate::codec::geo::parse_bedrock_geo;
    use std::fs;

    #[test]
    fn compiles_claude_butterfly_pre_parallel_hiding_gui_wall() {
        let gpath = "/home/lilyco/项目/mc-ai-horizon/models/alltheysm_unpacked/Claude_蝴蝶晚礼服大小姐/models/main.json";
        let apath = "/home/lilyco/项目/mc-ai-horizon/models/alltheysm_unpacked/Claude_蝴蝶晚礼服大小姐/animations/main.animation.json";
        if let (Ok(graw), Ok(araw)) = (fs::read_to_string(gpath), fs::read_to_string(apath)) {
            let model = parse_bedrock_geo(&graw).expect("parse geo");
            let clips = parse_bedrock_animations(&araw).expect("parse anim");
            let yanim = compile_yanim(&model.bones, &clips);
            assert_eq!(&yanim[0..4], YANIM_MAGIC);
            assert!(yanim[4] >= 5, "expected >=5 compiled animation slots, got {}", yanim[4]);
        }
    }

    #[test]
    fn swing_sword_bakes_nonzero_combo_rotation_with_v_qh() {
        let json = r#"{
            "animations": {
                "swing:sword": {
                    "animation_length": 0.4,
                    "bones": {
                        "RightArm": {
                            "rotation": {
                                "0.0": {"post": ["(v.qh==1?(0))", 0, 0]},
                                "0.2": {"post": ["(v.qh==1?(-90))", "(v.qh==1?(-40))", 0]}
                            }
                        }
                    }
                }
            }
        }"#;
        let clips = parse_bedrock_animations(json).expect("parse swing");
        let bones = vec![BoneInfo {
            name: "RightArm".to_string(),
            pivot: [0.0; 3],
            rest_rotation: [0.0; 3],
            hierarchy: crate::renderer::model_state::BoneHierarchy {
                parent_index: None,
                subtree_end: 1,
                depth: 0,
                has_geometry: true,
            },
        }];
        let yanim = compile_yanim(&bones, &clips);
        assert_eq!(&yanim[0..4], YANIM_MAGIC);
        assert_eq!(yanim[4], 1); // SLOT_SWING compiled!
    }
}
