use mittens_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Minimal scene with a camera so the window opens.
    let clear = universe
        .world
        .add_component(engine::ecs::component::BackgroundColorComponent::new());
    let clear_c = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.08, 0.08, 0.08, 1.0,
        ));
    let _ = universe.world.add_child(clear, clear_c);
    universe.add(clear);

    // Input-driven camera rig.
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(2.0));
    let rig_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(2.0, 0.0, 7.0),
    );
    let input_mode = universe.world.add_component(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let camera3d = universe.world.add_component(
        engine::ecs::component::Camera3DComponent::new()
            .with_far(250.0)
            .with_fov(70.0),
    );
    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig_transform);
    let _ = universe.attach(rig_transform, camera3d);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);
    universe.add(input);

    // Light so we can see non-emissive materials (even though our cubes are emissive).
    let light_tx = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 2.5, 4.0),
    );
    let light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(25.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.attach(light_tx, light);
    universe.add(light_tx);

    // ClockComponent sets global tempo.
    let clock = universe
        .world
        .add_component(engine::ecs::component::ClockComponent::new().with_bpm(128.0));
    universe.add(clock);

    // Audio output + oscillators driven by scheduled actions.
    let audio_out = universe
        .world
        .add_component(engine::ecs::component::AudioOutputComponent::new());
    universe.add(audio_out);

    // Noise oscillator: short white-noise hits.
    let osc_noise = engine::ecs::component::AudioOscillator::noise()
        .with_frequency(0.0)
        .with_amplitude(0.06)
        .with_enabled(false);
    let osc_noise_comp =
        universe
            .world
            .add_component(engine::ecs::component::AudioOscillatorComponent::single(
                osc_noise,
            ));
    let _ = universe.attach(audio_out, osc_noise_comp);

    // Drum oscillator: retriggers (phase + sweep) every enable.
    let osc_drum = engine::ecs::component::AudioOscillator::drum()
        // A low-ish pitch scale; actual pitch is set by scheduled notes.
        .with_frequency(32.0)
        .with_amplitude(0.40)
        .with_enabled(false);
    let osc_drum_comp =
        universe
            .world
            .add_component(engine::ecs::component::AudioOscillatorComponent::single(
                osc_drum,
            ));

    let _ = universe.attach(audio_out, osc_drum_comp);

    // --- Visual layout helpers ---
    fn spawn_text(universe: &mut engine::Universe, pos: (f32, f32, f32), scale: f32, text: &str) {
        let tx = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(pos.0, pos.1, pos.2)
                .with_scale(scale, scale, 1.0),
        );
        let t =
            universe
                .world
                .add_component(engine::ecs::component::TextComponent::with_word_wrap(
                    text, 38,
                ));
        let _ = universe.attach(tx, t);
        universe.add(tx);
    }

    fn spawn_emissive_cube(
        universe: &mut engine::Universe,
        parent: engine::ecs::ComponentId,
        pos: (f32, f32, f32),
        scale: f32,
        rgba: [f32; 4],
    ) {
        let tx = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(pos.0, pos.1, pos.2)
                .with_scale(scale, scale, scale),
        );
        let r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                rgba[0], rgba[1], rgba[2], rgba[3],
            ));
        let e = universe
            .world
            .add_component(engine::ecs::component::EmissiveComponent::on());
        let _ = universe.attach(parent, tx);
        let _ = universe.attach(tx, r);
        let _ = universe.attach(r, c);
        let _ = universe.attach(r, e);
    }

    fn spawn_op_cube(
        universe: &mut engine::Universe,
        parent: engine::ecs::ComponentId,
        pos: (f32, f32, f32),
        scale: f32,
        base_rgba: [f32; 4],
    ) -> engine::ecs::ComponentId {
        let tx = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(pos.0, pos.1, pos.2)
                .with_scale(scale, scale, scale),
        );
        let r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                base_rgba[0],
                base_rgba[1],
                base_rgba[2],
                base_rgba[3],
            ));
        let e = universe
            .world
            .add_component(engine::ecs::component::EmissiveComponent::on());

        let _ = universe.attach(parent, tx);
        let _ = universe.attach(tx, r);
        let _ = universe.attach(r, c);
        let _ = universe.attach(r, e);

        tx
    }

    // --- HUD / lanes ---
    let lane_x = -2.6_f32;
    let lane_title_z = -0.4_f32;
    let lane_cfg_z = -0.4_f32;
    let _lane_cube_z = -0.8_f32;
    let drum_y = 0.9_f32;
    let noise_y = -0.4_f32;

    spawn_text(
        &mut universe,
        (lane_x, drum_y + 0.55, lane_title_z),
        0.09,
        "Audio Source A: Drum (kick)",
    );
    spawn_text(
        &mut universe,
        (lane_x, drum_y + 0.25, lane_cfg_z),
        0.07,
        "type=Drum\namp=0.40\nbase_freq=32Hz\nkick: C0 dur=0.12 vel=0.90\nlookahead=0.10s",
    );

    spawn_text(
        &mut universe,
        (lane_x, noise_y + 0.55, lane_title_z),
        0.09,
        "Audio Source B: Noise",
    );
    spawn_text(
        &mut universe,
        (lane_x, noise_y + 0.25, lane_cfg_z),
        0.07,
        "type=Noise\namp=0.06\nnoise: C9 dur=0.06 vel=0.25\noffset=+0.5 beats\nlookahead=0.10s",
    );

    // Representative cubes for the two audio sources.
    let viz_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 0.0),
    );
    universe.add(viz_root);
    spawn_emissive_cube(
        &mut universe,
        viz_root,
        (lane_x - 0.28, drum_y + 0.55, lane_title_z),
        0.16,
        [1.00, 0.90, 0.10, 1.0],
    );
    spawn_emissive_cube(
        &mut universe,
        viz_root,
        (lane_x - 0.28, noise_y + 0.55, lane_title_z),
        0.16,
        [1.00, 1.00, 1.00, 1.0],
    );

    // Timeline roots so we can "reset all" via a single set_color action.
    let kick_timeline_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 0.0),
    );
    let noise_timeline_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 0.0),
    );
    let _ = universe.attach(viz_root, kick_timeline_root);
    let _ = universe.attach(viz_root, noise_timeline_root);

    // Animation with 16 keyframes: each keyframe schedules a kick at offset 0.0,
    // and a noise hit at offset 0.5.
    let anim = universe
        .world
        .add_component(engine::ecs::component::AnimationComponent::new());

    let kick_dur = 0.12_f32;
    let noise_dur = 0.06_f32;

    // Requested palette:
    // - kick: dark yellow -> bright yellow
    // - noise: grey -> white
    let kick_dark = [0.35, 0.28, 0.02, 1.0];
    let kick_bright = [1.00, 0.90, 0.10, 1.0];
    let noise_dark = [0.35, 0.35, 0.35, 1.0];
    let noise_bright = [1.00, 1.00, 1.00, 1.0];

    let beat_spacing = 0.38_f32;
    for i in 0..16 {
        let kf_beat = i as f64;
        let kf = universe
            .world
            .add_component(engine::ecs::component::KeyframeComponent::new(kf_beat));

        let kick = universe
            .world
            .add_component(engine::ecs::component::ActionComponent::new(
                engine::ecs::IntentValue::AudioSchedulePlay {
                    component_ids: vec![osc_drum_comp],
                    beat_offset: 0.0,
                    beat_context: None,
                    note: Some(
                        engine::ecs::component::MusicNote::c(0, kick_dur).with_velocity(0.9),
                    ),
                    gain: None,
                    rate: None,
                    duration: None,
                },
            ));

        let noise = universe
            .world
            .add_component(engine::ecs::component::ActionComponent::new(
                engine::ecs::IntentValue::AudioSchedulePlay {
                    component_ids: vec![osc_noise_comp],
                    beat_offset: 0.5,
                    beat_context: None,
                    note: Some(
                        engine::ecs::component::MusicNote::c(9, noise_dur).with_velocity(0.25),
                    ),
                    gain: None,
                    rate: None,
                    duration: None,
                },
            ));

        let _ = universe.attach(anim, kf);
        let _ = universe.attach(kf, kick);
        let _ = universe.attach(kf, noise);

        // Each keyframe drives visualization purely via normal actions:
        // 1) reset all timeline cubes to their base colors
        // 2) brighten the current beat's cubes
        let reset_kick_lane =
            universe
                .world
                .add_component(engine::ecs::component::ActionComponent::new(
                    engine::ecs::IntentValue::SetColor {
                        component_ids: vec![kick_timeline_root],
                        rgba: kick_dark,
                    },
                ));
        let reset_noise_lane =
            universe
                .world
                .add_component(engine::ecs::component::ActionComponent::new(
                    engine::ecs::IntentValue::SetColor {
                        component_ids: vec![noise_timeline_root],
                        rgba: noise_dark,
                    },
                ));
        let _ = universe.attach(kf, reset_kick_lane);
        let _ = universe.attach(kf, reset_noise_lane);

        // Timeline cubes: one cube per scheduled audio operation.
        // Kick (offset 0.0) on the drum lane.
        let kick_x = (kf_beat as f32) * beat_spacing;
        let kick_cube = spawn_op_cube(
            &mut universe,
            kick_timeline_root,
            (kick_x, drum_y, -1.3),
            0.10,
            kick_dark,
        );

        // Noise (offset 0.5) on the noise lane.
        let noise_x = ((kf_beat + 0.5) as f32) * beat_spacing;
        let noise_cube = spawn_op_cube(
            &mut universe,
            noise_timeline_root,
            (noise_x, noise_y, -1.3),
            0.10,
            noise_dark,
        );

        let brighten_kick =
            universe
                .world
                .add_component(engine::ecs::component::ActionComponent::new(
                    engine::ecs::IntentValue::SetColor {
                        component_ids: vec![kick_cube],
                        rgba: kick_bright,
                    },
                ));
        let brighten_noise =
            universe
                .world
                .add_component(engine::ecs::component::ActionComponent::new(
                    engine::ecs::IntentValue::SetColor {
                        component_ids: vec![noise_cube],
                        rgba: noise_bright,
                    },
                ));
        let _ = universe.attach(kf, brighten_kick);
        let _ = universe.attach(kf, brighten_noise);
    }

    universe.add(anim);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
