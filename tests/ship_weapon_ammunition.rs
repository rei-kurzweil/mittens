use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, KeyboardEvent, SignalEmitter, SignalKind, World,
        component::{AnimationComponent, AnimationState, TextComponent},
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

#[derive(Default)]
struct TestClock(AtomicU64);
impl mittens_engine::engine::ecs::system::clock_system::ClockDriver for TestClock {
    fn name(&self) -> &'static str {
        "ammunition_test"
    }
    fn time_now_sec(&self) -> f64 {
        self.0.load(Ordering::Relaxed) as f64 / 1000.0
    }
}
fn advance(scene: &mut Universe, clock: &TestClock) {
    clock.0.fetch_add(20, Ordering::Relaxed);
    scene.update(0.02, &Default::default());
}

fn named(world: &World, name: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn count_shots(scene: &mut Universe, name: &str) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let recorded = count.clone();
    scene.systems.rx.add_handler_closure(SignalKind::DataEvent,
        named(&scene.world, name), move |_, _, signal| {
            if matches!(&signal.event, Some(EventSignal::DataEvent {name,..}) if name == "ShotFired") {
                recorded.fetch_add(1, Ordering::Relaxed);
            }
        });
    count
}

fn finish_shaders(scene: &Universe) {
    let definitions: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|id| {
            let registry = &scene.visuals.materials;
            registry
                .definition(registry.source_definition(id)?)?
                .custom
                .clone()
        })
        .collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while definitions.iter().any(|d| d.compiled.get().is_none()) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn key(scene: &mut Universe, down: bool) {
    let event = KeyboardEvent {
        code: Some("Space".into()),
        key: " ".into(),
    };
    scene.command_queue.push_event(
        ComponentId::default(),
        if down {
            EventSignal::KeyDown(event)
        } else {
            EventSignal::KeyUp(event)
        },
    );
}

fn click(scene: &mut Universe, name: &str) {
    let target = named(&scene.world, name);
    scene.command_queue.push_event(
        target,
        EventSignal::Click {
            raycaster: ComponentId::default(),
            renderable: target,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
}

fn grab(scene: &mut Universe, name: &str) {
    let target = named(&scene.world, name);
    scene.command_queue.push_event(
        target,
        EventSignal::GrabStart {
            pointer: ComponentId::default(),
            raycaster: ComponentId::default(),
            renderable: target,
            target,
            ray_origin_world: [0.0; 3],
            ray_dir_world: [0.0, 0.0, -1.0],
        },
    );
}

fn text<'a>(scene: &'a Universe, label: &str) -> &'a str {
    &scene
        .world
        .get_component_by_id_as::<TextComponent>(named(&scene.world, label))
        .unwrap()
        .text
}

#[test]
fn ship_pistol_stops_at_75_and_throttles_its_counter() {
    let mut scene = Universe::new(World::default());
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(
            r#"
        import { ship_input_controller } from "../assets/components/ship_input_controller.mms"
        import { ship_machine_pistol } from "../assets/components/weapons/ship_machine_pistol.mms"
        ship_input_controller(false)
        ship_machine_pistol("ship_rack_pistol_0")
        ship_machine_pistol("ship_rack_pistol_1")
    "#,
            "examples/_ship_ammunition_test.mms",
        )
        .unwrap();
    finish_shaders(&scene);
    let shots = count_shots(&mut scene, "ship_rack_pistol_0_laser_completion");
    let other_shots = count_shots(&mut scene, "ship_rack_pistol_1_laser_completion");
    let counter = "ship_rack_pistol_0_ammo_count";
    assert_eq!(text(&scene, counter), "75");
    grab(&mut scene, "ship_rack_pistol_0_grip");
    click(&mut scene, "ship_rack_pistol_0_laser_mode_auto");
    advance(&mut scene, &clock);
    key(&mut scene, true);
    let mut previous = "75".to_string();
    let mut updates = Vec::new();
    for step in 1..=500 {
        advance(&mut scene, &clock);
        let displayed = text(&scene, counter);
        if displayed != previous {
            updates.push(step as f64 * 0.02);
            previous = displayed.into();
        }
    }
    assert_eq!(
        shots.load(Ordering::Relaxed),
        75,
        "a held automatic trigger must stop at 75"
    );
    assert_eq!(other_shots.load(Ordering::Relaxed), 0);
    assert_eq!(
        text(&scene, counter),
        "0",
        "the delayed counter must eventually show empty"
    );
    assert_eq!(text(&scene, "ship_rack_pistol_1_ammo_count"), "75");
    assert!(!updates.is_empty() && updates[0] >= 1.0,"counter update times: {updates:?}");
    for pair in updates.windows(2) {
        assert!(
            pair[1] - pair[0] >= 1.0 - 1e-6,
            "counter updates must be at least one second apart: {updates:?}"
        );
    }
    let auto = scene
        .world
        .get_component_by_id_as::<AnimationComponent>(named(
            &scene.world,
            "ship_rack_pistol_0_laser_auto",
        ))
        .unwrap();
    assert_eq!(
        auto.state,
        AnimationState::Paused,
        "empty auto fire must stop its animation"
    );
    for mode in ["semi", "burst", "auto"] {
        key(&mut scene, false);
        advance(&mut scene, &clock);
        click(&mut scene, &format!("ship_rack_pistol_0_laser_mode_{mode}"));
        key(&mut scene, true);
        for _ in 0..60 {
            advance(&mut scene, &clock);
        }
    }
    assert_eq!(
        shots.load(Ordering::Relaxed),
        75,
        "release/retrigger or changing modes must not refill ammo"
    );
    key(&mut scene, false);
    let grip = named(&scene.world, "ship_rack_pistol_0_grip");
    scene.command_queue.push_event(
        grip,
        EventSignal::GrabEnd {
            pointer: ComponentId::default(),
            target: grip,
        },
    );
    grab(&mut scene, "ship_rack_pistol_1_grip");
    advance(&mut scene, &clock);
    key(&mut scene, true);
    for _ in 0..10 {
        advance(&mut scene, &clock);
    }
    key(&mut scene, false);
    for _ in 0..60 {
        advance(&mut scene, &clock);
    }
    assert_eq!(other_shots.load(Ordering::Relaxed), 1);
    assert_eq!(
        text(&scene, "ship_rack_pistol_1_ammo_count"),
        "74",
        "a single shot must refresh after release, without another shot to flush it"
    );
}

#[test]
fn a_partial_burst_spends_only_the_remaining_rounds() {
    let mut scene = Universe::new(World::default());
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(
            r#"
        import { laser_machine_pistol } from "../assets/components/laser-machine-pistol.mms"
        let tool = laser_machine_pistol({name="partial" ammunition=2})
        let empty = laser_machine_pistol({name="empty" ammunition=0})
        let control = T {name="ammo_test_control"}
        tool.set_mode("burst")
        empty.set_mode("burst")
        T { tool.root empty.root control for projectile in tool.projectiles {projectile} }
        on(control,"DataEvent",fn(event) {
            tool.use_end()
            tool.use_start({user=null})
            empty.use_start({user=null})
        })
    "#,
            "examples/_partial_ammunition_test.mms",
        )
        .unwrap();
    finish_shaders(&scene);
    let shots = count_shots(&mut scene, "partial_completion");
    let empty_shots = count_shots(&mut scene, "empty_completion");
    for _ in 0..3 {
        scene.command_queue.push_event(
            named(&scene.world, "ammo_test_control"),
            EventSignal::DataEvent {
                name: "Fire".into(),
                payload: None,
            },
        );
        for _ in 0..60 {
            advance(&mut scene, &clock);
        }
    }
    assert_eq!(
        shots.load(Ordering::Relaxed),
        2,
        "the third burst keyframe must not launch a projectile"
    );
    assert_eq!(empty_shots.load(Ordering::Relaxed), 0);
}
