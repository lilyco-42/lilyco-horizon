//! Full Minecraft Bedrock `minecraft:geometry` parser producing DFS-ordered
//! `BoneInfo` hierarchy + per-bone local `BakedQuad` geometry for `ModelState`.

use std::collections::HashMap;

use serde::Deserialize;
use thiserror::Error;

use crate::math::euler::{euler_zyx, mat4_transform_point, Vec3};
use crate::renderer::model_state::{BoneHierarchy, BoneInfo, MODEL_UNIT};

#[derive(Debug, Error)]
pub enum GeoError {
    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing minecraft:geometry array")]
    MissingGeometry,
}

#[derive(Clone, Copy, Debug)]
pub struct LocalQuad {
    pub corners: [Vec3; 4],
    pub uv16: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct ParsedModel {
    pub bones: Vec<BoneInfo>,
    pub bone_quads: Vec<Vec<LocalQuad>>,
    pub tex_size: [f32; 2],
}

#[derive(Deserialize)]
struct RawGeoFile {
    #[serde(rename = "minecraft:geometry")]
    geometries: Option<Vec<RawGeometry>>,
}

#[derive(Deserialize)]
struct RawGeometry {
    description: Option<RawDesc>,
    bones: Option<Vec<RawBone>>,
}

#[derive(Deserialize)]
struct RawDesc {
    texture_width: Option<f32>,
    texture_height: Option<f32>,
}

#[derive(Deserialize)]
struct RawBone {
    name: String,
    parent: Option<String>,
    pivot: Option<Vec3>,
    rotation: Option<Vec3>,
    cubes: Option<Vec<RawCube>>,
}

#[derive(Deserialize)]
struct RawCube {
    origin: Vec3,
    size: Vec3,
    inflate: Option<f32>,
    pivot: Option<Vec3>,
    rotation: Option<Vec3>,
    mirror: Option<bool>,
    uv: Option<RawUv>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawUv {
    Box([f32; 2]),
    Faces(HashMap<String, RawFaceUv>),
}

#[derive(Deserialize)]
struct RawFaceUv {
    uv: [f32; 2],
    uv_size: [f32; 2],
}

pub fn parse_bedrock_geo(json_str: &str) -> Result<ParsedModel, GeoError> {
    let raw: RawGeoFile = serde_json::from_str(json_str)?;
    let geo = raw
        .geometries
        .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) })
        .ok_or(GeoError::MissingGeometry)?;

    let tw = geo.description.as_ref().and_then(|d| d.texture_width).unwrap_or(64.0).max(1.0);
    let th = geo.description.as_ref().and_then(|d| d.texture_height).unwrap_or(64.0).max(1.0);
    let raw_bones = geo.bones.unwrap_or_default();

    // Build parent -> children adjacency list to emit strict DFS pre-order
    let mut name_to_raw: HashMap<String, usize> = HashMap::new();
    for (i, b) in raw_bones.iter().enumerate() {
        name_to_raw.insert(b.name.clone(), i);
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); raw_bones.len()];
    let mut roots: Vec<usize> = Vec::new();
    for (i, b) in raw_bones.iter().enumerate() {
        if let Some(p_idx) = b.parent.as_ref().and_then(|p| name_to_raw.get(p).copied()) {
            children[p_idx].push(i);
        } else {
            roots.push(i);
        }
    }

    let mut bones = Vec::with_capacity(raw_bones.len());
    let mut bone_quads = Vec::with_capacity(raw_bones.len());

    fn dfs(
        raw_idx: usize,
        parent_out: Option<u16>,
        depth: usize,
        raw_bones: &[RawBone],
        children: &[Vec<usize>],
        tw: f32,
        th: f32,
        bones: &mut Vec<BoneInfo>,
        bone_quads: &mut Vec<Vec<LocalQuad>>,
    ) {
        let out_idx = bones.len();
        let rb = &raw_bones[raw_idx];
        let pivot = rb.pivot.unwrap_or([0.0, 0.0, 0.0]);
        let rest_deg = rb.rotation.unwrap_or([0.0, 0.0, 0.0]);
        // Bedrock Euler convention in native Bedrock coords (+X=left,+Y=up,+Z=back)
        // mapped to right-handed euler_zyx: (-X, +Y, -Z) in radians!
        let rest_rad = [
            -rest_deg[0].to_radians(),
            rest_deg[1].to_radians(),
            -rest_deg[2].to_radians(),
        ];

        let mut quads = Vec::new();
        if let Some(cubes) = &rb.cubes {
            for c in cubes {
                emit_cube_quads(c, tw, th, &mut quads);
            }
        }
        let has_geometry = !quads.is_empty();
        bones.push(BoneInfo {
            name: rb.name.clone(),
            pivot,
            rest_rotation: rest_rad,
            hierarchy: BoneHierarchy {
                parent_index: parent_out,
                subtree_end: out_idx + 1,
                depth,
                has_geometry,
            },
        });
        bone_quads.push(quads);

        for &child_raw in &children[raw_idx] {
            dfs(
                child_raw,
                Some(out_idx as u16),
                depth + 1,
                raw_bones,
                children,
                tw,
                th,
                bones,
                bone_quads,
            );
        }
        bones[out_idx].hierarchy.subtree_end = bones.len();
    }

    for r in roots {
        dfs(r, None, 0, &raw_bones, &children, tw, th, &mut bones, &mut bone_quads);
    }

    Ok(ParsedModel {
        bones,
        bone_quads,
        tex_size: [tw, th],
    })
}

fn emit_cube_quads(c: &RawCube, tw: f32, th: f32, out: &mut Vec<LocalQuad>) {
    let inf = c.inflate.unwrap_or(0.0);
    let [ox, oy, oz] = c.origin;
    let [sx, sy, sz] = c.size;
    let (x0, y0, z0) = (ox - inf, oy - inf, oz - inf);
    let (x1, y1, z1) = (ox + sx + inf, oy + sy + inf, oz + sz + inf);

    let mirror = c.mirror.unwrap_or(false);
    let face_uvs = compute_face_uvs(&c.uv, sx, sy, sz, tw, th, mirror);

    // Outward CCW faces in Bedrock coordinates (before final 180-deg Y-flip at render time)
    let faces: [(&str, [Vec3; 4]); 6] = [
        ("north", [[x1, y1, z0], [x1, y0, z0], [x0, y0, z0], [x0, y1, z0]]),
        ("south", [[x0, y1, z1], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1]]),
        ("west", [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]]),
        ("east", [[x1, y1, z1], [x1, y0, z1], [x1, y0, z0], [x1, y1, z0]]),
        ("up", [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]]),
        ("down", [[x1, y0, z0], [x1, y0, z1], [x0, y0, z1], [x0, y0, z0]]),
    ];

    let rot_deg = c.rotation.unwrap_or([0.0, 0.0, 0.0]);
    let has_rot = rot_deg[0] != 0.0 || rot_deg[1] != 0.0 || rot_deg[2] != 0.0;
    let rot_mat = if has_rot {
        Some(euler_zyx([
            -rot_deg[0].to_radians(),
            rot_deg[1].to_radians(),
            -rot_deg[2].to_radians(),
        ]))
    } else {
        None
    };
    let cpiv = c.pivot.unwrap_or([0.0, 0.0, 0.0]);

    for (fname, verts) in faces {
        let Some(uv16) = face_uvs.get(fname).copied() else {
            continue;
        };
        let mut corners = [[0.0f32; 3]; 4];
        for (i, v) in verts.iter().enumerate() {
            let p = if let Some(ref m) = rot_mat {
                let rel = [v[0] - cpiv[0], v[1] - cpiv[1], v[2] - cpiv[2]];
                let r = mat4_transform_point(m, rel);
                [r[0] + cpiv[0], r[1] + cpiv[1], r[2] + cpiv[2]]
            } else {
                *v
            };
            corners[i] = [p[0] * MODEL_UNIT, p[1] * MODEL_UNIT, p[2] * MODEL_UNIT];
        }
        out.push(LocalQuad { corners, uv16 });
    }
}

fn compute_face_uvs(
    uv: &Option<RawUv>,
    sx: f32,
    sy: f32,
    sz: f32,
    tw: f32,
    th: f32,
    mirror: bool,
) -> HashMap<&'static str, [f32; 4]> {
    let mut map = HashMap::new();
    let to16 = |u0: f32, v0: f32, u1: f32, v1: f32| -> [f32; 4] {
        [
            (u0 / tw) * 16.0,
            (v0 / th) * 16.0,
            (u1 / tw) * 16.0,
            (v1 / th) * 16.0,
        ]
    };
    match uv {
        Some(RawUv::Box([u, v])) => {
            let (u, v) = (*u, *v);
            let mut n = to16(u + sz, v + sz, u + sz + sx, v + sz + sy);
            let mut s = to16(u + sz + sx + sz, v + sz, u + sz + sx + sz + sx, v + sz + sy);
            let mut w = to16(u, v + sz, u + sz, v + sz + sy);
            let mut e = to16(u + sz + sx, v + sz, u + sz + sx + sz, v + sz + sy);
            let mut up = to16(u + sz, v, u + sz + sx, v + sz);
            let mut dn = to16(u + sz + sx, v + sz, u + sz + sx + sx, v);
            if mirror {
                n = [n[2], n[1], n[0], n[3]];
                s = [s[2], s[1], s[0], s[3]];
                std::mem::swap(&mut w, &mut e);
                w = [w[2], w[1], w[0], w[3]];
                e = [e[2], e[1], e[0], e[3]];
                up = [up[2], up[1], up[0], up[3]];
                dn = [dn[2], dn[1], dn[0], dn[3]];
            }
            map.insert("north", n);
            map.insert("south", s);
            map.insert("west", w);
            map.insert("east", e);
            map.insert("up", up);
            map.insert("down", dn);
        }
        Some(RawUv::Faces(faces)) => {
            for (k, fd) in faces {
                let key: Option<&'static str> = match k.as_str() {
                    "north" => Some("north"),
                    "south" => Some("south"),
                    "west" => Some("west"),
                    "east" => Some("east"),
                    "up" => Some("up"),
                    "down" => Some("down"),
                    _ => None,
                };
                if let Some(kk) = key {
                    map.insert(
                        kk,
                        to16(fd.uv[0], fd.uv[1], fd.uv[0] + fd.uv_size[0], fd.uv[1] + fd.uv_size[1]),
                    );
                }
            }
        }
        None => {}
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_real_173_bone_deepseek_model_in_dfs_order() {
        let path = "/home/lilyco/项目/mc-ai-horizon/models/alltheysm_unpacked/DeepSeek_DeepSeek酱/models/main.json";
        if let Ok(raw) = fs::read_to_string(path) {
            let model = parse_bedrock_geo(&raw).expect("parse deepseek main.json");
            assert_eq!(model.bones.len(), 173);
            // Verify DFS subtree_end invariant for every bone
            for (i, b) in model.bones.iter().enumerate() {
                assert!(b.hierarchy.subtree_end > i && b.hierarchy.subtree_end <= model.bones.len());
                if let Some(p) = b.hierarchy.parent_index {
                    assert!((p as usize) < i, "parent must precede child in DFS order");
                    assert_eq!(b.hierarchy.depth, model.bones[p as usize].hierarchy.depth + 1);
                }
            }
            let total_quads: usize = model.bone_quads.iter().map(Vec::len).sum();
            assert!(total_quads > 3000, "expected >3000 quads, got {total_quads}");
        }
    }

    #[test]
    fn bedrock_cube_euler_zyx_places_rotated_hair_snugly_on_head() {
        let json = r#"{
            "minecraft:geometry": [{
                "description": {"texture_width": 128, "texture_height": 128},
                "bones": [{
                    "name": "Hair",
                    "pivot": [0, 37.77, 0.42],
                    "cubes": [{
                        "origin": [1.84924, 33.14972, 7.48277],
                        "size": [2, 6, 1],
                        "pivot": [2.36604, 17.10616, 2.88004],
                        "rotation": [49.1895, 86.7138, 42.8839],
                        "uv": [0, 0]
                    }]
                }]
            }]
        }"#;
        let model = parse_bedrock_geo(json).expect("parse hair cube");
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        let mut cz = 0.0f32;
        let mut n = 0.0f32;
        for q in &model.bone_quads[0] {
            for p in q.corners {
                cx += p[0] * 16.0;
                cy += p[1] * 16.0;
                cz += p[2] * 16.0;
                n += 1.0;
            }
        }
        cx /= n; cy /= n; cz /= n;
        assert!((cx - 5.38).abs() < 0.15, "expected X~5.38, got {cx}");
        assert!((cy - 36.56).abs() < 0.15, "expected Y~36.56, got {cy}");
        assert!((cz - 1.77).abs() < 0.15, "expected Z~1.77, got {cz}");
    }
}
