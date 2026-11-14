use crate::engine::{
    Universe,
    ecs::{
        World,
        component::{TransformComponent, UdpChannelComponent},
    },
};
use std::{net::UdpSocket, time::Duration};
fn port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn load(source: &str) -> Universe {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(source, "examples/_udp-test.mms")
        .unwrap();
    scene
}
#[test]
fn mms_socket_methods_publish_bind_and_deliver_samples() {
    let a = port();
    let b = port();
    let host = format!(
        r#"
import {{ vec3, vec4, f32 }} from "types"
let pose_type = {{ translation: vec3, rotation_quat_xyzw: vec4, scale: vec3 }}
let socket = UdpSocket.bind("127.0.0.1:{a}") {{}}
socket
let pose = socket.publish("pose").to("127.0.0.1:{b}").type(pose_type)
pose
let temperature = socket.publish("temperature").to("127.0.0.1:{b}").type(f32)
temperature
let source = T.position(2.0, 3.0, 4.0) {{}}
source
on_global("FrameTick", fn(event) {{
    if pose.ready() {{ pose.write(source.world_trs()) }}
    if temperature.ready() {{ temperature.write(22.0) }}
}})
"#
    );
    let client = format!(
        r#"
import {{ vec3, vec4, f32 }} from "types"
let pose_type = {{ translation: vec3, rotation_quat_xyzw: vec4, scale: vec3 }}
let socket = UdpSocket.bind("127.0.0.1:{b}") {{}}
socket
let target = T {{ name = "remote_target" }}
target
let pose = socket.subscribe("pose").from("127.0.0.1:{a}").type(pose_type)
pose.bind_transform(target, {{ name = "pose_binding", space = "world" }})
let temperature = socket.subscribe("temperature").from("127.0.0.1:{a}").type(f32)
temperature
let observation = T {{ name = "observation" }}
observation
on(temperature, "UdpSample", fn(event) {{ observation.update_transform([event.value, 0.0, 0.0]) }})
"#
    );
    let mut host = load(&host);
    let mut client = load(&client);
    for _ in 0..80 {
        host.update(1.0 / 60.0, &Default::default());
        client.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(4));
    }
    let named = |scene: &Universe, name: &str| {
        scene
            .world
            .all_components()
            .find(|id| scene.world.component_label(*id) == Some(name))
            .unwrap()
    };
    let target = named(&client, "remote_target");
    let pose = client
        .world
        .get_component_by_id_as::<TransformComponent>(target)
        .unwrap();
    assert_eq!(pose.transform.translation, [2., 3., 4.]);
    let binding = named(&client, "pose_binding");
    assert!(
        client
            .world
            .get_component_by_id_as::<UdpChannelComponent>(binding)
            .unwrap()
            .ready
    );
    let observation = named(&client, "observation");
    assert_eq!(
        client
            .world
            .get_component_by_id_as::<TransformComponent>(observation)
            .unwrap()
            .transform
            .translation,
        [22., 0., 0.]
    );
}

#[test]
fn mms_channel_handles_attached_from_callbacks_initialize() {
    let a = port();
    let b = port();
    let source = format!(
        r#"
import {{ f32 }} from "types"
let socket = UdpSocket.bind("127.0.0.1:{a}") {{}}
socket
let groups = T {{ name = "groups" }}
groups
let state = {{ channel = null }}
on_global("FrameTick", fn(event) {{
    if !state.channel {{
        let group = T {{ name = "group" }}
        groups.attach(group)
        let channel = socket.publish("test").to("127.0.0.1:{b}").type(f32)
        group.attach(channel)
        state.channel = channel
    }}
}})
"#
    );
    let mut scene = load(&source);
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    for id in scene.world.all_components() {
        if let Some(c) = scene
            .world
            .get_component_by_id_as::<UdpChannelComponent>(id)
        {
            assert!(
                scene.world.is_initialized(id),
                "channel parent={:?} initialized={:?}",
                scene.world.parent_of(id),
                scene
                    .world
                    .parent_of(id)
                    .map(|p| scene.world.is_initialized(p))
            );
            assert!(c.active, "{:?}", c.error);
        }
    }
}

#[test]
fn imperative_transform_channels_merge_at_execution() {
    let mut scene = load(
        r#"
let target = T.scale(2.0, 3.0, 4.0) { name = "partial_target" }
target
on_global("FrameTick", fn(event) {
    target.set_translation([4.0, 5.0, 6.0])
    target.set_rotation_quat([0.0, 1.0, 0.0, 0.0])
})
"#,
    );
    scene.update(1.0 / 60.0, &Default::default());
    let id = scene
        .world
        .all_components()
        .find(|id| scene.world.component_label(*id) == Some("partial_target"))
        .unwrap();
    let pose = scene
        .world
        .get_component_by_id_as::<TransformComponent>(id)
        .unwrap()
        .trs();
    assert_eq!(pose.translation, [4., 5., 6.]);
    assert_eq!(pose.rotation_quat_xyzw, [0., 1., 0., 0.]);
    assert_eq!(pose.scale, [2., 3., 4.]);
}

#[test]
fn morph_subscription_owns_only_its_avatar_slot_and_releases_on_removal() {
    use crate::engine::ecs::component::{
        GLTFComponent, MorphFactorState, MorphTargetInfo, MorphTargetKey,
    };
    let a = port();
    let b = port();
    let mut sender = load(&format!(
        r#"
import {{ f32 }} from "types"
let socket = UdpSocket.bind("127.0.0.1:{a}") {{}}
socket
let left = socket.publish("left").to("127.0.0.1:{b}").type(f32)
left
let right = socket.publish("right").to("127.0.0.1:{b}").type(f32)
right
on_global("FrameTick", fn(event) {{
    if left.ready() {{ left.write(0.4) }}
    if right.ready() {{ right.write(0.2) }}
}})
"#
    ));
    let mut receiver = load(&format!(
        r#"
import {{ f32 }} from "types"
let socket = UdpSocket.bind("127.0.0.1:{b}") {{}}
socket
let avatar = GLTF.new("synthetic.glb") {{
    name = "face"
    MorphTargetMap.new().slot("left_eye_blink", "left").slot("right_eye_blink", "right")
}}
avatar
let left = socket.subscribe("left").from("127.0.0.1:{a}").type(f32)
left.bind_morph(avatar, "left_eye_blink", {{ name = "left_binding" }})
let right = socket.subscribe("right").from("127.0.0.1:{a}").type(f32)
right.bind_morph(avatar, "right_eye_blink")
"#
    ));
    let gltf = receiver
        .world
        .all_components()
        .find(|id| receiver.world.component_label(*id) == Some("face"))
        .unwrap();
    let keys = [
        MorphTargetKey {
            node_index: 0,
            primitive_index: 0,
            target_index: 0,
        },
        MorphTargetKey {
            node_index: 0,
            primitive_index: 0,
            target_index: 1,
        },
    ];
    {
        let model = receiver
            .world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap();
        model.spawned = true;
        for (key, label) in keys.iter().zip(["left", "right"]) {
            model.morph_targets.push(MorphTargetInfo {
                key: *key,
                label: Some(label.into()),
                base_factor: 0.,
            });
            let mut factor = MorphFactorState::new(0.);
            factor.driver = Some(0.9);
            model.morph_factors.insert(*key, factor);
        }
    }
    let other = receiver
        .world
        .add_component(GLTFComponent::new("other.glb"));
    receiver
        .world
        .get_component_by_id_as_mut::<GLTFComponent>(other)
        .unwrap()
        .morph_factors
        .insert(keys[0], MorphFactorState::new(0.7));
    for _ in 0..80 {
        sender.update(1.0 / 60.0, &Default::default());
        receiver.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(4));
    }
    let model = receiver
        .world
        .get_component_by_id_as::<GLTFComponent>(gltf)
        .unwrap();
    assert!((model.morph_factors[&keys[0]].effective() - 0.4).abs() < 1e-6);
    assert!((model.morph_factors[&keys[1]].effective() - 0.2).abs() < 1e-6);
    assert_eq!(
        receiver
            .world
            .get_component_by_id_as::<GLTFComponent>(other)
            .unwrap()
            .morph_factors[&keys[0]]
            .effective(),
        0.7
    );
    let binding = receiver
        .world
        .all_components()
        .find(|id| receiver.world.component_label(*id) == Some("left_binding"))
        .unwrap();
    receiver
        .systems
        .remove_subtree_immediate(&mut receiver.world, &mut receiver.visuals, binding);
    let model = receiver
        .world
        .get_component_by_id_as::<GLTFComponent>(gltf)
        .unwrap();
    assert_eq!(model.morph_factors[&keys[0]].effective(), 0.9);
    assert!((model.morph_factors[&keys[1]].effective() - 0.2).abs() < 1e-6);
}

#[test]
fn socket_bind_failures_are_observable_without_reporting_readiness() {
    use crate::engine::ecs::component::UdpSocketComponent;
    let reserved = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    let mut scene = load(&format!(
        r#"
let socket = UdpSocket.bind("{address}") {{ name = "blocked_socket" }}
socket
let observation = T {{ name = "socket_error" }}
observation
on(socket, "UdpState", fn(event) {{ if event.message != "" {{ observation.set_translation([1.0, 0.0, 0.0]) }} }})
"#
    ));
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let id = scene
        .world
        .all_components()
        .find(|id| scene.world.component_label(*id) == Some("blocked_socket"))
        .unwrap();
    let socket = scene
        .world
        .get_component_by_id_as::<UdpSocketComponent>(id)
        .unwrap();
    assert!(!socket.ready && socket.error.is_some() && socket.local_address.is_none());
    let observed = scene
        .world
        .all_components()
        .find(|id| scene.world.component_label(*id) == Some("socket_error"))
        .unwrap();
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<TransformComponent>(observed)
            .unwrap()
            .trs()
            .translation,
        [1., 0., 0.]
    );
}
