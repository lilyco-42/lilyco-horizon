//! Full Minecraft Bedrock `.animation.json` parser & Catmull-Rom/Linear keyframe sampler.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::math::euler::Vec3;
use crate::molang::{eval_molang, MolangContext};

#[derive(Debug, Error)]
pub enum AnimError {
    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug)]
pub enum ExprVal {
    Num(f32),
    Molang(String),
}

impl ExprVal {
    pub fn eval(&self, ctx: &MolangContext) -> f32 {
        match self {
            Self::Num(n) => *n,
            Self::Molang(s) => eval_molang(s, ctx),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExprVec3 {
    pub x: ExprVal,
    pub y: ExprVal,
    pub z: ExprVal,
}

impl ExprVec3 {
    pub fn eval(&self, ctx: &MolangContext) -> Vec3 {
        [self.x.eval(ctx), self.y.eval(ctx), self.z.eval(ctx)]
    }
}

#[derive(Clone, Debug)]
pub struct Keyframe {
    pub time: f32,
    pub post: ExprVec3,
    pub catmullrom: bool,
}

#[derive(Clone, Debug)]
pub enum Channel {
    Constant(ExprVec3),
    Keyframes(Vec<Keyframe>),
}

impl Channel {
    pub fn sample(&self, time: f32, ctx: &MolangContext) -> Vec3 {
        match self {
            Self::Constant(v) => v.eval(ctx),
            Self::Keyframes(kfs) => {
                if kfs.is_empty() {
                    return [0.0, 0.0, 0.0];
                }
                if kfs.len() == 1 || time <= kfs[0].time {
                    return kfs[0].post.eval(ctx);
                }
                let last = kfs.len() - 1;
                if time >= kfs[last].time {
                    return kfs[last].post.eval(ctx);
                }
                let mut i = 0;
                while i + 1 < kfs.len() && kfs[i + 1].time < time {
                    i += 1;
                }
                let k0 = &kfs[i];
                let k1 = &kfs[i + 1];
                let dt = (k1.time - k0.time).max(1e-6);
                let t = ((time - k0.time) / dt).clamp(0.0, 1.0);
                let p1 = k0.post.eval(ctx);
                let p2 = k1.post.eval(ctx);
                if k0.catmullrom || k1.catmullrom {
                    let p0 = if i > 0 { kfs[i - 1].post.eval(ctx) } else { p1 };
                    let p3 = if i + 2 < kfs.len() { kfs[i + 2].post.eval(ctx) } else { p2 };
                    catmull_rom_vec3(p0, p1, p2, p3, t)
                } else {
                    [
                        p1[0] + (p2[0] - p1[0]) * t,
                        p1[1] + (p2[1] - p1[1]) * t,
                        p1[2] + (p2[2] - p1[2]) * t,
                    ]
                }
            }
        }
    }
}

fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

fn catmull_rom_vec3(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    [
        catmull_rom(p0[0], p1[0], p2[0], p3[0], t),
        catmull_rom(p0[1], p1[1], p2[1], p3[1], t),
        catmull_rom(p0[2], p1[2], p2[2], p3[2], t),
    ]
}

#[derive(Clone, Debug, Default)]
pub struct BoneTrack {
    pub rotation: Option<Channel>,
    pub position: Option<Channel>,
    pub scale: Option<Channel>,
}

pub const LOOP_ONCE: u8 = 0;
pub const LOOP_REPEAT: u8 = 1;
pub const LOOP_HOLD_LAST: u8 = 2;

#[derive(Clone, Debug)]
pub struct AnimClip {
    pub name: String,
    pub looping: bool,
    pub loop_mode: u8,
    pub length: f32,
    pub bones: HashMap<String, BoneTrack>,
}

#[derive(Deserialize)]
struct RawAnimFile {
    animations: Option<HashMap<String, RawAnim>>,
}

#[derive(Deserialize)]
struct RawAnim {
    #[serde(rename = "loop")]
    loop_val: Option<Value>,
    animation_length: Option<f32>,
    bones: Option<HashMap<String, RawBoneTrack>>,
}

#[derive(Deserialize)]
struct RawBoneTrack {
    rotation: Option<Value>,
    position: Option<Value>,
    scale: Option<Value>,
}

pub fn parse_bedrock_animations(json_str: &str) -> Result<HashMap<String, AnimClip>, AnimError> {
    let raw: RawAnimFile = serde_json::from_str(json_str)?;
    let mut out = HashMap::new();
    for (name, anim) in raw.animations.unwrap_or_default() {
        let (looping, loop_mode) = match anim.loop_val {
            Some(Value::Bool(true)) => (true, LOOP_REPEAT),
            Some(Value::Bool(false)) => (false, LOOP_ONCE),
            Some(Value::String(ref s)) if s.eq_ignore_ascii_case("true") => (true, LOOP_REPEAT),
            Some(Value::String(ref s)) if s.eq_ignore_ascii_case("hold_on_last_frame") => {
                (false, LOOP_HOLD_LAST)
            }
            _ => (false, LOOP_ONCE),
        };
        let mut bones = HashMap::new();
        for (bname, bt) in anim.bones.unwrap_or_default() {
            bones.insert(
                bname,
                BoneTrack {
                    rotation: bt.rotation.as_ref().and_then(parse_channel),
                    position: bt.position.as_ref().and_then(parse_channel),
                    scale: bt.scale.as_ref().and_then(parse_channel),
                },
            );
        }
        out.insert(
            name.clone(),
            AnimClip {
                name,
                looping,
                loop_mode,
                length: anim.animation_length.unwrap_or(1.0).max(0.001),
                bones,
            },
        );
    }
    Ok(out)
}

fn parse_expr(v: &Value) -> ExprVal {
    match v {
        Value::Number(n) => ExprVal::Num(n.as_f64().unwrap_or(0.0) as f32),
        Value::String(s) => ExprVal::Molang(s.clone()),
        _ => ExprVal::Num(0.0),
    }
}

fn parse_vec3(v: &Value) -> Option<ExprVec3> {
    match v {
        Value::Array(arr) if arr.len() >= 3 => Some(ExprVec3 {
            x: parse_expr(&arr[0]),
            y: parse_expr(&arr[1]),
            z: parse_expr(&arr[2]),
        }),
        Value::Number(_) | Value::String(_) => {
            let e = parse_expr(v);
            Some(ExprVec3 {
                x: e.clone(),
                y: e.clone(),
                z: e,
            })
        }
        _ => None,
    }
}

fn parse_channel(v: &Value) -> Option<Channel> {
    match v {
        Value::Array(_) | Value::Number(_) | Value::String(_) => {
            parse_vec3(v).map(Channel::Constant)
        }
        Value::Object(map) => {
            if let Some(vec_val) = map.get("vector") {
                return parse_vec3(vec_val).map(Channel::Constant);
            }
            let mut kfs = Vec::new();
            for (k, kv) in map {
                let Ok(t) = k.parse::<f32>() else { continue };
                let (post, catmull) = match kv {
                    Value::Object(obj) => {
                        let p = obj
                            .get("post")
                            .or_else(|| obj.get("vector"))
                            .and_then(parse_vec3)?;
                        let cm = obj
                            .get("lerp_mode")
                            .and_then(Value::as_str)
                            .map(|s| s.eq_ignore_ascii_case("catmullrom"))
                            .unwrap_or(false);
                        (p, cm)
                    }
                    _ => (parse_vec3(kv)?, false),
                };
                kfs.push(Keyframe {
                    time: t,
                    post,
                    catmullrom: catmull,
                });
            }
            kfs.sort_by(|a, b| a.time.total_cmp(&b.time));
            Some(Channel::Keyframes(kfs))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_and_evaluates_real_deepseek_main_animation() {
        let path = "/home/lilyco/项目/mc-ai-horizon/models/alltheysm_unpacked/DeepSeek_DeepSeek酱/animations/main.animation.json";
        if let Ok(raw) = fs::read_to_string(path) {
            let clips = parse_bedrock_animations(&raw).expect("parse main.animation.json");
            assert!(!clips.is_empty());
            let ctx = MolangContext {
                anim_time: 0.45,
                life_time: 5.0,
                modified_distance_moved: 12.0,
                modified_move_speed: 0.7,
                is_on_ground: true,
                ..Default::default()
            };
            for clip in clips.values() {
                for track in clip.bones.values() {
                    if let Some(ch) = &track.rotation {
                        let r = ch.sample(0.25, &ctx);
                        assert!(r.iter().all(|v| v.is_finite()));
                    }
                }
            }
        }
    }
}
