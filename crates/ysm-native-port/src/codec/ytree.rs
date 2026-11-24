//! Compact binary `.ytree` serializer/deserializer for full hierarchical YSM bone trees.
//! Magic: `YTR1` (4 bytes) + `bone_count: u16` + `scale: f32`
//! Per bone:
//!   - `name_len: u8`, `name_bytes: [u8; name_len]`
//!   - `parent_index: i16` (-1 for root), `depth: u16`, `subtree_end: u16`
//!   - `pivot: [f32; 3]`, `rest_rotation: [f32; 3]`
//!   - `quad_count: u32`, followed by `quad_count` × 64-byte `LocalQuad` (`[4*3*f32, 4*f32]`).

use crate::codec::geo::{LocalQuad, ParsedModel};
use crate::renderer::model_state::{BoneHierarchy, BoneInfo};

pub const YTREE_MAGIC: &[u8; 4] = b"YTR1";

pub fn encode_ytree(model: &ParsedModel, model_scale: f32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(65536);
    buf.extend_from_slice(YTREE_MAGIC);
    buf.extend_from_slice(&(model.bones.len() as u16).to_le_bytes());
    buf.extend_from_slice(&model_scale.to_le_bytes());

    for (b, quads) in model.bones.iter().zip(model.bone_quads.iter()) {
        let name_bytes = b.name.as_bytes();
        let nlen = name_bytes.len().min(255) as u8;
        buf.push(nlen);
        buf.extend_from_slice(&name_bytes[..nlen as usize]);

        let parent_i16: i16 = b.hierarchy.parent_index.map(|p| p as i16).unwrap_or(-1);
        buf.extend_from_slice(&parent_i16.to_le_bytes());
        buf.extend_from_slice(&(b.hierarchy.depth as u16).to_le_bytes());
        buf.extend_from_slice(&(b.hierarchy.subtree_end as u16).to_le_bytes());

        for v in b.pivot {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        for v in b.rest_rotation {
            buf.extend_from_slice(&v.to_le_bytes());
        }

        buf.extend_from_slice(&(quads.len() as u32).to_le_bytes());
        for q in quads {
            for c in q.corners {
                for axis in c {
                    buf.extend_from_slice(&axis.to_le_bytes());
                }
            }
            for uv in q.uv16 {
                buf.extend_from_slice(&uv.to_le_bytes());
            }
        }
    }
    buf
}

pub fn decode_ytree(bytes: &[u8]) -> Option<(ParsedModel, f32)> {
    if bytes.len() < 10 || &bytes[0..4] != YTREE_MAGIC {
        return None;
    }
    let mut off = 4usize;
    let bone_count = u16::from_le_bytes([bytes[off], bytes[off + 1]]) as usize;
    off += 2;
    let scale = read_f32(bytes, &mut off)?;

    let mut bones = Vec::with_capacity(bone_count);
    let mut bone_quads = Vec::with_capacity(bone_count);
    for _ in 0..bone_count {
        let nlen = *bytes.get(off)? as usize;
        off += 1;
        let name = std::str::from_utf8(bytes.get(off..off + nlen)?).ok()?.to_string();
        off += nlen;

        let parent_i16 = i16::from_le_bytes([*bytes.get(off)?, *bytes.get(off + 1)?]);
        off += 2;
        let depth = u16::from_le_bytes([*bytes.get(off)?, *bytes.get(off + 1)?]) as usize;
        off += 2;
        let subtree_end = u16::from_le_bytes([*bytes.get(off)?, *bytes.get(off + 1)?]) as usize;
        off += 2;

        let pivot = [read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?];
        let rest_rotation = [read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?];

        let q_count = u32::from_le_bytes([
            *bytes.get(off)?,
            *bytes.get(off + 1)?,
            *bytes.get(off + 2)?,
            *bytes.get(off + 3)?,
        ]) as usize;
        off += 4;

        let mut quads = Vec::with_capacity(q_count);
        for _ in 0..q_count {
            let c0 = [read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?];
            let c1 = [read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?];
            let c2 = [read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?];
            let c3 = [read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?, read_f32(bytes, &mut off)?];
            let uv16 = [
                read_f32(bytes, &mut off)?,
                read_f32(bytes, &mut off)?,
                read_f32(bytes, &mut off)?,
                read_f32(bytes, &mut off)?,
            ];
            quads.push(LocalQuad {
                corners: [c0, c1, c2, c3],
                uv16,
            });
        }

        bones.push(BoneInfo {
            name,
            pivot,
            rest_rotation,
            hierarchy: BoneHierarchy {
                parent_index: if parent_i16 >= 0 { Some(parent_i16 as u16) } else { None },
                subtree_end,
                depth,
                has_geometry: !quads.is_empty(),
            },
        });
        bone_quads.push(quads);
    }

    Some((
        ParsedModel {
            bones,
            bone_quads,
            tex_size: [128.0, 128.0],
        },
        scale,
    ))
}

fn read_f32(bytes: &[u8], off: &mut usize) -> Option<f32> {
    let slice = bytes.get(*off..*off + 4)?;
    *off += 4;
    Some(f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::geo::parse_bedrock_geo;
    use std::fs;

    #[test]
    fn roundtrips_173_bone_deepseek_ytree_losslessly() {
        let path = "/home/lilyco/项目/mc-ai-horizon/models/alltheysm_unpacked/DeepSeek_DeepSeek酱/models/main.json";
        if let Ok(raw) = fs::read_to_string(path) {
            let model = parse_bedrock_geo(&raw).expect("parse geo");
            let encoded = encode_ytree(&model, 0.66);
            let (decoded, scale) = decode_ytree(&encoded).expect("decode ytree");
            assert_eq!(decoded.bones.len(), model.bones.len());
            assert!((scale - 0.66).abs() < 1e-6);
            for (a, b) in model.bones.iter().zip(decoded.bones.iter()) {
                assert_eq!(a.name, b.name);
                assert_eq!(a.hierarchy.depth, b.hierarchy.depth);
                assert_eq!(a.hierarchy.subtree_end, b.hierarchy.subtree_end);
            }
        }
    }
}
