//! Smoke coverage for the optional, ignored local pool viewer when present.
use crate::engine::Universe;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    AvatarControlComponent, ControllerXRComponent, HandTrackingComponent, InputXRComponent,
    SliderComponent,
};
use crate::engine::ecs::system::{XrInputState, external_xr_frame::ExternalXrFrame};
use openxr as xr;

#[test]
fn local_pool_viewer_uses_xr_settings_and_keeps_grip_input_after_avatar_init() {
    let path = "assets/models/tmp/pool-model-viewer.mms";
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
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
        aim: [Some(pose([-0.3, 1.2, -0.4])), Some(pose([0.3, 1.2, -0.4]))],
        grip: [Some(pose([-0.3, 1.2, -0.4])), Some(pose([0.3, 1.2, -0.4]))],
        ..Default::default()
    };
    let mut scene = Universe::new(World::default());
    scene.systems.xr.set_external_frame(Some(frame));
    scene.load_mms_source_at_path(&source, path).unwrap();
    for _ in 0..12 {
        scene.update(1.0 / 90.0, &Default::default());
    }
    assert!(scene.world.all_components().any(|id| {
        scene
            .world
            .get_component_by_id_as::<HandTrackingComponent>(id)
            .is_some()
    }));
    assert!(
        !scene
            .world
            .all_components()
            .any(|id| scene.world.component_label(id) == Some("rei_mu_mouth_response_panel"))
    );
    assert!(
        scene
            .world
            .all_components()
            .any(|id| scene.world.component_label(id) == Some("rei_mu_xr_settings"))
    );
    assert!(scene.world.all_components().any(|id| {
        scene.world.component_label(id) == Some("xr_floor_correction_slider")
            && scene
                .world
                .get_component_by_id_as::<SliderComponent>(id)
                .is_some()
    }));
    assert!(
        scene
            .world
            .all_components()
            .filter_map(|id| scene
                .world
                .get_component_by_id_as::<AvatarControlComponent>(id))
            .any(|avc| avc.left_hand_visual_target_id.is_some()
                && avc.right_hand_visual_target_id.is_some())
    );
    assert!(
        scene
            .world
            .all_components()
            .filter_map(|id| scene.world.get_component_by_id_as::<InputXRComponent>(id))
            .any(|input| input.pose_valid)
    );
    assert_eq!(
        scene
            .world
            .all_components()
            .filter_map(|id| scene
                .world
                .get_component_by_id_as::<ControllerXRComponent>(id))
            .filter(|hand| hand.pose_valid)
            .count(),
        2
    );
    let activations = scene.systems.pointer.build_activations(
        &scene.world,
        &Default::default(),
        &XrInputState {
            grip_pressed: [true, false],
            grip_down: [true, false],
            ..Default::default()
        },
    );
    assert_eq!(
        activations.grip_pressed.len(),
        1,
        "left grip must activate a mounted avatar pointer"
    );
}
