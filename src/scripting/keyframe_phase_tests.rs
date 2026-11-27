//! Keyframe callbacks change script state once per firing (the audio pass
//! only runs audio statements), and a keyframe just inside a looping clip's
//! end fires before the wrap.
use super::rocket_motion_tests::{TestClock, advance};
use crate::engine::Universe;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::World;
use std::sync::Arc;

fn load(source: &str) -> (Universe, Arc<TestClock>) {
    let clock = Arc::new(TestClock::default());
    let mut scene = Universe::new(World::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(source, "examples/_keyframe_phase_test.mms")
        .unwrap();
    (scene, clock)
}

fn count(scene: &Universe) -> f32 {
    let marker = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("keyframe_count"))
        .expect("marker");
    TransformSystem::world_position(&scene.world, marker).unwrap()[0]
}

// The marker's x is how many times the keyframe's state change ran. A call
// to a script function makes the body's effects unknown, which used to run
// the whole body in both passes.
const COUNTER: &str = r#"
    Clock.bpm(120.0)
    let marker = T { name = "keyframe_count" }
    marker
    let state = { count = 0.0 }
    fn bump() {
        state.count = state.count + 1.0
        marker.update_transform([state.count, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    }
"#;

#[test]
fn keyframe_calling_a_function_changes_state_once() {
    let (mut scene, clock) = load(&format!(
        "{COUNTER}
        Animation.looping().length(4.0) {{
            Keyframe.at(0.0) {{ bump() }}
        }}"
    ));
    for _ in 0..30 {
        advance(&mut scene, &clock);
    }
    assert_eq!(count(&scene), 1.0, "one firing, one state change");
}

#[test]
fn keyframe_just_inside_a_loop_end_fires_before_the_wrap() {
    // Frames are 1/60 beat apart, so the keyframe almost always falls
    // between the last frame of a cycle and the wrap.
    let (mut scene, clock) = load(&format!(
        "{COUNTER}
        Animation.looping().length(2.0) {{
            Keyframe.at(0.0) {{}}
            Keyframe.at(1.999) {{ bump() }}
        }}"
    ));
    for _ in 0..100 {
        advance(&mut scene, &clock);
    }
    assert_eq!(count(&scene), 0.0, "not reached yet");
    for _ in 0..40 {
        advance(&mut scene, &clock);
    }
    assert_eq!(count(&scene), 1.0, "fired once as the cycle ended");
}
