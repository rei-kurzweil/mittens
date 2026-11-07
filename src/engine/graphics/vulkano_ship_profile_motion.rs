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
