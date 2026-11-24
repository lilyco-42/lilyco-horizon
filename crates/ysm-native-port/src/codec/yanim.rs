//! Compact binary `.yanim` compiler & reader for real Bedrock `main.animation.json` clips.
//! Follows official `YesSteveModel/AnimationRegister.java` slot order:
//! 0=pre_parallel, 1=idle, 2=walk, 3=run, 4=sneak, 5=sneaking, 6=jump, 7=attacked, 8=parallel.

use std::collections::HashMap;

use crate::codec::animation::AnimClip;
use crate::molang::MolangContext;
use crate::renderer::model_state::BoneInfo;

pub const YANIM_MAGIC: &[u8; 4] = b"YAN1";
pub const SAMPLE_FPS: f32 = 20.0;
pub const MAX_FRAMES: usize = 80;

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

fn collect_slot_clips<'a>(
    all: &'a HashMap<String, AnimClip>,
    slot: u8,
) -> Vec<&'a AnimClip> {
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
        SLOT_HOLD_SPEAR => first_of(all, &["hold_mainhand:spear", "hold_mainhand$minecraft:mace", "hold_mainhand:sword"]),
        // Official YSM ConditionalHold.java: plain blocks/misc items do NOT trigger tool/sword hold poses!
        SLOT_HOLD_ITEM => first_of(all, &["hold_mainhand"]),
        SLOT_SWING => first_of(all, &["swing:sword", "sword_attack_01", "swing_hand", "swing:fishing_rod"]),
        SLOT_USE_BOW => first_of(all, &["use_mainhand:bow", "hold_mainhand:bow"]),
        SLOT_USE_EAT => first_of(all, &["use_mainhand:eat", "use_mainhand:drink"]),
        SLOT_RIDE => first_of(all, &["ride", "sit", "boat", "ride_pig"]),
        SLOT_SWIM => first_of(all, &["swim", "swim_stand", "fly"]),
        _ => Vec::new(),
    }
}

pub fn compile_yanim(bones: &[BoneInfo], clips: &HashMap<String, AnimClip>) -> Vec<u8> {
    let mut name_to_idx: HashMap<&str, u16> = HashMap::new();
    for (i, b) in bones.iter().enumerate() {
        name_to_idx.insert(b.name.as_str(), i as u16);
    }

    let mut encoded_slots: Vec<Vec<u8>> = Vec::new();
    for slot in 0u8..TOTAL_SLOTS {
        let matched = collect_slot_clips(clips, slot);
        if matched.is_empty() {
            continue;
        }
        let max_len = matched
            .iter()
            .map(|c| c.length)
            .fold(0.1f32, f32::max)
            .clamp(0.1, 4.0);
        let frame_count = if slot == SLOT_PRE_PARALLEL {
            1usize
        } else {
            ((max_len * SAMPLE_FPS).round() as usize).clamp(1, MAX_FRAMES)
        };

        // Gather all touched bone indices & their combined flags across matched clips
        let mut bone_flags: HashMap<u16, u8> = HashMap::new();
        for c in &matched {
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
            continue;
        }
        let mut sorted_bones: Vec<(u16, u8)> = bone_flags.into_iter().collect();
        sorted_bones.sort_by_key(|(idx, _)| *idx);

        let mut sbuf = Vec::new();
        sbuf.push(slot);
        sbuf.extend_from_slice(&max_len.to_le_bytes());
        sbuf.extend_from_slice(&(frame_count as u16).to_le_bytes());
        sbuf.extend_from_slice(&(sorted_bones.len() as u16).to_le_bytes());

        for (bidx, flags) in sorted_bones {
            let bname = bones[bidx as usize].name.as_str();
            sbuf.extend_from_slice(&bidx.to_le_bytes());
            sbuf.push(flags);

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
                    modified_move_speed: if slot == SLOT_RUN {
                        1.0
                    } else if slot == SLOT_WALK || slot == SLOT_SNEAK {
                        0.6
                    } else {
                        0.0
                    },
                    ground_speed: if slot == SLOT_RUN { 5.6 } else if slot == SLOT_WALK { 4.3 } else { 0.0 },
                    is_sneaking: slot == SLOT_SNEAK || slot == SLOT_SNEAKING,
                    is_on_ground: slot != SLOT_JUMP,
                    is_sprinting: slot == SLOT_RUN,
                    hurt_time: if slot == SLOT_ATTACKED { 5.0 } else { 0.0 },
                    ..Default::default()
                };

                let mut rot = [0.0f32; 3];
                let mut pos = [0.0f32; 3];
                let mut scale = [1.0f32; 3];
                for c in &matched {
                    if let Some(bt) = c.bones.get(bname) {
                        let ct = if c.looping && c.length > 0.001 {
                            time % c.length
                        } else {
                            time.min(c.length)
                        };
                        if let Some(ch) = &bt.rotation {
                            let deg = ch.sample(ct, &ctx);
                            // Convert Bedrock degrees to right-handed ZYX radians (-X, +Y, -Z)
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
                    for v in rot { sbuf.extend_from_slice(&v.to_le_bytes()); }
                }
                if (flags & FLAG_POS) != 0 {
                    for v in pos { sbuf.extend_from_slice(&v.to_le_bytes()); }
                }
                if (flags & FLAG_SCALE) != 0 {
                    for v in scale { sbuf.extend_from_slice(&v.to_le_bytes()); }
                }
            }
        }
        encoded_slots.push(sbuf);
    }

    let mut out = Vec::new();
    out.extend_from_slice(YANIM_MAGIC);
    out.push(encoded_slots.len() as u8);
    for s in encoded_slots {
        out.extend_from_slice(&s);
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
        let base = "/home/lilyco/项目/mc-ai-horizon/models/alltheysm_unpacked/Claude_蝴蝶晚礼服大小姐";
        if let (Ok(geo_raw), Ok(anim_raw)) = (
            fs::read_to_string(format!("{base}/models/main.json")),
            fs::read_to_string(format!("{base}/animations/main.animation.json")),
        ) {
            let model = parse_bedrock_geo(&geo_raw).expect("geo");
            let clips = parse_bedrock_animations(&anim_raw).expect("anim");
            let yanim = compile_yanim(&model.bones, &clips);
            assert_eq!(&yanim[0..4], YANIM_MAGIC);
            assert!(yanim[4] >= 4, "expected pre_parallel + locomotion clips");
        }
    }
}
