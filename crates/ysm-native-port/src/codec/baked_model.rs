//! 1:1 port of `YesSteveModel-Native` `baked_model.h` (`QuadType` alpha classification).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuadType {
    /// Every texel in the quad UV rect has alpha == 0 (culled at bake time).
    Empty,
    /// Every non-empty texel in the quad UV rect is fully opaque (alpha >= 250).
    Opaque,
    /// Contains semi-transparent texels (0 < alpha < 250).
    Translucent,
}

/// Classifies a UV rectangle (`[u0, v0, u1, v1]` in 0..16 atlas coordinates) against an RGBA alpha plane.
pub fn classify_quad_alpha(uv16: [f32; 4], width: usize, height: usize, alpha: &[u8]) -> QuadType {
    if width == 0 || height == 0 || alpha.len() < width * height {
        return QuadType::Opaque;
    }
    let u0 = (uv16[0].min(uv16[2]) / 16.0).clamp(0.0, 1.0);
    let u1 = (uv16[0].max(uv16[2]) / 16.0).clamp(0.0, 1.0);
    let v0 = (uv16[1].min(uv16[3]) / 16.0).clamp(0.0, 1.0);
    let v1 = (uv16[1].max(uv16[3]) / 16.0).clamp(0.0, 1.0);
    if (u1 - u0) < 1e-6 || (v1 - v0) < 1e-6 {
        return QuadType::Empty;
    }
    let x0 = ((u0 * width as f32).floor() as usize).min(width - 1);
    let x1 = ((u1 * width as f32).ceil() as usize).clamp(x0 + 1, width);
    let y0 = ((v0 * height as f32).floor() as usize).min(height - 1);
    let y1 = ((v1 * height as f32).ceil() as usize).clamp(y0 + 1, height);

    let mut any_visible = false;
    let mut any_translucent = false;
    for y in y0..y1 {
        let row = y * width;
        for x in x0..x1 {
            let a = alpha[row + x];
            if a >= 32 {
                any_visible = true;
                if a < 250 {
                    any_translucent = true;
                }
            }
        }
    }
    if !any_visible {
        QuadType::Empty
    } else if any_translucent {
        QuadType::Translucent
    } else {
        QuadType::Opaque
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_empty_opaque_and_translucent_quads() {
        let empty = vec![0u8; 16];
        let opaque = vec![255u8; 16];
        let mut trans = vec![0u8; 16];
        trans[5] = 128;
        assert_eq!(classify_quad_alpha([0.0, 0.0, 16.0, 16.0], 4, 4, &empty), QuadType::Empty);
        assert_eq!(classify_quad_alpha([0.0, 0.0, 16.0, 16.0], 4, 4, &opaque), QuadType::Opaque);
        assert_eq!(classify_quad_alpha([0.0, 0.0, 16.0, 16.0], 4, 4, &trans), QuadType::Translucent);
    }
}
