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

    // Small on-screen help.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);
    universe.add(input);

    // Light so we can see non-emissive materials.
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

    use engine::ecs::component::{
        ColorComponent, TextComponent, TextShadowComponent, TextureComponent,
        TextureFilteringComponent, TransformComponent, TransparentCutoutComponent,
    };

    // Styled text root.
    let text_root = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.0, 0.0)
            .with_scale(0.25, 0.25, 1.0),
    );

    // Color must be an ancestor of the glyph renderables.
    let color_id = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let _ = universe.attach(text_root, color_id);

    // This is the component we animate via SetText.
    let text_id = universe.world.add_component(TextComponent::new(">w<"));
    let _ = universe.attach(color_id, text_id);

    // Route into cutout pass for cleaner edges.
    let cutout = universe
        .world
        .add_component(TransparentCutoutComponent::new());
    let _ = universe.attach(text_id, cutout);

    // Font atlas.
    let tex = universe.world.add_component(TextureComponent::with_uri(
        "assets/textures/font_system.dds",
    ));
    let _ = universe.attach(text_id, tex);

    let shadow = universe.world.add_component(
        TextShadowComponent::new()
            .with_scale(1.35)
            .with_offset([0.06, -0.06, 0.0015]),
    );
    let filtering = universe
        .world
        .add_component(TextureFilteringComponent::nearest_magnification());

    let _ = universe.attach(text_id, shadow);
    let _ = universe.attach(text_id, filtering);

    universe.add(text_root);

    let clock_component = universe
        .world
        .add_component(engine::ecs::component::ClockComponent::new().with_bpm(140.0));
    let _ = universe.add(clock_component);

    // Looping animation: 4 beats long, 4 keyframes.
    let anim = universe
        .world
        .add_component(engine::ecs::component::AnimationComponent::new());

    let faces = [">w<", "^w^", "-_-", "o_o"];
    for (i, &face) in faces.iter().enumerate() {
        let kf = universe
            .world
            .add_component(engine::ecs::component::KeyframeComponent::new(i as f64));

        // Each keyframe emits a SetText intent.
        let action = universe
            .world
            .add_component(engine::ecs::component::ActionComponent::new(
                engine::ecs::IntentValue::SetText {
                    component_ids: vec![text_id],
                    text: face.to_string(),
                },
            ));

        let _ = universe.attach(anim, kf);
        let _ = universe.attach(kf, action);
    }

    universe.add(anim);

    // Process init-time registrations (Text expands into glyph subtrees here).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
