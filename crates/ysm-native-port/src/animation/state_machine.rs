//! 1:1 port of official `YesSteveModel` `AnimationRegister.java` + `PlayerMainPredicate.java`
//! primary player animation priority state machine.

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerAnimInput {
    pub is_dead: bool,
    pub is_sleeping: bool,
    pub hurt_time: i32,
    pub is_gliding: bool,
    pub is_flying: bool,
    pub is_swimming: bool,
    pub is_in_water: bool,
    pub is_riding: bool,
    pub is_riding_pig: bool,
    pub is_in_boat: bool,
    pub is_sitting: bool,
    pub is_climbing: bool,
    pub on_ground: bool,
    pub is_crouching: bool,
    pub is_sprinting: bool,
    pub is_moving: bool,
    pub walk_speed: f32,
}

/// Evaluates official YSM primary locomotion priority ladder.
pub fn select_primary_animation(inp: &PlayerAnimInput) -> &'static str {
    if inp.is_dead {
        return "death";
    }
    if inp.is_sleeping {
        return "sleep";
    }
    if inp.hurt_time > 0 {
        return "attacked";
    }
    if inp.is_gliding {
        return "elytra_fly";
    }
    if inp.is_flying {
        return "fly";
    }
    if inp.is_swimming {
        return "swim";
    }
    if inp.is_in_water && !inp.on_ground {
        return "swim_stand";
    }
    if inp.is_in_boat {
        return "boat";
    }
    if inp.is_riding_pig {
        return "ride_pig";
    }
    if inp.is_riding {
        return "ride";
    }
    if inp.is_sitting {
        return "sit";
    }
    if inp.is_climbing {
        return "climb";
    }
    if !inp.on_ground {
        return "jump";
    }
    let moving = inp.is_moving || inp.walk_speed > 0.03;
    if inp.is_crouching && moving {
        return "sneak";
    }
    if inp.is_crouching {
        return "sneaking";
    }
    if (inp.is_sprinting && moving) || inp.walk_speed > 0.65 {
        return "run";
    }
    if moving {
        return "walk";
    }
    "idle"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_ysm_primary_priority_ladder() {
        let base = PlayerAnimInput { on_ground: true, ..Default::default() };
        assert_eq!(select_primary_animation(&base), "idle");
        assert_eq!(select_primary_animation(&PlayerAnimInput { is_moving: true, ..base }), "walk");
        assert_eq!(select_primary_animation(&PlayerAnimInput { is_moving: true, is_sprinting: true, ..base }), "run");
        assert_eq!(select_primary_animation(&PlayerAnimInput { is_crouching: true, ..base }), "sneaking");
        assert_eq!(select_primary_animation(&PlayerAnimInput { is_crouching: true, is_moving: true, ..base }), "sneak");
        assert_eq!(select_primary_animation(&PlayerAnimInput { on_ground: false, ..base }), "jump");
        assert_eq!(select_primary_animation(&PlayerAnimInput { is_riding: true, on_ground: false, ..base }), "ride");
        assert_eq!(select_primary_animation(&PlayerAnimInput { is_in_boat: true, is_riding: true, ..base }), "boat");
        assert_eq!(select_primary_animation(&PlayerAnimInput { is_swimming: true, is_riding: true, ..base }), "swim");
        assert_eq!(select_primary_animation(&PlayerAnimInput { is_gliding: true, is_flying: true, ..base }), "elytra_fly");
        assert_eq!(select_primary_animation(&PlayerAnimInput { hurt_time: 5, is_riding: true, ..base }), "attacked");
        assert_eq!(select_primary_animation(&PlayerAnimInput { is_dead: true, hurt_time: 5, ..base }), "death");
    }
}
