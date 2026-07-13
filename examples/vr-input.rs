use cat_engine::engine::ecs::component::{
    AmbientLightComponent, AvatarControlComponent, BackgroundColorComponent, BloomComponent,
    BlurPassComponent, Camera3DComponent, CameraXRComponent, ColorComponent, ControllerHand,
    ControllerPoseKind, ControllerXRComponent, DirectionalLightComponent, EditorComponent,
    EmissiveComponent, EmissivePassComponent, GLTFComponent, InputComponent,
    InputTransformModeComponent, InputXRComponent, PointerComponent, QuatTemporalFilterComponent,
    RaycastableComponent, RenderGraphComponent, RenderableComponent, RendererSettingsComponent,
    RendererStatsComponent, TransformComponent, TransformForkTRSComponent,
    TransformMapRotationComponent, TransformMapScaleComponent, TransformMapTranslationComponent,
    XrComponent,
};
use cat_engine::engine::graphics::BuiltinMeshType;
use cat_engine::engine::graphics::CameraTarget;
use cat_engine::engine::graphics::primitives::{MaterialHandle, Renderable};
use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

#[derive(Debug, Clone, Copy)]
struct VrInputOptions {
    xr_controller_rotation_filter: bool,
}

impl Default for VrInputOptions {
    fn default() -> Self {
        Self {
            xr_controller_rotation_filter: true,
        }
    }
}

fn parse_options() -> Result<VrInputOptions, String> {
    let mut options = VrInputOptions::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--xr-controller-rotation-filter" => {
                options.xr_controller_rotation_filter = true;
            }
            "--no-xr-controller-rotation-filter" => {
                options.xr_controller_rotation_filter = false;
            }
            "--help" | "-h" => {
                return Err(
                    "usage: cargo run --example vr-input -- [--xr-controller-rotation-filter | --no-xr-controller-rotation-filter]"
                        .to_string(),
                );
            }
            other => {
                return Err(format!(
                    "unknown arg: {other}\nusage: cargo run --example vr-input -- [--xr-controller-rotation-filter | --no-xr-controller-rotation-filter]"
                ));
            }
        }
    }

    Ok(options)
}

fn spawn_sun_background(universe: &mut engine::Universe) {
    let bg_root = universe
        .world
        .add_component(engine::ecs::component::BackgroundComponent::new());
    universe.add(bg_root);

    let circle_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Circle2D);

    // Big yellow disk.
    let sun_t = universe.world.add_component(
        TransformComponent::new()
            .with_position(2.0, 1.5, -8.0)
            .with_scale(3.5, 3.5, 3.5),
    );
    let sun_r = universe
        .world
        .add_component(RenderableComponent::new(Renderable::new(
            circle_mesh,
            MaterialHandle::TOON_MESH,
        )));
    let sun_color = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 0.85, 0.15, 1.0));
    let sun_emissive = universe.world.add_component(EmissiveComponent::on());

    let _ = universe.attach(bg_root, sun_t);
    let _ = universe.attach(sun_t, sun_r);
    let _ = universe.attach(sun_r, sun_color);
    let _ = universe.attach(sun_r, sun_emissive);

    // Small white highlight disk.
    let highlight_t = universe.world.add_component(
        TransformComponent::new()
            .with_position(-0.35, 0.35, -0.01)
            .with_scale(0.45, 0.45, 0.45),
    );
    let highlight_r = universe
        .world
        .add_component(RenderableComponent::new(Renderable::new(
            circle_mesh,
            MaterialHandle::TOON_MESH,
        )));
    let highlight_color = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let highlight_emissive = universe.world.add_component(EmissiveComponent::on());

    let _ = universe.attach(sun_t, highlight_t);
    let _ = universe.attach(highlight_t, highlight_r);
    let _ = universe.attach(highlight_r, highlight_color);
    let _ = universe.attach(highlight_r, highlight_emissive);
}

fn spawn_controller_cube(
    universe: &mut engine::Universe,
    xr_rig: engine::ecs::ComponentId,
    hand: ControllerHand,
    color: (f32, f32, f32, f32),
    rotation_smoothing: f32,
    use_rotation_filter_pipeline: bool,
) -> engine::ecs::ComponentId {
    let controller_marker = universe.world.add_component(ControllerXRComponent::new(
        true,
        hand,
        ControllerPoseKind::Aim,
    ));
    let _ = universe.attach(xr_rig, controller_marker);

    // Transform driven by OpenXRSystem (TransformComponent child of ControllerXRComponent).
    let controller_t = universe
        .world
        .add_component(TransformComponent::new().with_scale(0.06, 0.06, 0.12));
    let _ = universe.attach(controller_marker, controller_t);

    if !use_rotation_filter_pipeline {
        let cube = universe.world.add_component(RenderableComponent::cube());
        let cube_color = universe
            .world
            .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));

        let _ = universe.attach(controller_t, cube);
        let _ = universe.attach(cube, cube_color);

        return controller_marker;
    }

    let fork = universe
        .world
        .add_component(TransformForkTRSComponent::new());
    let _ = universe.attach(controller_t, fork);

    let map_translation = universe
        .world
        .add_component(TransformMapTranslationComponent::new());
    let _ = universe.attach(fork, map_translation);

    let map_rotation = universe
        .world
        .add_component(TransformMapRotationComponent::new());
    let _ = universe.attach(fork, map_rotation);
    let rotation_filter = universe.world.add_component(
        QuatTemporalFilterComponent::new().with_smoothing_factor(rotation_smoothing),
    );
    let _ = universe.attach(map_rotation, rotation_filter);

    let map_scale = universe
        .world
        .add_component(TransformMapScaleComponent::new());
    let _ = universe.attach(fork, map_scale);

    let filtered_t = universe.world.add_component(TransformComponent::new());
    let _ = universe.attach(fork, filtered_t);

    let cube = universe.world.add_component(RenderableComponent::cube());
    let cube_color = universe
        .world
        .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));

    let _ = universe.attach(filtered_t, cube);
    let _ = universe.attach(cube, cube_color);

    controller_marker
}

fn main() {
    utils::logger::init();

    let options = match parse_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    println!(
        "[vr-input] xr controller rotation filter pipeline: {}",
        if options.xr_controller_rotation_filter {
            "enabled"
        } else {
            "disabled"
        }
    );

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let renderer_settings = universe
        .world
        .add_component(RendererSettingsComponent::msaa_off().with_window_size(320, 240));
    universe.add(renderer_settings);

    let render_graph = universe.world.add_component(RenderGraphComponent::new());
    let emissive_pass = universe.world.add_component(EmissivePassComponent::new());
    let blur_pass = universe.world.add_component(
        BlurPassComponent::new()
            .with_radius_ndc(0.06)
            .with_half_res(true),
    );
    let bloom = universe.world.add_component(
        BloomComponent::new()
            .with_intensity(0.95)
            .with_emissive_scale(1.2),
    );
    let _ = universe.attach(emissive_pass, blur_pass);
    let _ = universe.attach(render_graph, emissive_pass);
    let _ = universe.attach(render_graph, bloom);
    universe.add(render_graph);

    // Sky base.
    let background = universe
        .world
        .add_component(BackgroundColorComponent::new());
    let background_c = universe
        .world
        .add_component(ColorComponent::rgba(0.62, 0.80, 1.00, 1.0));
    let _ = universe.world.add_child(background, background_c);
    universe.add(background);

    // Lighting for the model.
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

    // --- Desktop camera rig (for debugging while in VR) ---
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(1.5));
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    let desktop_rig = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 1.2, 3.5));
    let _ = universe.attach(input, desktop_rig);

    let camera3d = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(desktop_rig, camera3d);

    let pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(camera3d, pointer);

    example_util::spawn_desktop_camera_controls_hint(&mut universe, desktop_rig);
    universe.add(input);

    // --- XR rig (Aim controller debug cubes only; camera has moved to AVC) ---
    let xr_input = universe.world.add_component(InputXRComponent::on());
    let xr_gamepad = universe.world.add_component(
        cat_engine::engine::ecs::component::InputXRGamepadComponent::new().speed(1.5),
    );
    let xr_rig = universe.world.add_component(TransformComponent::new());
    let _ = universe.attach(xr_input, xr_rig);
    let _ = universe.attach(xr_input, xr_gamepad);

    // renderer stats
    let renderer_stats = universe
        .world
        .add_component(RendererStatsComponent::new().with_camera_target(CameraTarget::Xr));
    let render_stats_rig = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 1.85, 0.6));
    let _ = universe.attach(render_stats_rig, renderer_stats);
    let _ = universe.attach(xr_rig, render_stats_rig);

    universe.add(xr_input);

    // Background "skybox" content.
    spawn_sun_background(&mut universe);

    // --- VTuber model — single-input topology ---
    //
    // InputXRComponent drives body translation and head rotation through AvatarControlSystem.
    // AvatarControlSystem:
    //   - Splices a TransformComponent under J_Bip_C_Head's parent (the neck) to drive
    //     head rotation directly. Rotating the head — not the neck — isolates the spine
    //     so the torso doesn't twist with HMD yaw.
    //   - Strips rotation from model_root (body faces body_yaw, not raw HMD yaw).
    //   - Bakes the π Y handedness correction into the head rotation math.
    //   - Smoothly rotates body to follow head when yaw delta exceeds threshold.
    //   - Measures J_Bip_C_Head local Y in the rest pose and sets model_root.y = -bone_local_y,
    //     so the head bone sits at driven_t world Y (= HMD height) with no hardcoded constant.
    //   - Re-parents CameraXRComponent under J_Bip_C_Head for first-person alignment.
    //
    // Topology (after AVC init):
    //   editor_root
    //     └── avatar_input_xr (InputXRComponent)
    //           └── avatar_driven_t (TransformComponent)
    //                 └── AvatarControlComponent
    //                       ├── body_pipeline → pipeline_output
    //                       │     └── model_root (y auto-calibrated from J_Bip_C_Head)
    //                       │           └── GLTFComponent → ... → J_Bip_C_Head
    //                       │                                           └── CameraXRComponent
    //                       ├── CTLXR(Left, Grip) → re-parented to lower_arm
    //                       └── CTLXR(Right, Grip) → re-parented to lower_arm

    let editor_root = universe.world.add_component(EditorComponent::new());

    let avatar_input_xr = universe.world.add_component(InputXRComponent::on());
    let avatar_xr_gamepad = universe.world.add_component(
        cat_engine::engine::ecs::component::InputXRGamepadComponent::new().speed(1.5),
    );
    let avatar_driven_t = universe.world.add_component(TransformComponent::new());
    let _ = universe.attach(avatar_input_xr, avatar_driven_t);
    let _ = universe.attach(avatar_input_xr, avatar_xr_gamepad);

    // AvatarControlComponent: -Z forward (OpenXR default), body starts facing -Z (π yaw).
    // camera_bone triggers auto-calibration of model_root.y from J_Bip_C_Head rest pose height,
    // and causes any CameraXR/Camera3D direct children of AVC to be re-parented to that bone.
    let avatar_control = universe.world.add_component(
        AvatarControlComponent::new()
            .with_head_bone("J_Bip_C_Head")
            .with_camera_bone("J_Bip_C_Head")
            .with_left_hand_bone("J_Bip_L_Hand")
            .with_right_hand_bone("J_Bip_R_Hand")
            .with_initial_yaw(std::f32::consts::PI)
            .with_hand_rotation_smoothing(220.0),
    );
    let _ = universe.attach(avatar_driven_t, avatar_control);

    // CameraXR as a direct child of AVC — discovered and re-parented to J_Bip_C_Head at init.
    let camera_xr = universe.world.add_component(CameraXRComponent::on());
    let _ = universe.attach(avatar_control, camera_xr);
    let head_pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(camera_xr, head_pointer);

    // Grip controllers for hand bone splicing — children of AvatarControlComponent so
    // AvatarControlSystem discovers them by topology. Each needs a TransformComponent
    // child (driven_t) that OpenXRSystem writes each tick.
    let left_grip = universe.world.add_component(ControllerXRComponent::new(
        true,
        ControllerHand::Left,
        ControllerPoseKind::Grip,
    ));
    let left_grip_t = universe.world.add_component(TransformComponent::new());
    let _ = universe.attach(left_grip, left_grip_t);
    let left_pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(left_grip_t, left_pointer);
    let _ = universe.attach(avatar_control, left_grip);

    let right_grip = universe.world.add_component(ControllerXRComponent::new(
        true,
        ControllerHand::Right,
        ControllerPoseKind::Grip,
    ));
    let right_grip_t = universe.world.add_component(TransformComponent::new());
    let _ = universe.attach(right_grip, right_grip_t);
    let right_pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(right_grip_t, right_pointer);
    let _ = universe.attach(avatar_control, right_grip);

    // model_root: no explicit Y offset — AvatarControlSystem calibrates it from J_Bip_C_Head.
    let model_root = universe.world.add_component(TransformComponent::new());
    let model = universe
        .world
        .add_component(GLTFComponent::new("assets/models/pc-rei.hoodie.glb"));
    let emissive = universe.world.add_component(EmissiveComponent::on());
    let _ = universe.attach(model, emissive);

    let _ = universe.attach(editor_root, avatar_input_xr);
    let _ = universe.attach(avatar_control, model_root);
    let _ = universe.attach(model_root, model);
    universe.add(editor_root);

    // --- Controller debug cubes (tracked poses) ---
    let _left = spawn_controller_cube(
        &mut universe,
        xr_rig,
        ControllerHand::Left,
        (0.10, 0.90, 1.00, 1.0),
        220.0,
        options.xr_controller_rotation_filter,
    );
    let _right = spawn_controller_cube(
        &mut universe,
        xr_rig,
        ControllerHand::Right,
        (1.00, 0.35, 0.35, 1.0),
        220.0,
        options.xr_controller_rotation_filter,
    );

    // Enable OpenXR runtime.
    let xr_root = universe.world.add_component(XrComponent::on());
    universe.add(xr_root);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    // Force the glTF subtree to spawn so we can query the armature for bone markers.
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
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    // Bone markers for editor inspection.
    let marker_joints: &[(&str, (f32, f32, f32, f32))] = &[
        ("[name='J_Bip_C_Head']", (0.85, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_C_Neck']", (0.20, 0.85, 0.85, 0.9)),
        ("[name='J_Bip_C_UpperChest']", (0.20, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_L_UpperArm']", (0.85, 0.85, 0.20, 0.9)),
        ("[name='J_Bip_R_UpperArm']", (0.85, 0.60, 0.20, 0.9)),
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
        let marker_rcast = universe
            .world
            .add_component(RaycastableComponent::enabled());
        let _ = universe.world.add_child(marker_r, marker_c);
        let _ = universe.world.add_child(marker_r, marker_rcast);
        let _ = universe.world.add_child(marker_t, marker_r);
        let _ = universe.attach(bone, marker_t);
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
