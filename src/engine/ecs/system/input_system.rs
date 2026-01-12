use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{ForwardAxis, InputComponent, InputTransformModeComponent, RollAxis, TransformComponent};
use crate::engine::ecs::system::System;
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use winit::keyboard::{Key, NamedKey};

/// System that processes input components and updates transforms based on WASD input.
///
/// Intended topology (simple one-way data flow):
/// InputComponent -> TransformComponent -> (Camera2DComponent, RenderableComponent, ...)
#[derive(Debug, Default)]
pub struct InputSystem {
    inputs: Vec<ComponentId>,
}

impl InputSystem {
    pub fn new() -> Self {
        Self { inputs: Vec::new() }
    }

    /// Register an InputComponent.
    pub fn register_input(&mut self, component: ComponentId) {
        if !self.inputs.iter().any(|c| *c == component) {
            self.inputs.push(component);
        }
    }

    fn compute_transform(
        &self,
        forward_axis: ForwardAxis,
        roll_axis: RollAxis,
        speed_units_per_sec: f32,
        input: &InputState,
        dt_sec: f32,
        transform: &mut crate::engine::graphics::primitives::Transform,
    ) {
        // Read movement keys.
        let w = input.key_down(&Key::Character("w".into()));
        let a = input.key_down(&Key::Character("a".into()));
        let s = input.key_down(&Key::Character("s".into()));
        let d = input.key_down(&Key::Character("d".into()));
        let r: bool = input.key_down(&Key::Character("r".into()));
        let f: bool = input.key_down(&Key::Character("f".into()));

        // Roll keys.
        let q = input.key_down(&Key::Character("q".into()));
        let e = input.key_down(&Key::Character("e".into()));

        // Apply rotation first so translation happens "after" rotation.
        if q || e {
            const ROT_SPEED_RAD_PER_SEC: f32 = 1.5;
            let dir = (q as i32) as f32 - (e as i32) as f32;
            let dtheta = dir * ROT_SPEED_RAD_PER_SEC * dt_sec;
            let (s, c) = (0.5 * dtheta).sin_cos();
            let q_inc = match roll_axis {
                RollAxis::X => [s, 0.0f32, 0.0f32, c],
                RollAxis::Y => [0.0f32, s, 0.0f32, c],
                RollAxis::Z => [0.0f32, 0.0f32, s, c],
            };

            fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
                let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
                let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
                [
                    aw * bx + ax * bw + ay * bz - az * by,
                    aw * by - ax * bz + ay * bw + az * bx,
                    aw * bz + ax * by - ay * bx + az * bw,
                    aw * bw - ax * bx - ay * by - az * bz,
                ]
            }

            // Apply local rotation increment.
            transform.rotation = quat_mul(transform.rotation, q_inc);
        }

        // Holding Shift increases movement speed.
        let speed_multiplier = if input.key_down(&Key::Named(NamedKey::Shift)) {
            3.0
        } else {
            1.0
        };

        let speed = speed_units_per_sec * speed_multiplier * dt_sec;

        fn quat_conjugate(q: [f32; 4]) -> [f32; 4] {
            [-q[0], -q[1], -q[2], q[3]]
        }

        fn quat_rotate_vec3(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
            // v' = q * (v,0) * conj(q)
            fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
                let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
                let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
                [
                    aw * bx + ax * bw + ay * bz - az * by,
                    aw * by - ax * bz + ay * bw + az * bx,
                    aw * bz + ax * by - ay * bx + az * bw,
                    aw * bw - ax * bx - ay * by - az * bz,
                ]
            }

            let vq = [v[0], v[1], v[2], 0.0f32];
            let t = quat_mul(q, vq);
            let r = quat_mul(t, quat_conjugate(q));
            [r[0], r[1], r[2]]
        }

        match forward_axis {
            ForwardAxis::Y => {
                // Legacy 2D-style translation delta (x/y).
                let mut dx = 0.0f32;
                let mut dy = 0.0f32;

                if w {
                    dy -= 1.0;
                }
                if s {
                    dy += 1.0;
                }
                if a {
                    dx -= 1.0;
                }
                if d {
                    dx += 1.0;
                }

                // Normalize diagonal movement.
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.0 {
                    dx /= len;
                    dy /= len;
                }

                // Translate in the transform's local (rotated) axes.
                let v = quat_rotate_vec3(transform.rotation, [dx, dy, 0.0]);
                transform.translation[0] += v[0] * speed;
                transform.translation[1] += v[1] * speed;

            }

            ForwardAxis::Z => {
                // 3D-friendly translation delta (x/z). We intentionally do not apply the
                // current rotation to this movement; it's meant for a camera rig.
                let mut dx = 0.0f32;
                let mut dy: f32 = 0.0f32;
                let mut dz = 0.0f32;

                if a {
                    dx -= 1.0;
                }
                if d {
                    dx += 1.0;
                }
                if r {
                    dy -= 1.0;
                }
                if f {
                    dy += 1.0;
                }
                if w {
                    dz -= 1.0;
                }
                if s {
                    dz += 1.0;
                }
                

                // Normalize diagonal movement.
                let len = (dx * dx + dy * dy + dz * dz).sqrt();
                if len > 0.0 {
                    dx /= len;
                    dy /= len;
                    dz /= len;
                }

                // Move in the rig's local space (so yaw affects movement).
                let v = quat_rotate_vec3(transform.rotation, [dx, dy, dz]);
                transform.translation[0] += v[0] * speed;
                transform.translation[1] += v[1] * speed;
                transform.translation[2] += v[2] * speed;
            }
        }

        transform.recompute_model();
    }

    /// Process input and queue at most one transform update per InputComponent.
    ///
    /// This only supports the intended topology:
    /// InputComponent -> TransformComponent (child)
    pub fn process_input(
        &mut self,
        world: &mut World,
        input: &InputState,
        queue: &mut crate::engine::ecs::CommandQueue,
        dt_sec: f32,
    ) {
        // We gate early to avoid scanning inputs if nothing relevant is pressed.
        let any_move = input.key_down(&Key::Character("w".into()))
            || input.key_down(&Key::Character("a".into()))
            || input.key_down(&Key::Character("s".into()))
            || input.key_down(&Key::Character("d".into()))
            || input.key_down(&Key::Character("r".into()))
            || input.key_down(&Key::Character("f".into()))
            || input.key_down(&Key::Character("q".into()))
            || input.key_down(&Key::Character("e".into()));

        if !any_move {
            return;
        }

        for &input_cid in &self.inputs {
            let speed_units_per_sec =
                match world.get_component_by_id_as::<InputComponent>(input_cid) {
                    Some(input_comp) => input_comp.speed,
                    None => continue,
                };

            // Find TransformComponent child. If absent, we don't compute.
            let transform_child = world.children_of(input_cid).iter().copied().find(|&cid| {
                world
                    .get_component_by_id_as::<TransformComponent>(cid)
                    .is_some()
            });

            // Optional mode child.
            let (forward_axis, roll_axis) = world
                .children_of(input_cid)
                .iter()
                .copied()
                .find_map(|cid| {
                    world
                        .get_component_by_id_as::<InputTransformModeComponent>(cid)
                        .map(|m| (m.forward_axis, m.roll_axis))
                })
                .unwrap_or((ForwardAxis::Y, RollAxis::Z));

            let Some(transform_cid) = transform_child else {
                continue;
            };

            if let Some(transform_comp_mut) =
                world.get_component_by_id_as_mut::<TransformComponent>(transform_cid)
            {
                self.compute_transform(
                    forward_axis,
                    roll_axis,
                    speed_units_per_sec,
                    input,
                    dt_sec,
                    &mut transform_comp_mut.transform,
                );
                queue.queue_update_transform(transform_cid, transform_comp_mut.transform);
            }
        }
    }
}

impl System for InputSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // InputSystem is driven by SystemWorld::tick calling process_input with a CommandQueue.
    }
}
