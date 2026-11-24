//! 1:1 Rust port of `YesSteveModel-Native/modules/gfx/include/gfx/math/pose_stack.h`
//! and `modules/gfx/src/math/pose_stack_impl.inc`.

use crate::math::euler::{
    euler_zyx, mat3_mul, mat4_mul, mat4_pick3, Mat3, Mat4, MAT3_IDENTITY, MAT4_IDENTITY,
};

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub pose: Mat4,
    pub normal: Mat3,
    pub uniform_scale: bool,
    pub tangent_orientation: f32,
    pub normal_scale: f32,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            pose: MAT4_IDENTITY,
            normal: MAT3_IDENTITY,
            uniform_scale: true,
            tangent_orientation: 1.0,
            normal_scale: 1.0,
        }
    }
}

pub struct PoseStack {
    poses: Vec<Pose>,
    pose_index: usize,
}

impl Default for PoseStack {
    fn default() -> Self {
        Self::new()
    }
}

impl PoseStack {
    pub fn new() -> Self {
        Self {
            poses: vec![Pose::default(); 12],
            pose_index: 0,
        }
    }

    pub fn reserve(&mut self, capacity: usize) {
        if self.poses.len() < capacity {
            self.poses.resize(capacity, Pose::default());
        }
    }

    pub fn reset(&mut self) {
        self.pose_index = 0;
        self.poses[0] = Pose::default();
    }

    pub fn push_pose(&mut self) {
        let next = self.pose_index + 1;
        if next == self.poses.len() {
            self.poses.push(Pose::default());
        }
        self.poses[next] = self.poses[self.pose_index];
        self.pose_index = next;
    }

    pub fn pop_pose(&mut self, depth: usize) {
        if depth <= self.pose_index {
            self.pose_index = depth;
        }
    }

    pub fn last(&self) -> &Pose {
        &self.poses[self.pose_index]
    }

    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        if x == 0.0 && y == 0.0 && z == 0.0 {
            return;
        }
        let m = &mut self.poses[self.pose_index].pose;
        for row in 0..4 {
            m[3][row] += m[0][row] * x + m[1][row] * y + m[2][row] * z;
        }
    }

    pub fn rotate_zyx(&mut self, x: f32, y: f32, z: f32) {
        if x == 0.0 && y == 0.0 && z == 0.0 {
            return;
        }
        let current = &mut self.poses[self.pose_index];
        let rot4 = euler_zyx([x, y, z]);
        let rot3 = mat4_pick3(&rot4);
        current.pose = mat4_mul(&current.pose, &rot4);
        current.normal = mat3_mul(&current.normal, &rot3);
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) {
        if x == 1.0 && y == 1.0 && z == 1.0 {
            return;
        }
        let current = &mut self.poses[self.pose_index];
        let uniform = x.abs() == y.abs() && y.abs() == z.abs();
        current.uniform_scale = current.uniform_scale && uniform;
        if uniform {
            current.normal_scale *= x.abs();
        }
        if x.is_sign_negative() ^ y.is_sign_negative() ^ z.is_sign_negative() {
            current.tangent_orientation = -current.tangent_orientation;
        }
        let s = [x, y, z];
        for col in 0..3 {
            for row in 0..4 {
                current.pose[col][row] *= s[col];
            }
            let n_scale = if uniform {
                1.0f32.copysign(s[col])
            } else {
                1.0 / s[col]
            };
            for row in 0..3 {
                current.normal[col][row] *= n_scale;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    /// Ported 1:1 from `YesSteveModel-Native/modules/test/src/pose_stack_test.cc`.
    #[test]
    fn maintains_direction_state_across_hierarchy() {
        let mut stack = PoseStack::new();
        stack.reserve(3);

        stack.translate(1.0, 2.0, 3.0);
        stack.rotate_zyx(0.0, 0.0, FRAC_PI_2);
        stack.scale(-2.0, 2.0, 2.0);
        let parent = *stack.last();
        assert!(parent.uniform_scale);
        assert_eq!(parent.tangent_orientation, -1.0);
        assert_eq!(parent.normal_scale, 2.0);
        assert!((parent.pose[0][1] - (-2.0)).abs() < 1e-5);
        assert!((parent.normal[0][1] - (-1.0)).abs() < 1e-5);

        stack.push_pose();
        stack.scale(1.0, 2.0, 1.0);
        stack.scale(-1.0, -1.0, 1.0);
        let child = *stack.last();
        assert!(!child.uniform_scale);
        assert_eq!(child.tangent_orientation, -1.0);
        assert_eq!(child.normal_scale, 2.0);

        stack.push_pose();
        stack.translate(3.0, 4.0, 5.0);
        stack.pop_pose(1);
        assert!(!stack.last().uniform_scale);
        assert_eq!(stack.last().tangent_orientation, -1.0);
        assert_eq!(stack.last().normal_scale, 2.0);

        stack.pop_pose(0);
        assert!(stack.last().uniform_scale);
        assert_eq!(stack.last().tangent_orientation, -1.0);
        assert_eq!(stack.last().normal_scale, 2.0);

        stack.push_pose();
        assert!(stack.last().uniform_scale);
        assert!((stack.last().pose[0][1] - (-2.0)).abs() < 1e-5);
        assert!((stack.last().normal[0][1] - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn copies_pose_into_native_snapshot_layout() {
        let mut stack = PoseStack::new();
        stack.scale(-2.0, 3.0, 4.0);
        let snap = *stack.last();
        assert!(!snap.uniform_scale);
        assert_eq!(snap.tangent_orientation, -1.0);
        assert_eq!(snap.normal_scale, 1.0);
        assert_eq!(snap.pose[0][0], -2.0);
        assert_eq!(snap.normal[0][0], -0.5);
        assert!((snap.normal[1][1] - (1.0 / 3.0)).abs() < 1e-6);
        assert_eq!(snap.normal[2][2], 0.25);
    }
}
