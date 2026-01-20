use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{ForwardAxis, InputComponent, InputTransformModeComponent, RollAxis, TransformComponent};
use crate::engine::ecs::system::System;
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use crate::utils::math;
use std::collections::HashMap;
use winit::keyboard::{Key, NamedKey};

/// System that processes input components and updates transforms based on WASD input.
///
/// Intended topology (simple one-way data flow):
/// InputComponent -> TransformComponent -> (Camera2DComponent, RenderableComponent, ...)
#[derive(Debug, Default)]
pub struct InputSystem {
    inputs: Vec<ComponentId>,

    // FPS mode needs stable yaw/pitch without per-frame extraction.
    // Keyed by the controlled TransformComponent id.
    fps_yaw_pitch: HashMap<ComponentId, (f32, f32)>,
}

impl InputSystem {
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            fps_yaw_pitch: HashMap::new(),
        }
    }

    /// Register an InputComponent.
    pub fn register_input(&mut self, component: ComponentId) {
        if !self.inputs.iter().any(|c| *c == component) {
            self.inputs.push(component);
        }
    }

    fn compute_rotation(
        &self,
        roll_axis: RollAxis,
        fps_rotation: bool,
        input: &InputState,
        dt_sec: f32,
        rotation: &mut [f32; 4],
    ) {
        // Roll keys.
        let q = input.key_down(&Key::Character("q".into()));
        let e = input.key_down(&Key::Character("e".into()));

        // Mouse drag rotates the rig (yaw + pitch).
        let (drag_dx, drag_dy) = if input.mouse_dragging() {
            input.mouse_drag_delta()
        } else {
            (0.0, 0.0)
        };

        // Sensitivity is radians per pixel.
        const MOUSE_SENS_RAD_PER_PX: f32 = 0.003;
        let yaw_delta = drag_dx * MOUSE_SENS_RAD_PER_PX;
        let pitch_delta = -drag_dy * MOUSE_SENS_RAD_PER_PX;

        if fps_rotation {
            // FPS-style semantics:
            // - keep rotation as TRS yaw/pitch (stable)
            // - yaw about world up
            // - pitch about camera-right after yaw
            // - clamp pitch to avoid flipping

            // Derive current yaw/pitch from basis vectors.
            // Important: yaw-from-forward becomes ill-defined when looking straight up/down;
            // using the right vector keeps yaw stable under pitch.
            let right = math::quat_rotate_vec3(*rotation, [1.0, 0.0, 0.0]);
            let right_n = math::vec3_normalize(right);
            let fwd = math::quat_rotate_vec3(*rotation, [0.0, 0.0, -1.0]);
            let fwd_n = math::vec3_normalize(fwd);

            let mut yaw = right_n[2].atan2(right_n[0]);
            let mut pitch = fwd_n[1].clamp(-1.0, 1.0).asin();

            yaw += yaw_delta;
            pitch += pitch_delta;

            const MAX_PITCH: f32 = 1.55; // ~88.8deg
            pitch = pitch.clamp(-MAX_PITCH, MAX_PITCH);

            let q_yaw = math::quat_from_axis_angle([0.0, 1.0, 0.0], yaw);
            let right = math::quat_rotate_vec3(q_yaw, [1.0, 0.0, 0.0]);
            let q_pitch = math::quat_from_axis_angle(right, pitch);

            // Apply yaw then pitch (pitch axis depends on yaw).
            *rotation = math::quat_mul(q_pitch, q_yaw);

            // For now, disable roll keys in FPS mode to keep the camera stable.
        } else {
            // Relative/flight-style semantics: apply local incremental rotations.
            if yaw_delta != 0.0 {
                let q_yaw = math::quat_from_axis_angle([0.0, 1.0, 0.0], yaw_delta);
                *rotation = math::quat_mul(*rotation, q_yaw);
            }
            if pitch_delta != 0.0 {
                let q_pitch = math::quat_from_axis_angle([1.0, 0.0, 0.0], pitch_delta);
                *rotation = math::quat_mul(*rotation, q_pitch);
            }

            if q || e {
                const ROT_SPEED_RAD_PER_SEC: f32 = 1.5;
                let dir = (q as i32) as f32 - (e as i32) as f32;
                let dtheta = dir * ROT_SPEED_RAD_PER_SEC * dt_sec;
                let axis = match roll_axis {
                    RollAxis::X => [1.0, 0.0, 0.0],
                    RollAxis::Y => [0.0, 1.0, 0.0],
                    RollAxis::Z => [0.0, 0.0, 1.0],
                };
                let q_roll = math::quat_from_axis_angle(axis, dtheta);
                *rotation = math::quat_mul(*rotation, q_roll);
            }
        }
    }

    fn compute_rotation_fps(
        &mut self,
        transform_cid: ComponentId,
        input: &InputState,
        rotation: &mut [f32; 4],
    ) {
        let (drag_dx, drag_dy) = if input.mouse_dragging() {
            input.mouse_drag_delta()
        } else {
            (0.0, 0.0)
        };

        // Sensitivity is radians per pixel.
        const MOUSE_SENS_RAD_PER_PX: f32 = 0.003;
        let yaw_delta = drag_dx * MOUSE_SENS_RAD_PER_PX;
        let pitch_delta = -drag_dy * MOUSE_SENS_RAD_PER_PX;

        if yaw_delta == 0.0 && pitch_delta == 0.0 {
            return;
        }

        // Initialize once from current rotation.
        let (mut yaw, mut pitch) = self.fps_yaw_pitch.get(&transform_cid).copied().unwrap_or_else(|| {
            let right = math::vec3_normalize(math::quat_rotate_vec3(*rotation, [1.0, 0.0, 0.0]));
            let fwd = math::vec3_normalize(math::quat_rotate_vec3(*rotation, [0.0, 0.0, -1.0]));

            // Yaw is global (world up): angle around +Y.
            let yaw = right[2].atan2(right[0]);
            // Pitch comes from forward Y.
            let pitch = fwd[1].clamp(-1.0, 1.0).asin();
            (yaw, pitch)
        });

        // Apply deltas.
        yaw += yaw_delta;
        pitch += pitch_delta;

        const MAX_PITCH: f32 = 1.55; // ~88.8deg
        pitch = pitch.clamp(-MAX_PITCH, MAX_PITCH);

        // Persist state.
        self.fps_yaw_pitch.insert(transform_cid, (yaw, pitch));

        // Rebuild rotation from TRS yaw/pitch.
        // Yaw: global axis. Pitch: relative to yaw (around yaw-rotated right).
        let q_yaw = math::quat_from_axis_angle([0.0, 1.0, 0.0], yaw);
        let right = math::quat_rotate_vec3(q_yaw, [1.0, 0.0, 0.0]);
        let q_pitch = math::quat_from_axis_angle(right, pitch);

        *rotation = math::quat_mul(q_pitch, q_yaw);
    }

    fn compute_translation(
        &self,
        forward_axis: ForwardAxis,
        fps_rotation: bool,
        fps_yaw: Option<f32>,
        speed_units_per_sec: f32,
        input: &InputState,
        dt_sec: f32,
        rotation: [f32; 4],
        translation: &mut [f32; 3],
    ) {
        // Read movement keys.
        let w = input.key_down(&Key::Character("w".into()));
        let a = input.key_down(&Key::Character("a".into()));
        let s = input.key_down(&Key::Character("s".into()));
        let d = input.key_down(&Key::Character("d".into()));
        let r: bool = input.key_down(&Key::Character("r".into()));
        let f: bool = input.key_down(&Key::Character("f".into()));

        // Holding Shift increases movement speed.
        let speed_multiplier = if input.key_down(&Key::Named(NamedKey::Shift)) {
            3.0
        } else {
            1.0
        };

        let speed = speed_units_per_sec * speed_multiplier * dt_sec;

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
                let v = math::quat_rotate_vec3(rotation, [dx, dy, 0.0]);
                translation[0] += v[0] * speed;
                translation[1] += v[1] * speed;
            }

            ForwardAxis::Z => {
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

                if fps_rotation {
                    // FPS: yaw drives horizontal movement; pitch doesn't.
                    let yaw = fps_yaw.unwrap_or_else(|| {
                        let right = math::quat_rotate_vec3(rotation, [1.0, 0.0, 0.0]);
                        right[2].atan2(right[0])
                    });
                    let q_yaw = math::quat_from_axis_angle([0.0, 1.0, 0.0], yaw);
                    let v = math::quat_rotate_vec3(q_yaw, [dx, 0.0, dz]);
                    translation[0] += v[0] * speed;
                    translation[1] += dy * speed;
                    translation[2] += v[2] * speed;
                } else {
                    // Flight/relative: full rotation drives movement.
                    let v = math::quat_rotate_vec3(rotation, [dx, dy, dz]);
                    translation[0] += v[0] * speed;
                    translation[1] += v[1] * speed;
                    translation[2] += v[2] * speed;
                }
            }
        }
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

        let any_drag = input.mouse_dragging();

        if !any_move && !any_drag {
            return;
        }

        let inputs = self.inputs.clone();
        for input_cid in inputs {
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
            let (forward_axis, roll_axis, fps_rotation) = world
                .children_of(input_cid)
                .iter()
                .copied()
                .find_map(|cid| {
                    world
                        .get_component_by_id_as::<InputTransformModeComponent>(cid)
                        .map(|m| (m.forward_axis, m.roll_axis, m.fps_rotation))
                })
                .unwrap_or((ForwardAxis::Y, RollAxis::Z, false));

            let Some(transform_cid) = transform_child else {
                continue;
            };

            if let Some(transform_comp_mut) =
                world.get_component_by_id_as_mut::<TransformComponent>(transform_cid)
            {
                if fps_rotation {
                    self.compute_rotation_fps(
                        transform_cid,
                        input,
                        &mut transform_comp_mut.transform.rotation,
                    );
                } else {
                    self.compute_rotation(
                        roll_axis,
                        fps_rotation,
                        input,
                        dt_sec,
                        &mut transform_comp_mut.transform.rotation,
                    );
                }
                let rot = transform_comp_mut.transform.rotation;
                let fps_yaw = if fps_rotation {
                    self.fps_yaw_pitch.get(&transform_cid).map(|(y, _)| *y)
                } else {
                    None
                };
                self.compute_translation(
                    forward_axis,
                    fps_rotation,
                    fps_yaw,
                    speed_units_per_sec,
                    input,
                    dt_sec,
                    rot,
                    &mut transform_comp_mut.transform.translation,
                );

                transform_comp_mut.transform.recompute_model();
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
