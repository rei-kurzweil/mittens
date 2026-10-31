use super::*;
use crate::engine::Universe;
use crate::engine::ecs::component::{PointerComponent, TransformComponent};
use crate::engine::ecs::system::external_xr_frame::ExternalXrFrame;
use openxr as xr;

struct Fixture {
    scene: Universe,
    frame: ExternalXrFrame,
    root: ComponentId,
    raycaster: ComponentId,
    ui: ComponentId,
}
impl Fixture {
    fn new(marker: &str, hand_z: f32, scene_wrapper: &str) -> Self {
        let source = format!(
            r#"
            {scene_wrapper} {{
                T.position(0,1,-1).scale(0.2,0.2,0.02) {{ {marker} R.cube() {{ name = "rung" }} }}
                T.position(0,1,-2).scale(0.2,0.2,0.02) {{ Raycastable.enabled() R.cube() {{ name = "ui" }} }}
                T {{ name = "climber" InputXR.on() {{
                    T {{ CXR {{}} }}
                    XRHand.new(true,"Left","GripAim") {{ T {{ Pointer {{}} }} }}
                }} }}
            }}
        "#
        );
        let mut scene = Universe::new(World::default());
        scene
            .load_mms_source_at_path(&source, "examples/climbing-origin-test.mms")
            .unwrap();
        let pose = |p: [f32; 3]| xr::Posef {
            orientation: xr::Quaternionf {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            position: xr::Vector3f {
                x: p[0],
                y: p[1],
                z: p[2],
            },
        };
        let view = xr::View {
            pose: pose([0.0, 1.65, 0.0]),
            fov: xr::Fovf {
                angle_left: -0.7,
                angle_right: 0.7,
                angle_up: 0.7,
                angle_down: -0.7,
            },
        };
        let frame = ExternalXrFrame {
            views: Some([view, view]),
            aim: [Some(pose([0.0, 1.0, hand_z])), None],
            grip: [Some(pose([0.0, 1.0, hand_z])), None],
            ..Default::default()
        };
        scene.systems.xr.set_external_frame(Some(frame));
        scene.update(1.0 / 90.0, &Default::default());
        let root = scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some("climber"))
            .unwrap();
        let ui = scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some("ui"))
            .unwrap();
        let pointer = scene
            .world
            .all_components()
            .find(|&id| {
                scene
                    .world
                    .get_component_by_id_as::<PointerComponent>(id)
                    .is_some()
            })
            .unwrap();
        let raycaster = scene.systems.pointer.raycast_for_pointer(pointer).unwrap();
        Self {
            scene,
            frame,
            root,
            raycaster,
            ui,
        }
    }
    fn tick(&mut self) {
        self.scene.systems.xr.set_external_frame(Some(self.frame));
        self.scene.update(1.0 / 90.0, &Default::default());
    }
    fn y(&self) -> f32 {
        self.scene
            .world
            .get_component_by_id_as::<TransformComponent>(self.root)
            .unwrap()
            .transform
            .translation[1]
    }
    fn press(&mut self, grip: bool) {
        self.frame.input.trigger_pressed[0] = true;
        self.frame.input.trigger_down[0] = true;
        self.frame.input.grip_pressed[0] = grip;
        self.frame.input.grip_down[0] = grip;
        self.tick();
    }
    fn pull(&mut self) {
        self.frame.input.grip_pressed[0] = false;
        self.frame.input.trigger_pressed[0] = false;
        self.frame.aim[0].as_mut().unwrap().position.y -= 0.2;
        self.frame.grip[0].as_mut().unwrap().position.y -= 0.2;
        self.tick();
    }
}

#[test]
fn climbing_starts_nine_cm_back_and_keeps_simultaneous_trigger_targeting() {
    let mut f = Fixture::new("Climbable {}", -1.075, "T");
    f.press(true);
    let normal = f.scene.systems.raycast.current_ray(f.raycaster).unwrap();
    let climbing = f
        .scene
        .systems
        .raycast
        .climbing_hit(f.raycaster)
        .expect("rung just behind fingertip should be acquired");
    assert!((normal.origin_world[2] + 1.075).abs() < 1e-5);
    assert!((climbing.ray.origin_world[2] + 0.985).abs() < 1e-5);
    assert!(climbing.distance_from_pointer < 0.0);
    assert_eq!(f.scene.systems.gesture.state().drag_renderable, Some(f.ui));
    f.pull();
    assert!(
        (f.y() - 0.2).abs() < 1e-5,
        "pull must move the tracking origin"
    );
    assert!(
        f.scene.systems.raycast.climbing_hit(f.raycaster).is_none(),
        "grip acquisition is only queried on press"
    );
    f.frame.input.grip_down[0] = false;
    f.frame.input.grip_released[0] = true;
    f.tick();
    f.pull();
    assert!(
        (f.y() - 0.2).abs() < 1e-5,
        "released grip cannot keep moving the rig"
    );
}

#[test]
fn backoff_is_world_space_under_a_rotated_scaled_rig() {
    let mut f = Fixture::new(
        "Climbable {}",
        -1.02,
        "T.rotation_euler(0,1.5707963268,0).scale(2,2,2)",
    );
    f.press(true);
    let normal = f.scene.systems.raycast.current_ray(f.raycaster).unwrap();
    let climbing = f.scene.systems.raycast.climbing_hit(f.raycaster).unwrap();
    let delta: [f32; 3] =
        std::array::from_fn(|i| normal.origin_world[i] - climbing.ray.origin_world[i]);
    assert!(
        (math::vec3_len(delta) - 0.09).abs() < 1e-5,
        "backoff must stay nine centimetres in world space"
    );
    assert!(
        delta[0].abs() > 0.089 && delta[2].abs() < 1e-5,
        "backoff must follow the rotated ray"
    );
}

#[test]
fn trigger_only_distant_back_hits_and_ordinary_props_do_not_climb() {
    for (marker, z, grip) in [
        ("Climbable {}", -1.04, false),
        ("Climbable {}", -1.12, true),
        ("Grabbable {}", -1.04, true),
    ] {
        let mut f = Fixture::new(marker, z, "T");
        f.press(grip);
        assert!(f.scene.systems.raycast.climbing_hit(f.raycaster).is_none());
        assert_eq!(f.scene.systems.gesture.state().drag_renderable, Some(f.ui));
        f.pull();
        assert_eq!(f.y(), 0.0);
    }
}
