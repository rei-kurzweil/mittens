use cat_engine::{engine, utils};

use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, InputComponent,
    InputTransformModeComponent, TextComponent, TextureComponent, TextureFilteringComponent,
    TransformComponent,
};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Dark background so the font texture pops.
    let background = universe
        .world
        .add_component(BackgroundColorComponent::rgba(0.12, 0.05, 0.20, 1.0));
    universe
        .world
        .init_component_tree(background, &mut universe.command_queue);

    // Ambient so text is readable even without explicit lights.
    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.85, 0.85, 0.95));
    universe
        .world
        .init_component_tree(ambient, &mut universe.command_queue);

    // I {
    //   // not fps rotation, just relative rotation
    //   with_forward_z()
    //   with_roll_axis_z()
    //   C3D {}
    // }
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(2.0));
    let input_mode = universe
        .world
        .add_component(InputTransformModeComponent::forward_z().with_roll_axis_z());
    let _ = universe.world.add_child(input, input_mode);

    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 0.0));
    let _ = universe.world.add_child(input, rig_transform);

    let camera = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.world.add_child(rig_transform, camera);

    universe
        .world
        .init_component_tree(input, &mut universe.command_queue);

    // T {
    //   with_translation(0,0, -2)
    //   TXT {
    //     "ababaabbaabbaaabbbaaabbbaaaabbbbaaaaabbbbbababababa"
    //     TextureComponent { assets/images/test.font_system.png }
    //   }
    // }
    let text_root = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, -2.0));

    // Scale down so the long string fits in view.
    let text_scale = universe
        .world
        .add_component(TransformComponent::new().with_scale(0.3, 0.3, 1.0));
    let _ = universe.world.add_child(text_root, text_scale);

    let text = universe.world.add_component(TextComponent::new(
        "ababaabbaabbaaabbbaaabbbaaaabbbbaaaaabbbbbababababa",
    ));
    let _ = universe.world.add_child(text_scale, text);

    // Keep it crisp.
    let filtering = universe
        .world
        .add_component(TextureFilteringComponent::nearest());
    let _ = universe.world.add_child(text, filtering);

    universe
        .world
        .init_component_tree(text_root, &mut universe.command_queue);

    universe.enable_repl();

    // Process init-time registrations (Text expands into glyph subtrees here).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
