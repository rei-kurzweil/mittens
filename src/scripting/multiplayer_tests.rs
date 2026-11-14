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
    let source = std::fs::read_to_string("examples/multiplayer-basic.mms").unwrap()
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true");
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
        assert_eq!(channels.len(), 21);
        assert!(named(scene, "shared_gohei").is_some());
        for avatar_name in ["local_avatar", "remote_avatar"] {
            let id = named(scene, avatar_name).unwrap();
            let feet = scene.systems.lower_body_ik.feet(id).unwrap_or_else(||
                panic!("{avatar_name}: {:?}", scene.systems.lower_body_ik.status(id))
            );
            for foot in feet {
                assert!(foot.support.is_some(), "{avatar_name} has no foot support: {foot:?}");
                assert!(foot.solved_sole[1].abs() < 0.015, "{avatar_name} foot must contact floor: {foot:?}");
            }
        }
        let local_avatar = scene.world.get_component_by_id_as::<AvatarControlComponent>(
            named(scene, "local_avatar").unwrap()
        ).unwrap();
        assert!(local_avatar.splice_camera_bone.is_some());
        assert_eq!(
            scene.world.parent_of(named(scene, "local_desktop_camera_anchor").unwrap()),
            local_avatar.splice_camera_bone,
            "desktop camera must follow the local avatar's mapped camera anchor"
        );
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
    let source = std::fs::read_to_string("examples/multiplayer-basic.mms").unwrap()
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true");
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
        let desktop_anchor = named(&scene, "local_desktop_camera_anchor");
        assert_eq!(desktop_anchor.is_some(), !xr);
        if let Some(anchor) = desktop_anchor {
            use crate::engine::ecs::component::TransformComponent;
            assert_eq!(scene.world.get_component_by_id_as::<TransformComponent>(anchor).unwrap().transform.translation, [0.0, 0.08, 0.12]);
            assert_eq!(scene.world.parent_of(anchor), named(&scene, "local_avatar"));
        }
        let camera_anchor = named(&scene, "local_xr_camera_anchor");
        assert_eq!(camera_anchor.is_some(), xr);
        if let Some(anchor) = camera_anchor {
            use crate::engine::ecs::component::TransformComponent;
            let transform = scene.world.get_component_by_id_as::<TransformComponent>(anchor).unwrap();
            assert_eq!(transform.transform.translation, [0.0, 0.08, 0.12]);
            assert!(scene.world.children_of(anchor).iter().any(|id|
                scene.world.get_component_by_id_as::<CameraXRComponent>(*id).is_some()
            ));
        }
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

#[test]
fn multiplayer_desktop_render_camera_tracks_local_head() {
    use crate::engine::ecs::component::{AvatarControlComponent, Camera3DComponent};
    use crate::engine::graphics::CameraTarget;
    let source = std::fs::read_to_string("examples/multiplayer-basic.mms").unwrap()
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true")
        .replace("0.0.0.0:", "127.0.0.1:")
        .replace("7011", "0")
        .replace("MultiplayerSession.from_env()", r#"MultiplayerSession.host("127.0.0.1:0")"#);
    let mut scene = load(&source, "examples/multiplayer-basic.mms");
    for _ in 0..20 { scene.update(1. / 60., &Default::default()); }
    let camera = scene.world.all_components().find(|id|
        scene.world.get_component_by_id_as::<Camera3DComponent>(*id).is_some()
    ).unwrap();
    assert_eq!(scene.systems.camera.active_window_camera_component(), Some(camera));
    let local = named(&scene, "local_avatar").unwrap();
    assert_eq!(scene.world.get_component_by_id_as::<AvatarControlComponent>(local).unwrap().humanoid_map_gltf, named(&scene, "local_model"));
    let root = named(&scene, "local_root").unwrap();
    let check = |scene: &Universe| {
        let actual = scene.visuals.visual_camera(CameraTarget::Window).unwrap().eyes[0];
        let expected = TransformSystem::world_position(&scene.world, camera).unwrap();
        eprintln!("desktop rendered {:?}; camera world {:?}; root {:?}", actual.transform.translation, expected, world_position(scene, "local_root"));
        assert!((0..3).all(|i| (actual.transform.translation[i] - expected[i]).abs() < 1e-4));
        let view = scene.systems.camera.active_window_camera_matrices().unwrap().0;
        assert_eq!(actual.view, view);
        expected
    };
    let before = check(&scene);
    use crate::engine::ecs::{SignalEmitter, IntentValue};
    scene.command_queue.push_intent_now(root, IntentValue::PatchTransformTrs {
        component_id: root, space: crate::engine::transform::TransformSpace::World,
        translation: Some([3., 0., 2.]), rotation: None,
    });
    for _ in 0..12 { scene.update(1. / 60., &Default::default()); }
    let after = check(&scene);
    assert!((after[0] - before[0]).abs() > 1.0 && (after[2] - before[2]).abs() > 1.0);
}

#[test]
fn multiplayer_eye_offset_roundtrips_and_rejects_nonfinite_calibration() {
    use crate::engine::ecs::component::{AvatarControlComponent, Component};
    let original = AvatarControlComponent::new().with_eye_offset([0.0, 0.08, 0.12]).unwrap();
    for offset in [[f32::NAN, 0., 0.], [0., f32::INFINITY, 0.]] {
        assert!(AvatarControlComponent::new().with_eye_offset(offset).is_err());
    }
    let ast = original.to_mms_ast(&World::default());
    let source = crate::scripting::unparser::unparse_component(&ast);
    let scene = load(&source, "examples/_eye-offset-test.mms");
    let restored = scene.world.all_components().find_map(|id|
        scene.world.get_component_by_id_as::<AvatarControlComponent>(id)
    ).unwrap();
    assert_eq!(restored.eye_offset, original.eye_offset);
    assert_eq!(AvatarControlComponent::new().eye_offset, None);
}
