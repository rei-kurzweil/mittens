use cat_engine::{engine, utils};

fn main() {
    utils::logger::init();

    // Debug toggle: allow isolating stack overflows to audio vs. non-audio init.
    // Set `CAT_AUDIO_EXAMPLE_DISABLE_AUDIO=1` to skip creating audio components and
    // skip scheduling audio notes. The UI/text/cube visualization will still spawn.
    let audio_enabled = std::env::var("CAT_AUDIO_EXAMPLE_DISABLE_AUDIO")
        .ok()
        .as_deref()
        != Some("1");

    // If set, we still build audio graph components but we don't start the CPAL output stream.
    // This helps distinguish "audio graph / scheduling" issues from "CPAL backend" issues.
    let audio_output_enabled = std::env::var("CAT_AUDIO_EXAMPLE_AUDIO_OUTPUT_OFF")
        .ok()
        .as_deref()
        != Some("1");

    println!("[audio-graph-example] start");

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    println!("[audio-graph-example] universe created");

    // Minimal scene with a camera so the window opens (copied from animation-example).
    let clear = universe
        .world
        .register(engine::ecs::component::BackgroundColorComponent::rgba(
            0.05, 0.05, 0.08, 1.0,
        ));
    universe.add(clear);

    // Input-driven camera rig.
    let input = universe
        .world
        .register(engine::ecs::component::InputComponent::new().with_speed(2.0));
    let rig_transform = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(2.0, 0.0, 7.0));
    let input_mode = universe.world.register(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let camera3d = universe.world.register(
        engine::ecs::component::Camera3DComponent::new()
            .with_far(250.0)
            .with_fov(70.0),
    );
    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig_transform);
    let _ = universe.attach(rig_transform, camera3d);
    universe.add(input);

    // Light.
    let light_tx = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 2.5, 4.0));
    let light = universe.world.register(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(25.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.attach(light_tx, light);
    universe.add(light_tx);

    // ClockComponent sets global tempo.
    if audio_enabled {
        let clock = universe
            .world
            .register(engine::ecs::component::ClockComponent::new().with_bpm(128.0));
        universe.add(clock);
    }

    if audio_enabled {
        println!("[audio-graph-example] clock added");
    } else {
        println!("[audio-graph-example] audio disabled; skipping clock");
    }

    // Audio output + 2 oscillator sources.
    let audio_out = if audio_enabled {
        let audio_out_comp = if audio_output_enabled {
            engine::ecs::component::AudioOutputComponent::new()
        } else {
            engine::ecs::component::AudioOutputComponent::off()
        };
        let audio_out = universe.world.register(audio_out_comp);
        universe.add(audio_out);
        Some(audio_out)
    } else {
        None
    };

    match (audio_enabled, audio_output_enabled) {
        (false, _) => println!("[audio-graph-example] audio disabled; skipping audio output"),
        (true, true) => println!("[audio-graph-example] audio output added (CPAL on)"),
        (true, false) => println!("[audio-graph-example] audio output added (CPAL off)"),
    }

    // Track A: a bright saw lead.
    let osc_a_comp = if audio_enabled {
        let osc_a = engine::ecs::component::AudioOscillator::saw()
            .with_frequency(110.0)
            .with_amplitude(0.12)
            .with_enabled(false);
        let osc_a_comp =
            universe
                .world
                .register(engine::ecs::component::AudioOscillatorComponent::single(
                    osc_a,
                ));
        if let Some(audio_out) = audio_out {
            let _ = universe.attach(audio_out, osc_a_comp);
        }
        Some(osc_a_comp)
    } else {
        None
    };

    if audio_enabled {
        println!("[audio-graph-example] track A created");
    } else {
        println!("[audio-graph-example] audio disabled; skipping track A");
    }

    // Effect tree A (branching):
    // osc_a
    //   Gain
    //     Mix(weights)
    //     LowPass
    //       Limiter
    //     HighPass
    if let Some(osc_a_comp) = osc_a_comp {
        let gain_a = universe
            .world
            .register(engine::ecs::component::AudioGainComponent::new(0.8));
        let mix_a = universe
            .world
            .register(engine::ecs::component::AudioMixComponent::new(vec![
                0.75, 0.25,
            ]));
        let lp_a =
            universe
                .world
                .register(engine::ecs::component::AudioLowPassFilterComponent::new(
                    1200.0, 0.40,
                ));
        let lim_a = universe
            .world
            .register(engine::ecs::component::AudioLimiterComponent::new(
                4.0, 80.0, 0.90,
            ));
        let hp_a =
            universe
                .world
                .register(engine::ecs::component::AudioHighPassFilterComponent::new(
                    300.0, 0.20,
                ));

        let _ = universe.attach(osc_a_comp, gain_a);
        let _ = universe.attach(gain_a, mix_a);
        let _ = universe.attach(gain_a, lp_a);
        let _ = universe.attach(lp_a, lim_a);
        let _ = universe.attach(gain_a, hp_a);
    }

    if audio_enabled {
        println!("[audio-graph-example] track A effects attached");
    }

    // --- Visual layout helpers (copied/adapted from animation-example) ---
    fn spawn_text(
        universe: &mut engine::Universe,
        pos: (f32, f32, f32),
        scale: f32,
        wrap_cols: usize,
        text: &str,
    ) {
        let tx = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(pos.0, pos.1, pos.2)
                .with_scale(scale, scale, 1.0),
        );
        let t = universe
            .world
            .register(engine::ecs::component::TextComponent::with_word_wrap(
                text, wrap_cols,
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
        let tx = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(pos.0, pos.1, pos.2)
                .with_scale(scale, scale, scale),
        );
        let r = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let c = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(
                rgba[0], rgba[1], rgba[2], rgba[3],
            ));
        let e = universe
            .world
            .register(engine::ecs::component::EmissiveComponent::on());
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
        let tx = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(pos.0, pos.1, pos.2)
                .with_scale(scale, scale, scale),
        );
        let r = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let c = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(
                base_rgba[0],
                base_rgba[1],
                base_rgba[2],
                base_rgba[3],
            ));
        let e = universe
            .world
            .register(engine::ecs::component::EmissiveComponent::on());

        let _ = universe.attach(parent, tx);
        let _ = universe.attach(tx, r);
        let _ = universe.attach(r, c);
        let _ = universe.attach(r, e);

        tx
    }

    // --- HUD / lane (single track) ---
    let lane_x = -2.6_f32;
    let lane_title_z = -0.4_f32;
    let lane_cfg_z = -0.4_f32;
    let lane_pat_z = -1.3_f32;
    let lane_chain_z = -1.3_f32;
    let lane_graph_z = -1.15_f32;

    let track_a_y = 0.9_f32;

    spawn_text(
        &mut universe,
        (lane_x, track_a_y + 0.55, lane_title_z),
        0.09,
        42,
        "Track A: AudioOscillator::saw()",
    );
    spawn_text(
        &mut universe,
        (lane_x, track_a_y + 0.25, lane_cfg_z),
        0.07,
        48,
        "oscillators=1\nfrequency_hz=110.0\namplitude=0.12\nenabled=false\nlookahead=0.10s",
    );

    let viz_root = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 0.0));
    universe.add(viz_root);

    println!("[audio-graph-example] viz root added");

    // Small identifier cubes next to titles.
    spawn_emissive_cube(
        &mut universe,
        viz_root,
        (lane_x - 0.28, track_a_y + 0.55, lane_title_z),
        0.16,
        [0.85, 0.40, 1.00, 1.0],
    );

    // Pattern roots so we can reset via one SetColor action.
    let track_a_pattern_root = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 0.0));
    let _ = universe.attach(viz_root, track_a_pattern_root);

    // Labels for pattern/chain/graph sections.
    spawn_text(
        &mut universe,
        (lane_x, track_a_y + 0.05, lane_title_z),
        0.06,
        42,
        "pattern →",
    );
    spawn_text(
        &mut universe,
        (lane_x, track_a_y - 0.30, lane_title_z),
        0.06,
        42,
        "chain →",
    );
    spawn_text(
        &mut universe,
        (lane_x, track_a_y - 0.78, lane_title_z),
        0.06,
        42,
        "graph →",
    );

    // Animation with 16 keyframes drives scheduled notes + pattern highlights.
    let anim = universe
        .world
        .register(engine::ecs::component::AnimationComponent::new());

    let dur_a = 0.20_f32;
    let beat_spacing = 0.38_f32;

    let a_dark = [0.18, 0.08, 0.22, 1.0];
    let a_bright = [0.85, 0.40, 1.00, 1.0];

    // Simple repeating patterns.
    let a_notes = [
        engine::ecs::component::MusicNote::c(4, dur_a).with_velocity(0.80),
        engine::ecs::component::MusicNote::e(4, dur_a).with_velocity(0.80),
        engine::ecs::component::MusicNote::g(4, dur_a).with_velocity(0.80),
        engine::ecs::component::MusicNote::b(4, dur_a).with_velocity(0.80),
    ];

    for i in 0..16 {
        let kf_beat = i as f64;
        let kf = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(kf_beat));

        let _ = universe.attach(anim, kf);

        // Scheduled audio notes (sample-accurate via lookahead).
        if audio_enabled {
            let Some(osc_a_comp) = osc_a_comp else {
                panic!("audio_enabled but track A was not created");
            };

            let note_a = a_notes[i % a_notes.len()];

            let act_a = universe
                .world
                .register(engine::ecs::component::ActionComponent::new(
                    engine::ecs::component::Action::oscillator_schedule_music_note(
                        vec![osc_a_comp],
                        0.0,
                        note_a,
                    ),
                ));

            let _ = universe.attach(kf, act_a);
        }

        // Visualization reset per keyframe.
        let reset_a = universe
            .world
            .register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::set_color(vec![track_a_pattern_root], a_dark),
            ));
        let _ = universe.attach(kf, reset_a);

        // Pattern cube per step for each track.
        let x = (kf_beat as f32) * beat_spacing;
        let cube_a = spawn_op_cube(
            &mut universe,
            track_a_pattern_root,
            (x, track_a_y, lane_pat_z),
            0.10,
            a_dark,
        );

        let bright_a = universe
            .world
            .register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::set_color(vec![cube_a], a_bright),
            ));
        let _ = universe.attach(kf, bright_a);
    }
    universe.add(anim);

    println!("[audio-graph-example] animation created");

    // --- Processing chain + graph visualization (compiled graph → cubes + labels) ---
    use engine::ecs::system::audio_graph_compiler::{
        AudioGraphCompiler, AudioGraphNode, AudioGraphNodeKind,
    };

    fn node_label(node: &AudioGraphNode) -> String {
        match &node.kind {
            AudioGraphNodeKind::OscillatorSource { voices } => {
                format!("AudioOscillatorComponent {{ oscillators: <len={voices}> }}")
            }
            AudioGraphNodeKind::Gain { gain } => {
                format!("AudioGainComponent {{ gain: {gain:.3} }}")
            }
            AudioGraphNodeKind::LowPass {
                cutoff_hz,
                resonance,
            } => {
                format!(
                    "AudioLowPassFilterComponent {{ cutoff_hz: {cutoff_hz:.1}, resonance: {resonance:.3} }}"
                )
            }
            AudioGraphNodeKind::HighPass {
                cutoff_hz,
                resonance,
            } => {
                format!(
                    "AudioHighPassFilterComponent {{ cutoff_hz: {cutoff_hz:.1}, resonance: {resonance:.3} }}"
                )
            }
            AudioGraphNodeKind::Limiter {
                attack_ms,
                release_ms,
                threshold,
            } => {
                format!(
                    "AudioLimiterComponent {{ attack_ms: {attack_ms:.1}, release_ms: {release_ms:.1}, threshold: {threshold:.3} }}"
                )
            }
        }
    }

    fn flatten_preorder<'a>(node: &'a AudioGraphNode, out: &mut Vec<&'a AudioGraphNode>) {
        out.push(node);
        for ch in node.children.iter() {
            flatten_preorder(ch, out);
        }
    }

    fn subtree_units(node: &AudioGraphNode) -> i32 {
        if node.children.is_empty() {
            1
        } else {
            node.children.iter().map(subtree_units).sum::<i32>().max(1)
        }
    }

    fn spawn_graph(
        universe: &mut engine::Universe,
        parent: engine::ecs::ComponentId,
        node: &AudioGraphNode,
        origin: (f32, f32, f32),
        depth: i32,
        y_cursor_units: &mut i32,
        dx: f32,
        dy: f32,
        cube_scale: f32,
        rgba: [f32; 4],
    ) {
        let my_units = subtree_units(node);
        let my_center_units = *y_cursor_units + my_units / 2;

        let x = origin.0 + (depth as f32) * dx;
        let y = origin.1 - (my_center_units as f32) * dy;
        let z = origin.2;

        let cube_tx = spawn_op_cube(universe, parent, (x, y, z), cube_scale, rgba);

        // Label next to the cube.
        let label = node_label(node);
        let tx = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(x + 0.14, y + 0.02, z)
                .with_scale(0.06, 0.06, 1.0),
        );
        let t = universe
            .world
            .register(engine::ecs::component::TextComponent::with_word_wrap(
                &label, 56,
            ));
        let _ = universe.attach(parent, tx);
        let _ = universe.attach(tx, t);
        universe.add(tx);

        // Mix/branch label if branching.
        if node.children.len() > 1 {
            let mut weights: Vec<f32> = Vec::with_capacity(node.children.len());
            for i in 0..node.children.len() {
                let w = node
                    .mix
                    .as_ref()
                    .map(|m| m.weights.get(i).copied().unwrap_or(1.0))
                    .unwrap_or(1.0);
                weights.push(w);
            }
            let mix_label = if let Some(_m) = &node.mix {
                format!("mix: AudioMixComponent {{ weights: {:?} }}", weights)
            } else {
                format!("mix: <implicit> (weights: {:?})", weights)
            };
            let mix_tx = universe.world.register(
                engine::ecs::component::TransformComponent::new()
                    .with_position(x + 0.14, y - 0.09, z)
                    .with_scale(0.05, 0.05, 1.0),
            );
            let mix_t =
                universe
                    .world
                    .register(engine::ecs::component::TextComponent::with_word_wrap(
                        &mix_label, 60,
                    ));
            let _ = universe.attach(parent, mix_tx);
            let _ = universe.attach(mix_tx, mix_t);
            universe.add(mix_tx);
        }

        // Children, stacked vertically under this subtree.
        let mut child_cursor = *y_cursor_units;
        for ch in node.children.iter() {
            spawn_graph(
                universe,
                parent,
                ch,
                origin,
                depth + 1,
                &mut child_cursor,
                dx,
                dy,
                cube_scale,
                rgba,
            );
            child_cursor += subtree_units(ch);
        }

        *y_cursor_units += my_units;

        // Keep the cube_tx alive (it is already part of the world tree via parent).
        let _ = cube_tx;
    }

    // Compile + display chain and graph for the track.
    let compiled_a = if audio_enabled {
        println!("[audio-graph-example] compiling graph...");
        let Some(osc_a_comp) = osc_a_comp else {
            panic!("audio_enabled but track A was not created");
        };
        let compiled_a =
            AudioGraphCompiler::compile(&universe.world, osc_a_comp).expect("compile A");
        println!("[audio-graph-example] graph compiled");
        Some(compiled_a)
    } else {
        println!("[audio-graph-example] audio disabled; skipping graph compilation");
        None
    };

    // Processing chain: flattened pre-order row with labels.
    fn spawn_chain_row(
        universe: &mut engine::Universe,
        parent: engine::ecs::ComponentId,
        compiled: &engine::ecs::system::audio_graph_compiler::CompiledAudioGraph,
        origin: (f32, f32, f32),
        x_step: f32,
        rgba: [f32; 4],
    ) {
        let mut nodes = Vec::new();
        flatten_preorder(&compiled.root, &mut nodes);
        for (i, n) in nodes.iter().enumerate() {
            let x = origin.0 + (i as f32) * x_step;
            let y = origin.1;
            let z = origin.2;
            let _cube = spawn_op_cube(universe, parent, (x, y, z), 0.09, rgba);

            let label = node_label(n);
            let tx = universe.world.register(
                engine::ecs::component::TransformComponent::new()
                    .with_position(x - 0.03, y - 0.16, z)
                    .with_scale(0.05, 0.05, 1.0),
            );
            let t = universe
                .world
                .register(engine::ecs::component::TextComponent::with_word_wrap(
                    &label, 28,
                ));
            let _ = universe.attach(parent, tx);
            let _ = universe.attach(tx, t);
            universe.add(tx);
        }
    }

    if let Some(compiled_a) = &compiled_a {
        spawn_chain_row(
            &mut universe,
            viz_root,
            compiled_a,
            (0.0, track_a_y - 0.25, lane_chain_z),
            0.48,
            [0.22, 0.10, 0.28, 1.0],
        );
    } else {
        spawn_text(
            &mut universe,
            (0.0, track_a_y - 0.25, lane_chain_z),
            0.06,
            60,
            "(audio disabled)",
        );
    }

    // Full compiled graph visualization (tree layout + mix labels).
    if let Some(compiled_a) = &compiled_a {
        {
            let mut cursor = 0;
            spawn_graph(
                &mut universe,
                viz_root,
                &compiled_a.root,
                (0.0, track_a_y - 0.55, lane_graph_z),
                0,
                &mut cursor,
                0.55,
                0.18,
                0.08,
                [0.35, 0.16, 0.45, 1.0],
            );
        }
    } else {
        spawn_text(
            &mut universe,
            (0.0, track_a_y - 0.55, lane_graph_z),
            0.06,
            60,
            "(audio disabled)",
        );
    }

    // Keep window open.
    println!("[audio-graph-example] processing commands...");
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    println!("[audio-graph-example] commands processed; launching window");

    let user_input = engine::user_input::UserInput::new();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
