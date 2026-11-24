//! 1:1 port of `com.elfmcys.ysm.client.animation.physics.{FirstOrder, SecondOrder}`.

use std::f32::consts::PI;

/// 1:1 port of `FirstOrder.java` — exponential smoothing filter.
#[derive(Clone, Copy, Debug)]
pub struct FirstOrder {
    input: f32,
    response: f32,
    last_simulation: f32,
}

impl FirstOrder {
    pub fn new(input: f32, response: f32) -> Self {
        Self {
            input,
            response,
            last_simulation: 0.0,
        }
    }

    pub fn set_args(&mut self, input: f32, response: f32) {
        self.input = input;
        self.response = response;
    }

    pub fn update(&mut self, time_step: f32) {
        if self.response.abs() < 1e-6 || time_step <= 0.0 {
            return;
        }
        let alpha = (time_step / self.response).clamp(0.0, 1.0);
        self.last_simulation = (1.0 - alpha) * self.last_simulation + alpha * self.input;
    }

    pub fn value(&self) -> f32 {
        self.last_simulation
    }
}

/// 1:1 port of `SecondOrder.java` — second-order spring-damper dynamics with stability sub-stepping.
#[derive(Clone, Copy, Debug)]
pub struct SecondOrder {
    input_function: f32,
    last_simulation: f32,
    last_simulation_dot: f32,
    input: f32,
    frequency: f32,
    coefficient: f32,
    response: f32,
}

impl SecondOrder {
    pub fn new(input: f32, frequency: f32, coefficient: f32, response: f32) -> Self {
        Self {
            input_function: 0.0,
            last_simulation: 0.0,
            last_simulation_dot: 0.0,
            input,
            frequency: frequency.clamp(0.0, 5.0),
            coefficient: coefficient.clamp(0.0, 1.0),
            response,
        }
    }

    pub fn set_args(&mut self, input: f32, frequency: f32, coefficient: f32, response: f32) {
        self.input = input;
        self.frequency = frequency.clamp(0.0, 5.0);
        self.coefficient = coefficient.clamp(0.0, 1.0);
        self.response = response;
    }

    pub fn update(&mut self, time_step: f32) {
        if time_step <= 0.0 || self.frequency <= 1e-5 {
            return;
        }
        let f = self.frequency.clamp(1e-5, 5.0);
        let z = self.coefficient.clamp(0.0, 1.0);
        let r = self.response;

        let k1 = z / (PI * f);
        let two_pi_f = 2.0 * PI * f;
        let k2 = 1.0 / (two_pi_f * two_pi_f);
        let k3 = r * z / two_pi_f;

        let input_dot = (self.input - self.input_function) / time_step;
        self.input_function = self.input;

        let max_step = (4.0 * k2 + k1 * k1).sqrt() - k1;
        let cycles = if max_step > 1e-6 {
            ((time_step / max_step).ceil() as usize).clamp(1, 32)
        } else {
            1
        };
        let dt = time_step / cycles as f32;

        let mut y = self.last_simulation;
        let mut yd = self.last_simulation_dot;
        for _ in 0..cycles {
            y += dt * yd;
            yd += dt * (k3 * input_dot + self.input - y - k1 * yd) / k2;
        }
        if y.is_finite() && yd.is_finite() {
            self.last_simulation = y;
            self.last_simulation_dot = yd;
        }
    }

    pub fn value(&self) -> f32 {
        self.last_simulation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_order_converges_monotonically_to_target() {
        let mut f = FirstOrder::new(10.0, 0.2);
        for _ in 0..40 {
            f.update(0.05);
        }
        assert!((f.value() - 10.0).abs() < 1e-3, "got {}", f.value());
    }

    #[test]
    fn second_order_converges_stably_even_with_large_dt() {
        let mut s = SecondOrder::new(5.0, 2.5, 0.85, 0.0);
        for _ in 0..120 {
            s.update(0.05);
        }
        assert!((s.value() - 5.0).abs() < 0.05, "got {}", s.value());
    }
}
