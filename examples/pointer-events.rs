/// Pointer events demo — Click, DragMove, DragEnd.
///
/// Three groups of interactive cubes, each wired to a different gesture subset:
///
///  LEFT  — "click" cubes: flash color on Click; ignore drags.
///  MID   — "drag" cube: follows the pointer while dragging.
///  RIGHT — "both" cube: drag to reposition AND click to cycle color.
///
/// This example is the primary test bed for EventSignal::Click, which is
/// emitted by GestureSystem at DragEnd time when pointer travel < 8 px.
use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Shared click-cube state
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ClickCubeState {
    color_node: engine::ecs::ComponentId,
    click_count: u32,
}

static CLICK_CUBES: OnceLock<
    Mutex<std::collections::HashMap<engine::ecs::ComponentId, ClickCubeState>>,
> = OnceLock::new();

fn click_cube_colors() -> &'static [[f32; 4]] {
    &[
        [0.25, 0.55, 1.00, 1.0], // blue (default)
        [1.00, 0.35, 0.35, 1.0], // red
        [0.30, 0.85, 0.45, 1.0], // green
        [1.00, 0.80, 0.15, 1.0], // yellow
        [0.75, 0.30, 1.00, 1.0], // purple
    ]
}

fn on_click_cube(
    world: &mut engine::ecs::World,
    emit: &mut dyn engine::ecs::SignalEmitter,
    env: &engine::ecs::Signal,
) {
    if !matches!(
        env.event.as_ref(),
        Some(engine::ecs::EventSignal::Click { .. })
    ) {
        return;
    }

    // env.scope is the hit renderable; walk up to find the cube root in the map.
    let map = CLICK_CUBES.get_or_init(|| Mutex::new(Default::default()));
    let mut guard = map.lock().unwrap();
    let mut cur = Some(env.scope);
    let mut found_root = None;
    while let Some(id) = cur {
        if guard.contains_key(&id) {
            found_root = Some(id);
            break;
        }
        cur = world.parent_of(id);
    }
    let Some(root) = found_root else { return };
    let Some(state) = guard.get_mut(&root) else {
        return;
    };

    state.click_count += 1;
    let colors = click_cube_colors();
    let rgba = colors[state.click_count as usize % colors.len()];

    emit.push_intent_now(
        root,
        engine::ecs::IntentValue::SetColor {
            component_ids: vec![state.color_node],
            rgba,
        },
    );
}

fn spawn_click_cube(
    universe: &mut engine::Universe,
    parent: engine::ecs::ComponentId,
    pos: [f32; 3],
) -> engine::ecs::ComponentId {
    use engine::ecs::component::{
        ColorComponent, RaycastableComponent, RenderableComponent, TransformComponent,
    };

    let root = universe.world.add_component(
        TransformComponent::new()
            .with_position(pos[0], pos[1], pos[2])
            .with_scale(0.40, 0.40, 0.40),
    );
    let color_node = universe
        .world
        .add_component(ColorComponent::rgba(0.25, 0.55, 1.0, 1.0));
    let r = universe.world.add_component(RenderableComponent::cube());
    let rc = universe
        .world
        .add_component(RaycastableComponent::enabled());

    // Build the subtree before connecting to the live parent so that
    // RaycastableComponent is already a child of RenderableComponent when
    // RegisterRenderable is processed (renderable_is_raycastable checks children).
    let _ = universe.attach(root, color_node);
    let _ = universe.attach(color_node, r);
    let _ = universe.attach(r, rc);
    let _ = universe.attach(parent, root);

    CLICK_CUBES
        .get_or_init(|| Mutex::new(Default::default()))
        .lock()
        .unwrap()
        .insert(
            root,
            ClickCubeState {
                color_node,
                click_count: 0,
            },
        );

    universe.add_signal_handler(engine::ecs::SignalKind::Click, root, on_click_cube);

    root
}

// ---------------------------------------------------------------------------
// Drag cube — follows pointer while held
// ---------------------------------------------------------------------------

fn on_drag_move(
    world: &mut engine::ecs::World,
    emit: &mut dyn engine::ecs::SignalEmitter,
    env: &engine::ecs::Signal,
) {
    let Some(engine::ecs::EventSignal::DragMove { delta_world, .. }) = env.event.as_ref() else {
        return;
    };
    let delta = *delta_world;

    // Walk up from the hit renderable to find the TransformComponent to move.
    let mut cur = Some(env.scope);
    while let Some(id) = cur {
        if world
            .get_component_by_id_as::<engine::ecs::component::TransformComponent>(id)
            .is_some()
        {
            let [x, y, z] = world
                .get_component_by_id_as::<engine::ecs::component::TransformComponent>(id)
                .map(|t| t.transform.translation)
                .unwrap_or([0.0; 3]);

            emit.push_intent_now(
                id,
                engine::ecs::IntentValue::SetPosition {
                    component_ids: vec![id],
                    position: [x + delta[0], y + delta[1], z + delta[2]],
                },
            );
            return;
        }
        cur = world.parent_of(id);
    }
}

fn spawn_drag_cube(
    universe: &mut engine::Universe,
    parent: engine::ecs::ComponentId,
    pos: [f32; 3],
    color: [f32; 4],
) -> engine::ecs::ComponentId {
    use engine::ecs::component::{
        ColorComponent, RaycastableComponent, RenderableComponent, TransformComponent,
    };

    let root = universe.world.add_component(
        TransformComponent::new()
            .with_position(pos[0], pos[1], pos[2])
            .with_scale(0.45, 0.45, 0.45),
    );
    let color_node = universe
        .world
        .add_component(ColorComponent::rgba(color[0], color[1], color[2], color[3]));
    let r = universe.world.add_component(RenderableComponent::cube());
    let rc = universe
        .world
        .add_component(RaycastableComponent::enabled());

    let _ = universe.attach(root, color_node);
    let _ = universe.attach(color_node, r);
    let _ = universe.attach(r, rc);
    let _ = universe.attach(parent, root);

    universe.add_signal_handler(engine::ecs::SignalKind::DragMove, r, on_drag_move);

    root
}

// ---------------------------------------------------------------------------
// "Both" cube — drag to move, click to cycle color
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct BothCubeState {
    color_node: engine::ecs::ComponentId,
    click_count: u32,
}

static BOTH_CUBES: OnceLock<
    Mutex<std::collections::HashMap<engine::ecs::ComponentId, BothCubeState>>,
> = OnceLock::new();

fn on_both_cube(
    world: &mut engine::ecs::World,
    emit: &mut dyn engine::ecs::SignalEmitter,
    env: &engine::ecs::Signal,
) {
    match env.event.as_ref() {
        Some(engine::ecs::EventSignal::DragMove { delta_world, .. }) => {
            on_drag_move(world, emit, env);
            let _ = delta_world;
        }
        Some(engine::ecs::EventSignal::Click { .. }) => {
            let map = BOTH_CUBES.get_or_init(|| Mutex::new(Default::default()));
            let mut guard = map.lock().unwrap();
            // scope is the renderable; walk up to find the cube root registered in the map
            let mut cur = Some(env.scope);
            let mut found_root = None;
            while let Some(id) = cur {
                if guard.contains_key(&id) {
                    found_root = Some(id);
                    break;
                }
                cur = world.parent_of(id);
            }
            let Some(root) = found_root else { return };
            let Some(state) = guard.get_mut(&root) else {
                return;
            };

            state.click_count += 1;
            let colors = click_cube_colors();
            let rgba = colors[state.click_count as usize % colors.len()];
            emit.push_intent_now(
                root,
                engine::ecs::IntentValue::SetColor {
                    component_ids: vec![state.color_node],
                    rgba,
                },
            );
        }
        _ => {}
    }
}

fn spawn_both_cube(
    universe: &mut engine::Universe,
    parent: engine::ecs::ComponentId,
    pos: [f32; 3],
) -> engine::ecs::ComponentId {
    use engine::ecs::component::{
        ColorComponent, RaycastableComponent, RenderableComponent, TransformComponent,
    };

    let root = universe.world.add_component(
        TransformComponent::new()
            .with_position(pos[0], pos[1], pos[2])
            .with_scale(0.45, 0.45, 0.45),
    );
    let color_node = universe
        .world
        .add_component(ColorComponent::rgba(0.95, 0.60, 0.20, 1.0));
    let r = universe.world.add_component(RenderableComponent::cube());
    let rc = universe
        .world
        .add_component(RaycastableComponent::enabled());

    let _ = universe.attach(root, color_node);
    let _ = universe.attach(color_node, r);
    let _ = universe.attach(r, rc);
    let _ = universe.attach(parent, root);

    BOTH_CUBES
        .get_or_init(|| Mutex::new(Default::default()))
        .lock()
        .unwrap()
        .insert(
            root,
            BothCubeState {
                color_node,
                click_count: 0,
            },
        );

    universe.add_signal_handler(engine::ecs::SignalKind::DragMove, r, on_both_cube);
    universe.add_signal_handler(engine::ecs::SignalKind::Click, r, on_both_cube);

    root
}

// ---------------------------------------------------------------------------
// Label helper
// ---------------------------------------------------------------------------

fn spawn_label(
    universe: &mut engine::Universe,
    parent: engine::ecs::ComponentId,
    pos: [f32; 3],
    text: &str,
) {
    use engine::ecs::component::{
        ColorComponent, EmissiveComponent, TextComponent, TextureFilteringComponent,
        TransformComponent, TransparentCutoutComponent,
    };

    let root = universe.world.add_component(
        TransformComponent::new()
            .with_position(pos[0], pos[1], pos[2])
            .with_scale(0.055, 0.055, 1.0),
    );
    let txt = universe.world.add_component(TextComponent::new(text));
    let color = universe
        .world
        .add_component(ColorComponent::rgba(0.05, 0.05, 0.08, 1.0));
    let emissive = universe.world.add_component(EmissiveComponent::on());
    let filtering = universe
        .world
        .add_component(TextureFilteringComponent::nearest_magnification());
    let cutout = universe
        .world
        .add_component(TransparentCutoutComponent::new());

    let _ = universe.attach(parent, root);
    let _ = universe.attach(root, txt);
    let _ = universe.attach(txt, color);
    let _ = universe.attach(txt, emissive);
    let _ = universe.attach(txt, filtering);
    let _ = universe.attach(txt, cutout);
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    use engine::ecs::component::{
        AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, ColorComponent,
        DirectionalLightComponent, InputComponent, InputTransformModeComponent, PointerComponent,
        TransformComponent,
    };

    // Background.
    let bg = universe
        .world
        .add_component(BackgroundColorComponent::new());
    let bg_c = universe
        .world
        .add_component(ColorComponent::rgba(0.12, 0.12, 0.16, 1.0));
    let _ = universe.world.add_child(bg, bg_c);
    universe.add(bg);

    // Lighting.
    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.30, 0.30, 0.32));
    universe.add(ambient);

    let sun_t = universe
        .world
        .add_component(TransformComponent::new().with_position(2.0, 4.0, 3.0));
    let sun = universe
        .world
        .add_component(DirectionalLightComponent::new());
    let _ = universe.attach(sun_t, sun);
    universe.add(sun_t);

    // Camera + pointer rig.
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(3.0));
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    let cam_t = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.5, 4.5));
    let cam = universe
        .world
        .add_component(Camera3DComponent::new().with_fov(70.0));
    let pointer = universe.world.add_component(PointerComponent::new());

    let _ = universe.attach(input, cam_t);
    let _ = universe.attach(cam_t, cam);
    let _ = universe.attach(cam_t, pointer);

    example_util::spawn_desktop_camera_controls_hint(&mut universe, cam_t);
    universe.add(input);

    // Scene root for all interactive objects.
    let scene = universe.world.add_component(TransformComponent::new());
    universe.add(scene);

    // --- LEFT: click-only cubes ---
    let click_group = universe
        .world
        .add_component(TransformComponent::new().with_position(-2.2, 0.0, 0.0));
    let _ = universe.attach(scene, click_group);

    for (i, dy) in [-0.55_f32, 0.0, 0.55].iter().enumerate() {
        spawn_click_cube(&mut universe, click_group, [0.0, *dy, 0.0]);
        let _ = i;
    }
    spawn_label(
        &mut universe,
        click_group,
        [-0.25, -1.05, 0.0],
        "click\ncycles color",
    );

    // --- MID: drag-only cube ---
    let drag_cube_root = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 0.0));
    let _ = universe.attach(scene, drag_cube_root);

    spawn_drag_cube(
        &mut universe,
        drag_cube_root,
        [0.0, 0.0, 0.0],
        [0.35, 0.85, 0.55, 1.0],
    );
    spawn_label(
        &mut universe,
        drag_cube_root,
        [-0.25, -0.80, 0.0],
        "drag\nto move",
    );

    // --- RIGHT: drag + click cube ---
    let both_root = universe
        .world
        .add_component(TransformComponent::new().with_position(2.2, 0.0, 0.0));
    let _ = universe.attach(scene, both_root);

    spawn_both_cube(&mut universe, both_root, [0.0, 0.0, 0.0]);
    spawn_label(
        &mut universe,
        both_root,
        [-0.30, -0.80, 0.0],
        "drag to move\nclick for color",
    );

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
