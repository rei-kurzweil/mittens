//! Unsupported legs trail measured world-space motion, independent of body yaw.
use super::FootGoal;
use crate::utils::math::{vec3_add, vec3_len, vec3_scale, vec3_sub};
#[derive(Debug, Default)]
pub(super) struct AirborneState {
    position: Option<[f32; 3]>,
    velocity: [f32; 3],
    trail: [f32; 3],
}
impl AirborneState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn advance(&mut self, position: [f32; 3], supported: bool, scale: f32, dt: f32) {
        if !dt.is_finite() || dt <= 0. || dt > 0.25 || !scale.is_finite() || scale <= 0. {
            self.reset();
            return;
        }
        let delta = self.position.map(|p| vec3_sub(position, p));
        if delta.is_some_and(|d| vec3_len(d) > scale.max(1.)) {
            // Teleports do not kick the legs or preserve the old velocity.
            self.reset();
            self.position = Some(position);
            return;
        }
        self.position = Some(position);
        if let Some(delta) = delta {
            let measured = [delta[0] / dt, 0., delta[2] / dt];
            let alpha = 1. - (-dt / 0.08).exp();
            self.velocity = vec3_add(
                self.velocity,
                vec3_scale(vec3_sub(measured, self.velocity), alpha),
            );
        }
        if supported {
            self.trail = [0.; 3];
            return;
        }
        let mut desired = vec3_scale(self.velocity, -0.10);
        let length = vec3_len(desired);
        if length > 0.25 * scale {
            desired = vec3_scale(desired, 0.25 * scale / length);
        }
        let alpha = 1. - (-dt / 0.12).exp();
        self.trail = vec3_add(self.trail, vec3_scale(vec3_sub(desired, self.trail), alpha));
    }
    pub fn goal(&self, base: FootGoal, scale: f32) -> FootGoal {
        let amount = (vec3_len(self.trail) / (0.25 * scale)).clamp(0., 1.);
        FootGoal {
            sole: vec3_add(
                base.sole,
                [self.trail[0], 0.08 * scale * amount, self.trail[2]],
            ),
            ..base
        }
    }
}
