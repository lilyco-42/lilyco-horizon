use std::fs;
use std::path::PathBuf;

use ysm_native_port::codec::animation::parse_bedrock_animations;
use ysm_native_port::codec::geo::{parse_bedrock_geo, LocalQuad};
use ysm_native_port::codec::yanim::compile_yanim;
use ysm_native_port::codec::ytree::encode_ytree;
use ysm_native_port::math::euler::mat4_transform_point;
use ysm_native_port::renderer::bone_attribute::BoneAttribute;
use ysm_native_port::renderer::model_state::ModelState;

struct AlphaGrid {
    w: usize,
    h: usize,
    alpha: Vec<u8>,
}

impl AlphaGrid {
    fn load(path: &PathBuf) -> Option<Self> {
        let bytes = fs::read(path).ok()?;
        if bytes.len() < 8 {
            return None;
        }
        let w = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let h = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        if bytes.len() < 8 + w * h {
            return None;
        }
        Some(Self {
            w,
            h,
            alpha: bytes[8..8 + w * h].to_vec(),
        })
    }

    /// 1:1 port of `YesSteveModel-Native` `QuadType::kEmpty` culling:
    /// returns `false` if every texel inside the quad's UV rectangle is transparent.
    fn is_non_empty(&self, q: &LocalQuad) -> bool {
        let u0 = (q.uv16[0].min(q.uv16[2]) / 16.0).clamp(0.0, 1.0);
        let u1 = (q.uv16[0].max(q.uv16[2]) / 16.0).clamp(0.0, 1.0);
        let v0 = (q.uv16[1].min(q.uv16[3]) / 16.0).clamp(0.0, 1.0);
        let v1 = (q.uv16[1].max(q.uv16[3]) / 16.0).clamp(0.0, 1.0);
        if (u1 - u0) < 1e-6 || (v1 - v0) < 1e-6 {
            return false;
        }
        let x0 = ((u0 * self.w as f32).floor() as usize).min(self.w.saturating_sub(1));
        let x1 = ((u1 * self.w as f32).ceil() as usize).clamp(x0 + 1, self.w);
        let y0 = ((v0 * self.h as f32).floor() as usize).min(self.h.saturating_sub(1));
        let y1 = ((v1 * self.h as f32).ceil() as usize).clamp(y0 + 1, self.h);
        for y in y0..y1 {
            let row = y * self.w;
            for x in x0..x1 {
                if self.alpha[row + x] >= 32 {
                    return true;
                }
            }
        }
        false
    }
}

fn is_core_character_bone(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l == "head"
        || l == "body"
        || l == "upperbody"
        || l == "leftleg"
        || l == "rightleg"
        || l == "leftfoot"
        || l == "rightfoot"
        || l == "leftlowerleg"
        || l == "rightlowerleg"
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("Usage: bake_ytree <main.json> <out.ytree> <target_height_m> [alpha_grid.bin]");
        std::process::exit(1);
    }
    let src = PathBuf::from(&args[0]);
    let dst = PathBuf::from(&args[1]);
    let target_h: f32 = args[2].parse().unwrap_or(1.80);
    let alpha_grid = args.get(3).and_then(|p| AlphaGrid::load(&PathBuf::from(p)));
    let anim_paths: Vec<PathBuf> = args.iter().skip(4).map(PathBuf::from).collect();

    let raw = fs::read_to_string(&src)?;
    let mut model = parse_bedrock_geo(&raw)?;

    // 1. Cull QuadType::kEmpty (transparent alpha==0) faces 1:1 with YesSteveModel-Native!
    let mut culled = 0usize;
    if let Some(ref grid) = alpha_grid {
        for (i, quads) in model.bone_quads.iter_mut().enumerate() {
            let before = quads.len();
            quads.retain(|q| grid.is_non_empty(q));
            culled += before - quads.len();
            model.bones[i].hierarchy.has_geometry = !quads.is_empty();
        }
    }

    // 2. Parse animations early so we can apply `pre_parallel*` before measuring height
    // (fixes models like gemini_cheshire whose pre_parallel scales Root/AllBody!)
    let mut clips = std::collections::HashMap::new();
    for ap in &anim_paths {
        if let Ok(araw) = fs::read_to_string(ap) {
            if let Ok(parsed) = parse_bedrock_animations(&araw) {
                clips.extend(parsed);
            }
        }
    }

    // Measure height from CORE character bones (Head/Body/Legs) AFTER `pre_parallel`!
    let mut attrs = vec![BoneAttribute::default(); model.bones.len()];
    let mut pre_clips: Vec<_> = clips
        .iter()
        .filter(|(k, _)| k.starts_with("pre_parallel"))
        .map(|(_, c)| c)
        .collect();
    pre_clips.sort_by(|a, b| a.name.cmp(&b.name));
    let ctx0 = ysm_native_port::molang::MolangContext::default();
    for c in pre_clips {
        for (i, b) in model.bones.iter().enumerate() {
            if let Some(bt) = c.bones.get(&b.name) {
                if let Some(ch) = &bt.scale {
                    let s = ch.sample(0.0, &ctx0);
                    if s[0].abs() > 1e-3 && s[1].abs() > 1e-3 && s[2].abs() > 1e-3 {
                        attrs[i].scale = [
                            attrs[i].scale[0] * s[0],
                            attrs[i].scale[1] * s[1],
                            attrs[i].scale[2] * s[2],
                        ];
                    }
                }
            }
        }
    }
    let mut state = ModelState::default();
    let _ = state.extract(&model.bones, &attrs)?;

    let mut core_min_y = f32::MAX;
    let mut core_max_y = f32::MIN;
    let mut all_min_y = f32::MAX;
    let mut all_max_y = f32::MIN;
    for &bidx in &state.render_bone_indices {
        let i = bidx as usize;
        let is_core = is_core_character_bone(&model.bones[i].name);
        let m = &state.bone_poses[i].pose.pose;
        for q in &model.bone_quads[i] {
            for c in q.corners {
                let w = mat4_transform_point(m, c);
                all_min_y = all_min_y.min(w[1]);
                all_max_y = all_max_y.max(w[1]);
                if is_core {
                    core_min_y = core_min_y.min(w[1]);
                    core_max_y = core_max_y.max(w[1]);
                }
            }
        }
    }
    // Always anchor feet at Y=0.0 so models with renamed leg bones (e.g. LeftLeg_A/B in gemini_cheshire)
    // still measure full head-to-floor height instead of waist-to-head!
    let rest_h = if core_max_y > 0.8 {
        core_max_y
    } else {
        (all_max_y - all_min_y.min(0.0)).max(0.5)
    };
    let scale = (target_h / rest_h).clamp(0.15, 1.05);

    let bytes = encode_ytree(&model, scale);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&dst, &bytes)?;
    let mut yanim_info = String::new();
    if !clips.is_empty() {
        let yanim = compile_yanim(&model.bones, &clips);
        let yanim_dst = dst.with_extension("yanim");
        fs::write(&yanim_dst, &yanim)?;
        yanim_info = format!(", yanim={}B ({} slots)", yanim.len(), yanim.get(4).copied().unwrap_or(0));
    }
    let total_quads: usize = model.bone_quads.iter().map(Vec::len).sum();
    println!(
        "Baked {} -> {} ({} bones, {} quads [culled {} empty], core_h={:.2}m, scale={:.3}{})",
        src.display(),
        dst.display(),
        model.bones.len(),
        total_quads,
        culled,
        rest_h,
        scale,
        yanim_info
    );
    Ok(())
}
