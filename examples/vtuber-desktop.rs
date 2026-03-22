use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, ColorComponent,
    DirectionalLightComponent, EditorComponent, EmissiveComponent, GLTFComponent,
    InputComponent, InputTransformModeComponent, PointerComponent, RayCastComponent,
    RaycastableComponent, RenderableComponent, RendererSettingsComponent,
    TransformComponent, TransformDropComponent, TransformForkTRSComponent,
    TransformMapRotationComponent, TransformMapScaleComponent, TransformMapTranslationComponent,
    TransformMergeTRSComponent, TransformPipelineComponent, TransformPipelineOutputComponent,
    TransformSampleAncestorComponent,
};
use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

/// Splices a head-rotation InputComponent pipeline between a bone and its parent.
///
/// The splice inserts:
///   parent_of(head_bone)
///     └── Input (fps, Q/E = head yaw)
///           └── T (driven by InputSystem)
///                 └── TransformPipeline
///                       ├── TransformForkTRS
///                       │     ├── TransformMapTranslation
///                       │     │     └── TransformSampleAncestor (skip=1 → parent world pos)
///                       │     ├── TransformMapRotation
///                       │     ├── TransformMapScale
///                       │     └── TransformMergeTRS
///                       └── TransformPipelineOutput
///                             └── head_bone  (displaced from original parent)
fn attach_head_rotation_splice(
    universe: &mut engine::Universe,
    avatar_root: engine::ecs::ComponentId,
    selector: &str,
) -> Result<engine::ecs::ComponentId, String> {
    let head = universe
        .find_component(avatar_root, selector)
        .ok_or_else(|| format!("head selector did not match: {selector}"))?;
    let neck = universe
        .parent_of(head)
        .ok_or_else(|| format!("matched head bone has no parent: {selector}"))?;

    // Q/E rotate the head around Y (yaw). Speed 0 disables WASD translation on this input
    // so the pipeline's SampleAncestor always provides the correct head world position.
    let head_input = universe
        .world
        .add_component(InputComponent::new().with_speed(0.0));
    let head_input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(head_input, head_input_mode);

    let driven_t = universe.world.add_component(TransformComponent::new());
    let pipeline = universe.world.add_component(TransformPipelineComponent::new());
    let fork = universe.world.add_component(TransformForkTRSComponent::new());
    let map_translation = universe
        .world
        .add_component(TransformMapTranslationComponent::new());
    // skip=1: pipeline owner walks up → driven_T (skip=0) → neck bone (skip=1)
    let sample_ancestor = universe
        .world
        .add_component(TransformSampleAncestorComponent::new());
    let map_rotation = universe.world.add_component(TransformMapRotationComponent::new());
    let map_scale = universe.world.add_component(TransformMapScaleComponent::new());
    let merge = universe.world.add_component(TransformMergeTRSComponent::new());
    let output = universe
        .world
        .add_component(TransformPipelineOutputComponent::new());

    let _ = universe.attach(neck, head_input);
    let _ = universe.attach(head_input, driven_t);
    let _ = universe.attach(driven_t, pipeline);
    let _ = universe.attach(pipeline, fork);
    let _ = universe.attach(fork, map_translation);
    let _ = universe.attach(map_translation, sample_ancestor);
    let _ = universe.attach(fork, map_rotation);
    let _ = universe.attach(fork, map_scale);
    let _ = universe.attach(fork, merge);
    let _ = universe.attach(pipeline, output);
    let _ = universe.attach(output, head);

    Ok(head_input)
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let renderer_settings = universe
        .world
        .add_component(RendererSettingsComponent::msaa_off().with_window_size(1280, 720));
    universe.add(renderer_settings);

    let background = universe
        .world
        .add_component(BackgroundColorComponent::rgba(0.62, 0.80, 1.00, 1.0));
    universe.add(background);

    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.18, 0.18, 0.22));
    universe.add(ambient);

    let sun = universe.world.add_component(
        DirectionalLightComponent::new()
            .with_intensity(1.1)
            .with_color(1.0, 0.98, 0.95),
    );
    let sun_dir = universe
        .world
        .add_component(TransformComponent::new().with_position(0.15, -0.45, 1.0));
    let _ = universe.attach(sun_dir, sun);
    universe.add(sun_dir);

    // --- Viewer camera (separate from avatar) ---
    // Positioned behind and above the avatar so the streaming audience sees the model.
    let camera_input = universe
        .world
        .add_component(InputComponent::new().with_speed(2.0));
    let camera_input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(camera_input, camera_input_mode);

    let camera_rig = universe.world.add_component(
        TransformComponent::new().with_position(0.0, 1.2, 3.0),
    );
    let _ = universe.attach(camera_input, camera_rig);

    let camera3d = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(camera_rig, camera3d);

    // Raycaster + pointer for desktop bone-picking / gizmo interaction.
    let raycaster = universe
        .world
        .add_component(RayCastComponent::event_driven().with_max_distance(100.0));
    let _ = universe.attach(camera_rig, raycaster);
    let pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(raycaster, pointer);

    example_util::spawn_desktop_camera_controls_hint(&mut universe, camera_rig);
    universe.add(camera_input);

    // --- VTuber avatar ---
    // Body InputComponent drives the avatar's position via a translation-only pipeline.
    // Rotation is stripped so the avatar never tilts with the camera view.
    // Q/E are bound to roll_axis_y here, but rotation is dropped downstream, so they have
    // no effect on the body — only on the head splice below.
    const AVATAR_HEIGHT_M: f32 = 1.6;

    let body_input = universe
        .world
        .add_component(InputComponent::new().with_speed(1.5));
    let body_input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z().with_fps_rotation(),
    );
    let _ = universe.attach(body_input, body_input_mode);

    let body_driven_t = universe.world.add_component(TransformComponent::new());

    // Translation-only pipeline: strips rotation from the body input.
    let av_pipeline = universe.world.add_component(TransformPipelineComponent::new());
    let av_fork = universe.world.add_component(TransformForkTRSComponent::new());
    let av_map_t = universe.world.add_component(TransformMapTranslationComponent::new());
    let av_map_r = universe.world.add_component(TransformMapRotationComponent::new());
    let av_drop_r = universe.world.add_component(TransformDropComponent::new());
    let av_map_s = universe.world.add_component(TransformMapScaleComponent::new());
    let av_merge = universe.world.add_component(TransformMergeTRSComponent::new());
    let av_output = universe.world.add_component(TransformPipelineOutputComponent::new());

    let _ = universe.attach(body_input, body_driven_t);
    let _ = universe.attach(body_driven_t, av_pipeline);
    let _ = universe.attach(av_pipeline, av_fork);
    let _ = universe.attach(av_fork, av_map_t);
    let _ = universe.attach(av_fork, av_map_r);
    let _ = universe.attach(av_map_r, av_drop_r);
    let _ = universe.attach(av_fork, av_map_s);
    let _ = universe.attach(av_fork, av_merge);
    let _ = universe.attach(av_pipeline, av_output);

    // EditorComponent makes the avatar subtree selectable + gizmo-interactive.
    let editor_root = universe.world.add_component(EditorComponent::new());
    let _ = universe.attach(av_output, editor_root);

    // model_root sits at -AVATAR_HEIGHT_M so the avatar stands at floor level
    // (the body pipeline carries only translation, model_root local Y offsets below that).
    let model_root = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, -AVATAR_HEIGHT_M, 0.0)
            .with_scale(1.0, 1.0, 1.0),
    );
    let model = universe
        .world
        .add_component(GLTFComponent::new("assets/models/pc-rei.hoodie.glb"));
    let emissive = universe.world.add_component(EmissiveComponent::on());
    let _ = universe.attach(model, emissive);

    let _ = universe.attach(editor_root, model_root);
    let _ = universe.attach(model_root, model);

    universe.add(body_input);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Force GLTF spawn so we can query bone ComponentIds.
    {
        let systems = &mut universe.systems;
        systems.gltf.tick_with_queue(
            &mut universe.world,
            &mut universe.visuals,
            &mut systems.skinned_mesh,
            &mut universe.command_queue,
            0.0,
        );
    }
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Add small colored bone markers (raycastable cubes) at key joints so they can be
    // inspected or selected via the editor/raycaster. Markers are tiny (scale 0.025)
    // to not obstruct the avatar visuals.
    let marker_joints: &[(&str, (f32, f32, f32, f32))] = &[
        ("[name='J_Bip_C_Head']",       (0.85, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_C_Neck']",        (0.20, 0.85, 0.85, 0.9)),
        ("[name='J_Bip_C_UpperChest']",  (0.20, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_L_UpperArm']",    (0.85, 0.85, 0.20, 0.9)),
        ("[name='J_Bip_R_UpperArm']",    (0.85, 0.60, 0.20, 0.9)),
    ];
    for &(selector, color) in marker_joints {
        let Some(bone) = universe.find_component(model_root, selector) else {
            continue;
        };
        let marker_t = universe.world.add_component(
            TransformComponent::new().with_scale(0.025, 0.025, 0.025),
        );
        let marker_r = universe
            .world
            .add_component(RenderableComponent::cube());
        let marker_c = universe
            .world
            .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));
        let marker_rcast = universe
            .world
            .add_component(RaycastableComponent::enabled());
        let _ = universe.world.add_child(marker_r, marker_c);
        let _ = universe.world.add_child(marker_r, marker_rcast);
        let _ = universe.world.add_child(marker_t, marker_r);
        let _ = universe.attach(bone, marker_t);
    }

    // Splice head rotation: Q/E drives head bone yaw.
    let head_selector = "[name='J_Bip_C_Head']";
    match attach_head_rotation_splice(&mut universe, model_root, head_selector) {
        Ok(input_id) => println!(
            "[vtuber-desktop] head rotation splice done: Input {:?} drives '{}'",
            input_id, head_selector,
        ),
        Err(e) => eprintln!("[vtuber-desktop] head rotation splice failed: {e}"),
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
