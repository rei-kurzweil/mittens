use cat_engine::{engine, utils};

use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, BackgroundComponent, Camera3DComponent,
    CameraXRComponent, ColorComponent, InputComponent, InputTransformModeComponent,
    InputXRComponent, PointerComponent, RayCastComponent, RaycastableComponent, TextComponent,
    TextShadowComponent, TextureComponent, TextureFilteringComponent, TransformComponent,
    TransparentCutoutComponent,
};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Dark background so the font texture pops.
    let background = universe
        .world
        .add_component(BackgroundColorComponent::rgba(0.20, 0.2, 0.20, 1.0));
    universe.add(background);

    // Ambient so text is readable even without explicit lights.
    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.50, 0.50, 0.50));
    universe.add(ambient);

    let directional_tx = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.5, 1.0));
    let directional_light = universe.world.add_component(
        engine::ecs::component::DirectionalLightComponent::new()
            .with_color(1.0, 1.0, 1.0)
            .with_intensity(0.8),
    );
    let _ = universe.attach(directional_tx, directional_light);
    universe.add(directional_tx);

    // --- background clouds ---
    // Background stage (occluded + lit) so the cloud volume self-occludes but won't occlude
    // the foreground text (renderer clears depth before foreground).
    let bg_root = universe
        .world
        .add_component(BackgroundComponent::new().with_occlusion_and_lighting());
    universe.add(bg_root);

    let mut bg_cloud_params = example_util::CloudRingParams::default();
    bg_cloud_params.cloud_count = 6;
    bg_cloud_params.seed = 0xF0_17_C10u32;
    example_util::spawn_cloud_ring(&mut universe, bg_root, bg_cloud_params);

    // I {
    //   // not fps rotation, just relative rotation
    //   with_forward_z()
    //   with_roll_axis_y()
    //   C3D {}
    // }
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(2.0));
    let input_mode = universe
        .world
        .add_component(InputTransformModeComponent::forward_z().with_roll_axis_y());
    let _ = universe.attach(input, input_mode);

    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(1.8, -0.5, 2.5));
    let _ = universe.attach(input, rig_transform);

    let camera = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(rig_transform, camera);

    // Click-to-pick: prints which renderable (glyph quad) is under the cursor.
    let raycast = universe
        .world
        .add_component(RayCastComponent::event_driven().with_max_distance(50.0));
    let _ = universe.attach(rig_transform, raycast);

    // Opt-in: treat this camera raycaster as a pointer.
    let pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(raycast, pointer);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);

    universe.add(input);

    // --- foreground clouds ---
    // Normal foreground renderables (not background stage).
    // Offset the ring forward (negative Z) so several clusters are in view.
    let fg_cloud_root = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, -6.0, -10.0));
    universe.add(fg_cloud_root);

    let mut fg_cloud_params = example_util::CloudRingParams::default();
    fg_cloud_params.cloud_count = 4;
    fg_cloud_params.radius = 9.0;
    fg_cloud_params.center_y = 1.0;
    fg_cloud_params.puffs_per_cloud = 22;
    fg_cloud_params.angle_jitter = 0.35;
    fg_cloud_params.high_y_probability = 0.35;
    fg_cloud_params.high_y_multiplier = 1.4;
    fg_cloud_params.seed = 0xF0_17_C102u32;
    example_util::spawn_cloud_ring(&mut universe, fg_cloud_root, fg_cloud_params);

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
        font_texture_uri: Option<&str>,
        text_color_rgba: Option<[f32; 4]>,
        shadow: Option<TextShadowComponent>,
        filtering: TextureFilteringComponent,
    ) -> f32 {
        // T_root { T_scale { [Color] { TXT { shadow, filtering } } } }
        let text_root = universe.world.add_component(
            TransformComponent::new().with_position(position.0, position.1, position.2),
        );

        let text_scale = universe
            .world
            .add_component(TransformComponent::new().with_scale(scale, scale, 1.0));
        let _ = universe.attach(text_root, text_scale);

        let text_parent = if let Some([r, g, b, a]) = text_color_rgba {
            let color = universe
                .world
                .add_component(ColorComponent::rgba(r, g, b, a));
            let _ = universe.attach(text_scale, color);
            color
        } else {
            text_scale
        };

        let text_c = universe.world.add_component(TextComponent::new(text));
        let _ = universe.attach(text_parent, text_c);

        // Explicit opt-in: make the glyph renderables pickable.
        // TextSystem will propagate this to all spawned glyph quads.
        let raycastable = universe
            .world
            .add_component(RaycastableComponent::enabled());
        let _ = universe.attach(text_c, raycastable);

        // Route glyph quads into the alpha-to-coverage cutout pass.
        let cutout = universe
            .world
            .add_component(TransparentCutoutComponent::new());
        let _ = universe.attach(text_c, cutout);

        if let Some(shadow) = shadow {
            let shadow_id = universe.world.add_component(shadow);
            let _ = universe.attach(text_c, shadow_id);
        }

        // Optional: override the font atlas for this text block.
        // TextSystem will propagate this to all glyph renderables.
        if let Some(uri) = font_texture_uri {
            let tex = universe
                .world
                .add_component(TextureComponent::with_uri(uri));
            let _ = universe.attach(text_c, tex);
        }

        let filtering_id = universe.world.add_component(filtering);
        let _ = universe.attach(text_c, filtering_id);

        universe.add(text_root);

        estimate_text_height_world(text, scale)
    }

    // --- text blocks ---
    // Multi-line samples at different scales.
    // (Text literals omitted in the README/snippet; see constants below.)
    const TEXT_BIG: &str = "CAT ENGINE\nfont example\nBIG TEXT";
    const TEXT_MED: &str = "multi-line\ntext block\nmedium";
    const TEXT_SMALL: &str = "small\ntext";
    const TEXT_TINY: &str = "tiny\ntext\n(zoom in)";

    // Stack vertically; advance by (measured height + gap) so big text gets more room.
    let x = -1.2;
    let z = -2.0;
    let mut y = 1.2;
    let gap = 0.15;

    let shadow_crisp = Some(
        TextShadowComponent::new()
            .with_scale(1.35)
            .with_offset([0.06, -0.06, 0.0015]),
    );

    y -= spawn_text_block(
        &mut universe,
        (x, y, z),
        0.55,
        TEXT_BIG,
        None,
        Some([1.0, 1.0, 1.0, 1.0]),
        shadow_crisp,
        TextureFilteringComponent::nearest_magnification(),
    ) + gap;
    y -= spawn_text_block(
        &mut universe,
        (x, y, z),
        0.25,
        TEXT_MED,
        None,
        Some([0.60, 0.95, 1.00, 1.0]),
        Some(
            TextShadowComponent::new()
                .with_rgba([0.0, 0.0, 0.15, 1.0])
                .with_scale(1.20)
                .with_offset([0.05, -0.04, 0.0015]),
        ),
        TextureFilteringComponent::linear(),
    ) + gap;
    y -= spawn_text_block(
        &mut universe,
        (x, y, z),
        0.14,
        TEXT_SMALL,
        None,
        Some([1.0, 0.88, 0.35, 1.0]),
        Some(
            TextShadowComponent::new()
                .with_rgba([0.15, 0.0, 0.0, 1.0])
                .with_scale(1.55)
                .with_offset([0.08, -0.08, 0.0015]),
        ),
        TextureFilteringComponent::nearest(),
    ) + gap;
    let _ = spawn_text_block(
        &mut universe,
        (x, y, z),
        0.08,
        TEXT_TINY,
        None,
        Some([0.90, 1.0, 0.70, 1.0]),
        shadow_crisp,
        TextureFilteringComponent::nearest_magnification(),
    );

    // Left block: explicit multi-line text using the default font_system atlas.
    const TEXT_LEFT: &str = "even though there's hexes\nto the solar plexus in my lexus\ni'm feelin' reckless,\nwhen i'm eating breakfast";
    let _ = spawn_text_block(
        &mut universe,
        (x - 8.1, 1.1, z),
        0.22,
        TEXT_LEFT,
        Some("assets/textures/font_system.dds"),
        Some([0.95, 0.95, 0.95, 1.0]),
        Some(
            TextShadowComponent::new()
                .with_scale(1.25)
                .with_offset([0.05, -0.05, 0.0015]),
        ),
        TextureFilteringComponent::nearest_magnification(),
    );

    // Alt atlas: put it *behind* the original stack (slightly farther from the camera)
    // and tint it dark grey.
    let alt_atlas = Some("assets/textures/font_system.0.0.dds");
    let alt_z = z - 0.05;
    let alt_grey = Some([0.25, 0.25, 0.25, 1.0]);

    let mut y_alt = 1.2;
    y_alt -= spawn_text_block(
        &mut universe,
        (x, y_alt, alt_z),
        0.55,
        TEXT_BIG,
        alt_atlas,
        alt_grey,
        Some(
            TextShadowComponent::new()
                .with_rgba([0.0, 0.0, 0.0, 1.0])
                .with_scale(1.15)
                .with_offset([0.03, -0.03, 0.0015]),
        ),
        TextureFilteringComponent::linear(),
    ) + gap;
    y_alt -= spawn_text_block(
        &mut universe,
        (x, y_alt, alt_z),
        0.25,
        TEXT_MED,
        alt_atlas,
        alt_grey,
        Some(
            TextShadowComponent::new()
                .with_rgba([0.0, 0.0, 0.0, 1.0])
                .with_scale(1.15)
                .with_offset([0.03, -0.03, 0.0015]),
        ),
        TextureFilteringComponent::linear(),
    ) + gap;
    y_alt -= spawn_text_block(
        &mut universe,
        (x, y_alt, alt_z),
        0.14,
        TEXT_SMALL,
        alt_atlas,
        alt_grey,
        Some(
            TextShadowComponent::new()
                .with_rgba([0.0, 0.0, 0.0, 1.0])
                .with_scale(1.15)
                .with_offset([0.03, -0.03, 0.0015]),
        ),
        TextureFilteringComponent::linear(),
    ) + gap;
    let _ = spawn_text_block(
        &mut universe,
        (x, y_alt, alt_z),
        0.08,
        TEXT_TINY,
        alt_atlas,
        alt_grey,
        Some(
            TextShadowComponent::new()
                .with_rgba([0.0, 0.0, 0.0, 1.0])
                .with_scale(1.15)
                .with_offset([0.03, -0.03, 0.0015]),
        ),
        TextureFilteringComponent::linear(),
    );

    universe.enable_repl();

    let xr_input = universe.world.add_component(InputXRComponent::on());
    let xr_head = universe.world.add_component(TransformComponent::new());
    let xr_camera = universe.world.add_component(CameraXRComponent::on());
    let _ = universe.attach(xr_input, xr_head);
    let _ = universe.attach(xr_head, xr_camera);
    universe.add(xr_input);

    // Add an OpenXR component so OpenXRSystem initializes and starts polling events.
    let xr_root = universe
        .world
        .add_component(engine::ecs::component::OpenXRComponent::on());
    universe.add(xr_root);

    // Process init-time registrations (Text expands into glyph subtrees here).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
