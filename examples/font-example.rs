use cat_engine::{engine, utils};

use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, InputComponent,
    InputTransformModeComponent, TextComponent, TextureFilteringComponent, TransformComponent,
};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Dark background so the font texture pops.
    let background = universe
        .world
        .register(BackgroundColorComponent::rgba(0.12, 0.05, 0.20, 1.0));
    universe.add(background);

    // Ambient so text is readable even without explicit lights.
    let ambient = universe
        .world
        .register(AmbientLightComponent::rgb(0.85, 0.85, 0.95));
    universe.add(ambient);

    // I {
    //   // not fps rotation, just relative rotation
    //   with_forward_z()
    //   with_roll_axis_z()
    //   C3D {}
    // }
    let input = universe
        .world
        .register(InputComponent::new().with_speed(2.0));
    let input_mode = universe
        .world
        .register(InputTransformModeComponent::forward_z().with_roll_axis_z());
    let _ = universe.attach(input, input_mode);

    let rig_transform = universe
        .world
        .register(TransformComponent::new().with_position(1.8, -0.5, 2.5));
    let _ = universe.attach(input, rig_transform);

    let camera = universe.world.register(Camera3DComponent::new());
    let _ = universe.attach(rig_transform, camera);

    universe.add(input);

    // T {
    //   with_translation(0,0, -2)
    //   TXT {
    //     "ababaabbaabbaaabbbaaabbbaaaabbbbaaaaabbbbbababababa"
    //     TextureComponent { assets/images/test.font_system.png }
    //   }
    // }
    fn estimate_text_height_world(text: &str, scale: f32) -> f32 {
        let line_count = text.lines().count().max(1) as f32;
        // Text quads are ~1 unit tall per line in text-space.
        // Add some padding so blocks don't feel cramped.
        let pad_lines = 1.25;
        (line_count + pad_lines) * scale
    }

    fn spawn_text_block(
        universe: &mut engine::Universe,
        position: (f32, f32, f32),
        scale: f32,
        text: &str,
    ) -> f32 {
        // T_root { T_scale { TXT { filtering } } }
        let text_root = universe
            .world
            .register(TransformComponent::new().with_position(position.0, position.1, position.2));

        let text_scale = universe
            .world
            .register(TransformComponent::new().with_scale(scale, scale, 1.0));
        let _ = universe.attach(text_root, text_scale);

        let text_c = universe.world.register(TextComponent::new(text));
        let _ = universe.attach(text_scale, text_c);

        // Keep it crisp.
        let filtering = universe
            .world
            .register(TextureFilteringComponent::nearest());
        let _ = universe.attach(text_c, filtering);

        universe.add(text_root);

        estimate_text_height_world(text, scale)
    }

    // --- text blocks ---
    // Multi-line samples at different scales.
    // (Text literals omitted in the README/snippet; see constants below.)
    const TEXT_BIG: &str = "CAT ENGINE\nfont example\nBIG TEXT";
    const TEXT_MED: &str = "multi-line\ntext block\nmedium";
    const TEXT_SMALL: &str = "small\nmono-ish\ntext";
    const TEXT_TINY: &str = "tiny\ntext\n(zoom in)";

    // Stack vertically; advance by (measured height + gap) so big text gets more room.
    let x = -1.2;
    let z = -2.0;
    let mut y = 1.2;
    let gap = 0.15;

    y -= spawn_text_block(&mut universe, (x, y, z), 0.55, TEXT_BIG) + gap;
    y -= spawn_text_block(&mut universe, (x, y, z), 0.25, TEXT_MED) + gap;
    y -= spawn_text_block(&mut universe, (x, y, z), 0.14, TEXT_SMALL) + gap;
    let _ = spawn_text_block(&mut universe, (x, y, z), 0.08, TEXT_TINY);

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
