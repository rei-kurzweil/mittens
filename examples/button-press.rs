use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy)]
struct ButtonBorder {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl ButtonBorder {
    fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    fn uniform(v: f32) -> Self {
        Self {
            left: v,
            top: v,
            right: v,
            bottom: v,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ButtonState {
    pressed: bool,

    cap_root: engine::ecs::ComponentId,
    cap_raised_z: f32,
    cap_pressed_z: f32,

    frame_color: engine::ecs::ComponentId,
    face_color: engine::ecs::ComponentId,

    text_id: engine::ecs::ComponentId,
    text_color: engine::ecs::ComponentId,

    frame_color_up: [f32; 4],
    face_color_up: [f32; 4],
    text_color_up: [f32; 4],

    frame_color_down: [f32; 4],
    face_color_down: [f32; 4],
    text_color_down: [f32; 4],
}

static BUTTONS: OnceLock<Mutex<HashMap<engine::ecs::ComponentId, ButtonState>>> = OnceLock::new();

fn darken([r, g, b, a]: [f32; 4], factor: f32) -> [f32; 4] {
    [r * factor, g * factor, b * factor, a]
}

fn set_button_pressed(
    world: &mut engine::ecs::World,
    emit: &mut dyn engine::ecs::SignalEmitter,
    scope: engine::ecs::ComponentId,
    pressed: bool,
) {
    let buttons = BUTTONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = buttons.lock().expect("button state lock");

    // Signals are scoped to the hit renderable; handlers are invoked for all ancestors.
    // Resolve the button root by walking up the topology chain until we find a registered button.
    let mut cursor = Some(scope);
    let mut button_root = None;
    while let Some(node) = cursor {
        if guard.contains_key(&node) {
            button_root = Some(node);
            break;
        }
        cursor = world.parent_of(node);
    }
    let Some(button_root) = button_root else {
        return;
    };

    let Some(state) = guard.get_mut(&button_root) else {
        return;
    };

    if state.pressed == pressed {
        return;
    }
    state.pressed = pressed;

    let (frame_rgba, face_rgba, text_rgba, cap_z) = if pressed {
        (
            state.frame_color_down,
            state.face_color_down,
            state.text_color_down,
            state.cap_pressed_z,
        )
    } else {
        (
            state.frame_color_up,
            state.face_color_up,
            state.text_color_up,
            state.cap_raised_z,
        )
    };

    emit.push_intent_now(
        button_root,
        engine::ecs::IntentValue::SetColor {
            component_ids: vec![state.frame_color],
            rgba: frame_rgba,
        },
    );
    emit.push_intent_now(
        button_root,
        engine::ecs::IntentValue::SetColor {
            component_ids: vec![state.face_color],
            rgba: face_rgba,
        },
    );

    // Swap the text color component node rather than mutating in-place.
    // This demonstrates late-attached style application for already-built text.
    emit.push_intent_now(
        button_root,
        engine::ecs::IntentValue::RemoveSubtree {
            component_ids: vec![state.text_color],
        },
    );
    let new_text_color =
        world.add_component(engine::ecs::component::ColorComponent { rgba: text_rgba });
    emit.push_intent_now(
        button_root,
        engine::ecs::IntentValue::Attach {
            parents: vec![state.text_id],
            child: new_text_color,
        },
    );
    state.text_color = new_text_color;

    // Move the cap flush into the frame (clamped to two fixed depths).
    // Use SetPosition so we don't accidentally stomp cap TRS.
    emit.push_intent_now(
        button_root,
        engine::ecs::IntentValue::SetPosition {
            component_ids: vec![state.cap_root],
            position: [0.0, 0.0, cap_z],
        },
    );
}

fn button_press_handler(
    world: &mut engine::ecs::World,
    emit: &mut dyn engine::ecs::SignalEmitter,
    env: &engine::ecs::Signal,
) {
    match env.event.as_ref() {
        Some(engine::ecs::EventSignal::DragStart { .. }) => {
            set_button_pressed(world, emit, env.scope, true);
        }
        Some(engine::ecs::EventSignal::DragEnd { .. }) => {
            set_button_pressed(world, emit, env.scope, false);
        }
        _ => {}
    }
}

fn spawn_raycastable_cube(
    universe: &mut engine::Universe,
    parent: engine::ecs::ComponentId,
    pos: [f32; 3],
    scale: [f32; 3],
) -> engine::ecs::ComponentId {
    use engine::ecs::component::{RaycastableComponent, RenderableComponent, TransformComponent};

    let t = universe.world.add_component(
        TransformComponent::new()
            .with_position(pos[0], pos[1], pos[2])
            .with_scale(scale[0], scale[1], scale[2]),
    );
    let r = universe.world.add_component(RenderableComponent::cube());
    let rc = universe
        .world
        .add_component(RaycastableComponent::enabled());

    let _ = universe.attach(parent, t);
    let _ = universe.attach(t, r);
    let _ = universe.attach(r, rc);

    r
}

fn spawn_raycastable_colored_cube(
    universe: &mut engine::Universe,
    parent: engine::ecs::ComponentId,
    pos: [f32; 3],
    scale: [f32; 3],
    rgba: [f32; 4],
) -> engine::ecs::ComponentId {
    use engine::ecs::component::ColorComponent;

    let r = spawn_raycastable_cube(universe, parent, pos, scale);
    let c = universe
        .world
        .add_component(ColorComponent::rgba(rgba[0], rgba[1], rgba[2], rgba[3]));
    let _ = universe.attach(r, c);
    r
}

fn wrap_at_for_button_middle_width(middle_w: f32, text_scale: f32) -> usize {
    // TextSystem layout: each glyph advances by 1.0 in local X.
    // With `text_root` scaled by `text_scale`, each glyph is ~`text_scale` world units wide.
    // Reserve some horizontal padding so letters don't collide with the face border.
    let padding_world = (middle_w * 0.10).clamp(0.02, 0.25);
    let usable_world = (middle_w - padding_world).max(0.05);
    let glyph_w_world = text_scale.abs().max(1e-4);

    let chars_per_line = (usable_world / glyph_w_world).floor() as isize;
    (chars_per_line.max(1) as usize).min(200)
}

fn measure_word_wrapped_block(text: &str, wrap_at: usize) -> (usize, usize) {
    // Approximate the block size (max columns, rows) for word-wrap-at-space behavior.
    // This is used only for centering the text root visually.
    if text.is_empty() {
        return (0, 0);
    }

    let wrap_at = wrap_at.max(1);

    let mut rows = 1usize;
    let mut col = 0usize;
    let mut max_col = 0usize;

    for (wi, word) in text.split_whitespace().enumerate() {
        let wlen = word.chars().count();
        if wlen == 0 {
            continue;
        }

        // If this isn't the first word on the line, account for a space.
        let needs_space = col > 0;
        let add = wlen + if needs_space { 1 } else { 0 };

        if col > 0 && col + add > wrap_at {
            // Wrap at the previous whitespace opportunity.
            max_col = max_col.max(col);
            rows += 1;
            col = 0;
        }

        // Place the word (and preceding space if any).
        col += add;

        // If a single word exceeds wrap_at, we don't break it (matching TextSystem word_wrap).
        max_col = max_col.max(col);

        // Preserve explicit newlines in the input.
        if wi == 0 {
            // no-op
        }
    }

    max_col = max_col.max(col);
    (max_col, rows)
}

fn spawn_button(
    universe: &mut engine::Universe,
    pos: [f32; 3],
    middle_wh: [f32; 2],
    border: ButtonBorder,
    text: &str,
    text_scale: f32,
    color: [f32; 4],
    background_color: [f32; 4],
) -> engine::ecs::ComponentId {
    use engine::ecs::component::{
        ColorComponent, TextComponent, TextureFilteringComponent, TransformComponent,
        TransparentCutoutComponent,
    };

    let button_root = universe
        .world
        .add_component(TransformComponent::new().with_position(pos[0], pos[1], pos[2]));

    // Frame: static border around the button.
    let frame_root = universe.world.add_component(TransformComponent::new());
    let frame_color = universe
        .world
        .add_component(ColorComponent::rgba(color[0], color[1], color[2], color[3]));
    let _ = universe.attach(button_root, frame_root);
    let _ = universe.attach(frame_root, frame_color);

    // Cap: pressable face + text.
    let cap_raised_z = 0.030;
    let cap_pressed_z = 0.000;
    let cap_root = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, cap_raised_z));
    let _ = universe.attach(button_root, cap_root);

    let face_color = universe.world.add_component(ColorComponent::rgba(
        background_color[0],
        background_color[1],
        background_color[2],
        background_color[3],
    ));
    let _ = universe.attach(cap_root, face_color);

    // Geometry.
    let depth = 0.025;
    let middle_w = middle_wh[0];
    let middle_h = middle_wh[1];

    // Border pieces.
    let full_w = middle_w + border.left + border.right;
    let full_h = middle_h + border.top + border.bottom;

    // Left / right.
    let _left = spawn_raycastable_cube(
        universe,
        frame_color,
        [-(middle_w * 0.5 + border.left * 0.5), 0.0, 0.0],
        [border.left, full_h, depth],
    );
    let _right = spawn_raycastable_cube(
        universe,
        frame_color,
        [(middle_w * 0.5 + border.right * 0.5), 0.0, 0.0],
        [border.right, full_h, depth],
    );

    // Top / bottom.
    let _top = spawn_raycastable_cube(
        universe,
        frame_color,
        [0.0, (middle_h * 0.5 + border.top * 0.5), 0.0],
        [full_w, border.top, depth],
    );
    let _bottom = spawn_raycastable_cube(
        universe,
        frame_color,
        [0.0, -(middle_h * 0.5 + border.bottom * 0.5), 0.0],
        [full_w, border.bottom, depth],
    );

    // Face (pressable center).
    let _face = spawn_raycastable_cube(
        universe,
        face_color,
        [0.0, 0.0, 0.0],
        [middle_w, middle_h, depth],
    );

    // Text. (Heuristic centering; TextComponent is top-left anchored today.)
    let text_root = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.0, depth * 0.5 + 0.006)
            .with_scale(text_scale, text_scale, 1.0),
    );
    let _ = universe.attach(cap_root, text_root);

    let wrap_at = wrap_at_for_button_middle_width(middle_w, text_scale);
    let (cols, rows) = measure_word_wrapped_block(text, wrap_at);
    let cols_f = cols.max(1) as f32;
    let rows_f = rows.max(1) as f32;

    // Center the text block around (0,0) in glyph-local units.
    // X is left-to-right (0..cols-1), Y is down (0, -1, -2...).
    let x_center = -0.5 * (cols_f - 1.0);
    let y_center = 0.5 * (rows_f - 1.0);
    let text_offset = universe
        .world
        .add_component(TransformComponent::new().with_position(x_center, y_center, 0.0));
    let _ = universe.attach(text_root, text_offset);

    let text_id = universe
        .world
        .add_component(TextComponent::with_word_wrap(text, wrap_at));
    let _ = universe.attach(text_offset, text_id);

    let cutout = universe
        .world
        .add_component(TransparentCutoutComponent::new());
    let _ = universe.attach(text_id, cutout);

    let filtering = universe
        .world
        .add_component(TextureFilteringComponent::nearest_magnification());
    let _ = universe.attach(text_id, filtering);

    universe.add(button_root);

    // Attach text color *after* text has been built/initialized.
    // We intentionally initialize the ColorComponent before attachment so that its `init()` will
    // NOT re-run on attach; this exercises the TextSystem ParentChanged refresh behavior.
    let text_color = universe
        .world
        .add_component(ColorComponent::rgba(0.0, 0.0, 0.0, 1.0));
    universe.add(text_color);
    let _ = universe.attach(text_id, text_color);

    // Stash state for the signal handler.
    let state = ButtonState {
        pressed: false,
        cap_root,
        cap_raised_z,
        cap_pressed_z,
        frame_color,
        face_color,
        text_id,
        text_color,
        frame_color_up: color,
        face_color_up: background_color,
        text_color_up: [0.0, 0.0, 0.0, 1.0],
        frame_color_down: darken(color, 0.75),
        face_color_down: darken(background_color, 0.75),
        text_color_down: [1.0, 1.0, 1.0, 1.0],
    };

    BUTTONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("button state lock")
        .insert(button_root, state);

    button_root
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Minimal lit scene + pointer raycaster.
    use engine::ecs::component::{
        AmbientLightComponent, BackgroundColorComponent, Camera3DComponent,
        DirectionalLightComponent, InputComponent, InputTransformModeComponent, PointerComponent,
        RayCastComponent, TransformComponent,
    };

    let bg = universe.world.add_component(BackgroundColorComponent::new());
    let bg_c = universe.world.add_component(engine::ecs::component::ColorComponent::rgba(0.92, 0.92, 0.96, 1.0));
    let _ = universe.world.add_child(bg, bg_c);
    universe.add(bg);

    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.35, 0.35, 0.35));
    universe.add(ambient);

    let sun_t = universe
        .world
        .add_component(TransformComponent::new().with_position(1.0, 1.0, 1.0));
    let sun = universe
        .world
        .add_component(DirectionalLightComponent::new());
    let _ = universe.attach(sun_t, sun);
    universe.add(sun_t);

    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(2.5));
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    let rig_t = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 2.5));
    let _ = universe.attach(input, rig_t);

    let cam = universe
        .world
        .add_component(Camera3DComponent::new().with_far(600.0).with_fov(70.0));
    let _ = universe.attach(rig_t, cam);

    let raycaster = universe
        .world
        .add_component(RayCastComponent::event_driven().with_max_distance(100.0));
    let _ = universe.attach(rig_t, raycaster);

    let pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(raycaster, pointer);

    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_t);
    universe.add(input);

    // The button.
    let button_root = spawn_button(
        &mut universe,
        [0.0, 0.0, 0.0],
        [1.20, 0.45],
        ButtonBorder::new(0.025, 0.025, 0.025, 0.025),
        "click me",
        0.18,
        [0.15, 0.15, 0.20, 1.0],
        [0.85, 0.85, 0.92, 1.0],
    );

    // A small 3-cube stack to the left of the button for gizmo testing:
    //   [cyan]
    // [yellow][light brown]
    //
    // These live under an EditorComponent subtree so clicking attaches the editor gizmo.
    {
        use engine::ecs::component::{EditorComponent, TransformComponent};

        let editor_root = universe.world.add_component(EditorComponent::new());

        // Position the stack in world space (left of the button).
        let stack_root = universe
            .world
            .add_component(TransformComponent::new().with_position(-1.55, 0.0, 0.0));
        let _ = universe.attach(editor_root, stack_root);

        let cube = 0.22_f32;
        let gap = 0.04_f32;
        let step = cube + gap;
        let y_bottom = -0.5 * step;
        let y_top = 0.5 * step;
        let x_left = -0.5 * step;
        let x_right = 0.5 * step;

        let yellow = [1.0, 0.92, 0.22, 1.0];
        let light_brown = [0.80, 0.66, 0.46, 1.0];
        let cyan = [0.20, 0.95, 1.0, 1.0];

        let _bottom_left = spawn_raycastable_colored_cube(
            &mut universe,
            stack_root,
            [x_left, y_bottom, 0.0],
            [cube, cube, 0.06],
            yellow,
        );
        let _bottom_right = spawn_raycastable_colored_cube(
            &mut universe,
            stack_root,
            [x_right, y_bottom, 0.0],
            [cube, cube, 0.06],
            light_brown,
        );
        let _top = spawn_raycastable_colored_cube(
            &mut universe,
            stack_root,
            [0.0, y_top, 0.0],
            [cube, cube, 0.06],
            cyan,
        );

        universe.add(editor_root);
    }

    universe.add_signal_handler(
        engine::ecs::SignalKind::DragStart,
        button_root,
        button_press_handler,
    );
    universe.add_signal_handler(
        engine::ecs::SignalKind::DragEnd,
        button_root,
        button_press_handler,
    );

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
