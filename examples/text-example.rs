use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Input-driven camera rig.
    // Topology: I { T { C3D } }
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(1.5));
    let camera3d = universe
        .world
        .add_component(engine::ecs::component::Camera3DComponent::new());
    let rig_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 2.5),
    );
    let input_mode = universe.world.add_component(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig_transform);
    let _ = universe.attach(rig_transform, camera3d);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);
    universe.add(input);

    // Debug square: show the full font texture.
    let debug_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new()
            .with_position(0.6, -0.2, 0.0)
            .with_scale(0.8, 0.8, 1.0),
    );
    let debug_renderable = universe
        .world
        .add_component(engine::ecs::component::RenderableComponent::square());
    let debug_tex =
        universe
            .world
            .add_component(engine::ecs::component::TextureComponent::with_uri(
                "assets/textures/font_system.dds",
            ));
    let debug_filtering = universe
        .world
        .add_component(engine::ecs::component::TextureFilteringComponent::nearest_magnification());
    let _ = universe.attach(debug_root, debug_renderable);
    let _ = universe.attach(debug_renderable, debug_tex);
    let _ = universe.attach(debug_renderable, debug_filtering);
    universe.add(debug_root);

    // Light so we can actually see non-emissive materials.
    let light_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 2.0),
    );
    let light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(25.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.attach(light_transform, light);
    universe.add(light_transform);

    // 4 red cubes around the perimeter of the world (easy visual anchors).
    fn spawn_red_cube(universe: &mut engine::Universe, x: f32, y: f32, z: f32, s: f32) {
        let t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(s, s, s),
        );
        let r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                1.0, 0.0, 0.0, 1.0,
            ));
        let e = universe
            .world
            .add_component(engine::ecs::component::EmissiveComponent::on());

        let _ = universe.attach(t, r);
        let _ = universe.attach(r, c);
        let _ = universe.attach(r, e);

        universe.add(t);
    }

    let p = 1.5;
    let s = 0.25;
    spawn_red_cube(&mut universe, -p, -p, 0.0, s);
    spawn_red_cube(&mut universe, p, -p, 0.0, s);
    spawn_red_cube(&mut universe, -p, p, 0.0, s);
    spawn_red_cube(&mut universe, p, p, 0.0, s);

    use engine::ecs::component::{
        ColorComponent, TextComponent, TextShadowComponent, TextureComponent,
        TextureFilteringComponent, TransformComponent, TransparentCutoutComponent,
    };

    fn spawn_text_style(
        universe: &mut engine::Universe,
        pos: [f32; 3],
        scale: f32,
        text: &str,
        color: [f32; 4],
        shadow: TextShadowComponent,
        filtering: TextureFilteringComponent,
    ) {
        let root = universe.world.add_component(
            TransformComponent::new()
                .with_position(pos[0], pos[1], pos[2])
                .with_scale(scale, scale, 1.0),
        );

        // Color must be an ancestor of the glyph renderables.
        let color_id = universe
            .world
            .add_component(ColorComponent::rgba(color[0], color[1], color[2], color[3]));
        let _ = universe.attach(root, color_id);

        let text_id = universe.world.add_component(TextComponent::new(text));
        let _ = universe.attach(color_id, text_id);

        // Route into cutout pass for cleaner edges.
        let cutout = universe
            .world
            .add_component(TransparentCutoutComponent::new());
        let _ = universe.attach(text_id, cutout);

        // Use the same atlas as the debug quad.
        let tex = universe
            .world
            .add_component(TextureComponent::with_uri("assets/textures/font.dds"));
        let _ = universe.attach(text_id, tex);

        let shadow_id = universe.world.add_component(shadow);
        let _ = universe.attach(text_id, shadow_id);

        let filtering_id = universe.world.add_component(filtering);
        let _ = universe.attach(text_id, filtering_id);

        universe.add(root);
    }

    // Multiple text samples to show:
    // - different inherited colors
    // - different shadow settings
    // - different texture filtering
    spawn_text_style(
        &mut universe,
        [-0.95, 0.45, 0.0],
        0.12,
        "NEAREST_MAG\n(crisp)\nAaBbCc 123",
        [1.0, 1.0, 1.0, 1.0],
        TextShadowComponent::new()
            .with_scale(1.35)
            .with_offset([0.06, -0.06, 0.0015]),
        TextureFilteringComponent::nearest_magnification(),
    );

    spawn_text_style(
        &mut universe,
        [-0.95, 0.05, 0.0],
        0.12,
        "LINEAR\n(softer)\nAaBbCc 123",
        [0.55, 0.90, 1.0, 1.0],
        TextShadowComponent::new()
            .with_rgba([0.0, 0.0, 0.15, 1.0])
            .with_scale(1.20)
            .with_offset([0.05, -0.04, 0.0015]),
        TextureFilteringComponent::linear(),
    );

    spawn_text_style(
        &mut universe,
        [-0.95, -0.35, 0.0],
        0.12,
        "NEAREST\n(pixelly)\nAaBbCc 123",
        [1.0, 0.85, 0.35, 1.0],
        TextShadowComponent::new()
            .with_rgba([0.15, 0.0, 0.0, 1.0])
            .with_scale(1.55)
            .with_offset([0.08, -0.08, 0.0015]),
        TextureFilteringComponent::nearest(),
    );

    // Process init-time registrations (Text expands into glyph subtrees here).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
