use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

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
        .add_component(engine::ecs::component::BackgroundColorComponent::new());
    let clear_c = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.07, 0.07, 0.07, 1.0,
        ));
    let _ = universe.world.add_child(clear, clear_c);
    universe.add(clear);

    // Ambient light so unlit areas aren't pitch black.
    // Keep it dark to match the background clear color.
    // (User request) 2.5x brighter.
    let ambient = universe
        .world
        .add_component(engine::ecs::component::AmbientLightComponent::rgb(
            0.075, 0.075, 0.075,
        ));
    universe.add(ambient);

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

    // Directional light (sun-ish). Note: the renderer interprets the node's world position
    // as a direction vector (see DirectionalLightComponent docs).
    let light_tx = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.2, 0.7, 1.0),
    );
    let light = universe.world.add_component(
        engine::ecs::component::DirectionalLightComponent::new()
            .with_color(1.0, 1.0, 1.0)
            .with_intensity(0.35),
    );
    let _ = universe.attach(light_tx, light);
    universe.add(light_tx);

    // --- Background clouds (occluded + lit) ---
    // Mirrors the vtuber example: use a BackgroundComponent stage so the cloud volume
    // self-occludes and is lit, but renders as background.
    let bg_root = universe.world.add_component(
        engine::ecs::component::BackgroundComponent::new().with_occlusion_and_lighting(),
    );
    universe.add(bg_root);
    let mut cloud_params = example_util::CloudRingParams::default();
    cloud_params.cloud_count = 7;
    cloud_params.radius = 22.0;
    // Move the ring up by ~one cloud height. The cloud generator uses a ~4.0 unit
    // vertical spread for puff offsets, so +4.0 is a good "one height" bump.
    cloud_params.center_y = 6.0;
    cloud_params.puffs_per_cloud = 26;
    cloud_params.angle_jitter = 0.0;
    cloud_params.high_y_probability = 0.0;
    cloud_params.high_y_multiplier = 1.0;
    cloud_params.seed = 0xA0_D1_0C_01u32;
    example_util::spawn_cloud_ring(&mut universe, bg_root, cloud_params);

    // ClockComponent sets global tempo.
    if audio_enabled {
        let clock = universe
            .world
            .add_component(engine::ecs::component::ClockComponent::new().with_bpm(128.0));
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
        let audio_out = universe.world.add_component(audio_out_comp);
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

    // Track A: square lead.
    let osc_a_comp =
        if audio_enabled {
            let osc_a = engine::ecs::component::AudioOscillator::square()
                .with_frequency(110.0)
                .with_amplitude(0.12)
                .with_enabled(false);
            let osc_a_comp = universe.world.add_component(
                engine::ecs::component::AudioOscillatorComponent::single(osc_a),
            );
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

    // Effect tree A (single chain):
    // osc_a
    //   Gain
    //     BandPass
    //       Limiter
    let mut bp_a_comp: Option<engine::ecs::ComponentId> = None;
    if let Some(osc_a_comp) = osc_a_comp {
        let gain_a = universe
            .world
            .add_component(engine::ecs::component::AudioGainComponent::new(3.2));
        let bp_a = universe.world.add_component(
            engine::ecs::component::AudioBandPassFilterComponent::new(120.0, 3.0, 0.40),
        );
        bp_a_comp = Some(bp_a);
        let lim_a =
            universe
                .world
                .add_component(engine::ecs::component::AudioLimiterComponent::new(
                    4.0, 80.0, 0.90,
                ));

        let _ = universe.attach(osc_a_comp, gain_a);
        let _ = universe.attach(gain_a, bp_a);
        let _ = universe.attach(bp_a, lim_a);
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
        let tx = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(pos.0, pos.1, pos.2)
                .with_scale(scale, scale, 1.0),
        );
        let t =
            universe
                .world
                .add_component(engine::ecs::component::TextComponent::with_word_wrap(
                    text, wrap_cols,
                ));
        let _ = universe.attach(tx, t);

        // TextSystem looks for an immediate TextureFilteringComponent child.
        let filtering = universe
            .world
            .add_component(engine::ecs::component::TextureFilteringComponent::nearest());
        let _ = universe.attach(t, filtering);

        // TextSystem also supports styling from immediate Emissive children.
        let emissive = universe
            .world
            .add_component(engine::ecs::component::EmissiveComponent::on());
        let _ = universe.attach(t, emissive);

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

    // --- HUD / lane (single track) ---
    let lane_x = -2.6_f32;
    let lane_title_z = -0.4_f32;
    let lane_cfg_z = -0.4_f32;
    let lane_pat_z = -1.3_f32;
    let lane_graph_z = -1.15_f32;

    // Content for pattern/chain/graph starts at x=0; keep section labels aligned there.
    // This also keeps them from overlapping the config block (which is anchored at lane_x).
    let lane_labels_x = lane_x + 1.6_f32;

    let track_a_y = 0.9_f32;

    spawn_text(
        &mut universe,
        (lane_x, track_a_y + 0.55, lane_title_z),
        0.09,
        42,
        "Track A: AudioOscillator::square()",
    );
    spawn_text(
        &mut universe,
        (lane_x, track_a_y + 0.25, lane_cfg_z),
        0.07,
        48,
        "oscillators=1\nfrequency_hz=110.0\namplitude=0.12\nenabled=false\nlookahead=0.10s",
    );

    let viz_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 0.0),
    );
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
    let track_a_pattern_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 0.0),
    );
    let _ = universe.attach(viz_root, track_a_pattern_root);

    // Labels for pattern/chain/graph sections.
    spawn_text(
        &mut universe,
        (lane_labels_x, track_a_y, lane_title_z),
        0.06,
        42,
        "pattern",
    );
    spawn_text(
        &mut universe,
        (lane_labels_x, track_a_y - 0.40, lane_title_z),
        0.06,
        42,
        "graph",
    );

    // --- Processing chain + graph visualization (compiled graph → cubes + labels) ---
    use engine::ecs::system::audio_graph_compiler::{
        AudioGraphCompiler, AudioGraphNode, AudioGraphNodeKind,
    };

    fn effect_grey_for_depth(depth: usize) -> f32 {
        // depth=1 => medium grey; deeper => lighter, capped.
        let base = 0.55;
        let step = 0.13;
        let d = depth.saturating_sub(1) as f32;
        (base + step * d).min(0.92)
    }

    fn node_rgba(node: &AudioGraphNode, depth: usize) -> [f32; 4] {
        match node.kind {
            AudioGraphNodeKind::OscillatorSource { .. } => [1.0, 0.78, 0.22, 1.0],
            _ => {
                let g = effect_grey_for_depth(depth);
                [g, g, g, 1.0]
            }
        }
    }

    fn node_label(node: &AudioGraphNode) -> String {
        match &node.kind {
            AudioGraphNodeKind::OscillatorSource { voices } => {
                format!("OscillatorSource voices={voices}")
            }
            AudioGraphNodeKind::Gain { gain } => {
                format!("Gain gain={gain:.3}")
            }
            AudioGraphNodeKind::LowPass {
                cutoff_hz,
                resonance,
            } => {
                format!("LowPass cutoff={cutoff_hz:.1}Hz res={resonance:.3}")
            }
            AudioGraphNodeKind::BandPass {
                center_hz,
                bandwidth_octaves,
                resonance,
            } => {
                format!(
                    "BandPass center={center_hz:.1}Hz bw={bandwidth_octaves:.3}oct res={resonance:.3}"
                )
            }
            AudioGraphNodeKind::HighPass {
                cutoff_hz,
                resonance,
            } => {
                format!("HighPass cutoff={cutoff_hz:.1}Hz res={resonance:.3}")
            }
            AudioGraphNodeKind::Limiter {
                attack_ms,
                release_ms,
                threshold,
            } => {
                format!("Limiter atk={attack_ms:.1}ms rel={release_ms:.1}ms thr={threshold:.3}")
            }
            AudioGraphNodeKind::ClipSource => "ClipSource".to_string(),
        }
    }

    fn compute_layout(
        node: &AudioGraphNode,
        depth: usize,
        x_cursor: &mut i32,
        out: &mut std::collections::HashMap<*const AudioGraphNode, (i32, usize)>,
    ) -> i32 {
        // Depth-only layout:
        // - keep children directly below their parent on X
        // - use Z sibling offsets (in spawn_graph_tree) to show branching
        let _ = x_cursor;
        for ch in node.children.iter() {
            let _ = compute_layout(ch, depth + 1, x_cursor, out);
        }

        let my_x = 0;
        out.insert(node as *const AudioGraphNode, (my_x, depth));
        my_x
    }

    fn spawn_graph_tree(
        universe: &mut engine::Universe,
        parent: engine::ecs::ComponentId,
        node: &AudioGraphNode,
        bp_component: Option<engine::ecs::ComponentId>,
        bp_label_out: &mut Option<engine::ecs::ComponentId>,
        origin: (f32, f32, f32),
        layout: &std::collections::HashMap<*const AudioGraphNode, (i32, usize)>,
        dx: f32,
        dy: f32,
        cube_scale: f32,
        sibling_index: usize,
        sibling_count: usize,
    ) {
        let Some((x_unit, depth)) = layout.get(&(node as *const AudioGraphNode)).copied() else {
            return;
        };

        let x = origin.0 + (x_unit as f32) * dx;
        let y = origin.1 - (depth as f32) * dy;

        // Push siblings “behind” each other along Z to reduce overlap between
        // a sibling's cube and another node's label.
        let dz_sibling = 0.18;
        let z = origin.2 - (sibling_index as f32) * dz_sibling;

        // Keep text slightly in front of its cube.
        let z_text = z + 0.03;

        let rgba = node_rgba(node, depth);
        let cube_tx = spawn_op_cube(universe, parent, (x, y, z), cube_scale, rgba);
        let _ = cube_tx;

        // Label next to the cube.
        let label = node_label(node);
        let tx = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(x + 0.14, y + 0.02, z_text)
                .with_scale(0.06, 0.06, 1.0),
        );
        let t =
            universe
                .world
                .add_component(engine::ecs::component::TextComponent::with_word_wrap(
                    &label, 25,
                ));
        let _ = universe.attach(parent, tx);
        let _ = universe.attach(tx, t);

        if bp_component == Some(node.component) {
            *bp_label_out = Some(t);
        }

        let filtering = universe
            .world
            .add_component(engine::ecs::component::TextureFilteringComponent::nearest());
        let _ = universe.attach(t, filtering);

        let emissive = universe
            .world
            .add_component(engine::ecs::component::EmissiveComponent::on());
        let _ = universe.attach(t, emissive);

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
            let mix_label = if node.mix.is_some() {
                format!("Mix weights={weights:?}")
            } else {
                format!("Mix <implicit> w={weights:?}")
            };
            let mix_tx = universe.world.add_component(
                engine::ecs::component::TransformComponent::new()
                    .with_position(x + 0.14, y - 0.11, z_text)
                    .with_scale(0.05, 0.05, 1.0),
            );
            let mix_t = universe.world.add_component(
                engine::ecs::component::TextComponent::with_word_wrap(&mix_label, 25),
            );
            let _ = universe.attach(parent, mix_tx);
            let _ = universe.attach(mix_tx, mix_t);

            let filtering = universe
                .world
                .add_component(engine::ecs::component::TextureFilteringComponent::nearest());
            let _ = universe.attach(mix_t, filtering);

            let emissive = universe
                .world
                .add_component(engine::ecs::component::EmissiveComponent::on());
            let _ = universe.attach(mix_t, emissive);

            universe.add(mix_tx);
        }

        let child_count = node.children.len().max(1);
        for (i, ch) in node.children.iter().enumerate() {
            // Each node's children form a sibling group; offset them in Z.
            // For the root node, sibling_index is 0.
            let _ = sibling_count;
            spawn_graph_tree(
                universe,
                parent,
                ch,
                bp_component,
                bp_label_out,
                origin,
                layout,
                dx,
                dy,
                cube_scale,
                i,
                child_count,
            );
        }
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

    // Full compiled graph visualization (tree layout + mix labels).
    let mut bp_graph_label_text: Option<engine::ecs::ComponentId> = None;
    if let Some(compiled_a) = &compiled_a {
        let mut x_cursor = 0;
        let mut layout: std::collections::HashMap<*const AudioGraphNode, (i32, usize)> =
            std::collections::HashMap::new();
        let _root_x = compute_layout(&compiled_a.root, 0, &mut x_cursor, &mut layout);

        // With depth-only X layout, we keep the tree centered at x=0.
        // Pull the graph up now that we don't show the separate chain view.
        let origin = (0.0, track_a_y - 0.40, lane_graph_z);

        spawn_graph_tree(
            &mut universe,
            viz_root,
            &compiled_a.root,
            bp_a_comp,
            &mut bp_graph_label_text,
            origin,
            &layout,
            0.45,
            0.28,
            0.08,
            0,
            1,
        );
    } else {
        spawn_text(
            &mut universe,
            (0.0, track_a_y - 0.40, lane_graph_z),
            0.06,
            60,
            "(audio disabled)",
        );
    }

    // Animation with 16 keyframes drives scheduled notes + pattern highlights.
    let anim = universe
        .world
        .add_component(engine::ecs::component::AnimationComponent::new());

    let dur_a = 0.85_f32;
    let beat_spacing = 0.38_f32;

    let a_dark = [0.18, 0.08, 0.22, 1.0];
    let a_bright = [0.85, 0.40, 1.00, 1.0];

    for i in 0..16 {
        let kf_beat = i as f64;
        let kf = universe
            .world
            .add_component(engine::ecs::component::KeyframeComponent::new(kf_beat));

        let _ = universe.attach(anim, kf);

        // Scheduled audio note (sample-accurate via lookahead).
        // Pulse every beat.
        if audio_enabled {
            let Some(osc_a_comp) = osc_a_comp else {
                panic!("audio_enabled but track A was not created");
            };

            let note_a = engine::ecs::component::MusicNote::c(1, dur_a).with_velocity(0.80);

            let act_a = universe
                .world
                .add_component(engine::ecs::component::ActionComponent::new(
                    engine::ecs::IntentValue::AudioSchedulePlay {
                        component_ids: vec![osc_a_comp],
                        beat_offset: 0.0,
                        beat_context: None,
                        note: Some(note_a),
                        gain: None,
                        rate: None,
                        duration: None,
                    },
                ));

            let _ = universe.attach(kf, act_a);
        }

        // Keyframed band-pass center.
        // This is an immediate parameter update applied RT-side (no graph rebuild).
        if audio_enabled {
            if let Some(bp_a_comp) = bp_a_comp {
                let t = (i as f32) / 15.0;
                let center_hz = 10.0 + t * (1000.0 - 10.0);

                let bp_center =
                    universe
                        .world
                        .add_component(engine::ecs::component::ActionComponent::new(
                            engine::ecs::IntentValue::AudioBandPassSetCenterHz {
                                component_ids: vec![bp_a_comp],
                                center_hz,
                            },
                        ));
                let _ = universe.attach(kf, bp_center);

                // Update the BandPass node label in the graph visualization.
                if let Some(text_id) = bp_graph_label_text {
                    let label =
                        universe
                            .world
                            .add_component(engine::ecs::component::ActionComponent::new(
                                engine::ecs::IntentValue::SetText {
                                    component_ids: vec![text_id],
                                    text: format!("BandPass center={center_hz:.1}Hz"),
                                },
                            ));
                    let _ = universe.attach(kf, label);
                }
            }
        }

        // Visualization reset per keyframe.
        let reset_a = universe
            .world
            .add_component(engine::ecs::component::ActionComponent::new(
                engine::ecs::IntentValue::SetColor {
                    component_ids: vec![track_a_pattern_root],
                    rgba: a_dark,
                },
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
            .add_component(engine::ecs::component::ActionComponent::new(
                engine::ecs::IntentValue::SetColor {
                    component_ids: vec![cube_a],
                    rgba: a_bright,
                },
            ));
        let _ = universe.attach(kf, bright_a);
    }
    universe.add(anim);

    println!("[audio-graph-example] animation created");

    // Keep window open.
    println!("[audio-graph-example] processing commands...");
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    println!("[audio-graph-example] commands processed; launching window");

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
