use crate::engine::ecs::component::{AvatarBodyYawComponent, TransformComponent};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct AvatarBodyYawSystem {
    followers: HashSet<ComponentId>,
}

impl AvatarBodyYawSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, dt_sec: f32) {
        let ids: Vec<_> = self.followers.iter().copied().collect();

        for id in ids {
            tick_one(id, world, emit, dt_sec);
        }
    }

    pub fn register(&mut self, component: ComponentId) {
        self.followers.insert(component);
    }

    pub fn remove(&mut self, component: ComponentId) {
        self.followers.remove(&component);
    }
}

fn tick_one(id: ComponentId, world: &mut World, emit: &mut dyn SignalEmitter, dt_sec: f32) {
    let (hmd_id, threshold, rate, body_yaw, forward_plus_z) = {
        let Some(c) = world.get_component_by_id_as::<AvatarBodyYawComponent>(id) else {
            return;
        };
        (
            c.hmd_driven_transform,
            c.threshold,
            c.rate,
            c.body_yaw,
            c.forward_plus_z,
        )
    };

    let Some(hmd_id) = hmd_id else { return };
    if !xr_source_is_valid(world, hmd_id) {
        return;
    }

    let hmd_yaw = {
        let Some(t) = world.get_component_by_id_as::<TransformComponent>(hmd_id) else {
            return;
        };
        extract_world_yaw(t.transform.matrix_world, forward_plus_z)
    };

    let delta = signed_yaw_diff(hmd_yaw, body_yaw);
    if delta.abs() <= threshold {
        return;
    }

    let target = hmd_yaw - delta.signum() * threshold;
    let step = rate * dt_sec;
    let new_body_yaw = lerp_angle(
        body_yaw,
        target,
        step.min(delta.abs()) / delta.abs().max(1e-9),
    );

    if (new_body_yaw - body_yaw).abs() < 1e-6 {
        return;
    }

    if let Some(c) = world.get_component_by_id_as_mut::<AvatarBodyYawComponent>(id) {
        c.body_yaw = new_body_yaw;
    }

    // Find the TransformComponent child (model_root).
    let model_root = world.children_of(id).iter().copied().find(|&ch| {
        world
            .get_component_by_id_as::<TransformComponent>(ch)
            .is_some()
    });
    let Some(model_root) = model_root else { return };

    let (translation, scale) = {
        let Some(t) = world.get_component_by_id_as::<TransformComponent>(model_root) else {
            return;
        };
        (t.transform.translation, t.transform.scale)
    };

    emit.push_intent_now(
        model_root,
        IntentValue::UpdateTransform {
            component_ids: vec![model_root],
            translation,
            rotation_quat_xyzw: quat_rotation_y(new_body_yaw),
            scale,
        },
    );
}

fn xr_source_is_valid(world: &World, start: ComponentId) -> bool {
    let mut current = Some(start);
    while let Some(component) = current {
        if let Some(input) = world
            .get_component_by_id_as::<crate::engine::ecs::component::InputXRComponent>(component)
        {
            return input.pose_valid;
        }
        current = world.parent_of(component);
    }
    true
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

/// Extract world-space yaw (Y-axis rotation) from a column-major 4×4 matrix.
/// `plus_z_forward`: use +Z as forward (desktop/keyboard). false = -Z (OpenXR).
fn extract_world_yaw(m: [[f32; 4]; 4], plus_z_forward: bool) -> f32 {
    if plus_z_forward {
        // +Z column: forward direction in world space.
        m[2][0].atan2(m[2][2])
    } else {
        // -Z column: forward direction in OpenXR space.
        (-m[2][0]).atan2(-m[2][2])
    }
}

/// Signed difference a - b, wrapped to [-π, π].
fn signed_yaw_diff(a: f32, b: f32) -> f32 {
    let diff = a - b;
    wrap_angle(diff)
}

fn wrap_angle(a: f32) -> f32 {
    let mut v = a % (2.0 * std::f32::consts::PI);
    if v > std::f32::consts::PI {
        v -= 2.0 * std::f32::consts::PI;
    } else if v < -std::f32::consts::PI {
        v += 2.0 * std::f32::consts::PI;
    }
    v
}

/// Interpolate angle `from` toward `to` by fraction `t` (0..=1).
fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
    let diff = signed_yaw_diff(to, from);
    from + diff * t.clamp(0.0, 1.0)
}

/// Unit quaternion for rotation of `yaw` radians around the Y axis.
/// Returns XYZW format.
fn quat_rotation_y(yaw: f32) -> [f32; 4] {
    let half = yaw * 0.5;
    [0.0, half.sin(), 0.0, half.cos()]
}
