use super::rocket_motion_tests::{TestClock, Uploader, advance};
use crate::engine::Universe;
use crate::engine::ecs::component::{GLTFComponent, TextComponent, TransformComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};
use std::sync::Arc;

fn named(scene: &Universe, label: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}

fn has_label(scene: &Universe, label: &str) -> bool {
    scene
        .world
        .all_components()
        .any(|id| scene.world.component_label(id) == Some(label))
}

fn load(source: &str, path: &str) -> (Universe, Arc<TestClock>) {
    let clock = Arc::new(TestClock::default());
    let mut scene = Universe::new(World::default());
    scene.systems.clock.set_driver(clock.clone());
    scene.load_mms_source_at_path(source, path).unwrap();
    // A few render preparations import and spawn the avatars' meshes, so
    // pistol shots meet the real (tagged) body rather than only the proxy.
    let mut uploader = Uploader::default();
    for _ in 0..4 {
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut uploader,
            &mut scene.command_queue,
        );
        advance(&mut scene, &clock);
    }
    (scene, clock)
}

fn step(scene: &mut Universe, clock: &TestClock, frames: usize) {
    for _ in 0..frames {
        advance(scene, clock);
    }
}

fn event(scene: &mut Universe, name: &str) {
    scene.command_queue.push_event(
        named(scene, "practice_test_events"),
        EventSignal::DataEvent {
            name: name.into(),
            payload: None,
        },
    );
}

fn fill(scene: &Universe, owner: &str) -> f32 {
    let meter = scene
        .world
        .find_component(named(scene, owner), "#health_bar_fill")
        .unwrap();
    scene
        .world
        .get_component_by_id_as::<TransformComponent>(meter)
        .unwrap()
        .scale()[0]
        / 9.6
}

fn report(scene: &Universe) -> String {
    scene
        .world
        .get_component_by_id_as::<TextComponent>(named(scene, "practice_report"))
        .unwrap()
        .text
        .clone()
}

#[test]
fn practice_target_shield_takes_pistol_hits_and_splash_takes_health() {
    let source = r#"
        import { practice_target } from "../assets/components/target_practice.mms"
        import { laser } from "../assets/components/laser.mms"
        import { apply_machine_pistol_impact } from "../assets/components/weapons/ship_machine_pistol.mms"
        let target = practice_target({ prefix = "npc" avatar = "mitten" use_xr = false use_desktop = false
            position = [0.0, 0.0, -3.0] yaw = 0.0 strafe = 0.0 })
        target.root
        let report = Text { name = "practice_report" "" }
        report
        let caster = laser({ name = "practice_test_laser" length = 10.0 })
        let muzzle = T.position(0.0, 1.0, 0.0) {}
        muzzle
        let events = T { name = "practice_test_events" }
        events
        on(events, "DataEvent", fn(event) {
            // As ship_machine_pistol's impact: the hit effect, then the rules.
            if event == "Pistol" {
                caster.launch({ pose = muzzle.world_trs() on_hit = fn(hit) {
                    apply_machine_pistol_impact(hit.hit_point, hit.damage)
                    target.pistol_hit(hit)
                } })
            }
            if event == "Splash" { target.rocket_splash(target.center()) }
            if event == "OuterSplash" {
                let c = target.center()
                target.rocket_splash([c[0] + 4.0, c[1], c[2]])
            }
            if event == "FarSplash" {
                let c = target.center()
                target.rocket_splash([c[0] + 6.5, c[1], c[2]])
            }
            if event == "Report" { report.set_text("" + target.shield_integrity()) }
        })
    "#;
    let (mut scene, clock) = load(source, "examples/_target_practice_test.mms");
    step(&mut scene, &clock, 10);
    assert!(has_label(&scene, "npc_shield"));
    assert!((fill(&scene, "npc_health_overhead") - 1.0).abs() < 1e-5);

    // The worn shield stands in front of the body and absorbs pistol hits.
    event(&mut scene, "Pistol");
    step(&mut scene, &clock, 12);
    assert!(has_label(&scene, "mini_explosion"), "the pistol hit effect should play");
    event(&mut scene, "Report");
    step(&mut scene, &clock, 2);
    assert_eq!(report(&scene), "90");
    assert!((fill(&scene, "npc_health_overhead") - 1.0).abs() < 1e-5);

    // Beyond 6 m nothing happens; the outer ring takes 25 shield only.
    event(&mut scene, "FarSplash");
    event(&mut scene, "OuterSplash");
    event(&mut scene, "Report");
    step(&mut scene, &clock, 2);
    assert!((fill(&scene, "npc_health_overhead") - 1.0).abs() < 1e-5);
    assert_eq!(report(&scene), "65");

    // Within 2 m, splash takes 49% health and 50 shield.
    event(&mut scene, "Splash");
    event(&mut scene, "Report");
    step(&mut scene, &clock, 2);
    assert!((fill(&scene, "npc_health_overhead") - 0.51).abs() < 1e-4);
    assert_eq!(report(&scene), "15");

    // Two close hits leave a sliver; the shield is broken but still worn.
    event(&mut scene, "Splash");
    event(&mut scene, "Report");
    step(&mut scene, &clock, 2);
    assert!((fill(&scene, "npc_health_overhead") - 0.02).abs() < 1e-4);
    assert_eq!(report(&scene), "0");
    assert!(has_label(&scene, "npc_shield"));

    // With the shield broken, a pistol shot reaches the tagged body and
    // takes the rest; the NPC drops its shield.
    assert!(
        scene
            .world
            .get_component_by_id_as::<GLTFComponent>(named(&scene, "npc_model"))
            .unwrap()
            .spawned,
        "the NPC body should be spawned so the shot meets its meshes"
    );
    event(&mut scene, "Pistol");
    step(&mut scene, &clock, 12);
    step(&mut scene, &clock, 2);
    assert!(fill(&scene, "npc_health_overhead").abs() < 1e-5);
    assert!(!has_label(&scene, "npc_shield"));

    // Six seconds later it is back, full, with a fresh shield.
    step(&mut scene, &clock, 6 * 120 + 10);
    assert!((fill(&scene, "npc_health_overhead") - 1.0).abs() < 1e-5);
    assert!(has_label(&scene, "npc_shield"));
}

#[test]
fn target_practice_scene_loads_and_the_npc_wanders() {
    let path = "examples/target-practice.mms";
    let source = std::fs::read_to_string(path)
        .unwrap()
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true");
    let (mut scene, clock) = load(&source, path);
    step(&mut scene, &clock, 4);
    assert!(has_label(&scene, "npc_shield"));
    assert!(has_label(&scene, "npc_health_overhead"));
    assert!(has_label(&scene, "local_health_hud"));
    let world = |scene: &Universe, label: &str| {
        TransformSystem::world_position(&scene.world, named(scene, label)).unwrap()
    };
    let start = world(&scene, "npc_root");

    // Under gravity it stands on the grass (top at y = 0) with its head at
    // eye height, and it has wandered away from where it started.
    step(&mut scene, &clock, 360);
    let (root, head) = (world(&scene, "npc_root"), world(&scene, "npc_head_target"));
    let moved = ((root[0] - start[0]).powi(2) + (root[2] - start[2]).powi(2)).sqrt();
    assert!(
        root[1].abs() < 0.3 && (1.2..2.0).contains(&head[1]) && moved > 0.05,
        "NPC root {start:?} -> {root:?}, head {head:?}"
    );
}

#[test]
fn practice_target_turns_to_the_player_and_fires() {
    let source = r#"
        import { practice_target } from "../assets/components/target_practice.mms"
        let me = T.position(0.0, 1.0, 0.0) { name = "practice_player" }
        me
        let report = Text { name = "practice_report" "0" }
        report
        let hits = { count = 0 }
        // Starts facing away (-Z) from the player at +Z.
        let target = practice_target({ prefix = "npc" avatar = "mitten" use_xr = false use_desktop = false
            position = [0.0, 0.0, -3.0] yaw = 0.0 strafe = 0.0 player = me
            on_player_hit = fn(points) {
                hits.count = hits.count + 1
                report.set_text("" + hits.count)
            } })
        target.root
    "#;
    let (mut scene, clock) = load(source, "examples/_target_practice_aim_test.mms");
    let head_yaw = |scene: &Universe| {
        let q = scene
            .world
            .get_component_by_id_as::<TransformComponent>(named(scene, "npc_head_target"))
            .unwrap()
            .transform
            .rotation;
        2.0 * q[1].atan2(q[3])
    };
    assert!(head_yaw(&scene).abs() < 1e-3);
    assert!(has_label(&scene, "npc_pistol_grip"));

    // During its first 4 s wait it holds its heading and has not fired.
    step(&mut scene, &clock, 420);
    assert!(head_yaw(&scene).abs() < 1e-3);
    assert_eq!(report(&scene), "0");

    // Then it turns half a revolution to face the player (0.6 s), and 0.1 s
    // later fires once.
    step(&mut scene, &clock, 240);
    let yaw = head_yaw(&scene).abs();
    assert!((yaw - std::f32::consts::PI).abs() < 0.05, "{yaw}");
    assert_eq!(report(&scene), "1");
}

#[test]
fn truck_bed_demo_drives_the_pickup_with_seated_mitten_aboard() {
    let path = "examples/truck-bed-demo.mms";
    let source = std::fs::read_to_string(path)
        .unwrap()
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true");
    let (mut scene, clock) = load(&source, path);
    step(&mut scene, &clock, 4);
    let world = |scene: &Universe, label: &str| {
        TransformSystem::world_position(&scene.world, named(scene, label)).unwrap()
    };
    // Mitten rides the chassis, shielded; nothing drives but the script.
    assert!(has_label(&scene, "npc_shield"));
    assert!(has_label(&scene, "demo_truck_patrol"));
    assert_eq!(
        scene.world.parent_of(named(&scene, "npc_frame")),
        Some(named(&scene, "pickup_chassis_frame"))
    );
    let start = world(&scene, "pickup_truck");

    // Four seconds of driving along the road (x), staying on it (z).
    step(&mut scene, &clock, 4 * 120);
    let moved = world(&scene, "pickup_truck");
    assert!(
        (moved[0] - start[0]).abs() > 3.0 && (moved[2] - start[2]).abs() < 1.0,
        "the truck drives along the road: {start:?} -> {moved:?}"
    );
    // Mitten's head stays in the cab, a little above the driver's seat.
    let head = world(&scene, "npc_head_target");
    let truck = world(&scene, "pickup_truck");
    assert!(
        (head[0] - truck[0]).abs() < 1.5 && (1.2..2.4).contains(&(head[1] - truck[1])),
        "head {head:?} in the truck at {truck:?}"
    );
}
