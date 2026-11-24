use super::rocket_motion_tests::{TestClock, Uploader, advance};
use crate::engine::Universe;
use crate::engine::ecs::component::{
    AnimationComponent, AnimationState, TextComponent, TransformComponent, UdpChannelComponent,
};
use crate::engine::ecs::system::TransformSystem;
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
            import { shared_mounts } from "../assets/components/network/shared_mounts.mms"
            import { arena_shields } from "../assets/components/network/arena_shields.mms"
            import { player_hitbox } from "../assets/components/characters/player_hitbox.mms"
            import { laser } from "../assets/components/laser.mms"
            import { rocket } from "../assets/components/projectiles/rocket.mms"
            let socket = UdpSocket.bind("__LOCAL__") {}
            socket
            let combat = shared_player_health(socket)
            combat.root
            let cues = { health = 0 shield = 0 combined = 0 deaths = 0 }
            combat.bind_death(fn(is_local) { cues.deaths = cues.deaths + 1 })
            combat.bind_damage(fn(cue) {
                if cue == 1.0 { cues.health = cues.health + 1 }
                if cue == 2.0 { cues.shield = cues.shield + 1 }
                if cue == 3.0 { cues.combined = cues.combined + 1 }
            })
            let cue_report = Text { name = "combat_cue_report" "" }
            cue_report
            let local = health_bar(null)
            let remote = health_bar(null)
            let local_mount = T.position(0, 1.24, 0) { name = "combat_local_shield_mount" }
            let remote_mount = T.position(0, 1.24, -2) { name = "combat_remote_shield_mount" }
            let local_root = T.position(4, 0, 0) { name = "combat_local_meter" local local_mount }
            local_root
            remote_mount
            T.position(4, 2, 0) { name = "combat_remote_meter" remote }
            let target = player_hitbox("combat_hit_zone")
            T.position(0, 1.62, -2) { target.root }
            combat.bind(local, remote, target.zone)
            let shield_pointer = Pointer { name = "combat_shield_pointer" }
            T { shield_pointer }
            let gear = arena_shields(socket, __RANK__, shared_mounts(),
                { root = local_root shield_mount = local_mount },
                { root = remote_mount shield_mount = remote_mount },
                [[4, 0.24, 3], [5, 0.24, 3]], combat.can_fire)
            gear.root
            gear.bind_pointer(shield_pointer)
            combat.bind_shields(gear)
            let integrity = Text { name = "combat_integrity_report" "" }
            integrity
            let caster = laser({ name = "combat_test_laser" length = 5.0 })
            let muzzle = T.position(0, 1.2, 0) {}
            muzzle
            let events = T { name = "combat_test_events" }
            events
            on(events, "DataEvent", fn(event) {
                if event == "Connect" {
                    combat.connect(events, "__REMOTE__")
                    gear.connect(events, "__REMOTE__")
                }
                if event == "Pistol" { caster.launch({ pose = muzzle.world_trs() on_hit = combat.pistol_hit }) }
                if event == "Rocket" {
                    rocket({ pose = muzzle.world_trs() on_hit = combat.rocket_hit impact = fn(point, amount, rotation) {} })
                }
                if event == "Reset" { combat.reset() }
                if event == "Splash" { combat.rocket_splash([4, 1.24, 0]) }
                if event == "ReportShield" { integrity.set_text("" + gear.local_equipped().integrity()) }
                if event == "ReportCues" { cue_report.set_text("" + cues.health + "/" + cues.shield + "/" + cues.combined + "/" + cues.deaths) }
                if event == "Gate" && combat.can_fire(muzzle) { T { name = "combat_fire_allowed" } }
            })
        "#
        .replace("__LOCAL__", &addresses[index].to_string())
        .replace("__REMOTE__", &addresses[1 - index].to_string())
        .replace("__RANK__", &(index + 1).to_string());
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

    let client_shield = named(&client, "arena_shield_1");
    let pointer = named(&client, "combat_shield_pointer");
    client.command_queue.push_event(
        pointer,
        EventSignal::PointerGrabbed {
            pointer,
            target: client_shield,
        },
    );
    step(&mut host, &mut client, 3);
    client.command_queue.push_event(
        pointer,
        EventSignal::PointerReleased {
            pointer,
            target: client_shield,
        },
    );
    step(&mut host, &mut client, 18);
    assert_eq!(
        client.world.parent_of(client_shield),
        Some(named(&client, "combat_local_shield_mount"))
    );
    let host_shield = named(&host, "arena_shield_1");
    assert_eq!(
        host.world.parent_of(host_shield),
        Some(named(&host, "combat_remote_shield_mount"))
    );

    let report_shield = |host: &mut Universe, client: &mut Universe| {
        event(client, "ReportShield");
        step(host, client, 3);
        client
            .world
            .get_component_by_id_as::<TextComponent>(named(client, "combat_integrity_report"))
            .unwrap()
            .text
            .clone()
    };
    event(&mut host, "Pistol");
    step(&mut host, &mut client, 12);
    check(&host, &client, 0.9);
    assert_eq!(report_shield(&mut host, &mut client), "90");
    event(&mut client, "Splash");
    step(&mut host, &mut client, 12);
    check(&host, &client, 0.4);
    assert_eq!(report_shield(&mut host, &mut client), "40");
    event(&mut client, "Splash");
    step(&mut host, &mut client, 12);
    check(&host, &client, 0.0);
    assert_eq!(
        TransformSystem::world_position(&client.world, client_shield).unwrap(),
        [4.0, 0.24, 0.0]
    );
    assert_ne!(
        client.world.parent_of(client_shield),
        Some(named(&client, "combat_local_shield_mount"))
    );
    event(&mut client, "Reset");
    step(&mut host, &mut client, 12);
    check(&host, &client, 1.0);
    assert_eq!(
        TransformSystem::world_position(&client.world, client_shield).unwrap(),
        [4.0, 0.24, 0.0]
    );
    step(&mut host, &mut client, 60);
    event(&mut host, "ReportCues");
    event(&mut client, "ReportCues");
    step(&mut host, &mut client, 3);
    for scene in [&host, &client] {
        let report = named(scene, "combat_cue_report");
        assert_eq!(
            scene
                .world
                .get_component_by_id_as::<TextComponent>(report)
                .unwrap()
                .text,
            "4/1/2/2"
        );
    }
}

#[test]
fn weapon_fire_gate_checks_trigger_and_deferred_launch() {
    let mut scene = Universe::new(World::default());
    let clock = Arc::new(TestClock::default());
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
    scene.systems.audio = Default::default();
    scene.systems.clock.set_driver(clock.clone());
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

#[test]
fn empty_weapons_stay_put_for_a_minute_then_return_loaded() {
    let mut scene = Universe::new(World::default());
    let clock = Arc::new(TestClock::default());
    scene
        .load_mms_source_at_path(
            r##"
            import { ship_machine_pistol } from "../assets/components/weapons/ship_machine_pistol.mms"
            import { shared_sta_launcher } from "../assets/components/weapons/shared_sta_launcher.mms"
            let socket = UdpSocket.bind("127.0.0.1:0") {}
            let gun = ship_machine_pistol("restock_pistol", {
                socket = socket weapon_id = "test.pistol" rank = 1 ammunition = 1 restock_beats_per_second = 2.0
            })
            let launcher = shared_sta_launcher("restock_launcher", {
                socket = socket weapon_id = "test.launcher" rank = 1 restock_beats_per_second = 2.0
                left_pointer = "#unused_left" right_pointer = "#unused_right"
            })
            T.position(2, 1, 0) { gun }
            T.position(-2, 1, 0) { launcher.root }
            let carrier = T.position(0, 0, 5) {}
            let pointer_frame = T {
                Pointer { name = "test_pointer" }
            }
            pointer_frame
            carrier
            let events = T { name = "combat_test_events" }
            events
            on(events, "DataEvent", fn(event) {
                if event == "Fire" {
                    gun.grip.ship_input("KeyDown", { code = "Space" })
                    launcher.grip.ship_input("KeyDown", { code = "Space" })
                }
                if event == "Move" {
                    carrier.attach(gun.grip)
                    carrier.attach(launcher.grip)
                    gun.grip.update_transform([0, 0, 0])
                    launcher.grip.update_transform([0, 0, 0])
                }
            })
            "##,
            "examples/_weapon_restock_test.mms",
        )
        .unwrap();
    scene.systems.audio = Default::default();
    scene.systems.clock.set_driver(clock.clone());
    let tick = |scene: &mut Universe, frames| {
        for _ in 0..frames {
            advance(scene, &clock);
        }
    };
    tick(&mut scene, 3);
    let pointer = named(&scene, "test_pointer");
    let pistol = named(&scene, "restock_pistol_grip");
    let launcher = named(&scene, "restock_launcher_grip");
    let stand = named(&scene, "test.launcher.grip_mount");
    let ammo = named(&scene, "restock_pistol_ammo_count");
    let timer = named(&scene, "restock_pistol_restock_timer");
    let timer_state = |scene: &Universe| {
        scene
            .world
            .get_component_by_id_as::<AnimationComponent>(timer)
            .unwrap()
            .state
    };
    let position =
        |scene: &Universe, id| TransformSystem::world_position(&scene.world, id).unwrap();
    let pistol_home = position(&scene, pistol);
    let launcher_home = position(&scene, launcher);
    for target in [pistol, launcher] {
        scene.command_queue.push_event(
            target,
            EventSignal::GrabStart {
                pointer,
                raycaster: pointer,
                renderable: target,
                target,
                ray_origin_world: [0.0; 3],
                ray_dir_world: [0.0, 0.0, -1.0],
            },
        );
    }
    tick(&mut scene, 2);
    event(&mut scene, "Fire");
    tick(&mut scene, 30);
    event(&mut scene, "Move");
    // Just short of a minute (60 s at 120 steps per second) since the shots.
    tick(&mut scene, 7_130);
    let rounds = |scene: &Universe| {
        scene
            .world
            .get_component_by_id_as::<TextComponent>(ammo)
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(rounds(&scene), "0");
    assert_eq!(timer_state(&scene), AnimationState::Playing);
    assert!(
        scene
            .world
            .find_component(stand, "#restock_launcher_initial_round")
            .is_none()
    );
    assert_eq!(position(&scene, pistol)[2], 5.0);
    assert_eq!(position(&scene, launcher)[2], 5.0);

    tick(&mut scene, 100);
    assert_eq!(
        rounds(&scene),
        "1",
        "timer state {:?}, beat {}",
        timer_state(&scene),
        scene.systems.clock.beat_now()
    );
    assert!(
        scene
            .world
            .find_component(stand, "#restock_launcher_initial_round")
            .is_some()
    );
    assert_eq!(position(&scene, pistol), pistol_home);
    assert_eq!(position(&scene, launcher), launcher_home);
}

#[test]
fn energy_shield_hides_the_shell_and_reopens_a_broken_bullet_path() {
    use crate::engine::ecs::component::OpacityComponent;
    let mut scene = Universe::new(World::default());
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(
            r#"
            import { energy_shield } from "../assets/components/energy_sheild.mms"
            let shield = energy_shield("test_shield", 2.0)
            shield
            let report = Text { name = "shield_report" "" }
            report
            let events = T { name = "combat_test_events" }
            events
            on(events, "DataEvent", fn(event) {
                if event == "Equip" { shield.set_worn(true, true) }
                if event == "Hit" { shield.take_damage(10.0) }
                if event == "Break" { shield.take_damage(100.0) }
                if event == "Drop" { shield.set_worn(false, false) }
                if event == "Report" { report.set_text("" + shield.integrity()) }
            })
            "#,
            "examples/_shield_test.mms",
        )
        .unwrap();
    let tick = |scene: &mut Universe, frames| {
        for _ in 0..frames {
            advance(scene, &clock);
        }
    };
    let has = |scene: &Universe, label| {
        scene
            .world
            .all_components()
            .any(|id| scene.world.component_label(id) == Some(label))
    };
    event(&mut scene, "Equip");
    tick(&mut scene, 3);
    assert!(has(&scene, "test_shield_bullet_proxy"));
    assert!(!has(&scene, "test_shield_shell"));
    event(&mut scene, "Hit");
    tick(&mut scene, 3);
    let shell = named(&scene, "test_shield_shell");
    let opacity = scene.world.find_component(shell, "opacity").unwrap();
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<OpacityComponent>(opacity)
            .unwrap()
            .opacity,
        0.4
    );
    event(&mut scene, "Break");
    tick(&mut scene, 3);
    assert!(!has(&scene, "test_shield_bullet_proxy"));
    // The breaking hit removes the shell at once instead of after its flash.
    assert!(!has(&scene, "test_shield_shell"));
    event(&mut scene, "Hit");
    tick(&mut scene, 3);
    assert!(!has(&scene, "test_shield_shell"));
    tick(&mut scene, 310);
    assert!(!has(&scene, "test_shield_shell"));
    assert!(!has(&scene, "test_shield_bullet_proxy"));
    tick(&mut scene, 180);
    assert!(has(&scene, "test_shield_bullet_proxy"));
    tick(&mut scene, 500);
    event(&mut scene, "Report");
    tick(&mut scene, 3);
    let report = named(&scene, "shield_report");
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<TextComponent>(report)
            .unwrap()
            .text,
        "100"
    );
    event(&mut scene, "Drop");
    tick(&mut scene, 3);
    assert!(!has(&scene, "test_shield_bullet_proxy"));
}

#[test]
fn player_death_plays_24_bursts_over_six_seconds_and_cleans_up() {
    use std::collections::HashSet;

    let mut scene = Universe::new(World::default());
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(
            r#"
            import { player_death } from "../assets/components/player_death.mms"
            player_death([3, 4, -5], 2.0)
            "#,
            "examples/_player_death_test.mms",
        )
        .unwrap();
    let sequence = named(&scene, "arena_player_death");
    let mut centers = HashSet::new();
    for frame in 0..740 {
        advance(&mut scene, &clock);
        if frame == 600 {
            assert!((20..=21).contains(&centers.len()));
            assert!(scene.world.get_component_record(sequence).is_some());
        }
        if frame % 10 == 0 {
            for id in scene.world.all_components() {
                if scene.world.component_label(id) == Some("mini_explosion") {
                    let position = TransformSystem::world_position(&scene.world, id).unwrap();
                    centers.insert(position.map(f32::to_bits));
                }
            }
        }
    }
    assert_eq!(centers.len(), 24);
    assert!(scene.world.get_component_record(sequence).is_none());
    assert!(
        !scene
            .world
            .all_components()
            .any(|id| { scene.world.component_label(id) == Some("mini_explosion") })
    );
}
