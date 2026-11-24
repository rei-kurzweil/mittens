use super::rocket_motion_tests::{TestClock, Uploader, advance};
use crate::engine::Universe;
use crate::engine::ecs::component::{TextComponent, TransformComponent, UdpChannelComponent};
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, World};
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::Duration;

fn named(scene: &Universe, label: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}

fn event(scene: &mut Universe, name: &str) {
    scene.command_queue.push_event(
        named(scene, "combat_test_events"),
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

#[test]
fn sakura_combat_native_hits_replicate_once_and_reset_health() {
    let first = UdpSocket::bind("127.0.0.1:0").unwrap();
    let second = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addresses = [first.local_addr().unwrap(), second.local_addr().unwrap()];
    drop((first, second));
    let clock = Arc::new(TestClock::default());
    let load = |index: usize| {
        let source = r#"
            import { health_bar } from "../assets/components/health_bar.mms"
            import { shared_player_health } from "../assets/components/network/shared_player_health.mms"
            import { player_hitbox } from "../assets/components/characters/player_hitbox.mms"
            import { laser } from "../assets/components/laser.mms"
            import { rocket } from "../assets/components/projectiles/rocket.mms"
            let socket = UdpSocket.bind("__LOCAL__") {}
            socket
            let combat = shared_player_health(socket)
            combat.root
            let local = health_bar(null)
            let remote = health_bar(null)
            T.position(4, 0, 0) { name = "combat_local_meter" local }
            T.position(4, 2, 0) { name = "combat_remote_meter" remote }
            let target = player_hitbox("combat_hit_zone")
            T.position(0, 1.62, -2) { target.root }
            combat.bind(local, remote, target.zone)
            let caster = laser({ name = "combat_test_laser" length = 5.0 })
            let muzzle = T.position(0, 1.2, 0) {}
            muzzle
            let events = T { name = "combat_test_events" }
            events
            on(events, "DataEvent", fn(event) {
                if event == "Connect" { combat.connect(events, "__REMOTE__") }
                if event == "Pistol" { caster.launch({ pose = muzzle.world_trs() on_hit = combat.pistol_hit }) }
                if event == "Rocket" {
                    rocket({ pose = muzzle.world_trs() on_hit = combat.rocket_hit impact = fn(point, amount, rotation) {} })
                }
                if event == "Reset" { combat.reset() }
                if event == "Gate" && combat.can_fire(muzzle) { T { name = "combat_fire_allowed" } }
            })
        "#
        .replace("__LOCAL__", &addresses[index].to_string())
        .replace("__REMOTE__", &addresses[1 - index].to_string());
        let mut scene = Universe::new(World::default());
        scene.systems.clock.set_driver(clock.clone());
        scene
            .load_mms_source_at_path(&source, "examples/_sakura_combat_test.mms")
            .unwrap();
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut Uploader::default(),
            &mut scene.command_queue,
        );
        scene
    };
    let (mut host, mut client) = (load(0), load(1));
    let step = |host: &mut Universe, client: &mut Universe, frames| {
        for _ in 0..frames {
            advance(host, &clock);
            advance(client, &clock);
        }
    };
    step(&mut host, &mut client, 3);
    event(&mut host, "Connect");
    event(&mut client, "Connect");
    let ready = |scene: &Universe| {
        scene
            .world
            .all_components()
            .filter_map(|id| {
                scene
                    .world
                    .get_component_by_id_as::<UdpChannelComponent>(id)
            })
            .filter(|channel| channel.channel_name == "arena.player.health" && channel.ready)
            .count()
            == 2
    };
    for _ in 0..300 {
        step(&mut host, &mut client, 1);
        if ready(&host) && ready(&client) {
            break;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    assert!(ready(&host) && ready(&client));
    step(&mut host, &mut client, 65);
    let check = |host: &Universe, client: &Universe, points: f32| {
        let peer = fill(host, "combat_remote_meter");
        let local = fill(client, "combat_local_meter");
        assert!(
            (peer - points).abs() < 1e-5 && (local - points).abs() < 1e-5,
            "after hit: expected {points}, host displays {peer}, client displays {local}"
        );
        assert!((fill(host, "combat_local_meter") - 1.0).abs() < 1e-5);
    };

    event(&mut host, "Pistol");
    step(&mut host, &mut client, 12);
    check(&host, &client, 0.9);
    event(&mut host, "Rocket");
    step(&mut host, &mut client, 95);
    check(&host, &client, 0.4);
    event(&mut host, "Rocket");
    step(&mut host, &mut client, 95);
    check(&host, &client, 0.0);
    event(&mut client, "Gate");
    step(&mut host, &mut client, 3);
    assert!(
        !client
            .world
            .all_components()
            .any(|id| { client.world.component_label(id) == Some("combat_fire_allowed") })
    );

    event(&mut client, "Reset");
    step(&mut host, &mut client, 70);
    check(&host, &client, 1.0);
    event(&mut client, "Gate");
    step(&mut host, &mut client, 3);
    named(&client, "combat_fire_allowed");
    event(&mut host, "Pistol");
    step(&mut host, &mut client, 12);
    check(&host, &client, 0.9);
}

#[test]
fn weapon_fire_gate_checks_trigger_and_deferred_launch() {
    let mut scene = Universe::new(World::default());
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(
            r#"
            import { laser_machine_pistol } from "../assets/components/laser-machine-pistol.mms"
            import { sta_rocket_launcher } from "../assets/components/sta-rocket-launcher.mms"
            let state = { alive = false }
            let events = T { name = "combat_test_events" }
            events
            fn can_fire(user) { return user == events && state.alive }
            let pistol = laser_machine_pistol({ name = "gate_pistol" can_fire = can_fire ammunition = 6 })
            pistol.set_mode("semi")
            pistol.root
            for shot in pistol.projectiles { shot }
            let launcher = sta_rocket_launcher({ name = "gate_launcher" can_fire = can_fire })
            launcher.root
            let report = Text { name = "gate_ammo_report" "" }
            report
            fn trigger() {
                pistol.use_start({ user = events })
                launcher.use_start({ user = events })
            }
            on(events, "DataEvent", fn(event) {
                if event == "Trigger" { trigger() }
                if event == "DieBeforeLaunch" { state.alive = true trigger() state.alive = false }
                if event == "Revive" { pistol.use_end() launcher.use_end() state.alive = true }
                if event == "Report" { report.set_text("" + pistol.device_state.ammunition + "/" + launcher.has_ammo()) }
            })
            "#,
            "examples/_weapon_gate_test.mms",
        )
        .unwrap();
    let tick = |scene: &mut Universe, frames| {
        for _ in 0..frames {
            advance(scene, &clock);
        }
    };
    let check = |scene: &mut Universe, expected| {
        event(scene, "Report");
        tick(scene, 3);
        let report = scene
            .world
            .get_component_by_id_as::<TextComponent>(named(scene, "gate_ammo_report"))
            .unwrap();
        assert_eq!(report.text, expected);
    };
    event(&mut scene, "Trigger");
    tick(&mut scene, 40);
    check(&mut scene, "6/true");
    event(&mut scene, "DieBeforeLaunch");
    tick(&mut scene, 40);
    check(&mut scene, "6/true");
    event(&mut scene, "Revive");
    event(&mut scene, "Trigger");
    tick(&mut scene, 40);
    check(&mut scene, "5/false");
}
