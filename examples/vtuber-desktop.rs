use cat_engine::engine::ecs::component::{
    AmbientLightComponent, AvatarControlComponent, BackgroundColorComponent, Camera3DComponent,
    ColorComponent, DirectionalLightComponent, EditorComponent, EmissiveComponent, GLTFComponent,
    InputComponent, InputTransformModeComponent, PointerComponent, RayCastComponent,
    RaycastableComponent, RenderableComponent, RendererSettingsComponent, TransformComponent,
};
use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

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

    example_util::spawn_desktop_camera_controls_hint(&mut universe, camera_rig);
    universe.add(camera_rig);

    // --- VTuber avatar — single-input topology ---
    //
    // InputComponent (fps_rotation, forward_z) drives both body translation and head rotation.
    // AvatarControlSystem:
    //   - Strips rotation from model_root (body faces body_yaw, not head yaw).
    //   - Splices a TransformComponent under J_Bip_C_Neck's parent to drive head rotation.
    //   - Smoothly rotates body to follow head when yaw delta exceeds threshold.
    //
    // Topology:
    //   editor_root
    //     └── body_input (InputComponent)
    //           └── driven_t (TransformComponent)
    //                 └── AvatarControlComponent
    //                       └── model_root (TransformComponent, Y offset)
    //                             └── GLTFComponent
    const AVATAR_HEIGHT_M: f32 = 1.6;

    let body_input = universe
        .world
        .add_component(InputComponent::new().with_speed(1.5));
    let body_input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(body_input, body_input_mode);

    let driven_t = universe.world.add_component(TransformComponent::new());
    let _ = universe.attach(body_input, driven_t);

    let avatar_control = universe.world.add_component(
        AvatarControlComponent::new()
            .with_head_bone("J_Bip_C_Neck")
            .with_forward_plus_z()
            .with_initial_yaw(0.0)
            //.with_body_pipeline_disabled(),
    );
    let _ = universe.attach(driven_t, avatar_control);

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
    let _ = universe.attach(avatar_control, model_root);
    let _ = universe.attach(model_root, model);

    // EditorComponent is the scene root containing the whole avatar pipeline.
    let editor_root = universe.world.add_component(EditorComponent::new());
    let _ = universe.attach(editor_root, body_input);
    universe.add(editor_root);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Force GLTF spawn so bones are available for marker placement.
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

    // Small colored bone markers for inspection.
    let marker_joints: &[(&str, (f32, f32, f32, f32))] = &[
        ("[name='J_Bip_C_Head']",      (0.85, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_C_Neck']",      (0.20, 0.85, 0.85, 0.9)),
        ("[name='J_Bip_C_UpperChest']",(0.20, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_L_UpperArm']",  (0.85, 0.85, 0.20, 0.9)),
        ("[name='J_Bip_R_UpperArm']",  (0.85, 0.60, 0.20, 0.9)),
    ];
    for &(selector, color) in marker_joints {
        let Some(bone) = universe.find_component(model_root, selector) else {
            continue;
        };
        let marker_t = universe.world.add_component(
            TransformComponent::new().with_scale(0.025, 0.025, 0.025),
        );
        let marker_r = universe.world.add_component(RenderableComponent::cube());
        let marker_c = universe
            .world
            .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));
        let marker_rcast = universe.world.add_component(RaycastableComponent::enabled());
        let _ = universe.world.add_child(marker_r, marker_c);
        let _ = universe.world.add_child(marker_r, marker_rcast);
        let _ = universe.world.add_child(marker_t, marker_r);
        let _ = universe.attach(bone, marker_t);
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
