//! 1:1 Rust port of `YesSteveModel-Native/modules/gfx/src/math/euler.h`.

use std::f64::consts::PI;

pub type Mat4 = [[f32; 4]; 4];
pub type Mat3 = [[f32; 3]; 3];
pub type Vec3 = [f32; 3];

pub const MAT4_IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub const MAT3_IDENTITY: Mat3 = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
];

/// Exact bit-level port of `ysm::gfx::math::internal::ModuloTwoPi`.
pub fn modulo_two_pi(value: f64) -> f64 {
    const TWO_PI: f64 = 2.0 * PI;
    const FRACTION_MASK: u64 = (1u64 << 52) - 1;
    const IMPLICIT_BIT: u64 = 1u64 << 52;
    let two_pi_bits = TWO_PI.to_bits();
    let two_pi_exp = ((two_pi_bits >> 52) & 0x7ff) as i32;
    let two_pi_sig = (two_pi_bits & FRACTION_MASK) | IMPLICIT_BIT;

    let mut value_bits = value.to_bits();
    let mut exponent = ((value_bits >> 52) & 0x7ff) as i32;
    if exponent == 0x7ff {
        return f64::NAN;
    }
    if value_bits <= two_pi_bits {
        return if value_bits == two_pi_bits { 0.0 } else { value };
    }

    let mut significand = (value_bits & FRACTION_MASK) | IMPLICIT_BIT;
    while exponent > two_pi_exp {
        let diff = significand.wrapping_sub(two_pi_sig);
        if (diff >> 63) == 0 {
            if diff == 0 {
                return 0.0;
            }
            significand = diff;
        }
        significand <<= 1;
        exponent -= 1;
    }

    let diff = significand.wrapping_sub(two_pi_sig);
    if (diff >> 63) == 0 {
        if diff == 0 {
            return 0.0;
        }
        significand = diff;
    }
    while (significand & IMPLICIT_BIT) == 0 {
        significand <<= 1;
        exponent -= 1;
    }
    if exponent > 0 {
        value_bits = (significand - IMPLICIT_BIT) | ((exponent as u64) << 52);
    } else {
        value_bits = significand >> ((-exponent + 1) as u32);
    }
    f64::from_bits(value_bits)
}

/// Exact port of `ysm::gfx::math::internal::ReduceAngle` into `(-pi, pi]`.
pub fn reduce_angle(angle: f32) -> f32 {
    const TWO_PI: f64 = 2.0 * PI;
    let negative = (angle.to_bits() >> 31) != 0;
    let mut reduced = modulo_two_pi((angle as f64).abs());
    if negative {
        reduced = -reduced;
    }
    if reduced > PI {
        reduced -= TWO_PI;
    }
    let final_val = if reduced <= -PI {
        reduced + TWO_PI
    } else {
        reduced
    };
    final_val as f32
}

/// Exact column-major `glm_euler_zyx` (Z * Y * X) matching `YesSteveModel-Native/euler.h`.
/// `angles` are in radians: `[x, y, z]`.
pub fn euler_zyx(angles: Vec3) -> Mat4 {
    let ax = if angles[0].abs() > 8192.0 { reduce_angle(angles[0]) } else { angles[0] };
    let ay = if angles[1].abs() > 8192.0 { reduce_angle(angles[1]) } else { angles[1] };
    let az = if angles[2].abs() > 8192.0 { reduce_angle(angles[2]) } else { angles[2] };

    let (sx, cx) = ax.sin_cos();
    let (sy, cy) = ay.sin_cos();
    let (sz, cz) = az.sin_cos();

    let czsx = cz * sx;
    let czcx = cz * cx;
    let szsx = sz * sx;
    let szcx = sz * cx;

    [
        [cy * cz, cy * sz, -sy, 0.0],
        [czsx * sy - szcx, szsx * sy + czcx, cy * sx, 0.0],
        [czcx * sy + szsx, szcx * sy - czsx, cy * cx, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn mat4_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            out[col][row] = a[0][row] * b[col][0]
                + a[1][row] * b[col][1]
                + a[2][row] * b[col][2]
                + a[3][row] * b[col][3];
        }
    }
    out
}

pub fn mat3_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0f32; 3]; 3];
    for col in 0..3 {
        for row in 0..3 {
            out[col][row] =
                a[0][row] * b[col][0] + a[1][row] * b[col][1] + a[2][row] * b[col][2];
        }
    }
    out
}

pub fn mat4_pick3(m: &Mat4) -> Mat3 {
    [
        [m[0][0], m[0][1], m[0][2]],
        [m[1][0], m[1][1], m[1][2]],
        [m[2][0], m[2][1], m[2][2]],
    ]
}

pub fn mat4_transform_point(m: &Mat4, p: Vec3) -> Vec3 {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_zyx(angles: Vec3) -> Mat4 {
        let (sx, cx) = (angles[0] as f64).sin_cos();
        let (sy, cy) = (angles[1] as f64).sin_cos();
        let (sz, cz) = (angles[2] as f64).sin_cos();
        let rx = [[1.0, 0.0, 0.0, 0.0], [0.0, cx, sx, 0.0], [0.0, -sx, cx, 0.0], [0.0, 0.0, 0.0, 1.0]];
        let ry = [[cy, 0.0, -sy, 0.0], [0.0, 1.0, 0.0, 0.0], [sy, 0.0, cy, 0.0], [0.0, 0.0, 0.0, 1.0]];
        let rz = [[cz, sz, 0.0, 0.0], [-sz, cz, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        let mul = |a: [[f64; 4]; 4], b: [[f64; 4]; 4]| {
            let mut o = [[0.0; 4]; 4];
            for c in 0..4 {
                for r in 0..4 {
                    o[c][r] = a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
                }
            }
            o
        };
        let m = mul(mul(rz, ry), rx);
        let mut out = [[0.0f32; 4]; 4];
        for c in 0..4 {
            for r in 0..4 {
                out[c][r] = m[c][r] as f32;
            }
        }
        out
    }

    #[test]
    fn reduces_large_angles_matches_reference() {
        let angles = [9000.0f32, -10000.0, 12000.0];
        let actual = euler_zyx(angles);
        let expected = reference_zyx(angles);
        for c in 0..4 {
            for r in 0..4 {
                assert!((actual[c][r] - expected[c][r]).abs() < 2e-6);
            }
        }
    }

    #[test]
    fn reduces_to_right_closed_principal_interval() {
        let pi = std::f32::consts::PI;
        let samples = [8193.0f32, -8193.0, 1_000_000.0, -1_000_000.0, f32::MAX, -f32::MAX];
        for angle in samples {
            let reduced = reduce_angle(angle);
            assert!(reduced > -pi && reduced <= pi, "angle={angle}, reduced={reduced}");
        }
        assert!(reduce_angle(-pi) > 0.0);
    }
}
