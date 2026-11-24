//! 1:1 Rust port of `YesSteveModel-Native/modules/gfx/include/gfx/renderer/model_state.h`
//! and `modules/gfx/src/renderer/model_state_impl.inc`.

use crate::math::euler::Vec3;
use crate::math::pose_stack::{Pose, PoseStack};
use crate::renderer::bone_attribute::BoneAttribute;

pub const MODEL_UNIT: f32 = 1.0 / 16.0;

#[derive(Clone, Debug)]
pub struct BoneInfo {
    pub name: String,
    pub pivot: Vec3,
    pub rest_rotation: Vec3,
    pub hierarchy: BoneHierarchy,
}

#[derive(Clone, Copy, Debug)]
pub struct BoneHierarchy {
    pub parent_index: Option<u16>,
    pub subtree_end: usize,
    pub depth: usize,
    pub has_geometry: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BonePose {
    pub pose: Pose,
    pub color: u32,
    pub glowing: u8,
    pub visible: bool,
}

#[derive(Default)]
pub struct ModelState {
    pub bone_poses: Vec<BonePose>,
    pub render_bone_indices: Vec<u16>,
    pose_stack: PoseStack,
}

/// Exact port of `ApplyLocalState` in `YesSteveModel-Native/model_state_impl.inc`.
fn apply_local_state(bone: &BoneInfo, attr: &BoneAttribute, stack: &mut PoseStack) {
    stack.translate(
        (bone.pivot[0] - attr.position[0]) * MODEL_UNIT,
        (bone.pivot[1] + attr.position[1]) * MODEL_UNIT,
        (bone.pivot[2] + attr.position[2]) * MODEL_UNIT,
    );
    stack.rotate_zyx(
        bone.rest_rotation[0] + attr.rotation[0],
        bone.rest_rotation[1] + attr.rotation[1],
        bone.rest_rotation[2] + attr.rotation[2],
    );
    stack.scale(attr.scale[0], attr.scale[1], attr.scale[2]);
    stack.translate(
        -bone.pivot[0] * MODEL_UNIT,
        -bone.pivot[1] * MODEL_UNIT,
        -bone.pivot[2] * MODEL_UNIT,
    );
}

impl ModelState {
    /// Exact port of `ModelState::Extraction::Run` (full tree, DFS pre-order).
    pub fn extract(&mut self, bones: &[BoneInfo], attributes: &[BoneAttribute]) -> Result<usize, String> {
        if bones.len() != attributes.len() {
            return Err(format!(
                "attribute count {} != bone count {}",
                attributes.len(),
                bones.len()
            ));
        }
        let count = bones.len();
        self.bone_poses.resize(count, BonePose::default());
        self.render_bone_indices.clear();
        self.pose_stack.reset();
        self.pose_stack.reserve(count + 1);

        let mut idx = 0usize;
        while idx < count {
            let bone = &bones[idx];
            let attr = &attributes[idx];
            self.pose_stack.pop_pose(bone.hierarchy.depth);
            self.pose_stack.push_pose();

            if !attr.is_finite()
                || attr.scale[0] == 0.0
                || attr.scale[1] == 0.0
                || attr.scale[2] == 0.0
            {
                self.bone_poses[idx].visible = false;
                idx = bone.hierarchy.subtree_end;
                continue;
            }

            apply_local_state(bone, attr, &mut self.pose_stack);
            let current = *self.pose_stack.last();
            let finite = current.pose.iter().flatten().all(|v| v.is_finite());
            if !finite {
                self.bone_poses[idx].visible = false;
                idx = bone.hierarchy.subtree_end;
                continue;
            }

            let visible = !attr.flags.cubes_hidden && bone.hierarchy.has_geometry;
            self.bone_poses[idx] = BonePose {
                pose: current,
                color: attr.flags.color,
                glowing: attr.flags.glowing,
                visible,
            };
            if visible {
                self.render_bone_indices.push(idx as u16);
            }

            if attr.flags.children_hidden {
                idx = bone.hierarchy.subtree_end;
            } else {
                idx += 1;
            }
        }
        Ok(self.render_bone_indices.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::euler::mat4_transform_point;
    use std::f32::consts::FRAC_PI_2;

    fn make_bone(name: &str, pivot: Vec3, parent: Option<u16>, depth: usize, subtree_end: usize) -> BoneInfo {
        BoneInfo {
            name: name.to_string(),
            pivot,
            rest_rotation: [0.0, 0.0, 0.0],
            hierarchy: BoneHierarchy {
                parent_index: parent,
                subtree_end,
                depth,
                has_geometry: true,
            },
        }
    }

    /// Mathematical proof: zero detached-hand ("断手") gap between RightArm -> RightForeArm -> RightHand!
    #[test]
    fn arm_forearm_hand_joint_continuity_zero_gap() {
        let elbow_pivot = [-3.0, 20.0, 0.0];
        let wrist_pivot = [-3.0, 14.0, 0.0];
        let bones = vec![
            make_bone("RightArm", [-3.0, 26.0, 0.0], None, 0, 3),
            make_bone("RightForeArm", elbow_pivot, Some(0), 1, 3),
            make_bone("RightHand", wrist_pivot, Some(1), 2, 3),
        ];

        let mut attrs = vec![BoneAttribute::default(); 3];
        attrs[0].rotation = [0.65, 0.25, -0.30];
        attrs[1].rotation = [-FRAC_PI_2 * 0.6, 0.0, 0.0];
        attrs[2].rotation = [0.2, 0.1, 0.0];

        let mut state = ModelState::default();
        let rendered = state.extract(&bones, &attrs).expect("extract ok");
        assert_eq!(rendered, 3);

        // The elbow point in RightArm space MUST coincide with the elbow point in RightForeArm space!
        let elbow_m = [elbow_pivot[0] * MODEL_UNIT, elbow_pivot[1] * MODEL_UNIT, elbow_pivot[2] * MODEL_UNIT];
        let p_arm = mat4_transform_point(&state.bone_poses[0].pose.pose, elbow_m);
        let p_forearm = mat4_transform_point(&state.bone_poses[1].pose.pose, elbow_m);
        for axis in 0..3 {
            assert!((p_arm[axis] - p_forearm[axis]).abs() < 1e-5, "elbow gap on axis {axis}");
        }

        // The wrist point in RightForeArm space MUST coincide with the wrist point in RightHand space!
        let wrist_m = [wrist_pivot[0] * MODEL_UNIT, wrist_pivot[1] * MODEL_UNIT, wrist_pivot[2] * MODEL_UNIT];
        let w_forearm = mat4_transform_point(&state.bone_poses[1].pose.pose, wrist_m);
        let w_hand = mat4_transform_point(&state.bone_poses[2].pose.pose, wrist_m);
        for axis in 0..3 {
            assert!((w_forearm[axis] - w_hand[axis]).abs() < 1e-5, "wrist gap on axis {axis}");
        }
    }

    #[test]
    fn children_hidden_skips_entire_subtree() {
        let bones = vec![
            make_bone("Root", [0.0, 0.0, 0.0], None, 0, 3),
            make_bone("ChildA", [0.0, 8.0, 0.0], Some(0), 1, 3),
            make_bone("GrandChild", [0.0, 16.0, 0.0], Some(1), 2, 3),
        ];
        let mut attrs = vec![BoneAttribute::default(); 3];
        attrs[0].flags.children_hidden = true;
        let mut state = ModelState::default();
        let rendered = state.extract(&bones, &attrs).expect("extract ok");
        assert_eq!(rendered, 1);
        assert_eq!(state.render_bone_indices, vec![0]);
    }
}
