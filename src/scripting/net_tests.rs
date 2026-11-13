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
    return T { name = "proxy_" + event.name  NetSync.remote_player(event.peer_id) }
}
on(net, "NetPeerJoined", fn(event) { proxies.attach(remote_player(event)) })
Gravity.enabled(true) {
    Velocity { name = "ball_velocity"
        T.position(0.0, 5.0, 0.0) { name = "ball" NetSync.host("ball") }
    }
}
"#;

#[test]
fn host_and_client_replicate_players_host_physics_and_messages() {
    let host_source = format!(
        r#"
let net = NetSession.host("127.0.0.1:47011").player_name("rei") {{}}
net
T.position(1.0, 2.0, 3.0) {{ name = "local_player" NetSync.local_player() }}
T {{ name = "message_flag" }}
on(net, "NetMessage", fn(event) {{
    if event.name == "hello" && event.text == "dan" {{
        query("[name='message_flag']").update_transform([7.0, 7.0, 7.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    }}
}})
{SHARED}"#
    );
    let client_source = format!(
        r#"
let net = NetSession.join("127.0.0.1:47011").player_name("dan") {{}}
net
T.position(-4.0, 0.0, 1.5) {{ name = "local_player" NetSync.local_player() }}
on(net, "NetConnected", fn(event) {{ net.send("hello", net.local_name()) }})
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
    let configured = |session: &str| {
        assert!(source.contains("NetSession.from_env()"));
        source.replace("NetSession.from_env()", session)
    };
    let mut host = load(
        &configured(r#"NetSession.host("127.0.0.1:47012").player_name("rei")"#),
        "examples/multiplayer-basic.mms",
    );
    let mut client = load(
        &configured(r#"NetSession.join("127.0.0.1:47012").player_name("dan")"#),
        "examples/multiplayer-basic.mms",
    );
    for _ in 0..90 {
        host.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(1));
        client.update(1.0 / 60.0, &Default::default());
        std::thread::sleep(Duration::from_millis(1));
    }
    let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3);
    assert!(near(
        world_position(&host, "net_proxy_2").unwrap(),
        world_position(&client, "local_player").unwrap()
    ));
    assert!(near(
        world_position(&client, "net_proxy_1").unwrap(),
        world_position(&host, "local_player").unwrap()
    ));
    let host_ball = world_position(&host, "ball").unwrap();
    let client_ball = world_position(&client, "ball").unwrap();
    assert!(host_ball[0] > 0.5, "host ball should drift: {host_ball:?}");
    assert!((client_ball[0] - host_ball[0]).abs() < 0.5);
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
