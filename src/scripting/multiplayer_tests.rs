use crate::engine::Universe;
use crate::engine::ecs::component::VelocityComponent;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, World};
use std::time::Duration;

fn named(scene: &Universe, name: &str) -> Option<ComponentId> {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(name))
}

fn world_position(scene: &Universe, name: &str) -> Option<[f32; 3]> {
    TransformSystem::world_position(&scene.world, named(scene, name)?)
}

fn load(source: &str, path: &str) -> Universe {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(source, path).unwrap();
    scene
}

const SHARED: &str = r#"
let proxies = T { name = "proxies" }
proxies
fn remote_player(event) {
    return T { name = "proxy_" + event.name  MultiplayerSync.remote_player(event.peer_id) }
}
on(net, "MultiplayerPeerJoined", fn(event) { proxies.attach(remote_player(event)) })
Gravity.enabled(true) {
    Velocity { name = "ball_velocity"
        T.position(0.0, 5.0, 0.0) { name = "ball" MultiplayerSync.host("ball") }
    }
}
"#;

#[test]
fn host_and_client_replicate_players_host_physics_and_messages() {
    let host_source = format!(
        r#"
let net = MultiplayerSession.host("127.0.0.1:47011").player_name("rei") {{}}
net
T.position(1.0, 2.0, 3.0) {{ name = "local_player" MultiplayerSync.local_player() }}
T {{ name = "message_flag" }}
on(net, "MultiplayerMessage", fn(event) {{
    if event.name == "hello" && event.text == "dan" {{
        query("[name='message_flag']").update_transform([7.0, 7.0, 7.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    }}
}})
{SHARED}"#
    );
    let client_source = format!(
        r#"
let net = MultiplayerSession.join("127.0.0.1:47011").player_name("dan") {{}}
net
T.position(-4.0, 0.0, 1.5) {{ name = "local_player" MultiplayerSync.local_player() }}
on(net, "MultiplayerConnected", fn(event) {{ net.send("hello", net.local_name()) }})
{SHARED}"#
    );
    let mut host = load(&host_source, "examples/_net-host-test.mms");
    let mut client = load(&client_source, "examples/_net-client-test.mms");

    for _ in 0..90 {
        host.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(1));
        client.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(1));
    }

    // Each side spawned a proxy for the other and it follows the owner's pose.
    let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3);
    assert!(near(
        world_position(&client, "proxy_rei").unwrap(),
        [1.0, 2.0, 3.0]
    ));
    assert!(near(
        world_position(&host, "proxy_dan").unwrap(),
        [-4.0, 0.0, 1.5]
    ));

    // The host simulates the ball; the client never does, yet sees it fall.
    let client_velocity = named(&client, "ball_velocity").unwrap();
    assert!(
        !client
            .world
            .get_component_by_id_as::<VelocityComponent>(client_velocity)
            .unwrap()
            .enabled
    );
    let host_ball = world_position(&host, "ball").unwrap()[1];
    let client_ball = world_position(&client, "ball").unwrap()[1];
    assert!(host_ball < 4.0, "host ball should have fallen: {host_ball}");
    assert!(
        client_ball < 4.5,
        "client ball should follow host: {client_ball}"
    );
    // At most a few send intervals behind the host.
    assert!((client_ball - host_ball).abs() < 1.0);

    // Client -> host app message, with the sender's name as payload.
    assert!(near(
        world_position(&host, "message_flag").unwrap(),
        [7.0, 7.0, 7.0]
    ));
}

fn status_text(scene: &Universe) -> String {
    use crate::engine::ecs::component::TextComponent;
    scene
        .world
        .all_components()
        .filter_map(|id| scene.world.get_component_by_id_as::<TextComponent>(id))
        .map(|text| text.text.clone())
        .find(|text| text.contains("remote players:"))
        .unwrap_or_default()
}

#[test]
fn multiplayer_basic_example_runs_as_host_and_client() {
    let source = std::fs::read_to_string("examples/multiplayer-basic.mms").unwrap();
    let host_data = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let client_data = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let host_port = host_data.local_addr().unwrap().port();
    let client_port = client_data.local_addr().unwrap().port();
    drop((host_data, client_data));
    let source = source
        .replace("7011", &host_port.to_string())
        .replace("7012", &client_port.to_string());
    let configured = |session: &str| source.replace("MultiplayerSession.from_env()", session);
    let mut host = load(
        &configured(r#"MultiplayerSession.host("127.0.0.1:47012").player_name("rei")"#),
        "examples/multiplayer-basic.mms",
    );
    let mut client = load(
        &configured(r#"MultiplayerSession.join("127.0.0.1:47012").player_name("dan")"#),
        "examples/multiplayer-basic.mms",
    );
    use crate::engine::ecs::{IntentValue, SignalEmitter};
    use crate::engine::transform::TransformSpace;
    for (scene, x) in [(&mut host, -2.0), (&mut client, 2.0)] {
        let root = named(scene, "local_root").unwrap();
        scene.command_queue.push_intent_now(
            root,
            IntentValue::PatchTransformTrs {
                component_id: root,
                space: TransformSpace::World,
                translation: Some([x, 0., 0.]),
                rotation: None,
            },
        );
    }
    let mut remote_blinked = [false; 2];
    let remote_models = [
        named(&host, "remote_model").unwrap(),
        named(&client, "remote_model").unwrap(),
    ];
    for _ in 0..90 {
        host.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(1));
        client.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(1));
        for (index, scene) in [&host, &client].into_iter().enumerate() {
            let model = scene
                .world
                .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(
                    remote_models[index],
                )
                .unwrap();
            remote_blinked[index] |= model
                .morph_factors
                .values()
                .any(|f| f.external.is_some_and(|v| v > 0.2));
        }
    }
    assert_eq!(
        remote_blinked,
        [true, true],
        "both peers must actually receive an eye closure"
    );
    for scene in [&host, &client] {
        use crate::engine::ecs::component::{AnimationComponent, AnimationState, MirrorComponent};
        assert!(scene.world.get_component_by_id_as::<MirrorComponent>(
            named(scene, "multiplayer_mirror").unwrap()
        ).is_some());
        let animations: Vec<_> = scene.world.all_components()
            .filter_map(|id| scene.world.get_component_by_id_as::<AnimationComponent>(id))
            .collect();
        assert_eq!(animations.iter().filter(|clip|
            clip.state == AnimationState::Looping && clip.length_beats == Some(64.2)
        ).count(), 2, "both avatar copies must have ambient gaze clips");
        assert_eq!(animations.iter().filter(|clip|
            clip.state == AnimationState::Looping && clip.length_beats == Some(5.6)
        ).count(), 1, "only the local desktop avatar authors the blink clip");
    }
    let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3);
    assert!(near(
        world_position(&host, "remote_root").unwrap(),
        world_position(&client, "local_root").unwrap()
    ));
    assert!(near(
        world_position(&client, "remote_root").unwrap(),
        world_position(&host, "local_root").unwrap()
    ));
    for scene in [&host, &client] {
        use crate::engine::ecs::component::{
            AvatarControlComponent, ControllerXRComponent, GLTFComponent, UdpChannelComponent,
        };
        let remote_avatar = scene
            .world
            .get_component_by_id_as::<AvatarControlComponent>(
                named(scene, "remote_avatar").unwrap(),
            )
            .unwrap();
        assert!(
            remote_avatar.head_mount.is_some(),
            "remote head solver was not initialized"
        );
        assert!(
            remote_avatar.left_hand_bone_id.is_some() && remote_avatar.right_hand_bone_id.is_some(),
            "remote arm IK was not initialized"
        );
        let model = scene
            .world
            .get_component_by_id_as::<GLTFComponent>(named(scene, "remote_model").unwrap())
            .unwrap();
        assert!(model.spawned && !model.morph_targets.is_empty());
        assert!(
            model.morph_factors.values().any(|f| f.external.is_some()),
            "no received face weights"
        );
        let channels: Vec<_> = scene
            .world
            .all_components()
            .filter_map(|id| {
                scene
                    .world
                    .get_component_by_id_as::<UdpChannelComponent>(id)
            })
            .collect();
        assert_eq!(channels.len(), 17);
        assert!(
            channels.iter().all(|c| c.ready),
            "unready channels: {:?}",
            channels
                .iter()
                .filter(|c| !c.ready)
                .map(|c| (&c.channel_name, &c.error))
                .collect::<Vec<_>>()
        );
        for hand in scene.world.all_components().filter_map(|id| {
            scene
                .world
                .get_component_by_id_as::<ControllerXRComponent>(id)
        }) {
            assert!(hand.externally_driven && hand.pose_valid);
        }
    }
    let host_hand = host
        .world
        .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(
            named(&host, "local_left_target").unwrap(),
        )
        .unwrap()
        .trs();
    let received_hand = client
        .world
        .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(
            named(&client, "remote_left_target").unwrap(),
        )
        .unwrap()
        .trs();
    assert!((host_hand.translation[1] - received_hand.translation[1]).abs() < 0.05);
    let host_ball = world_position(&host, "ball").unwrap();
    let client_ball = world_position(&client, "ball").unwrap();
    assert!(host_ball[0] > 0.5, "host ball should drift: {host_ball:?}");
    assert!(
        (client_ball[0] - host_ball[0]).abs() < 0.5,
        "ball host={host_ball:?} client={client_ball:?}"
    );
    assert!(
        status_text(&host).starts_with("rei (host, peer 1)"),
        "{}",
        status_text(&host)
    );
    assert!(
        status_text(&client).starts_with("dan (client, peer 2)"),
        "{}",
        status_text(&client)
    );
    assert!(status_text(&client).contains("remote players: 1"));
}

#[test]
fn multiplayer_example_supports_desktop_client_and_windowless_xr_host() {
    use crate::engine::ecs::component::{
        Camera3DComponent, CameraXRComponent, InputComponent, InputXRComponent,
        MultiplayerSessionComponent, RendererSettingsComponent, XrComponent,
    };
    let source = std::fs::read_to_string("examples/multiplayer-basic.mms").unwrap();
    for xr in [false, true] {
        let configured = source
            .replace("let use_xr = false", &format!("let use_xr = {xr}"))
            .replace(
                "let use_desktop = true",
                &format!("let use_desktop = {}", !xr),
            )
            .replace(
                "MultiplayerSession.from_env()",
                if xr {
                    r#"MultiplayerSession.host("127.0.0.1:0").player_name("rei")"#
                } else {
                    r#"MultiplayerSession.join("127.0.0.1:7010").player_name("dan")"#
                },
            );
        let scene = load(&configured, "examples/multiplayer-basic.mms");
        let count = |predicate: fn(&World, ComponentId) -> bool| {
            scene
                .world
                .all_components()
                .filter(|id| predicate(&scene.world, *id))
                .count()
        };
        assert_eq!(
            count(|w, id| w.get_component_by_id_as::<Camera3DComponent>(id).is_some()),
            usize::from(!xr)
        );
        assert_eq!(
            count(|w, id| w.get_component_by_id_as::<CameraXRComponent>(id).is_some()),
            usize::from(xr)
        );
        assert_eq!(
            count(|w, id| w.get_component_by_id_as::<InputXRComponent>(id).is_some()),
            usize::from(xr)
        );
        assert_eq!(
            count(|w, id| w.get_component_by_id_as::<InputComponent>(id).is_some()),
            usize::from(!xr)
        );
        let settings = scene
            .world
            .all_components()
            .find_map(|id| {
                scene
                    .world
                    .get_component_by_id_as::<RendererSettingsComponent>(id)
            })
            .unwrap();
        assert_eq!(settings.window_visible, !xr);
        assert_eq!(named(&scene, "multiplayer_xr_settings").is_some(), xr);
        assert_eq!(named(&scene, "xr_floor_correction_slider").is_some(), xr);
        let backend = scene
            .world
            .all_components()
            .find_map(|id| scene.world.get_component_by_id_as::<XrComponent>(id))
            .unwrap();
        assert_eq!(backend.enabled, xr);
        let session = scene
            .world
            .all_components()
            .find_map(|id| {
                scene
                    .world
                    .get_component_by_id_as::<MultiplayerSessionComponent>(id)
            })
            .unwrap();
        assert_eq!(session.is_host(), xr);
    }
}
