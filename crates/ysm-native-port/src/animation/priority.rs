//! 1:1 port of `com.elfmcys.ysm.client.animation.Priority`.

pub const HIGHEST: i32 = 0;
pub const HIGH: i32 = 1;
pub const NORMAL: i32 = 2;
pub const LOW: i32 = 3;
pub const LOWEST: i32 = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_order_matches_official_ysm() {
        assert!(HIGHEST < HIGH && HIGH < NORMAL && NORMAL < LOW && LOW < LOWEST);
        assert_eq!([HIGHEST, HIGH, NORMAL, LOW, LOWEST], [0, 1, 2, 3, 4]);
    }
}
