use super::rocket_motion_tests::{TestClock, advance as advance_clock};
use crate::engine::Universe;
use crate::engine::ecs::component::{
    AnimationComponent, AnimationState, ColorComponent, TransformComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::external_xr_frame::ExternalXrFrame;
use crate::engine::ecs::{ComponentId, EventSignal, KeyboardEvent, SignalEmitter, World};
use crate::engine::graphics::CameraTarget;
use crate::utils::math::mat4_mul_vec4;
use std::sync::Arc;

fn load(source: &str) -> Universe {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(source, "examples/_health_bar_test.mms")
        .unwrap();
    advance(&mut scene, 3);
    scene
}

fn advance(scene: &mut Universe, frames: usize) {
    for _ in 0..frames {
        scene.update(1.0 / 60.0, &Default::default());
    }
}

fn release(scene: &mut Universe, code: &str) {
    scene.command_queue.push_event(
        ComponentId::default(),
        EventSignal::KeyUp(KeyboardEvent {
            code: Some(code.into()),
            key: String::new(),
        }),
    );
}

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

#[test]
fn health_bar_anchors_fill_and_flashes_only_below_15_percent() {
    let mut scene = load(
        r#"
        import { health_bar } from "../assets/components/health_bar.mms"
        Clock.bpm(120.0)
        let bar = health_bar(Text { name = "custom_title" "SHIELDS" })
        let second = health_bar(null)
        T { name = "first_meter" bar }
        T { name = "second_meter" second }
        on_global("KeyUp", fn(event) {
            if event.code == "KeyA" { bar.set_health(0.15) }
            if event.code == "KeyB" { bar.set_health(0.10) }
            if event.code == "KeyC" { bar.set_health(-0.20) }
            if event.code == "KeyD" { bar.set_health(1.20) }
        })
        "#,
    );
    let first = named(&scene, "first_meter");
    let second = named(&scene, "second_meter");
    let bar = scene.world.find_component(first, "#health_bar").unwrap();
    assert!(scene.world.find_component(bar, "#custom_title").is_some());
    let fill = scene.world.find_component(bar, "#health_bar_fill").unwrap();
    let color = scene.world.find_component(fill, "color").unwrap();
    let pulse = scene
        .world
        .find_component(bar, "#health_bar_pulse")
        .unwrap();
    let second_pulse = scene
        .world
        .find_component(second, "#health_bar_pulse")
        .unwrap();
    let rgba = |scene: &Universe| {
        scene
            .world
            .get_component_by_id_as::<ColorComponent>(color)
            .unwrap()
            .rgba
    };
    let state = |scene: &Universe, id| {
        scene
            .world
            .get_component_by_id_as::<AnimationComponent>(id)
            .unwrap()
            .state
    };
    let check_fill = |scene: &Universe, fraction: f32| {
        let fill = scene
            .world
            .get_component_by_id_as::<TransformComponent>(fill)
            .unwrap();
        let width = fill.scale()[0];
        assert!((width - 9.6 * fraction).abs() < 1e-5);
        assert!((fill.translation()[0] - width / 2.0 + 4.8).abs() < 1e-5);
    };
    let pink = rgba(&scene);
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    let tick = |scene: &mut Universe, frames| {
        for _ in 0..frames {
            advance_clock(scene, &clock);
        }
    };
    check_fill(&scene, 1.0);
    release(&mut scene, "KeyA");
    tick(&mut scene, 3);
    check_fill(&scene, 0.15);
    assert_eq!(state(&scene, pulse), AnimationState::Paused);
    assert_eq!(rgba(&scene), pink);

    let mut saw_red = false;
    let mut saw_black = false;
    for _ in 0..20 {
        release(&mut scene, "KeyB");
        tick(&mut scene, 3);
        saw_red |= rgba(&scene) == [1.0, 0.025, 0.055, 1.0];
        saw_black |= rgba(&scene) == [0.0, 0.0, 0.0, 1.0];
    }
    assert!(saw_red && saw_black);
    check_fill(&scene, 0.10);
    assert_eq!(state(&scene, pulse), AnimationState::Looping);
    assert_eq!(state(&scene, second_pulse), AnimationState::Paused);

    release(&mut scene, "KeyC");
    tick(&mut scene, 3);
    check_fill(&scene, 0.0);
    release(&mut scene, "KeyD");
    tick(&mut scene, 70);
    check_fill(&scene, 1.0);
    assert_eq!(state(&scene, pulse), AnimationState::Paused);
    assert_eq!(rgba(&scene), pink);
}

#[test]
fn hud_lags_a_view_turn_and_settles_in_front_at_world_scale() {
    let mut scene = load(
        r#"
        import { hud } from "../assets/components/hud.mms"
        let view = T.scale(2.0, 2.0, 2.0) {
            hud("test_hud", [0.0, 0.34, -1.10], T { R.cube() {} })
        }
        view
        on_global("KeyUp", fn(event) {
            view.update_transform([0, 0, 0], [0, Math.pi / 6.0, 0], [2, 2, 2])
        })
        "#,
    );
    let spring = named(&scene, "test_hud_spring");
    assert!(
        scene
            .systems
            .secondary_motion
            .chain_anchor(spring)
            .is_some()
    );
    let display = named(&scene, "test_hud_display");
    let position =
        |scene: &Universe| TransformSystem::world_position(&scene.world, display).unwrap();
    assert_eq!(position(&scene), [0.0, 0.34, -1.10]);

    release(&mut scene, "KeyA");
    advance(&mut scene, 3);
    let turning_x = position(&scene)[0];
    assert!((-0.50..0.0).contains(&turning_x), "{turning_x}");

    advance(&mut scene, 70);
    let [x, y, z] = position(&scene);
    assert!((x + 0.55).abs() < 1e-4, "{x}");
    assert!((y - 0.34).abs() < 1e-4, "{y}");
    assert!(
        (z + 1.10 * (std::f32::consts::PI / 6.0).cos()).abs() < 1e-4,
        "{z}"
    );
}

#[test]
fn rooftop_health_hud_projects_above_center_in_desktop_and_xr() {
    let source = r#"
        import { make_rooftop_player } from "../assets/components/characters/rooftop_player.mms"
        let xr = __XR__
        let mic = AudioInput { enabled(false) }
        let voice = Amplitude.rolling_window(0.08).from(mic) {}
        let player = make_rooftop_player({
            prefix = "local" local_player = true avatar = "mitten"
            use_xr = xr use_desktop = !xr show_health = true
            voice_level = voice mouth_tuning = { center_rms = 0.0475 range_rms = 0.085 amount = 1.0 }
        })
        if xr { player.locomotion XR.on() } else { player.root }
    "#;
    for (xr, target) in [(false, CameraTarget::Window), (true, CameraTarget::Xr)] {
        let mut scene = Universe::new(World::default());
        if xr {
            let views = [-0.032, 0.032].map(|x| openxr::View {
                pose: openxr::Posef {
                    position: openxr::Vector3f { x, y: 1.62, z: 0.0 },
                    orientation: openxr::Quaternionf {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                        w: 1.0,
                    },
                },
                fov: openxr::Fovf {
                    angle_left: -0.7,
                    angle_right: 0.7,
                    angle_up: 0.7,
                    angle_down: -0.7,
                },
            });
            scene.systems.xr.set_external_frame(Some(ExternalXrFrame {
                views: Some(views),
                ..Default::default()
            }));
        }
        scene
            .load_mms_source_at_path(
                &source.replace("__XR__", if xr { "true" } else { "false" }),
                "examples/_health_bar_test.mms",
            )
            .unwrap();
        advance(&mut scene, 30);
        let model =
            TransformSystem::world_model(&scene.world, named(&scene, "health_bar")).unwrap();
        let camera = scene.visuals.visual_camera(target).unwrap();
        for eye in &camera.eyes {
            for point in [[-5.0, -0.5, 0.0, 1.0], [5.0, 1.32, 0.0, 1.0]] {
                let world = mat4_mul_vec4(model, point);
                let view = mat4_mul_vec4(eye.view, world);
                let clip = mat4_mul_vec4(eye.proj, view);
                assert!(view[1] > 0.0 && clip[3] > 0.5, "{target:?}: {view:?}");
                assert!(
                    clip[0].abs() < clip[3] && clip[1].abs() < clip[3],
                    "{target:?}: {clip:?}"
                );
            }
        }
    }
}
