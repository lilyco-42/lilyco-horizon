//! 1:1 Rust port of `YesSteveModel-Native/modules/gfx/include/gfx/renderer/bone_attribute.h`.

use crate::math::euler::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct BoneAttribute {
    /// Local Euler ZYX rotation in radians (`[x, y, z]`).
    pub rotation: Vec3,
    /// Local position offset in Bedrock model units (1/16 block).
    pub position: Vec3,
    /// Local scale (`[1.0, 1.0, 1.0]` default).
    pub scale: Vec3,
    /// Visibility & color flags.
    pub flags: BoneFlags,
}

#[derive(Clone, Copy, Debug)]
pub struct BoneFlags {
    pub cubes_hidden: bool,
    pub children_hidden: bool,
    pub color: u32,
    pub glowing: u8,
}

impl Default for BoneFlags {
    fn default() -> Self {
        Self {
            cubes_hidden: false,
            children_hidden: false,
            color: 0x00FF_FFFF,
            glowing: 0xFF,
        }
    }
}

impl Default for BoneAttribute {
    fn default() -> Self {
        Self {
            rotation: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            flags: BoneFlags::default(),
        }
    }
}

impl BoneAttribute {
    pub fn is_finite(&self) -> bool {
        self.rotation.iter().all(|v| v.is_finite())
            && self.position.iter().all(|v| v.is_finite())
            && self.scale.iter().all(|v| v.is_finite())
    }
}
