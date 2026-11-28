// Test-only native locomotion/mounting scenarios. No authored scene changes.
use super::*;
use crate::engine::ecs::component::{MirrorComponent, PointerComponent, TextComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::external_xr_frame::ExternalXrFrame;
use crate::engine::ecs::{EventSignal, SignalEmitter};

pub(super) struct ShipMotion {
    mode: String,
    player: ComponentId,
    truck: ComponentId,
    mirror: ComponentId,
    power_text: ComponentId,
    background: [ComponentId; 4],
}
#[derive(serde::Serialize)]
pub(super) struct MotionSample {
    pub player_position: [f32; 3],
    pub truck_position: [f32; 3],
    pub mounted: bool,
    pub mirror_enabled: bool,
    pub powered: bool,
}
impl ShipMotion {
    pub(super) fn setup(scene: &mut Universe, mode: &str) -> Result<Self, Box<dyn Error>> {
        let player = named(scene, "ship_player_root");
        let truck = named(scene, "pickup_truck");
        assert_eq!(
            scene
                .world
                .all_components()
                .filter(|&id| scene.world.component_label(id) == Some("pickup_truck"))
                .count(),
            1,
            "walking/driving fixture requires exactly one pickup"
        );
        let state = Self {
            mode: mode.into(),
            player,
            truck,
            mirror: named(scene, "ship_womens_washroom_mirror"),
            power_text: named(scene, "pickup_power_readout"),
            background: [
                named(scene, "ship_control_room"),
                named(scene, "ship_barracks"),
                named(scene, "ship_womens_washroom"),
                named(scene, "ship_mall_cafeteria"),
            ],
        };
        if mode == "driving" {
            let camera = named(scene, "ship_mitten_xr_camera");
            let pointer = scene
                .world
                .children_of(camera)
                .iter()
                .copied()
                .find(|&id| {
                    scene
                        .world
                        .get_component_by_id_as::<PointerComponent>(id)
                        .is_some()
                })
                .ok_or("XR camera pointer unavailable")?;
            let eye = TransformSystem::world_position(&scene.world, camera)
                .ok_or("XR eye pose unavailable")?;
            let entry =
                TransformSystem::world_position(&scene.world, named(scene, "pickup_entry_frame"))
                    .ok_or("truck entry pose unavailable")?;
            let delta = [entry[0] - eye[0], entry[1] - eye[1], entry[2] - eye[2]];
            let local_delta = scene
                .world
                .parent_of(player)
                .and_then(|p| TransformSystem::world_model(&scene.world, p))
                .and_then(mat4_inverse)
                .map(|inverse| {
                    crate::utils::math::mat4_mul_vec4(inverse, [delta[0], delta[1], delta[2], 0.])
                })
                .unwrap_or([delta[0], delta[1], delta[2], 0.]);
            let transform = scene
                .world
                .get_component_by_id_as_mut::<TransformComponent>(player)
                .unwrap();
            for axis in 0..3 {
                transform.transform.translation[axis] += local_delta[axis];
            }
            transform.transform.recompute_model();
            scene
                .systems
                .transform_changed(&mut scene.world, &mut scene.visuals, player);
            assert!(
                scene.systems.attachment.try_mount_from_hit(
                    &mut scene.world,
                    pointer,
                    truck,
                    &mut scene.command_queue
                ),
                "driving benchmark must use the existing mount transaction"
            );
            let button = named(scene, "pickup_power_button");
            scene.command_queue.push_event(
                button,
                EventSignal::Click {
                    raycaster: pointer,
                    renderable: button,
                    hit_point: entry,
                    screen_pos_px: None,
                },
            );
            // Process mount/power events before warmup and preserve the existing controls.
            for _ in 0..3 {
                scene.update(1. / 75., &Default::default());
            }
            assert!(
                scene
                    .systems
                    .attachment
                    .is_movement_root_mounted(&scene.world, player)
            );
            assert_eq!(
                scene
                    .world
                    .get_component_by_id_as::<TextComponent>(state.power_text)
                    .unwrap()
                    .text,
                "POWER ON"
            );
        }
        state.sample(scene);
        Ok(state)
    }
    pub(super) fn input(&self, scene: &mut Universe, view: openxr::View, frame: usize) {
        let mut tracked = view;
        // A small head turn stays in both scenarios; locomotion comes from input.
        let yaw = (frame as f32 / 75.).sin() * 0.06;
        let extra = quat_rotation_y(yaw);
        let original = [
            view.pose.orientation.x,
            view.pose.orientation.y,
            view.pose.orientation.z,
            view.pose.orientation.w,
        ];
        let rotation = quat_mul(original, extra);
        tracked.pose.orientation = openxr::Quaternionf {
            x: rotation[0],
            y: rotation[1],
            z: rotation[2],
            w: rotation[3],
        };
        let mut external = ExternalXrFrame {
            views: Some([tracked, tracked]),
            ..Default::default()
        };
        external.gamepad.active = true;
        external.gamepad.head_pose_rotation = Some(rotation);
        external.gamepad.hands[0].thumbstick = Some(if self.mode == "driving" {
            let direction = if (frame / 135) % 2 == 0 { 1. } else { -1. };
            [0.20, 0.32 * direction]
        } else {
            let direction = if (frame / 150) % 2 == 0 { 1. } else { -1. };
            [0., 0.60 * direction]
        });
        scene.systems.xr.set_external_frame(Some(external));
    }
    pub(super) fn sample(&self, scene: &Universe) -> MotionSample {
        for &id in &self.background {
            assert!(
                scene.world.component_label(id).is_some(),
                "background room unloaded during measurement"
            );
        }
        let mirror = scene
            .world
            .get_component_by_id_as::<MirrorComponent>(self.mirror)
            .expect("washroom mirror must remain loaded");
        assert!(
            !mirror.enabled,
            "cafeteria route unexpectedly enabled the washroom mirror"
        );
        let mounted = scene
            .systems
            .attachment
            .is_movement_root_mounted(&scene.world, self.player);
        assert_eq!(
            mounted,
            self.mode == "driving",
            "mount state changed during benchmark"
        );
        let powered = scene
            .world
            .get_component_by_id_as::<TextComponent>(self.power_text)
            .unwrap()
            .text
            == "POWER ON";
        assert_eq!(powered, self.mode == "driving");
        MotionSample {
            player_position: TransformSystem::world_position(&scene.world, self.player).unwrap(),
            truck_position: TransformSystem::world_position(&scene.world, self.truck).unwrap(),
            mounted,
            mirror_enabled: mirror.enabled,
            powered,
        }
    }
    pub(super) fn validate(&self, rows: &[serde_json::Value]) {
        let key = if self.mode == "driving" {
            "truck_position"
        } else {
            "player_position"
        };
        let positions: Vec<[f64; 3]> = rows
            .iter()
            .map(|row| std::array::from_fn(|i| row["motion_state"][key][i].as_f64().unwrap()))
            .collect();
        let range = |axis: usize| {
            positions
                .iter()
                .map(|p| p[axis])
                .fold(f64::NEG_INFINITY, f64::max)
                - positions
                    .iter()
                    .map(|p| p[axis])
                    .fold(f64::INFINITY, f64::min)
        };
        assert!(
            range(0) + range(2) > 0.6,
            "scenario did not produce meaningful physical movement"
        );
        for p in positions {
            assert!(
                p[0] > -11.5 && p[0] < 11.5 && p[2] > 6. && p[2] < 19.,
                "scenario left cafeteria: {p:?}"
            );
        }
    }
}

// Render-only camera replay isolates pixel workload from vehicle/input state.
// It deliberately does not move the player's ECS transform or tracked inputs.
pub(super) struct CameraReplay {
    pub path: String,
    views: Vec<[[f32; 4]; 4]>,
}
impl CameraReplay {
    pub(super) fn from_env() -> Result<Option<Self>, Box<dyn Error>> {
        let Ok(path) = std::env::var("SHIP_PROFILE_CAMERA_REFERENCE") else {
            return Ok(None);
        };
        let rows: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let views = rows
            .into_iter()
            .map(|r| serde_json::from_value(r["eye0_view"].clone()))
            .collect::<Result<Vec<_>, _>>()?;
        assert!(!views.is_empty());
        Ok(Some(Self { path, views }))
    }
    pub(super) fn apply(&self, scene: &mut Universe, frame: usize, warmup: usize) {
        let view = self.views[frame.saturating_sub(warmup) % self.views.len()];
        let model = mat4_inverse(view).expect("reference camera invertible");
        let trs = crate::engine::transform::TransformTrs::from_matrix(model).unwrap();
        let mut eyes = scene
            .visuals
            .visual_camera(crate::engine::graphics::CameraTarget::Xr)
            .unwrap()
            .eyes
            .clone();
        for eye in &mut eyes {
            eye.view = view;
            eye.transform.translation = trs.translation;
            eye.transform.rotation = trs.rotation_quat_xyzw;
            eye.transform.scale = trs.scale;
            eye.transform.model = model;
            eye.transform.matrix_world = model;
        }
        scene.visuals.set_xr_camera(eyes);
    }
}

// Interceptor play: pick up the spawn-side rocket launcher with the right
// laser, then walk about, look around and sway the hand while firing it
// every 2 s (the launcher restocks itself on a timer). The rig is unrotated
// at spawn, so world-space deltas are also reference-space deltas.
pub(super) struct InterceptorPlay {
    pointer: ComponentId,
    grip: ComponentId,
}
const RIGHT: usize = 1;
// The laser settles on the grip before the grip button goes down.
const INTERCEPTOR_GRAB_FRAME: usize = 60;
const INTERCEPTOR_HEAD: [f32; 3] = [0., 1.65, 0.];
impl InterceptorPlay {
    pub(super) fn setup(scene: &Universe) -> Self {
        Self {
            pointer: named(scene, "local_right_pointer"),
            grip: named(scene, "interceptor_sta_launcher_grip"),
        }
    }
    pub(super) fn input(&self, scene: &mut Universe, view: openxr::View, frame: usize) {
        let (head_rotation, hand, aim_rotation) = if frame < INTERCEPTOR_GRAB_FRAME {
            let hand = [0.22, 1.30, -0.30];
            let target = TransformSystem::world_position(&scene.world, self.grip);
            let origin = TransformSystem::world_position(&scene.world, self.pointer);
            let aim = match (target, origin) {
                (Some(t), Some(o)) => look_rotation([t[0] - o[0], t[1] - o[1], t[2] - o[2]]),
                _ => [0., 0., 0., 1.],
            };
            ([0., 0., 0., 1.], hand, aim)
        } else {
            // Sweep about ±70° left/right and ±15° up/down, hand following the view.
            let t = (frame - INTERCEPTOR_GRAB_FRAME) as f32 / 75.;
            let yaw = (t * 0.45).sin() * 1.2;
            let pitch = (t * 0.7).sin() * 0.25;
            let rotation = crate::utils::math::quat_mul(
                crate::utils::math::quat_from_axis_angle([0., 1., 0.], yaw),
                crate::utils::math::quat_from_axis_angle([1., 0., 0.], pitch),
            );
            // The hand sways a few centimetres around its place in view.
            let sway = [(t * 1.3).sin() * 0.06, (t * 1.7).sin() * 0.05, (t * 0.9).cos() * 0.04];
            let offset = crate::utils::math::quat_rotate_vec3(
                rotation,
                [0.22 + sway[0], -0.35 + sway[1], -0.40 + sway[2]],
            );
            let hand = [
                INTERCEPTOR_HEAD[0] + offset[0],
                INTERCEPTOR_HEAD[1] + offset[1],
                INTERCEPTOR_HEAD[2] + offset[2],
            ];
            (rotation, hand, rotation)
        };
        let quaternion = |q: [f32; 4]| openxr::Quaternionf {
            x: q[0],
            y: q[1],
            z: q[2],
            w: q[3],
        };
        let mut tracked = view;
        tracked.pose.orientation = quaternion(head_rotation);
        let hand_pose = openxr::Posef {
            orientation: quaternion(aim_rotation),
            position: openxr::Vector3f {
                x: hand[0],
                y: hand[1],
                z: hand[2],
            },
        };
        let mut external = ExternalXrFrame {
            views: Some([tracked, tracked]),
            ..Default::default()
        };
        external.aim[RIGHT] = Some(hand_pose);
        external.grip[RIGHT] = Some(hand_pose);
        external.gamepad.active = true;
        external.gamepad.head_pose_rotation = Some(head_rotation);
        // Walk with the left stick after the grab: slow zero-mean oscillations,
        // so the player wanders a few metres without leaving the deck.
        if frame >= INTERCEPTOR_GRAB_FRAME {
            let t = (frame - INTERCEPTOR_GRAB_FRAME) as f32 / 75.;
            external.gamepad.hands[0].thumbstick = Some([(t * 0.37).sin() * 0.5, (t * 0.5).sin() * 0.7]);
        }
        let holding = frame >= INTERCEPTOR_GRAB_FRAME;
        external.input.grip_pressed[RIGHT] = frame == INTERCEPTOR_GRAB_FRAME;
        external.input.grip_down[RIGHT] = holding;
        external.gamepad.hands[RIGHT].grip_pressed = Some((holding, if holding { 1. } else { 0. }));
        // Fire for 10 frames every 2 s, starting 1 s after the grab.
        let firing = frame >= INTERCEPTOR_GRAB_FRAME + 75
            && (frame - INTERCEPTOR_GRAB_FRAME - 75) % 150 < 10;
        let phase = frame.saturating_sub(INTERCEPTOR_GRAB_FRAME + 75) % 150;
        external.input.trigger_pressed[RIGHT] = firing && phase == 0;
        external.input.trigger_down[RIGHT] = firing;
        external.input.trigger_released[RIGHT] = frame > INTERCEPTOR_GRAB_FRAME + 75 && phase == 10;
        external.gamepad.hands[RIGHT].trigger_pressed = Some((firing, if firing { 1. } else { 0. }));
        external.gamepad.hands[RIGHT].trigger_value = Some(if firing { 1. } else { 0. });
        scene.systems.xr.set_external_frame(Some(external));
    }
    /// The launcher is in the right hand when its grip sits at the pointer.
    pub(super) fn holding(&self, scene: &Universe) -> bool {
        let grip = TransformSystem::world_position(&scene.world, self.grip);
        let pointer = TransformSystem::world_position(&scene.world, self.pointer);
        match (grip, pointer) {
            (Some(g), Some(p)) => {
                let d = [g[0] - p[0], g[1] - p[1], g[2] - p[2]];
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < 0.35
            }
            _ => false,
        }
    }
}

/// Rotation turning -Z towards `direction` with no roll.
fn look_rotation(direction: [f32; 3]) -> [f32; 4] {
    let horizontal = (direction[0] * direction[0] + direction[2] * direction[2]).sqrt();
    let yaw = (-direction[0]).atan2(-direction[2]);
    let pitch = direction[1].atan2(horizontal);
    crate::utils::math::quat_mul(
        crate::utils::math::quat_from_axis_angle([0., 1., 0.], yaw),
        crate::utils::math::quat_from_axis_angle([1., 0., 0.], pitch),
    )
}
