use cat_engine::{engine, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Minimal scene with a camera so the window opens.
    let clear = universe
        .world
        .register(engine::ecs::component::BackgroundColorComponent::rgba(
            0.05, 0.05, 0.08, 1.0,
        ));
    universe.add(clear);

    let rig_transform = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 5.0));
    let camera3d = universe
        .world
        .register(engine::ecs::component::Camera3DComponent::new().with_far(250.0).with_fov(70.0));
    let _ = universe.attach(rig_transform, camera3d);
    universe.add(rig_transform);

    // ClockComponent sets global tempo.
    let clock = universe
        .world
        .register(engine::ecs::component::ClockComponent::new().with_bpm(128.0));
    universe.add(clock);

    // Audio output + oscillators driven by scheduled actions.
    let audio_out = universe
        .world
        .register(engine::ecs::component::AudioOutputComponent::new());
    universe.add(audio_out);

    // Noise oscillator: short white-noise hits.
    let osc_noise = engine::ecs::component::AudioOscillator {
        oscillator_type: engine::ecs::component::OscillatorType::Noise,
        frequency: 0.0,
        amplitude: 0.06,
        enabled: false,
        music_note_applied: false,
    };
    let osc_noise_comp = universe
        .world
        .register(engine::ecs::component::AudioOscillatorComponent::single(osc_noise));
    let _ = universe.attach(audio_out, osc_noise_comp);

    // Drum oscillator: retriggers (phase + sweep) every enable.
    let osc_drum = engine::ecs::component::AudioOscillator {
        oscillator_type: engine::ecs::component::OscillatorType::Drum,
        // A low-ish pitch scale; actual pitch is set by scheduled notes.
        frequency: 32.0,
        amplitude: 0.40,
        enabled: false,
        music_note_applied: false,
    };
    let osc_drum_comp = universe
        .world
        .register(engine::ecs::component::AudioOscillatorComponent::single(osc_drum));

    let _ = universe.attach(audio_out, osc_drum_comp);

    // Animation with 16 keyframes: each keyframe schedules a kick at offset 0.0,
    // and a noise hit at offset 0.5.
    let anim = universe
        .world
        .register(engine::ecs::component::AnimationComponent::new());

    let kick_dur = 0.12_f32;
    let noise_dur = 0.06_f32;

    for i in 0..16 {
        let kf_beat = i as f64;
        let kf = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(kf_beat));

        let kick = universe
            .world
            .register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::oscillator_schedule_music_note(
                    vec![osc_drum_comp],
                    0.0,
                    engine::ecs::component::MusicNote::c(0, kick_dur)
                                                            .with_velocity(0.9),
                ),
            ));

        let noise = universe
            .world
            .register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::oscillator_schedule_music_note(
                    vec![osc_noise_comp],
                    0.5,
                    engine::ecs::component::MusicNote::c(9, noise_dur)
                                                            .with_velocity(0.25),
                ),
            ));

        let _ = universe.attach(anim, kf);
        let _ = universe.attach(kf, kick);
        let _ = universe.attach(kf, noise);
    }

    universe.add(anim);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
