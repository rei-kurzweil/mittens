//! Two native local grab lifecycles linked only by the MMS channel helper.
use crate::engine::Universe;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, World};
use crate::engine::transform::TransformSpace;
use std::time::Duration;

fn named(scene: &Universe, name: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
        .unwrap()
}
fn position(scene: &Universe, name: &str) -> [f32; 3] {
    TransformSystem::world_position(&scene.world, named(scene, name)).unwrap()
}
fn load(port: u16, other: u16, rank: u32) -> Universe {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            &format!(
                r#"
import {{ shared_grabbable }} from "../assets/components/network/shared_grabbable.mms"
let socket = UdpSocket.bind("127.0.0.1:{port}") {{}}
socket
let pointer = Pointer {{ name = "pointer" }}
T.position(0.0, 1.0, 2.0) {{ name = "driver" pointer }}
let prop = T.position(0.0, 1.0, 0.0) {{ name = "prop"
    Grabbable.to_hand(true) {{}}
    R.cube() {{ name = "mesh" Raycastable.enabled() }}
}}
let shared = shared_grabbable({{ socket = socket name = "prop.1" target = prop rank = {rank} }})
shared.root
shared.bind_pointer(pointer)
let channels = T {{ name = "channels" }}
channels
shared.connect(channels, "127.0.0.1:{other}")
let control = T {{ name = "control" }}
control
on(control, "DataEvent", fn(event) {{
    if event == "disconnect" {{ shared.disconnect() }}
}})
let observed = T {{ name = "observed" }}
observed
on_global("FrameTick", fn(event) {{
    let held = 0.0
    if shared.state.held {{ held = 1.0 }}
    observed.update_transform([shared.state.revision, shared.state.owner, held])
}})
"#
            ),
            "examples/_shared-grab-test.mms",
        )
        .unwrap();
    scene
}
fn tick(a: &mut Universe, b: &mut Universe, frames: usize) {
    for _ in 0..frames {
        a.update(1.0 / 60.0, &Default::default());
        b.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(4));
    }
}
fn grab(scene: &mut Universe) {
    let pointer = named(scene, "pointer");
    let target = named(scene, "prop");
    let renderable = named(scene, "mesh");
    scene.command_queue.push_event(
        renderable,
        EventSignal::GrabStart {
            pointer,
            target,
            renderable,
            raycaster: pointer,
            ray_origin_world: [0., 1., 2.],
            ray_dir_world: [0., 0., -1.],
        },
    );
}
fn release(scene: &mut Universe) {
    let pointer = named(scene, "pointer");
    let target = named(scene, "prop");
    scene
        .command_queue
        .push_event(target, EventSignal::GrabEnd { pointer, target });
}
fn move_driver(scene: &mut Universe, x: f32) {
    let id = named(scene, "driver");
    scene.command_queue.push_intent_now(
        id,
        IntentValue::PatchTransformTrs {
            component_id: id,
            space: TransformSpace::World,
            translation: Some([x, 1., 2.]),
            rotation: Some([0.0, 0.0, 0.38268343, 0.9238795]),
        },
    );
}
fn same_pose(a: &Universe, b: &Universe) {
    let p = position(a, "prop");
    let q = position(b, "prop");
    assert!((0..3).all(|i| (p[i] - q[i]).abs() < 0.01), "{p:?} != {q:?}");
    let p = TransformSystem::world_rotation_quat_xyzw(&a.world, named(a, "prop")).unwrap();
    let q = TransformSystem::world_rotation_quat_xyzw(&b.world, named(b, "prop")).unwrap();
    let dot: f32 = (0..4).map(|i| p[i] * q[i]).sum();
    assert!(dot.abs() > 0.999, "rotation {p:?} != {q:?}");
}
#[test]
fn mms_shared_grab_replicates_native_hold_move_release_and_handoff() {
    let a = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let b = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let ports = (
        a.local_addr().unwrap().port(),
        b.local_addr().unwrap().port(),
    );
    drop((a, b));
    let (mut a, mut b) = (load(ports.0, ports.1, 1), load(ports.1, ports.0, 2));
    tick(&mut a, &mut b, 100);
    grab(&mut a);
    tick(&mut a, &mut b, 25);
    assert_eq!(
        position(&a, "observed")[2],
        1.0,
        "local grab must notify MMS; parent {:?}",
        a.world.parent_of(named(&a, "prop"))
    );
    assert_eq!(
        position(&b, "observed")[2],
        1.0,
        "remote state {:?}; local {:?}",
        position(&b, "observed"),
        position(&a, "observed")
    );
    move_driver(&mut a, -2.0);
    tick(&mut a, &mut b, 25);
    same_pose(&a, &b);
    release(&mut a);
    tick(&mut a, &mut b, 20);
    assert_eq!(position(&b, "observed")[2], 0.0);
    let dropped = position(&a, "prop");
    move_driver(&mut a, 3.0);
    tick(&mut a, &mut b, 15);
    assert_eq!(position(&a, "prop"), dropped);
    same_pose(&a, &b);
    grab(&mut b);
    tick(&mut a, &mut b, 20);
    move_driver(&mut b, 2.0);
    tick(&mut a, &mut b, 25);
    assert_eq!(position(&a, "observed")[1..], [2.0, 1.0]);
    same_pose(&a, &b);
    release(&mut b);
    tick(&mut a, &mut b, 20);
    assert_eq!(position(&a, "observed")[2], 0.0);
    same_pose(&a, &b);
    // Both grab before either receives the other's transition. The rank tie
    // break must stop the losing native hold, not merely hide its packets.
    grab(&mut a);
    grab(&mut b);
    tick(&mut a, &mut b, 25);
    assert_eq!(position(&a, "observed")[1..], [2.0, 1.0]);
    assert_eq!(
        a.world.parent_of(named(&a, "prop")),
        Some(named(&a, "prop.1_mount"))
    );
    move_driver(&mut a, -5.0);
    move_driver(&mut b, 4.0);
    tick(&mut a, &mut b, 25);
    same_pose(&a, &b);
    release(&mut b);
    tick(&mut a, &mut b, 20);
    assert_eq!(position(&a, "observed")[2], 0.0);
    same_pose(&a, &b);
    grab(&mut b);
    tick(&mut a, &mut b, 20);
    assert_eq!(position(&a, "observed")[2], 1.0);
    let control = named(&a, "control");
    a.command_queue.push_event(
        control,
        EventSignal::DataEvent {
            name: "disconnect".into(),
            payload: None,
        },
    );
    tick(&mut a, &mut b, 10);
    assert_eq!(position(&a, "observed")[1..], [1.0, 0.0]);
    let retained = position(&a, "prop");
    move_driver(&mut b, 8.0);
    tick(&mut a, &mut b, 20);
    assert_eq!(
        position(&a, "prop"),
        retained,
        "obsolete incoming callback must not move the object"
    );
}

#[test]
fn mms_pointer_grab_notifications_are_scoped_and_have_component_payloads() {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
let pointer = Pointer { name = "pointer" }
let other = Pointer { name = "other" }
T { name = "driver" pointer other }
let target = T { name = "prop" Grabbable {} R.cube() { name = "mesh" } }
target
let flags = T { name = "flags" }
flags
on(pointer, "PointerGrabbed", fn(event) {
    if event.pointer == pointer && event.target == target { flags.update_transform([1.0, 0.0, 0.0]) }
})
on(pointer, "PointerReleased", fn(event) {
    if event.pointer == pointer && event.target == target { flags.update_transform([2.0, 0.0, 0.0]) }
})
on(other, "PointerGrabbed", fn(event) { flags.update_transform([99.0, 0.0, 0.0]) })
"#, "examples/_pointer-events-test.mms").unwrap();
    scene.update(1. / 60., &Default::default());
    grab(&mut scene);
    scene.update(1. / 60., &Default::default());
    assert_eq!(position(&scene, "flags")[0], 1.0);
    release(&mut scene);
    scene.update(1. / 60., &Default::default());
    assert_eq!(position(&scene, "flags")[0], 2.0);
}

#[test]
fn mms_shared_grab_late_receiver_gets_final_release_snapshot() {
    let a = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let b = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let ports = (
        a.local_addr().unwrap().port(),
        b.local_addr().unwrap().port(),
    );
    drop((a, b));
    let mut a = load(ports.0, ports.1, 1);
    // The receiver is absent for both transitions; neither event can reach it.
    for _ in 0..3 {
        a.update(1. / 60., &Default::default());
    }
    grab(&mut a);
    for _ in 0..3 {
        a.update(1. / 60., &Default::default());
    }
    move_driver(&mut a, -3.0);
    for _ in 0..20 {
        a.update(1. / 60., &Default::default());
    }
    release(&mut a);
    for _ in 0..3 {
        a.update(1. / 60., &Default::default());
    }
    assert_eq!(position(&a, "observed"), [2.0, 1.0, 0.0]);
    let mut b = load(ports.1, ports.0, 2);
    tick(&mut a, &mut b, 150);
    assert_eq!(position(&b, "observed"), [2.0, 1.0, 0.0]);
    same_pose(&a, &b);
}
