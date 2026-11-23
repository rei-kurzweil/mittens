//! The packed shot channel and the idle-to-active retry lifecycle.
use crate::engine::Universe;
use crate::engine::ecs::component::UdpChannelComponent;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter, SignalKind, World};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

fn peer(port: u16, remote: u16, rank: u32) -> Universe {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            &format!(
                r#"
                import {{ pistol_replication }} from "../assets/components/network/pistol_replication.mms"
                import {{ shared_grabbable }} from "../assets/components/network/shared_grabbable.mms"
                let socket = UdpSocket.bind("127.0.0.1:{port}") {{}}
                socket
                let group = T {{ name = "channels" }}
                group
                let pointer = Pointer {{ name = "test_pointer" }}
                T {{ pointer }}
                let target = T {{ name = "test_prop" }}
                let grab = shared_grabbable({{ socket = socket target = target name = "test.prop" rank = {rank} }})
                grab.root
                grab.bind_pointer(pointer)
                grab.connect(group, "127.0.0.1:{remote}")
                fn gun(name) {{
                    let report = T {{ name = name }}
                    report
                    let received = {{ count = 0 sum = 0 mode = 0 }}
                    let link = pistol_replication({{ socket = socket name = name play = fn(slot, pose, mode) {{
                        received.count = received.count + 1
                        received.sum = received.sum + pose.translation[0]
                        received.mode = received.mode + mode
                        report.update_transform([received.count, received.sum, received.mode])
                    }} }})
                    link.connect(group, "127.0.0.1:{remote}")
                    return link
                }}
                let a = gun("test.first")
                let b = gun("test.second")
                fn pose(x) {{
                    return {{ translation = [x,0,0] rotation_quat_xyzw = [0,0,0,1] scale = [1,1,1] }}
                }}
                let control = T {{ name = "test_control" }}
                control
                on(control, "DataEvent", fn(event) {{
                    if event == "burst" {{
                        a.shot(0, pose(1), "burst")
                        a.shot(1, pose(2), "burst")
                        a.shot(2, pose(3), "burst")
                        b.shot(0, pose(10), "auto")
                    }}
                    if event == "last" {{ a.shot(0, pose(4), "semi") }}
                }})
                "#,
            ),
            "examples/_pistol_replication_test.mms",
        )
        .unwrap();
    scene
}

fn named(scene: &Universe, label: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap()
}

fn wait_until(a: &mut Universe, b: &mut Universe, ready: impl Fn(&Universe, &Universe) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        a.update(1.0 / 60.0, &Default::default());
        b.update(1.0 / 60.0, &Default::default());
        if ready(a, b) {
            return;
        }
        assert!(Instant::now() < deadline, "replication timed out");
        std::thread::sleep(Duration::from_millis(4));
    }
}

fn idle(scene: &Universe) -> bool {
    !scene.systems.rx.has_global_handlers(SignalKind::FrameTick)
}

fn ready(scene: &Universe) -> bool {
    let channels: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|id| {
            scene
                .world
                .get_component_by_id_as::<UdpChannelComponent>(id)
        })
        .collect();
    channels.len() == 6 && channels.iter().all(|channel| channel.ready)
}

fn trigger(scene: &mut Universe, name: &str) {
    scene.command_queue.push_event(
        named(scene, "test_control"),
        EventSignal::DataEvent {
            name: name.into(),
            payload: None,
        },
    );
}

fn report(scene: &Universe, name: &str) -> [f32; 3] {
    TransformSystem::world_position(&scene.world, named(scene, name)).unwrap()
}

#[test]
fn pistol_replication_packs_bursts_and_retires_idle_callbacks() {
    let first = UdpSocket::bind("127.0.0.1:0").unwrap();
    let second = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (port_a, port_b) = (
        first.local_addr().unwrap().port(),
        second.local_addr().unwrap().port(),
    );
    drop((first, second));
    let (mut a, mut b) = (peer(port_a, port_b, 1), peer(port_b, port_a, 2));
    wait_until(&mut a, &mut b, |a, b| {
        ready(a) && ready(b) && idle(a) && idle(b)
    });

    let pointer = named(&a, "test_pointer");
    let target = named(&a, "test_prop");
    a.command_queue
        .push_event(pointer, EventSignal::PointerGrabbed { pointer, target });
    a.update(1.0 / 60.0, &Default::default());
    assert!(!idle(&a));
    a.command_queue
        .push_event(pointer, EventSignal::PointerReleased { pointer, target });
    trigger(&mut a, "burst");
    wait_until(&mut a, &mut b, |_, b| report(b, "test.first")[0] >= 3.0);

    wait_until(&mut a, &mut b, |a, b| idle(a) && idle(b));
    assert_eq!(report(&b, "test.first"), [3.0, 6.0, 3.0]);
    assert_eq!(report(&b, "test.second"), [1.0, 10.0, 2.0]);

    trigger(&mut a, "last");
    wait_until(&mut a, &mut b, |a, b| {
        idle(a) && report(b, "test.first")[0] >= 4.0
    });
    assert_eq!(report(&b, "test.first"), [4.0, 10.0, 3.0]);
}
