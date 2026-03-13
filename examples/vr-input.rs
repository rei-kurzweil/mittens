use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, CameraXRComponent, ColorComponent, ControllerHand, ControllerPoseKind, ControllerXRComponent, DirectionalLightComponent, EmissiveComponent, GLTFComponent, InputComponent, InputTransformModeComponent, OpenXRComponent, RenderableComponent, RendererSettingsComponent, RendererStatsComponent, TransformComponent
};
use cat_engine::engine::graphics::CameraTarget;
use cat_engine::engine::graphics::primitives::{MaterialHandle, Renderable};
use cat_engine::engine::graphics::BuiltinMeshType;
use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn spawn_sun_background(universe: &mut engine::Universe, parent: engine::ecs::ComponentId) {
    let bg_root = universe.world.add_component(engine::ecs::component::BackgroundComponent::new());
    let _ = universe.attach(parent, bg_root);

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
) -> engine::ecs::ComponentId {
    let controller_marker = universe.world.add_component(ControllerXRComponent::new(
        true,
        hand,
        ControllerPoseKind::Grip,
    ));
    let _ = universe.attach(xr_rig, controller_marker);

    // Transform driven by OpenXRSystem (TransformComponent child of ControllerXRComponent).
    let controller_t = universe.world.add_component(
        TransformComponent::new().with_scale(0.06, 0.06, 0.12),
    );
    let _ = universe.attach(controller_marker, controller_t);

    let cube = universe.world.add_component(RenderableComponent::cube());
    let cube_color = universe
        .world
        .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));

    let _ = universe.attach(controller_t, cube);
    let _ = universe.attach(cube, cube_color);

    controller_marker
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // disable aa
    let renderer_settings = universe
        .world
        .add_component(RendererSettingsComponent::msaa_off());
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

    example_util::spawn_desktop_camera_controls_hint(&mut universe, desktop_rig);
    universe.add(input);

    // --- XR rig ---
    let xr_rig = universe.world.add_component(TransformComponent::new());
    let camera_xr = universe.world.add_component(CameraXRComponent::on());
   
    // renderer stats
    let renderer_stats = universe
        .world
        .add_component(RendererStatsComponent::new().with_camera_target(CameraTarget::Xr));
    let render_stats_rig = universe.world.add_component(TransformComponent::new().with_position(0.0, 1.85, 0.6));
    let _ = universe.attach(render_stats_rig, renderer_stats);
    
    let _ = universe.attach(xr_rig, render_stats_rig);
    
    let _ = universe.attach(xr_rig, camera_xr);

    universe.add(xr_rig);

    // Background "skybox" content (renderer removes view translation for backgrounds).
    spawn_sun_background(&mut universe, xr_rig);

    // --- VTuber model (no editor subtree) ---
    let model_root = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.0, 0.0)
            .with_scale(1.0, 1.0, 1.0),
    );
    let model = universe
        .world
        .add_component(GLTFComponent::new("assets/models/pc-rei.hoodie.glb"));

    // Keep pc-rei's emissive bits enabled.
    let emissive = universe.world.add_component(EmissiveComponent::on());
    let _ = universe.attach(model, emissive);

    let _ = universe.attach(model_root, model);
    universe.add(model_root);

    // --- Controller debug cubes (tracked poses) ---
    let _left = spawn_controller_cube(
        &mut universe,
        xr_rig,
        ControllerHand::Left,
        (0.10, 0.90, 1.00, 1.0),
    );
    let _right = spawn_controller_cube(
        &mut universe,
        xr_rig,
        ControllerHand::Right,
        (1.00, 0.35, 0.35, 1.0),
    );

    // Enable OpenXR runtime.
    let xr_root = universe.world.add_component(OpenXRComponent::on());
    universe.add(xr_root);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // TODO (next): drive the VTuber skeleton hands using controller poses.
    // Likely approach:
    // - Find bone/joint ids for left/right hand + upperarm + lowerarm in the imported Skin.
    // - Implement a 2-bone IK solve (shoulder->elbow->wrist) targeting controller grip pose.
    // - Write joint local transforms each tick (similar to vtuber-joints-example).

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
