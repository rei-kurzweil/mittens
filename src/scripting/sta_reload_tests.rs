use super::rocket_motion_tests::{TestClock, Uploader, advance};
use crate::engine::Universe;
use crate::engine::ecs::component::{GrabbableComponent, TransformOperatorComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, World};
use std::sync::Arc;

fn find(world: &World, name: &str) -> Option<ComponentId> {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(name))
}

fn position(scene: &Universe, name: &str) -> [f32; 3] {
    TransformSystem::world_position(&scene.world, find(&scene.world, name).unwrap()).unwrap()
}

fn assert_position(scene: &Universe, name: &str, expected: [f32; 3]) {
    let actual = position(scene, name);
    for (got, want) in actual.into_iter().zip(expected) {
        assert!(
            (got - want).abs() < 1e-4,
            "{name}: {actual:?} != {expected:?}"
        );
    }
}

fn event(scene: &mut Universe, clock: &TestClock, name: &str) {
    scene.systems.rx.push_event(
        ComponentId::default(),
        EventSignal::DataEvent {
            name: name.into(),
            payload: None,
        },
    );
    for _ in 0..2 {
        advance(scene, clock);
    }
}

#[test]
fn hinge_mms_axis_angle_clamps_and_moves_the_local_output() {
    let mut scene = Universe::new(World::default());
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene
        .load_mms_source_at_path(
            r#"
        let half_pi = Math.pi / 2.0
        let arm = T.position(0,0,2) { name = "arm" }
        let hinge = TOP.hinge([2,0,0]).limits(-half_pi, half_pi).initial_angle(9) { arm }
        T.position(3,4,5).rotation(0,half_pi,0) { name="pivot" hinge }
        let readout = T { name="readout" }
        readout
        on_global("DataEvent", fn(event) {
            hinge.set_angle(-9.0)
            let axis_angle = hinge.axis_angle()
            readout.update_transform([axis_angle.axis[0], axis_angle.angle, 0])
            arm.set_translation([0, 0, 3])
        })
    "#,
            "examples/_hinge_test.mms",
        )
        .unwrap();
    assert_position(&scene, "arm", [3.0, 2.0, 5.0]);
    event(&mut scene, &clock, "SetAngle");
    assert_position(&scene, "arm", [3.0, 7.0, 5.0]);
    assert_position(&scene, "readout", [1.0, -std::f32::consts::FRAC_PI_2, 0.0]);
    let root = find(&scene.world, "pivot").unwrap();
    let tree = super::component_registry::subtree_to_ce_ast(&scene.world, root).unwrap();
    let source = super::unparser::unparse_component(&tree);
    let mut restored = Universe::new(World::default());
    restored
        .load_mms_source_at_path(&source, "examples/_hinge_reload_test.mms")
        .unwrap();
    assert_position(&restored, "arm", [3.0, 7.0, 5.0]);
}

#[test]
fn sta_cabinet_round_reloads_only_in_an_open_empty_launcher() {
    use crate::engine::graphics::materials::OpaqueBucket;

    let mut scene = Universe::new(World::default());
    let clock = Arc::new(TestClock::default());
    scene.systems.clock.set_driver(clock.clone());
    scene.load_mms_source_at_path(r#"
        import { sta_rocket_launcher } from "../assets/components/sta-rocket-launcher.mms"
        import { sta_ammo_cabinet } from "../assets/components/sta_ammo_cabinet.mms"
        let tally = { fired = 0 }
        let launcher = sta_rocket_launcher({ name="fixture" on_launch=fn(event) { tally.fired = tally.fired + 1 } })
        T { launcher.root launcher.model }
        let cabinet = sta_ammo_cabinet("test_cabinet")
        T.position(5,0,0) { cabinet }
        let readout = T { name = "reload_readout" }
        readout
        let round = cabinet.query("[name='test_cabinet_round_0_0']")
        on_global("DataEvent", fn(event) {
            if event == "fire" { launcher.use_start({user=null}) launcher.use_end() }
            if event == "open" { launcher.hinge.set_angle(2.0) }
            if event == "close" { launcher.hinge.set_angle(0.0) }
            if event == "insert" {
                round.world_trs({translation=[0, 0.12039, 0.2876] rotation_quat_xyzw=[0,0,0,1] scale=[1,1,1]})
            }
            if event == "withdraw" { round.set_translation([1.0, 0.12039, 0.2876], "world") }
            if event == "read" {
                let loaded = 0
                if launcher.has_ammo() { loaded = 1 }
                readout.update_transform([tally.fired, loaded, launcher.hinge.axis_angle().angle])
            }
        })
    "#, "examples/_sta_reload_test.mms").unwrap();
    let mut uploader = Uploader::default();
    advance(&mut scene, &clock);
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut uploader,
        &mut scene.command_queue,
    );
    let hinge = find(&scene.world, "fixture_hinge").unwrap();
    let back = find(&scene.world, "back_half").unwrap();
    assert_eq!(scene.world.parent_of(back), Some(hinge));
    assert!(scene.world.children_of(back).iter().any(|&id| {
        scene
            .world
            .get_component_by_id_as::<GrabbableComponent>(id)
            .is_some()
    }));
    assert_position(&scene, "back_half", [0.0, 0.0, 0.48522]);
    let chamber = find(&scene.world, "fixture_chamber").unwrap();
    let initial_round = find(&scene.world, "fixture_initial_round").unwrap();
    assert_eq!(scene.world.parent_of(initial_round), Some(chamber));
    let opaque_bucket = |scene: &Universe, owner| {
        let renderable = scene.world.find_component(owner, "renderable").unwrap();
        let instance = scene
            .visuals
            .instances()
            .iter()
            .find(|v| v.component == renderable)
            .unwrap_or_else(|| panic!("missing prepared renderable {renderable:?} for {owner:?}"));
        scene
            .visuals
            .materials
            .opaque_bucket(instance.material_instance, instance.renderable.material)
    };
    assert_eq!(opaque_bucket(&scene, back), OpaqueBucket::Middle);
    assert_eq!(opaque_bucket(&scene, initial_round), OpaqueBucket::Far);
    for shelf in 0..4 {
        assert!(find(&scene.world, &format!("test_cabinet_shelf_{shelf}")).is_some());
        for slot in 0..5 {
            assert!(find(&scene.world, &format!("test_cabinet_round_{shelf}_{slot}")).is_some());
        }
    }
    let round = find(&scene.world, "test_cabinet_round_0_0").unwrap();
    let model = scene.world.find_component(round, "GLTF").unwrap();

    event(&mut scene, &clock, "fire");
    for _ in 0..20 {
        advance(&mut scene, &clock);
    }
    event(&mut scene, &clock, "read");
    assert_position(&scene, "reload_readout", [1.0, 0.0, 0.0]);
    assert_ne!(scene.world.parent_of(initial_round), Some(chamber));
    event(&mut scene, &clock, "insert");
    event(&mut scene, &clock, "read");
    assert_position(&scene, "reload_readout", [1.0, 0.0, 0.0]);
    assert!(find(&scene.world, "test_cabinet_round_0_0").is_some());

    event(&mut scene, &clock, "withdraw");
    event(&mut scene, &clock, "open");
    assert!(position(&scene, "back_half")[1] < -0.15);
    event(&mut scene, &clock, "insert");
    let open = 75.0_f32.to_radians();
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<TransformOperatorComponent>(hinge)
            .unwrap()
            .hinge_settings()
            .unwrap()
            .angle(),
        open
    );
    assert_eq!(scene.world.parent_of(round), Some(chamber));
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut uploader,
        &mut scene.command_queue,
    );
    assert_eq!(opaque_bucket(&scene, round), OpaqueBucket::Far);
    assert_eq!(scene.world.find_component(round, "GLTF"), Some(model));
    assert!(scene.world.find_component(round, "Grabbable").is_none());
    assert!(scene.world.find_component(round, "Collidable").is_none());
    assert_position(&scene, "test_cabinet_round_0_0", [0.00031, 0.12039, 0.0]);
    event(&mut scene, &clock, "fire");
    event(&mut scene, &clock, "read");
    assert_position(&scene, "reload_readout", [1.0, 1.0, open]);

    event(&mut scene, &clock, "close");
    event(&mut scene, &clock, "fire");
    for _ in 0..20 {
        advance(&mut scene, &clock);
    }
    event(&mut scene, &clock, "read");
    assert_position(&scene, "reload_readout", [2.0, 0.0, 0.0]);
    assert_ne!(scene.world.parent_of(round), Some(chamber));
    assert_eq!(scene.world.find_component(round, "GLTF"), Some(model));
    assert!(position(&scene, "test_cabinet_round_0_0")[2] < -0.18366);
}

#[test]
fn sta_launcher_transform_binding_follows_local_rear_grabs() {
    use crate::engine::ecs::{SignalEmitter, component::UdpChannelComponent};
    use std::{net::UdpSocket, time::Duration};

    let addresses = ["127.0.0.1:0"; 2].map(|address| UdpSocket::bind(address).unwrap());
    let [a, b] = addresses.map(|socket| socket.local_addr().unwrap());
    let load = |address, peer, rank| {
        let mut scene = Universe::new(World::default());
        let source = format!(
            r#"
import {{ shared_sta_launcher }} from "../assets/components/weapons/shared_sta_launcher.mms"
let socket = UdpSocket.bind("{address}") {{}}
socket
let pointer = Pointer {{ name = "pointer" }}
T {{ pointer }}
let device = shared_sta_launcher("fixture", {{ socket = socket rank = {rank} weapon_id = "test.sta" }})
device.root
device.bind_pointer(pointer)
let group = T {{}}
group
device.connect(group, "{peer}")
"#
        );
        scene
            .load_mms_source_at_path(&source, "examples/_sta_sync_test.mms")
            .unwrap();
        scene.update(1.0 / 60.0, &Default::default());
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut Uploader::default(),
            &mut scene.command_queue,
        );
        scene
    };
    let (mut a, mut b) = (load(a, b, 1), load(b, a, 2));
    let tick = |a: &mut Universe, b: &mut Universe, frames| {
        for _ in 0..frames {
            a.update(1.0 / 60.0, &Default::default());
            b.update(1.0 / 60.0, &Default::default());
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    let angle = |scene: &Universe| {
        let hinge = find(&scene.world, "fixture_hinge").unwrap();
        scene
            .world
            .get_component_by_id_as::<TransformOperatorComponent>(hinge)
            .unwrap()
            .hinge_settings()
            .unwrap()
            .angle()
    };
    let set_angle = |scene: &mut Universe, angle| {
        let hinge = find(&scene.world, "fixture_hinge").unwrap();
        scene
            .world
            .get_component_by_id_as_mut::<TransformOperatorComponent>(hinge)
            .unwrap()
            .hinge_settings_mut()
            .unwrap()
            .set_angle(angle)
            .unwrap();
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, hinge);
    };
    let grab = |scene: &mut Universe| {
        let pointer = find(&scene.world, "pointer").unwrap();
        let target = find(&scene.world, "back_half").unwrap();
        let renderable = scene.world.find_component(target, "renderable").unwrap();
        scene.command_queue.push_event(
            renderable,
            EventSignal::GrabStart {
                pointer,
                target,
                renderable,
                raycaster: pointer,
                ray_origin_world: [0.0, 0.0, 2.0],
                ray_dir_world: [0.0, 0.0, -1.0],
            },
        );
    };
    let release = |scene: &mut Universe| {
        let pointer = find(&scene.world, "pointer").unwrap();
        let target = find(&scene.world, "back_half").unwrap();
        scene
            .command_queue
            .push_event(target, EventSignal::GrabEnd { pointer, target });
    };
    let bindings = |scene: &Universe| {
        let mut enabled = [false; 2];
        let mut count = 0;
        for id in scene.world.all_components() {
            if let Some(channel) = scene
                .world
                .get_component_by_id_as::<UdpChannelComponent>(id)
                && channel.channel_name == "test.sta.hinge"
            {
                assert!(channel.ready, "{:?}", channel.error);
                enabled[usize::from(channel.publishing)] = channel.binding_enabled;
                count += 1;
            }
        }
        assert_eq!(count, 2);
        enabled
    };

    tick(&mut a, &mut b, 150);
    assert_eq!(bindings(&a), [true, false]);
    let closed = position(&b, "back_half");
    grab(&mut a);
    tick(&mut a, &mut b, 4);
    assert_eq!(bindings(&a), [false, true]);
    set_angle(&mut a, 1.0);
    tick(&mut a, &mut b, 4);
    assert!((angle(&b) - 1.0).abs() < 1e-6);
    let open = position(&b, "back_half");
    assert!(crate::utils::math::vec3_len(crate::utils::math::vec3_sub(open, closed)) > 0.15);
    assert_position(&b, "back_half", position(&a, "back_half"));

    grab(&mut b);
    tick(&mut a, &mut b, 4);
    set_angle(&mut a, 0.5);
    tick(&mut a, &mut b, 4);
    assert!(
        (angle(&b) - 1.0).abs() < 1e-6,
        "receive binding paused during local grab"
    );
    release(&mut b);
    tick(&mut a, &mut b, 4);
    assert_eq!(bindings(&b), [true, false]);
    assert!(
        (angle(&b) - 1.0).abs() < 1e-6,
        "paused samples are discarded"
    );

    release(&mut a);
    tick(&mut a, &mut b, 1);
    assert_eq!(bindings(&a), [true, false]);
    set_angle(&mut a, 0.0);
    tick(&mut a, &mut b, 4);
    assert!(
        (angle(&b) - 0.5).abs() < 1e-6,
        "release sends one final snapshot"
    );
    assert_eq!(angle(&a), 0.0);
}
