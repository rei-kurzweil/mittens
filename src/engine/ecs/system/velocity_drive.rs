//! Fixed-step chassis commands for an opt-in Velocity drive provider.
use crate::engine::ecs::component::{AccelerationComponent, TransformComponent};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::utils::math::{self, vec3_add, vec3_cross, vec3_dot, vec3_len, vec3_scale, vec3_sub};

pub(super) fn step(
    world: &mut World,
    velocity: ComponentId,
    target: ComponentId,
    support_up: Option<[f32; 3]>,
    linear: &mut [f32; 3],
    angular: &mut [f32; 3],
    dt: f32,
    emit: &mut dyn SignalEmitter,
) {
    let Some(provider) = world.children_of(velocity).iter().copied().find(|&id| {
        world
            .get_component_by_id_as::<AccelerationComponent>(id)
            .is_some_and(|a| a.drive.is_some())
    }) else {
        return;
    };
    let Some(chassis) = world.get_component_by_id_as::<TransformComponent>(target) else {
        return;
    };
    let facing = math::quat_rotate_vec3(chassis.transform.rotation, [0., 0., -1.]);
    let up = math::quat_rotate_vec3(chassis.transform.rotation, [0., 1., 0.]);
    let provider = world
        .get_component_by_id_as_mut::<AccelerationComponent>(provider)
        .unwrap();
    let drive = provider.drive.as_mut().unwrap();
    let normal = support_up.unwrap_or(up);
    let tangent = vec3_sub(facing, vec3_scale(normal, vec3_dot(facing, normal)));
    let direction = math::vec3_normalize(tangent);
    let speed = vec3_dot(*linear, direction);

    if drive.braking {
        // Brake in the contact plane and retain gravity/normal motion.
        if support_up.is_some() {
            let tangent_velocity = vec3_sub(*linear, vec3_scale(normal, vec3_dot(*linear, normal)));
            let tangent_speed = vec3_len(tangent_velocity);
            if tangent_speed > 0.0 {
                *linear = vec3_sub(
                    *linear,
                    vec3_scale(tangent_velocity, (45.0 * dt / tangent_speed).min(1.0)),
                );
            }
        }
    } else if support_up.is_some() && drive.throttle != 0.0 {
        let along = direction.map(|axis| axis * drive.throttle.signum());
        let delta = (drive.throttle.abs() * 16.0 * dt)
            .min((provider.max_speed - vec3_dot(*linear, along)).max(0.0));
        *linear = vec3_add(*linear, vec3_scale(along, delta));
    }

    if support_up.is_some() {
        // Follow the supporting plane with bounded pitch/roll correction.
        // Independent angular velocity is still integrated by Velocity.
        let axis = vec3_cross(up, normal);
        let sin = vec3_len(axis);
        if sin > 1e-4 {
            let correction = (sin.atan2(vec3_dot(up, normal)) * 7.0).min(2.0);
            *angular = vec3_add(*angular, vec3_scale(axis, correction / sin));
        }
        let steering_rate = -drive.steering * (speed / 5.0).clamp(-1.0, 1.0) * 1.2;
        *angular = vec3_add(*angular, vec3_scale(normal, steering_rate));
    }

    for wheel in drive.wheels.iter_mut().flatten() {
        let steer_angle = if wheel.front {
            -0.48 * drive.steering
        } else {
            0.0
        };
        if steer_angle != wheel.steer_angle {
            wheel.steer_angle = steer_angle;
            rotate_pivot(wheel.steering, [0., 1., 0.], steer_angle, emit);
        }
        if speed.abs() > 0.01 {
            wheel.roll_angle =
                (wheel.roll_angle + speed * dt / wheel.radius).rem_euclid(std::f32::consts::TAU);
            rotate_pivot(wheel.spin, [1., 0., 0.], wheel.roll_angle, emit);
        }
    }
}

fn rotate_pivot(id: ComponentId, axis: [f32; 3], angle: f32, emit: &mut dyn SignalEmitter) {
    emit.push_intent_now(
        id,
        IntentValue::UpdateTransform {
            component_id: id,
            translation: [0.; 3],
            rotation_quat_xyzw: math::quat_from_axis_angle(axis, angle),
            scale: [1.; 3],
        },
    );
}
