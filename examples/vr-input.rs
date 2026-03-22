use cat_engine::engine::ecs::component::{
    AmbientLightComponent, AvatarBodyYawComponent, BackgroundColorComponent, Camera3DComponent,
    CameraXRComponent, ColorComponent, ControllerHand, ControllerPoseKind, ControllerXRComponent,
    DirectionalLightComponent, EditorComponent, EmissiveComponent, GLTFComponent, InputComponent,
    InputTransformModeComponent, InputXRComponent, OpenXRComponent, PointerComponent,
    QuatTemporalFilterComponent, RayCastComponent, RaycastableComponent, RenderableComponent,
    RendererSettingsComponent, RendererStatsComponent, TransformComponent, TransformDropComponent,
    TransformForkTRSComponent, TransformMapRotationComponent, TransformMapScaleComponent,
    TransformMapTranslationComponent, TransformMergeTRSComponent, TransformPipelineComponent,
    TransformPipelineOutputComponent, TransformSampleAncestorComponent,
};
use cat_engine::engine::graphics::CameraTarget;
use cat_engine::engine::graphics::primitives::{MaterialHandle, Renderable};
use cat_engine::engine::graphics::BuiltinMeshType;
use cat_engine::engine::ecs::{IntentValue, SignalEmitter};
use cat_engine::{engine, utils};
use std::collections::HashMap;

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


fn attach_controller_parent_to_named_wrist(
    universe: &mut engine::Universe,
    avatar_root: engine::ecs::ComponentId,
    selector: &str,
    hand: ControllerHand,
    use_rotation_filter_pipeline: bool,
) -> Result<engine::ecs::ComponentId, String> {
    let wrist = universe
        .find_component(avatar_root, selector)
        .ok_or_else(|| format!("wrist selector did not match: {selector}"))?;
    let lower_arm = universe
        .parent_of(wrist)
        .ok_or_else(|| format!("matched wrist has no parent: {selector}"))?;

    let controller = universe.world.add_component(ControllerXRComponent::new(
        true,
        hand,
        ControllerPoseKind::Grip,
    ));
    let controller_t = universe.world.add_component(TransformComponent::new());

    universe
        .attach(lower_arm, controller)
        .map_err(|e| format!("attach lower_arm -> controller failed: {e}"))?;
    universe
        .attach(controller, controller_t)
        .map_err(|e| format!("attach controller -> transform failed: {e}"))?;

    let wrist_parent = if use_rotation_filter_pipeline {
        let pipeline = universe
            .world
            .add_component(TransformPipelineComponent::new());
        let fork = universe.world.add_component(TransformForkTRSComponent::new());
        let map_translation = universe
            .world
            .add_component(TransformMapTranslationComponent::new());
        let map_rotation = universe
            .world
            .add_component(TransformMapRotationComponent::new());
        let rotation_filter = universe.world.add_component(
            QuatTemporalFilterComponent::new().with_smoothing_factor(220.0),
        );
        let map_scale = universe.world.add_component(TransformMapScaleComponent::new());
        let merge = universe.world.add_component(TransformMergeTRSComponent::new());
        let output = universe
            .world
            .add_component(TransformPipelineOutputComponent::new());

        let _ = universe.attach(controller_t, pipeline);
        let _ = universe.attach(pipeline, fork);
        let _ = universe.attach(fork, map_translation);
        let _ = universe.attach(fork, map_rotation);
        let _ = universe.attach(map_rotation, rotation_filter);
        let _ = universe.attach(fork, map_scale);
        let _ = universe.attach(fork, merge);
        let _ = universe.attach(pipeline, output);
        output
    } else {
        controller_t
    };

    universe
        .attach(wrist_parent, wrist)
        .map_err(|e| format!("attach filtered controller path -> wrist failed: {e}"))?;

    println!(
        "[vr-input] inserted {:?} controller {:?} between parent='{}' and wrist='{}' (rotation_pipeline={})",
        hand,
        controller,
        universe.component_name(lower_arm).unwrap_or("<unnamed>"),
        universe.component_name(wrist).unwrap_or("<unnamed>"),
        if use_rotation_filter_pipeline {
            "enabled"
        } else {
            "disabled"
        },
    );

    Ok(controller)
}

fn spawn_sun_background(universe: &mut engine::Universe) {
    let bg_root = universe.world.add_component(engine::ecs::component::BackgroundComponent::new());
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
        .add_component(RenderableComponent::new(Renderable::new(circle_mesh, MaterialHandle::TOON_MESH)));
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
        .add_component(RenderableComponent::new(Renderable::new(circle_mesh, MaterialHandle::TOON_MESH)));
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
        /*
        mms:
        ControllerXR.new(true, hand, Aim) {
            T.with_scale(0.06, 0.06, 0.12) {
                TransformPipeline {
                    TransformForkTRS {
                        TransformMapTranslation {}
                        TransformMapRotation {
                            QuatTemporalFilter.with_smoothing_factor(rotation_smoothing)
                        }
                        TransformMapScale {}
                        TransformMergeTRS {}
                    }
                    TransformPipelineOutput {
                        T {
                            Renderable.cube() {
                                Color.rgba(color.0, color.1, color.2, color.3)
                            }
                        }
                    }
                }
            }
        }
        */
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

    let pipeline = universe
        .world
        .add_component(TransformPipelineComponent::new());
    let _ = universe.attach(controller_t, pipeline);

    let fork = universe.world.add_component(TransformForkTRSComponent::new());
    let _ = universe.attach(pipeline, fork);

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

    let map_scale = universe.world.add_component(TransformMapScaleComponent::new());
    let _ = universe.attach(fork, map_scale);

    let merge = universe.world.add_component(TransformMergeTRSComponent::new());
    let _ = universe.attach(fork, merge);

    let output = universe
        .world
        .add_component(TransformPipelineOutputComponent::new());
    let _ = universe.attach(pipeline, output);

    let filtered_t = universe.world.add_component(TransformComponent::new());
    let _ = universe.attach(output, filtered_t);

    let cube = universe.world.add_component(RenderableComponent::cube());
    let cube_color = universe
        .world
        .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));

    let _ = universe.attach(filtered_t, cube);
    let _ = universe.attach(cube, cube_color);

    controller_marker
}

/// Returns `(input_xr, output, yaw_correction)`.
///
/// `yaw_correction` is a TransformComponent with π Y rotation permanently under `output`.
/// Displaced bones always sit under `yaw_correction` (not directly under `output`) so that
/// the HMD world rotation is composed with the avatar's π flip before being applied to the bone.
/// Without this, head forward (+Z model space) maps to +Z world instead of -Z world, making the
/// head appear mirrored relative to the π-rotated body.
/// Build the head-rotation splice pipeline as a floating, uninitialized subtree.
///
/// The pipeline is NOT attached to the armature here. Call this once at setup time,
/// then use `IntentValue::Attach` from a DragStart handler to wire it to a clicked bone.
/// The subtree is initialized automatically when `splice_input_xr` is first attached
/// to an initialized parent.
fn create_head_rotation_splice(
    universe: &mut engine::Universe,
) -> (engine::ecs::ComponentId, engine::ecs::ComponentId, engine::ecs::ComponentId) {
    let input_xr = universe.world.add_component(InputXRComponent::on());
    let driven_t = universe.world.add_component(TransformComponent::new());
    let pipeline = universe.world.add_component(TransformPipelineComponent::new());
    let fork = universe.world.add_component(TransformForkTRSComponent::new());
    let map_translation = universe.world.add_component(TransformMapTranslationComponent::new());
    // skip=1: pipeline owner walks up → driven_T (skip=0) → bone parent (skip=1)
    let sample_ancestor = universe
        .world
        .add_component(TransformSampleAncestorComponent::new());
    let map_rotation = universe.world.add_component(TransformMapRotationComponent::new());
    let map_scale = universe.world.add_component(TransformMapScaleComponent::new());
    let merge = universe.world.add_component(TransformMergeTRSComponent::new());
    let output = universe.world.add_component(TransformPipelineOutputComponent::new());
    // π Y rotation so that HMD identity (facing -Z) maps to the bone facing -Z (same as body).
    let yaw_correction = universe.world.add_component(
        TransformComponent::new().with_rotation_euler(0.0, std::f32::consts::PI, 0.0),
    );

    // Wire the internal pipeline topology. These are all uninitialized; they will be
    // initialized when splice_input_xr is attached to an initialized armature node.
    let _ = universe.world.add_child(input_xr, driven_t);
    let _ = universe.world.add_child(driven_t, pipeline);
    let _ = universe.world.add_child(pipeline, fork);
    let _ = universe.world.add_child(fork, map_translation);
    let _ = universe.world.add_child(map_translation, sample_ancestor);
    let _ = universe.world.add_child(fork, map_rotation);
    let _ = universe.world.add_child(fork, map_scale);
    let _ = universe.world.add_child(fork, merge);
    let _ = universe.world.add_child(pipeline, output);
    let _ = universe.world.add_child(output, yaw_correction);

    (input_xr, output, yaw_correction)
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

    // disable aa
    let renderer_settings = universe
        .world
        .add_component(RendererSettingsComponent::msaa_off().with_window_size(320, 240));
    universe.add(renderer_settings);

   
    // Sky base.
    let background = universe
        .world
        .add_component(BackgroundColorComponent::rgba(0.62, 0.80, 1.00, 1.0));
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

    // Raycaster + pointer for desktop bone-picking. Allows clicking on bone markers
    // to interactively select the head-rotation splice target.
    let raycaster = universe
        .world
        .add_component(RayCastComponent::event_driven().with_max_distance(100.0));
    let _ = universe.attach(desktop_rig, raycaster);
    let pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(raycaster, pointer);

    example_util::spawn_desktop_camera_controls_hint(&mut universe, desktop_rig);
    universe.add(input);

    // --- XR rig ---
    let xr_input = universe.world.add_component(InputXRComponent::on());
    let xr_rig = universe.world.add_component(TransformComponent::new());
    let camera_xr = universe.world.add_component(CameraXRComponent::on());
    let _ = universe.attach(xr_input, xr_rig);
   
    // renderer stats
    let renderer_stats = universe
        .world
        .add_component(RendererStatsComponent::new().with_camera_target(CameraTarget::Xr));
    let render_stats_rig = universe.world.add_component(TransformComponent::new().with_position(0.0, 1.85, 0.6));
    let _ = universe.attach(render_stats_rig, renderer_stats);
    
    let _ = universe.attach(xr_rig, render_stats_rig);
    
    let _ = universe.attach(xr_rig, camera_xr);

        universe.add(xr_input);

    // Background "skybox" content (renderer removes view translation for backgrounds).
    spawn_sun_background(&mut universe);

    // --- VTuber model ---
    // InputXR drives a T to the full HMD pose (position + rotation).
    // A translation-only pipeline then strips the rotation so the avatar root
    // never rotates — only translates with the HMD.
    // model_root sits at a local Y offset so the avatar appears at floor level
    // rather than floating at head height.
    // A second InputXR (spliced into the neck below) drives head rotation only.
    //
    // Approximate pc-rei standing height relative to HMD height.
    // OpenXR LOCAL space Y=0 ≈ head height at session start; offset the avatar
    // root down so its feet land at Y ≈ -AVATAR_HEIGHT_M.
    // Tune this constant if the avatar still floats or sinks.
    const AVATAR_HEIGHT_M: f32 = 1.6;

    // EditorComponent wraps the entire scene so any renderable is selectable.
    let editor_root = universe.world.add_component(EditorComponent::new());

    let avatar_input_xr = universe.world.add_component(InputXRComponent::on());
    // OpenXRSystem requires a direct TransformComponent child to drive.
    let avatar_driven_t = universe.world.add_component(TransformComponent::new());

    // Translation-only filter pipeline.
    let av_pipeline = universe.world.add_component(TransformPipelineComponent::new());
    let av_fork = universe.world.add_component(TransformForkTRSComponent::new());
    let av_map_t = universe.world.add_component(TransformMapTranslationComponent::new());
    let av_map_r = universe.world.add_component(TransformMapRotationComponent::new());
    let av_drop_r = universe.world.add_component(TransformDropComponent::new()); // drop HMD rotation
    let av_map_s = universe.world.add_component(TransformMapScaleComponent::new());
    let av_merge = universe.world.add_component(TransformMergeTRSComponent::new());
    let av_output = universe.world.add_component(TransformPipelineOutputComponent::new());

    let _ = universe.attach(editor_root, avatar_input_xr);
    let _ = universe.attach(avatar_input_xr, avatar_driven_t);
    let _ = universe.attach(avatar_driven_t, av_pipeline);
    let _ = universe.attach(av_pipeline, av_fork);
    let _ = universe.attach(av_fork, av_map_t);
    let _ = universe.attach(av_fork, av_map_r);
    let _ = universe.attach(av_map_r, av_drop_r);
    let _ = universe.attach(av_fork, av_map_s);
    let _ = universe.attach(av_fork, av_merge);
    let _ = universe.attach(av_pipeline, av_output);

    // AvatarBodyYawComponent sits between pipeline output and model_root.
    // It reads head yaw from avatar_driven_t and smoothly rotates the body to follow.
    let avatar_body_yaw = universe.world.add_component(
        AvatarBodyYawComponent::new()
            .with_threshold(std::f32::consts::FRAC_PI_4) // 45°
            .with_rate(3.0)
            .with_hmd_driven_transform(avatar_driven_t),
    );
    let _ = universe.attach(av_output, avatar_body_yaw);

    // model_root: local Y = -AVATAR_HEIGHT_M so the feet sit at floor level.
    // Rotated 180° around Y because the VRM model faces +Z by default, but OpenXR LOCAL
    // space uses -Z as forward (in front of the user). Without this, the avatar's back faces
    // the direction the user walks toward.
    let model_root = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, -AVATAR_HEIGHT_M, 0.0)
            .with_rotation_euler(0.0, std::f32::consts::PI, 0.0)
            .with_scale(1.0, 1.0, 1.0),
    );
    let model = universe
        .world
        .add_component(GLTFComponent::new("assets/models/pc-rei.hoodie.glb"));

    // Keep pc-rei's emissive bits enabled.
    let emissive = universe.world.add_component(EmissiveComponent::on());
    let _ = universe.attach(model, emissive);

    let _ = universe.attach(avatar_body_yaw, model_root);
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
    let xr_root = universe.world.add_component(OpenXRComponent::on());
    universe.add(xr_root);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Force the glTF subtree to spawn now so we can inspect/query the armature.
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

    let left_wrist_selector = "[name='J_Bip_L_Hand']";
    let right_wrist_selector = "[name='J_Bip_R_Hand']";

    if let Err(err) = attach_controller_parent_to_named_wrist(
        &mut universe,
        model_root,
        left_wrist_selector,
        ControllerHand::Left,
        options.xr_controller_rotation_filter,
    ) {
        eprintln!("[vr-input] left wrist attach failed: {err}");
    }

    if let Err(err) = attach_controller_parent_to_named_wrist(
        &mut universe,
        model_root,
        right_wrist_selector,
        ControllerHand::Right,
        options.xr_controller_rotation_filter,
    ) {
        eprintln!("[vr-input] right wrist attach failed: {err}");
    }

    // --- Bone markers for interactive splice target selection ---
    // Small colored cubes are attached at key joints. Clicking one (on the desktop
    // window) moves the existing head-rotation splice to that bone.
    //
    // Original parents are recorded HERE (before the initial splice) so the move
    // handler always knows the natural parent to restore displaced bones to.
    //
    // Colors: head=purple, neck=teal, upper-chest=blue, shoulders=yellow/orange.
    let marker_joints: &[(&str, (f32, f32, f32, f32))] = &[
        ("[name='J_Bip_C_Head']",       (0.85, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_C_Neck']",        (0.20, 0.85, 0.85, 0.9)),
        ("[name='J_Bip_C_UpperChest']",  (0.20, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_L_UpperArm']",    (0.85, 0.85, 0.20, 0.9)),
        ("[name='J_Bip_R_UpperArm']",    (0.85, 0.60, 0.20, 0.9)),
    ];

    // Maps bone ComponentId → original_parent_of_bone (before any splice).
    // Keyed on the bone itself so DragStart can find it by walking up the ancestor chain —
    // the clicked renderable may be a viz:* node (added by the GLTF system) or our marker cube,
    // but both are descendants of the bone.
    let mut bone_markers: HashMap<
        engine::ecs::ComponentId,
        engine::ecs::ComponentId,
    > = HashMap::new();

    for &(selector, color) in marker_joints {
        let Some(bone) = universe.find_component(model_root, selector) else {
            continue;
        };
        let Some(original_parent) = universe.parent_of(bone) else {
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
        bone_markers.insert(bone, original_parent);
    }

    // Build the head-rotation splice pipeline as a floating subtree.
    let (splice_input_xr, _splice_output, splice_yaw_correction) =
        create_head_rotation_splice(&mut universe);

    // Apply the splice to J_Bip_C_Neck by default so head rotation drives the neck from
    // the start. The splice can be moved to a different bone by clicking a bone marker.
    if let Some(neck) = universe.find_component(model_root, "[name='J_Bip_C_Neck']") {
        if let Some(neck_parent) = universe.parent_of(neck) {
            universe.command_queue.push_intent_now(
                neck_parent,
                IntentValue::Attach {
                    parents: vec![neck_parent],
                    child: splice_input_xr,
                },
            );
            universe.command_queue.push_intent_now(
                splice_yaw_correction,
                IntentValue::Attach {
                    parents: vec![splice_yaw_correction],
                    child: neck,
                },
            );
            universe.systems.process_commands(
                &mut universe.world,
                &mut universe.visuals,
                &mut universe.command_queue,
            );
            println!("[vr-input] head rotation splice attached to J_Bip_C_Neck by default");
        }
    }

    // Tracks which bone is currently displaced under splice_yaw_correction.
    // Initialized to the neck if the default splice succeeded.
    let neck_default = universe.find_component(model_root, "[name='J_Bip_C_Neck']");
    let splice_displaced: std::sync::Arc<std::sync::Mutex<Option<engine::ecs::ComponentId>>> =
        std::sync::Arc::new(std::sync::Mutex::new(neck_default));
    let splice_displaced_handler = std::sync::Arc::clone(&splice_displaced);

    // DragStart on any bone marker: activate or move the splice pipeline.
    //
    // On first click: attach splice_input_xr to the bone's original parent, displace the
    // bone under yaw_correction (initialises the whole pipeline subtree automatically).
    //
    // On subsequent clicks (3 Attach intents):
    //   1. Restore currently displaced bone → its natural parent.
    //   2. Move splice_input_xr → original_parent[target_bone].
    //   3. Displace target_bone → splice_yaw_correction.
    universe.systems.rx.add_handler_closure(
        engine::ecs::SignalKind::DragStart,
        editor_root,
        move |world, emit, signal| {
            let Some(engine::ecs::EventSignal::DragStart { renderable, .. }) = &signal.event
            else {
                return;
            };

            // Walk up from the clicked renderable to find the nearest ancestor bone in our map.
            // The clicked component may be a viz:* node, our marker cube, or a raycaster child —
            // all are descendants of the bone.
            let mut target_bone = None;
            let mut cur = Some(*renderable);
            while let Some(node) = cur {
                if bone_markers.contains_key(&node) {
                    target_bone = Some(node);
                    break;
                }
                cur = world.parent_of(node);
            }
            let Some(target_bone) = target_bone else {
                return;
            };
            let Some(&original_parent) = bone_markers.get(&target_bone) else {
                return;
            };

            let mut displaced = splice_displaced_handler.lock().unwrap();

            // Step 1: restore currently displaced bone (if any) to its natural parent.
            if let Some(prev_bone) = *displaced {
                if let Some(&prev_parent) = bone_markers.get(&prev_bone) {
                    emit.push_intent_now(
                        prev_parent,
                        IntentValue::Attach {
                            parents: vec![prev_parent],
                            child: prev_bone,
                        },
                    );
                }
            }

            // Step 2: move the splice root to the original parent of the new target bone.
            emit.push_intent_now(
                original_parent,
                IntentValue::Attach {
                    parents: vec![original_parent],
                    child: splice_input_xr,
                },
            );

            // Step 3: displace the new target bone under yaw_correction (child of output).
            emit.push_intent_now(
                splice_yaw_correction,
                IntentValue::Attach {
                    parents: vec![splice_yaw_correction],
                    child: target_bone,
                },
            );

            *displaced = Some(target_bone);

            let bone_name = world.component_name(target_bone).unwrap_or("<unknown>").to_string();
            println!("[vr-input] splice moved to bone '{bone_name}'");
        },
    );

    // Install editor selection/gizmo handlers.
    universe
        .systems
        .editor
        .install_scoped_handlers_for_editor(&mut universe.systems.rx, editor_root);

    // Spawn world-tree + inspector panels.
    universe.systems.inspector.setup_panels_for_editor(
        &mut universe.systems.rx,
        &mut universe.world,
        &mut universe.command_queue,
        editor_root,
        (-0.7, 1.6, -1.2),
        (0.5, 1.6, -1.2),
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
