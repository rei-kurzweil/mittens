/// Minimal vtuber desktop example: static avatar, neck splice only.
///
/// Purpose: isolate neck/head rotation behaviour without body movement pipeline
/// or AvatarBodyYawComponent. Right-click drag = head yaw+pitch. Q/E = head yaw.
///
/// Topology:
/// ```
/// editor_root
///   └── model_root  (TransformComponent, static at origin)
///         └── GLTFComponent
///               └── [armature]
///                     J_Bip_C_Neck's parent
///                       └── head_input  (InputComponent, fps_rotation, roll_axis_y)
///                             └── driven_t
///                                   └── pipeline  (SampleAncestor skip=1 → neck parent pos)
///                                         └── output
///                                               └── yaw_correction  (rotation_euler Y=π)
///                                                     └── J_Bip_C_Neck  ← displaced
///                                                           └── J_Bip_C_Head
/// ```
use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, ColorComponent,
    DirectionalLightComponent, EditorComponent, EmissiveComponent, GLTFComponent,
    InputComponent, InputTransformModeComponent, PointerComponent, RayCastComponent,
    RaycastableComponent, RenderableComponent, RendererSettingsComponent,
    TransformComponent, TransformForkTRSComponent,
    TransformMapRotationComponent, TransformMapScaleComponent, TransformMapTranslationComponent,
    TransformMergeTRSComponent, TransformPipelineComponent, TransformPipelineOutputComponent,
    TransformSampleAncestorComponent,
};
use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

/// Splice head-rotation input under neck's parent, displacing J_Bip_C_Neck.
/// Matches vr-input topology so neck + head rotate as a unit.
fn attach_neck_rotation_splice(
    universe: &mut engine::Universe,
    avatar_root: engine::ecs::ComponentId,
    neck_selector: &str,
) -> Result<engine::ecs::ComponentId, String> {
    let neck = universe
        .find_component(avatar_root, neck_selector)
        .ok_or_else(|| format!("neck selector did not match: {neck_selector}"))?;
    let neck_parent = universe
        .parent_of(neck)
        .ok_or_else(|| format!("neck bone has no parent: {neck_selector}"))?;

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
    // skip=1: from pipeline → driven_t (0) → neck_parent (1)
    let sample_ancestor = universe
        .world
        .add_component(TransformSampleAncestorComponent::new());
    let map_rotation = universe.world.add_component(TransformMapRotationComponent::new());
    let map_scale = universe.world.add_component(TransformMapScaleComponent::new());
    let merge = universe.world.add_component(TransformMergeTRSComponent::new());
    let output = universe
        .world
        .add_component(TransformPipelineOutputComponent::new());
    // π Y rotation: HMD/keyboard identity (-Z forward) → bone natural facing.
    let yaw_correction = universe.world.add_component(
        TransformComponent::new().with_rotation_euler(0.0, std::f32::consts::PI, 0.0),
    );

    let _ = universe.attach(neck_parent, head_input);
    let _ = universe.attach(head_input, driven_t);
    let _ = universe.attach(driven_t, pipeline);
    let _ = universe.attach(pipeline, fork);
    let _ = universe.attach(fork, map_translation);
    let _ = universe.attach(map_translation, sample_ancestor);
    let _ = universe.attach(fork, map_rotation);
    let _ = universe.attach(fork, map_scale);
    let _ = universe.attach(fork, merge);
    let _ = universe.attach(pipeline, output);
    let _ = universe.attach(output, yaw_correction);
    let _ = universe.attach(yaw_correction, neck);

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

    // Static camera — no input, just positioned to see the avatar.
    let camera_rig = universe.world.add_component(
        TransformComponent::new().with_position(0.0, 1.2, 3.0),
    );
    let camera3d = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(camera_rig, camera3d);

    let raycaster = universe
        .world
        .add_component(RayCastComponent::event_driven().with_max_distance(100.0));
    let _ = universe.attach(camera_rig, raycaster);
    let pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(raycaster, pointer);

    universe.add(camera_rig);

    // --- Avatar (static position, neck splice only) ---
    const AVATAR_HEIGHT_M: f32 = 1.6;

    let editor_root = universe.world.add_component(
        EditorComponent::new().with_panels(false),
    );

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
    universe.add(editor_root);

    // Force GLTF spawn so we can find bone ComponentIds.
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );
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

    // Small bone markers for inspection.
    let marker_joints: &[(&str, (f32, f32, f32, f32))] = &[
        ("[name='J_Bip_C_Head']",      (0.85, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_C_Neck']",       (0.20, 0.85, 0.85, 0.9)),
        ("[name='J_Bip_C_UpperChest']", (0.20, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_C_Spine']",      (0.20, 0.20, 0.85, 0.6)),
    ];
    for &(selector, color) in marker_joints {
        let Some(bone) = universe.find_component(model_root, selector) else {
            continue;
        };
        let marker_t = universe
            .world
            .add_component(TransformComponent::new().with_scale(0.025, 0.025, 0.025));
        let marker_r = universe.world.add_component(RenderableComponent::cube());
        let marker_c = universe
            .world
            .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));
        let marker_rc = universe.world.add_component(RaycastableComponent::enabled());
        let _ = universe.world.add_child(marker_r, marker_c);
        let _ = universe.world.add_child(marker_r, marker_rc);
        let _ = universe.world.add_child(marker_t, marker_r);
        let _ = universe.attach(bone, marker_t);
    }

    // Splice neck rotation: right-drag = yaw+pitch, Q/E = yaw.
    match attach_neck_rotation_splice(&mut universe, model_root, "[name='J_Bip_C_Neck']") {
        Ok(_) => println!("[vtuber-desktop-simple] neck rotation splice attached"),
        Err(e) => eprintln!("[vtuber-desktop-simple] splice failed: {e}"),
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
