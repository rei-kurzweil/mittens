//! Exercise real engine pointer/gesture/editor routing with a synthetic XR frame.
use mittens_engine::engine::{
    Universe,
    ecs::{
        component::{EditorComponent, EditorInteractionMode, TransformComponent},
        system::{XrGamepadState, XrInputState, external_xr_frame::ExternalXrFrame},
    },
    user_input::InputState,
};
use openxr as xr;

fn pose(x: f32, y: f32, z: f32) -> xr::Posef {
    xr::Posef {
        position: xr::Vector3f { x, y, z },
        orientation: xr::Quaternionf {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
    }
}
pub fn click_cursor_mode(universe: &mut Universe) {
    for (name, expected) in [
        (
            "editor_settings_mode_cursor_3d",
            EditorInteractionMode::Cursor3d,
        ),
        (
            "editor_settings_mode_select_cursor",
            EditorInteractionMode::SelectAndCursor,
        ),
        ("editor_settings_mode_select", EditorInteractionMode::Select),
    ] {
        click_row(universe, name, expected);
    }
    println!("PASS controller ray clicked cursor, select+cursor, and select settings rows");
}
fn click_row(universe: &mut Universe, row_name: &str, expected: EditorInteractionMode) {
    let world = &universe.world;
    let row = world
        .all_components()
        .find(|&id| world.component_label(id) == Some(row_name))
        .expect("settings row");
    let bg = world
        .children_of(row)
        .iter()
        .copied()
        .find(|&id| world.component_label(id) == Some("__bg"))
        .expect("settings row background");
    let target = world
        .get_component_by_id_as::<TransformComponent>(bg)
        .unwrap()
        .transform
        .matrix_world[3];
    let player = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("sakura_mobile_player"))
        .expect("player root");
    let player = world
        .get_component_by_id_as::<TransformComponent>(player)
        .unwrap()
        .transform
        .matrix_world[3];
    let view = xr::View {
        pose: pose(0.0, 0.0, 0.0),
        fov: xr::Fovf {
            angle_left: -0.6,
            angle_right: 0.6,
            angle_up: 0.6,
            angle_down: -0.6,
        },
    };
    let mut frame = ExternalXrFrame {
        views: Some([view, view]),
        aim: [
            Some(pose(
                target[0] - player[0] + 0.35,
                target[1] - player[1],
                target[2] - player[2] + 0.8,
            )),
            None,
        ],
        grip: [None, None],
        input: XrInputState::default(),
        gamepad: XrGamepadState {
            active: true,
            ..Default::default()
        },
    };
    for phase in 0..3 {
        frame.input = XrInputState::default();
        if phase == 1 {
            frame.input.trigger_down[0] = true;
            frame.input.trigger_pressed[0] = true;
        }
        if phase == 2 {
            frame.input.trigger_released[0] = true;
        }
        universe.systems.xr.set_external_frame(Some(frame));
        universe.update(1.0 / 90.0, &InputState::default());
    }
    let editor = universe
        .world
        .all_components()
        .find_map(|id| universe.world.get_component_by_id_as::<EditorComponent>(id))
        .expect("editor root");
    assert_eq!(
        editor.interaction_mode, expected,
        "controller click did not activate {row_name}"
    );
}
