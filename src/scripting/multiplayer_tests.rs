use crate::engine::Universe;
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

#[test]
fn multiplayer_session_connects_roster_and_app_messages_without_sync() {
    let mut host = load(
        r#"
XR.off()
let net = MultiplayerSession.host("127.0.0.1:47011").player_name("rei") {}
net
T { name = "message_flag" }
on(net, "MultiplayerMessage", fn(event) {
    if event.name == "hello" && event.text == "dan" {
        query("[name='message_flag']").update_transform([7., 7., 7.], [0., 0., 0.], [1., 1., 1.])
    }
})
"#,
        "examples/_session-host-test.mms",
    );
    let mut client = load(
        r#"
XR.off()
let net = MultiplayerSession.join("127.0.0.1:47011").player_name("dan") {}
net
on(net, "MultiplayerConnected", fn(event) { net.send("hello", net.local_name()) })
"#,
        "examples/_session-client-test.mms",
    );
    for _ in 0..90 {
        host.update(1. / 60., &Default::default());
        client.update(1. / 60., &Default::default());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(world_position(&host, "message_flag"), Some([7.; 3]));
    use crate::engine::ecs::component::MultiplayerSessionComponent;
    for scene in [&host, &client] {
        let session = scene
            .world
            .all_components()
            .find_map(|id| {
                scene
                    .world
                    .get_component_by_id_as::<MultiplayerSessionComponent>(id)
            })
            .unwrap();
        assert_ne!(session.peer_id, 0);
        assert_eq!(session.peer_ips.len(), 1);
    }
}

#[test]
fn removed_multiplayer_sync_is_not_an_mms_component() {
    let mut scene = Universe::new(World::default());
    assert!(
        scene
            .load_mms_source_at_path(
                "T { MultiplayerSync.local_player() }",
                "examples/_removed-sync-test.mms"
            )
            .is_err()
    );
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
    let source = std::fs::read_to_string("examples/multiplayer-basic.mms")
        .unwrap()
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
        assert!(
            scene
                .world
                .get_component_by_id_as::<MirrorComponent>(
                    named(scene, "multiplayer_mirror").unwrap()
                )
                .is_some()
        );
        let animations: Vec<_> = scene
            .world
            .all_components()
            .filter_map(|id| scene.world.get_component_by_id_as::<AnimationComponent>(id))
            .collect();
        assert_eq!(
            animations
                .iter()
                .filter(
                    |clip| clip.state == AnimationState::Looping && clip.length_beats == Some(64.2)
                )
                .count(),
            2,
            "both avatar copies must have ambient gaze clips"
        );
        assert_eq!(
            animations
                .iter()
                .filter(
                    |clip| clip.state == AnimationState::Looping && clip.length_beats == Some(5.6)
                )
                .count(),
            1,
            "only the local desktop avatar authors the blink clip"
        );
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
        assert!(remote_avatar.collision_enabled && remote_avatar.capsule_ready);
        assert_eq!(
            remote_avatar.collision_response,
            crate::engine::ecs::component::CollisionResponse::ReadOnly
        );
        let remote_collider = scene
            .world
            .get_component_by_id_as::<crate::engine::ecs::component::CollidableComponent>(
                remote_avatar.capsule_collidable_id.unwrap(),
            )
            .unwrap();
        assert!(remote_collider.enabled);
        assert_eq!(
            remote_collider.response,
            crate::engine::ecs::component::CollisionResponse::ReadOnly
        );
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
            let feet = scene.systems.lower_body_ik.feet(id).unwrap_or_else(|| {
                panic!(
                    "{avatar_name}: {:?}",
                    scene.systems.lower_body_ik.status(id)
                )
            });
            for foot in feet {
                assert!(
                    foot.support.is_some(),
                    "{avatar_name} has no foot support: {foot:?}"
                );
                assert!(
                    foot.solved_sole[1].abs() < 0.015,
                    "{avatar_name} foot must contact floor: {foot:?}"
                );
            }
        }
        let local_avatar = scene
            .world
            .get_component_by_id_as::<AvatarControlComponent>(named(scene, "local_avatar").unwrap())
            .unwrap();
        assert!(local_avatar.splice_camera_bone.is_some());
        assert_eq!(
            scene
                .world
                .parent_of(named(scene, "local_desktop_camera_anchor").unwrap()),
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
    let source = std::fs::read_to_string("examples/multiplayer-basic.mms")
        .unwrap()
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
            assert_eq!(
                scene
                    .world
                    .get_component_by_id_as::<TransformComponent>(anchor)
                    .unwrap()
                    .transform
                    .translation,
                [0.0, 0.08, 0.12]
            );
            assert_eq!(scene.world.parent_of(anchor), named(&scene, "local_avatar"));
        }
        let camera_anchor = named(&scene, "local_xr_camera_anchor");
        assert_eq!(camera_anchor.is_some(), xr);
        if let Some(anchor) = camera_anchor {
            use crate::engine::ecs::component::TransformComponent;
            let transform = scene
                .world
                .get_component_by_id_as::<TransformComponent>(anchor)
                .unwrap();
            assert_eq!(transform.transform.translation, [0.0, 0.08, 0.12]);
            assert!(scene.world.children_of(anchor).iter().any(|id| {
                scene
                    .world
                    .get_component_by_id_as::<CameraXRComponent>(*id)
                    .is_some()
            }));
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
    let source = std::fs::read_to_string("examples/multiplayer-basic.mms")
        .unwrap()
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true")
        .replace("0.0.0.0:", "127.0.0.1:")
        .replace("7011", "0")
        .replace(
            "MultiplayerSession.from_env()",
            r#"MultiplayerSession.host("127.0.0.1:0")"#,
        );
    let mut scene = load(&source, "examples/multiplayer-basic.mms");
    for _ in 0..20 {
        scene.update(1. / 60., &Default::default());
    }
    let camera = scene
        .world
        .all_components()
        .find(|id| {
            scene
                .world
                .get_component_by_id_as::<Camera3DComponent>(*id)
                .is_some()
        })
        .unwrap();
    assert_eq!(
        scene.systems.camera.active_window_camera_component(),
        Some(camera)
    );
    let local = named(&scene, "local_avatar").unwrap();
    assert_eq!(
        scene
            .world
            .get_component_by_id_as::<AvatarControlComponent>(local)
            .unwrap()
            .humanoid_map_gltf,
        named(&scene, "local_model")
    );
    let root = named(&scene, "local_root").unwrap();
    let check = |scene: &Universe| {
        let actual = scene
            .visuals
            .visual_camera(CameraTarget::Window)
            .unwrap()
            .eyes[0];
        let expected = TransformSystem::world_position(&scene.world, camera).unwrap();
        eprintln!(
            "desktop rendered {:?}; camera world {:?}; root {:?}",
            actual.transform.translation,
            expected,
            world_position(scene, "local_root")
        );
        assert!((0..3).all(|i| (actual.transform.translation[i] - expected[i]).abs() < 1e-4));
        let view = scene
            .systems
            .camera
            .active_window_camera_matrices()
            .unwrap()
            .0;
        assert_eq!(actual.view, view);
        expected
    };
    let before = check(&scene);
    use crate::engine::ecs::{IntentValue, SignalEmitter};
    scene.command_queue.push_intent_now(
        root,
        IntentValue::PatchTransformTrs {
            component_id: root,
            space: crate::engine::transform::TransformSpace::World,
            translation: Some([3., 0., 2.]),
            rotation: None,
        },
    );
    for _ in 0..12 {
        scene.update(1. / 60., &Default::default());
    }
    let after = check(&scene);
    assert!((after[0] - before[0]).abs() > 1.0 && (after[2] - before[2]).abs() > 1.0);
}

#[test]
fn multiplayer_eye_offset_roundtrips_and_rejects_nonfinite_calibration() {
    use crate::engine::ecs::component::{AvatarControlComponent, Component};
    let original = AvatarControlComponent::new()
        .with_eye_offset([0.0, 0.08, 0.12])
        .unwrap();
    for offset in [[f32::NAN, 0., 0.], [0., f32::INFINITY, 0.]] {
        assert!(
            AvatarControlComponent::new()
                .with_eye_offset(offset)
                .is_err()
        );
    }
    let ast = original.to_mms_ast(&World::default());
    let source = crate::scripting::unparser::unparse_component(&ast);
    let scene = load(&source, "examples/_eye-offset-test.mms");
    let restored = scene
        .world
        .all_components()
        .find_map(|id| {
            scene
                .world
                .get_component_by_id_as::<AvatarControlComponent>(id)
        })
        .unwrap();
    assert_eq!(restored.eye_offset, original.eye_offset);
    assert_eq!(AvatarControlComponent::new().eye_offset, None);
}

fn twilight_rooftop_source() -> String {
    std::fs::read_to_string("examples/twilight_rooftop.mms").unwrap()
}

#[test]
fn twilight_rooftop_loads_xr_only_without_a_peer() {
    use crate::engine::ecs::component::material::{
        MaterialComponent, MaterialDefinitionComponent, ShadingModel,
    };
    use crate::engine::ecs::component::texture::TextureSource;
    use crate::engine::ecs::component::{
        Camera3DComponent, CameraXRComponent, ColorComponent, RenderableComponent, TextureComponent,
    };
    use crate::engine::graphics::materials::OpaqueBucket;
    // Default flags: XR-only, solo host still waiting for a peer.
    let mut scene = load(&twilight_rooftop_source(), "examples/twilight_rooftop.mms");
    for _ in 0..10 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut super::rocket_motion_tests::Uploader::default(),
        &mut scene.command_queue,
    );
    scene.update(1.0 / 60.0, &Default::default());
    let world = &scene.world;
    let count = |predicate: &dyn Fn(ComponentId) -> bool| {
        world.all_components().filter(|&id| predicate(id)).count()
    };
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<CameraXRComponent>(id)
            .is_some()),
        1
    );
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<Camera3DComponent>(id)
            .is_some()),
        0
    );
    for label in [
        "twilight_rooftop_main",
        "twilight_rooftop_north",
        "twilight_rooftop_east",
        "twilight_rooftop_train",
        "twilight_rooftop_train_motion",
        "twilight_rooftop_graphics_settings",
        "twilight_rooftop_mirror",
    ] {
        assert!(named(&scene, label).is_some(), "missing {label}");
    }
    for label in [
        "twilight_rooftop_rail_arc_rails",
        "twilight_rooftop_rail_arc_wooden_sleepers",
        "twilight_rooftop_round_hole_wall",
    ] {
        assert!(named(&scene, label).is_some(), "missing {label}");
    }
    let grip = named(&scene, "rooftop_sta_launcher_grip").unwrap();
    let grip_position = TransformSystem::world_position(world, grip).unwrap();
    let model_origin = world_position(&scene, "front_half").unwrap();
    assert!(
        crate::utils::math::vec3_len(crate::utils::math::vec3_sub(model_origin, grip_position))
            < 0.001,
        "launcher GLB origin {model_origin:?} must line up with grip {grip_position:?}"
    );
    let hits = scene.systems.raycast.cast_against_renderables_bvh(
        world,
        &scene.systems.bvh,
        [grip_position[0], grip_position[1] + 0.3, grip_position[2]],
        [0.0, -1.0, 0.0],
        0.0,
        0.35,
    );
    assert!(
        hits.first().is_some_and(|&(renderable, _)| {
            crate::engine::ecs::system::grabbable_system::grabbable_owner_for_hit(world, renderable)
                == Some(grip)
        }),
        "a ray over the pedestal should hit the launcher grip before the stand: {hits:?}"
    );
    let sky = count(&|id| {
        world
            .get_component_by_id_as::<MaterialComponent>(id)
            .and_then(|m| world.get_component_by_id_as::<MaterialDefinitionComponent>(m.definition))
            .is_some_and(|d| d.fragment_uri == "assets/shaders/rooftop-dusk-sky.frag")
    });
    assert_eq!(sky, 1);
    let paint_sources = named(&scene, "pickup_painted_body").unwrap();
    let chassis = world.find_component(paint_sources, "renderable").unwrap();
    let paint = world.find_component(chassis, "color").unwrap();
    assert_eq!(
        world.get_component_by_id_as::<ColorComponent>(paint).unwrap().rgba,
        [1.0, 0.606, 0.047, 1.0]
    );
    let finish = named(&scene, "pickup_anime_material").unwrap();
    let finish = world.get_component_by_id_as::<MaterialComponent>(finish).unwrap();
    let definition = world
        .get_component_by_id_as::<MaterialDefinitionComponent>(finish.definition)
        .unwrap();
    assert_eq!(definition.opaque_bucket, Some(OpaqueBucket::Near));
    assert_eq!(definition.builtin, Some(ShadingModel::Anime));
    assert_eq!(finish.values["shade_strength"], [0.15]);
    assert_eq!(finish.values["shade_threshold"], [0.2875]);
    assert_eq!(finish.values["lit_threshold"], [0.6625]);
    for suffix in ["", ".001", ".002", ".003"] {
        let wheel = named(&scene, &format!("front-left-WHEEL{suffix}")).unwrap();
        let primitives: Vec<_> = world
            .children_of(wheel)
            .iter()
            .copied()
            .filter(|&id| world.get_component_by_id_as::<RenderableComponent>(id).is_some())
            .collect();
        let [tire, rim] = primitives.as_slice() else {
            panic!("expected rubber and rim primitives for wheel {suffix}");
        };
        let tread = world.find_component(*tire, "texture").unwrap();
        let tread = world.get_component_by_id_as::<TextureComponent>(tread).unwrap();
        assert!(matches!(&tread.source, TextureSource::Uri(uri)
            if uri == "assets/textures/vehicles/tire-tread.png"));
        assert!(world.find_component(*rim, "texture").is_none());
    }
    // Both players stand on the main roof (5 floors x 3.2 m).
    let local = world_position(&scene, "local_root").unwrap();
    assert!(
        (local[1] - 16.0).abs() < 1e-3 && (local[0] + 19.5).abs() < 1e-3,
        "{local:?}"
    );
    assert!(status_text(&scene).contains("remote players: 0"));
    // A solo session is the host: Mitten locally, the client's Lumae as remote.
    assert_eq!(model_uri(&scene, "local_model"), "assets/models/mitten.glb");
    assert_eq!(model_uri(&scene, "remote_model"), LUMAE);

    // Aim the rooftop controller from the player's side, slightly above the
    // grip center, then exercise the same ray, gesture and hold stages as XR.
    let pointer = named(&scene, "local_right_pointer").unwrap();
    let systems = &mut scene.systems;
    let ray = systems.pointer.raycast_for_pointer(pointer).unwrap();
    let mut hand = ray;
    while scene
        .world
        .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(hand)
        .is_none()
    {
        hand = scene.world.parent_of(hand).unwrap();
    }
    let mut pose = crate::utils::math::mat4_identity();
    pose[3] = [
        grip_position[0],
        grip_position[1] + 0.075,
        grip_position[2] + 0.45,
        1.0,
    ];
    scene
        .world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(hand)
        .unwrap()
        .transform
        .matrix_world = pose;
    let original_parent = scene.world.parent_of(grip);
    let input = Default::default();
    let activations = crate::engine::ecs::system::pointer_system::PointerActivations {
        grip_pressed: vec![pointer],
        grip_down: vec![pointer],
        ..Default::default()
    };
    systems.gesture.begin_frame();
    systems.raycast.tick_with_queue(
        &mut scene.world,
        &mut scene.visuals,
        &input,
        &mut systems.rx,
        &systems.bvh,
        &activations,
        &systems.pointer,
        1.0 / 60.0,
    );
    systems.process_signals(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut scene.command_queue,
        100_000,
    );
    systems.gesture.tick_with_rx_and_attachments(
        &mut scene.world,
        &input,
        &activations,
        &systems.pointer,
        &systems.raycast,
        &mut systems.attachment,
        &mut systems.rx,
    );
    systems.process_signals(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut scene.command_queue,
        100_000,
    );
    systems.grabbable.tick(
        &mut scene.world,
        &scene.render_assets,
        &mut systems.rx,
        1.0 / 60.0,
    );
    let held_parent = scene
        .world
        .parent_of(grip)
        .expect("launcher attached to hand");
    assert_ne!(
        Some(held_parent),
        original_parent,
        "launcher stayed on the pedestal"
    );
    let config = scene
        .world
        .get_component_by_id_as::<crate::engine::ecs::component::PointerComponent>(pointer)
        .unwrap();
    assert!(held_parent == hand || Some(held_parent) == config.avatar_grab_attachment);
}

const LUMAE: &str = "assets/components/characters/lumae/lumae.glb";

fn model_uri(scene: &Universe, name: &str) -> String {
    use crate::engine::ecs::component::GLTFComponent;
    scene
        .world
        .get_component_by_id_as::<GLTFComponent>(named(scene, name).unwrap())
        .unwrap()
        .uri
        .clone()
}

#[test]
fn twilight_rooftop_runs_as_host_and_client_on_the_roof() {
    let source = twilight_rooftop_source()
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
        &configured(r#"MultiplayerSession.host("127.0.0.1:47013").player_name("rei")"#),
        "examples/twilight_rooftop.mms",
    );
    let mut client = load(
        &configured(r#"MultiplayerSession.join("127.0.0.1:47013").player_name("dan")"#),
        "examples/twilight_rooftop.mms",
    );
    for _ in 0..120 {
        host.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(1));
        client.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(1));
    }
    for scene in [&host, &client] {
        assert!(
            status_text(scene).contains("remote players: 1"),
            "{}",
            status_text(scene)
        );
    }
    let near =
        |a: [f32; 3], b: [f32; 3], tolerance: f32| (0..3).all(|i| (a[i] - b[i]).abs() < tolerance);
    for (receiver, sender) in [(&host, &client), (&client, &host)] {
        let remote = world_position(receiver, "remote_root").unwrap();
        let local = world_position(sender, "local_root").unwrap();
        assert!(
            near(remote, local, 1e-3),
            "remote {remote:?} vs peer local {local:?}"
        );
        assert!(
            (remote[1] - 16.0).abs() < 0.05,
            "players stay on the roof: {remote:?}"
        );
    }
    // Each side renders the peer with the peer's role-derived avatar.
    for (scene, local, remote) in [
        (&host, "assets/models/mitten.glb", LUMAE),
        (&client, LUMAE, "assets/models/mitten.glb"),
    ] {
        assert_eq!(model_uri(scene, "local_model"), local);
        assert_eq!(model_uri(scene, "remote_model"), remote);
        let avatar = scene
            .world
            .get_component_by_id_as::<crate::engine::ecs::component::AvatarControlComponent>(
                named(scene, "remote_avatar").unwrap(),
            )
            .unwrap();
        assert!(
            avatar.head_mount.is_some() && avatar.left_hand_bone_id.is_some(),
            "remote {remote} rig was not initialized"
        );
    }
}

fn labelled_ancestor(world: &World, id: ComponentId, prefix: &str) -> bool {
    let mut cursor = Some(id);
    while let Some(id) = cursor {
        if world
            .component_label(id)
            .is_some_and(|label| label.starts_with(prefix))
        {
            return true;
        }
        cursor = world.parent_of(id);
    }
    false
}

#[test]
fn twilight_rooftop_train_clears_east_facades_and_reverses_at_both_ends() {
    use crate::engine::ecs::component::TransformComponent;
    use crate::engine::ecs::{EventSignal, SignalKind};
    use std::sync::{Arc, Mutex};
    let mut scene = load(&twilight_rooftop_source(), "examples/twilight_rooftop.mms");
    let train_motion = named(&scene, "twilight_rooftop_train_motion").unwrap();
    let contacts = Arc::new(Mutex::new(Vec::new()));
    let sink = contacts.clone();
    scene
        .systems
        .rx
        .add_global_handler_closure(SignalKind::ZoneContact, move |_, _, signal| {
            if let Some(EventSignal::ZoneContact {
                zone, other_zone, velocity, ..
            }) = signal.event
                && velocity == train_motion
            {
                sink.lock().unwrap().push((zone, other_zone));
            }
        });
    let pivot = named(&scene, "twilight_rooftop_train").unwrap();
    let yaw = |scene: &Universe| {
        let q = scene
            .world
            .get_component_by_id_as::<TransformComponent>(pivot)
            .unwrap()
            .transform
            .rotation;
        2.0 * q[1].atan2(q[3])
    };
    // (frame, yaw, contacts so far)
    let mut track = Vec::new();
    for frame in 0..420 {
        scene.update(1.0 / 30.0, &Default::default());
        track.push((frame, yaw(&scene), contacts.lock().unwrap().len()));
    }
    let contacts = contacts.lock().unwrap().clone();
    let world = &scene.world;
    let car_label = "twilight_rooftop_train_car_0";
    let car = named(&scene, car_label).unwrap();
    let car_world = TransformSystem::world_model(world, car).unwrap();
    let shell_boxes: Vec<_> = world
        .find_all_components(car, "renderable")
        .into_iter()
        .map(|id| {
            crate::utils::math::mat4_inverse(TransformSystem::world_model(world, id).unwrap())
                .unwrap()
        })
        .collect();
    assert!(!shell_boxes.is_empty());
    // A standing avatar can fit through either doorway and under the roof.
    for x in [-0.92, -0.85, 0.0, 0.85, 0.92] {
        for z in [-0.72, 0.0, 0.72] {
            for y in [0.30, 1.4, 2.52] {
                let point = crate::utils::math::mat4_mul_vec4(car_world, [x, y, z, 1.0]);
                assert!(
                    shell_boxes.iter().all(|&inverse| {
                        let local = crate::utils::math::mat4_mul_vec4(inverse, point);
                        local[..3].iter().any(|value| value.abs() > 0.5)
                    }),
                    "blocked car passage at {:?}",
                    [x, y, z]
                );
            }
        }
    }
    // Exactly two arrivals in 14 s: the round hole, then the main building.
    // Crossing the east building's rectangular openings reports nothing.
    assert_eq!(contacts.len(), 2, "{contacts:?}");
    let (zone, round) = contacts[0];
    assert!(
        labelled_ancestor(world, zone, "twilight_rooftop_train_car_1_sensor"),
        "leading car"
    );
    assert!(
        labelled_ancestor(world, round, "twilight_rooftop_round_hole_wall_ring_"),
        "blocked by the ring, not the wall around it"
    );
    let (zone, main) = contacts[1];
    assert!(
        labelled_ancestor(world, zone, "twilight_rooftop_train_car_0_sensor"),
        "leading car"
    );
    assert!(labelled_ancestor(world, main, "twilight_rooftop_main"));
    // Turning points match the geometry: the cars' corners meet the ring at
    // about 1.222 rad and the main balcony railing at about -1.074 rad.
    let first = track.iter().position(|&(_, _, n)| n == 1).unwrap();
    let second = track.iter().position(|&(_, _, n)| n == 2).unwrap();
    let north = track[..second]
        .iter()
        .map(|&(_, y, _)| y)
        .fold(f32::MIN, f32::max);
    let south = track[first..]
        .iter()
        .map(|&(_, y, _)| y)
        .fold(f32::MAX, f32::min);
    assert!((north - 1.222).abs() < 0.03, "north turn {north}");
    assert!((south + 1.074).abs() < 0.03, "south turn {south}");
    // Both runs pass the east building (yaw 0) with no contact.
    assert!(track[..first].iter().any(|&(_, y, _)| y > 0.3));
    assert!(track[first..second].iter().any(|&(_, y, _)| y < -0.3));
    // After the second reversal the train heads north again.
    assert!(track.last().unwrap().1 > south + 0.2);
}

#[test]
fn twilight_rooftop_local_player_has_voice_mouth_and_finger_tracking() {
    use crate::engine::ecs::component::{AvatarControlComponent, HandTrackingComponent};
    let mut scene = load(&twilight_rooftop_source(), "examples/twilight_rooftop.mms");
    for _ in 0..10 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let world = &scene.world;
    let avc = |name: &str| {
        world
            .get_component_by_id_as::<AvatarControlComponent>(named(&scene, name).unwrap())
            .unwrap()
    };
    assert!(
        avc("local_avatar").mouth_open_amplitude.is_some(),
        "local mouth follows voice"
    );
    assert!(
        avc("remote_avatar").mouth_open_amplitude.is_none(),
        "remote mouth is not local voice"
    );
    let tracked = |name: &str| {
        world
            .children_of(named(&scene, name).unwrap())
            .iter()
            .any(|&id| {
                world
                    .get_component_by_id_as::<HandTrackingComponent>(id)
                    .is_some()
            })
    };
    assert!(tracked("local_model"), "local XR model tracks fingers");
    assert!(!tracked("remote_model"), "remote model does not");
}
