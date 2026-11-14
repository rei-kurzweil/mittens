use super::endpoint::{NetEndpoint, NetEvent, PEER_TIMEOUT};
use super::protocol::{MAX_DATAGRAM, Message, Packet, PeerInfo, Pose, VERSION, snapshot_chunks};
use crate::engine::ecs::component::NetRole;
use std::time::{Duration, Instant};

fn pose(x: f32) -> Pose {
    Pose {
        p: [x, 1.0, -2.5],
        r: [0.0, 0.0, 0.0, 1.0],
        s: [1.0; 3],
    }
}

fn host_and_client(now: Instant) -> (NetEndpoint, NetEndpoint) {
    let host = NetEndpoint::bind(NetRole::Host, "127.0.0.1:0", "rei", now).unwrap();
    let addr = host.local_addr().unwrap().to_string();
    let client = NetEndpoint::bind(NetRole::Client, &addr, "dan", now).unwrap();
    (host, client)
}

/// Loopback delivery is immediate but not synchronous with `send_to`.
fn pump(host: &mut NetEndpoint, client: &mut NetEndpoint, now: Instant) {
    for _ in 0..4 {
        client.poll(now);
        std::thread::sleep(Duration::from_millis(5));
        host.poll(now);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn packets_round_trip_and_reject_other_versions() {
    let packet = Packet {
        v: VERSION,
        session: uuid::Uuid::new_v4(),
        seq: 7,
        msg: Message::PlayerPose { pose: pose(1.0) },
    };
    let bytes = packet.encode().unwrap();
    assert_eq!(Packet::decode(&bytes), Some(packet.clone()));
    let mut other = packet;
    other.v = VERSION + 1;
    assert_eq!(Packet::decode(&other.encode().unwrap()), None);
}

#[test]
fn snapshot_chunks_fit_the_datagram_budget_and_keep_every_entity() {
    let peers = vec![PeerInfo {
        id: 1,
        name: "rei".into(),
    }];
    let entities: Vec<_> = (0..40)
        .map(|i| (format!("crate_{i}"), pose(i as f32 * 0.123_456)))
        .collect();
    let chunks = snapshot_chunks(9, peers, vec![(1, pose(0.0))], entities);
    assert!(chunks.len() > 1);
    let mut seen = 0;
    for (index, msg) in chunks.into_iter().enumerate() {
        let Message::Snapshot {
            tick,
            peers,
            entities,
            ..
        } = &msg
        else {
            panic!("expected snapshot");
        };
        assert_eq!(*tick, 9);
        assert_eq!(
            peers.is_some(),
            index == 0,
            "only the first chunk carries the roster"
        );
        seen += entities.len();
        let packet = Packet {
            v: VERSION,
            session: uuid::Uuid::new_v4(),
            seq: u64::MAX,
            msg,
        };
        assert!(packet.encode().unwrap().len() <= MAX_DATAGRAM);
    }
    assert_eq!(seen, 40);
}

#[test]
fn client_handshakes_and_both_sides_see_each_other() {
    let now = Instant::now();
    let (mut host, mut client) = host_and_client(now);
    assert_eq!(host.take_events(), vec![NetEvent::Connected { peer_id: 1 }]);
    pump(&mut host, &mut client, now);
    assert_eq!(
        host.take_events(),
        vec![NetEvent::PeerJoined {
            peer_id: 2,
            name: "dan".into()
        }]
    );
    assert_eq!(
        client.take_events(),
        vec![NetEvent::Connected { peer_id: 2 }]
    );

    // First snapshot carries the roster, the host's player, and host entities.
    host.publish(1, Some(pose(3.0)), vec![("ball".into(), pose(5.0))]);
    client.publish(1, Some(pose(-3.0)), Vec::new());
    pump(&mut host, &mut client, now);
    assert_eq!(
        client.take_events(),
        vec![NetEvent::PeerJoined {
            peer_id: 1,
            name: "rei".into()
        }]
    );
    assert_eq!(client.take_remote_players().get(&1), Some(&pose(3.0)));
    assert_eq!(client.take_host_entities().get("ball"), Some(&pose(5.0)));
    assert_eq!(host.take_remote_players().get(&2), Some(&pose(-3.0)));
    assert_eq!(client.local_peer_id(), Some(2));
}

#[test]
fn stale_snapshots_are_dropped() {
    let now = Instant::now();
    let (mut host, mut client) = host_and_client(now);
    pump(&mut host, &mut client, now);
    host.publish(5, None, vec![("ball".into(), pose(5.0))]);
    pump(&mut host, &mut client, now);
    assert!(client.take_host_entities().contains_key("ball"));
    host.publish(4, None, vec![("ball".into(), pose(4.0))]);
    pump(&mut host, &mut client, now);
    assert!(client.take_host_entities().is_empty());
}

#[test]
fn app_messages_flow_client_to_host_with_trusted_sender() {
    let now = Instant::now();
    let (mut host, mut client) = host_and_client(now);
    pump(&mut host, &mut client, now);
    host.take_events();
    client.take_events();
    client.send_app("launch", "{\"power\":2}").unwrap();
    host.send_app("score", "1").unwrap();
    pump(&mut host, &mut client, now);
    assert_eq!(
        host.take_events(),
        vec![NetEvent::App {
            from: 2,
            name: "launch".into(),
            text: "{\"power\":2}".into()
        }]
    );
    assert_eq!(
        client.take_events(),
        vec![NetEvent::App {
            from: 1,
            name: "score".into(),
            text: "1".into()
        }]
    );
}

#[test]
fn host_expires_silent_clients_and_dropped_clients_say_bye() {
    let now = Instant::now();
    let (mut host, mut client) = host_and_client(now);
    pump(&mut host, &mut client, now);
    host.take_events();
    host.poll(now + PEER_TIMEOUT + Duration::from_millis(1));
    assert_eq!(
        host.take_events(),
        vec![NetEvent::PeerLeft {
            peer_id: 2,
            name: "dan".into()
        }]
    );

    let (mut host, mut client) = host_and_client(now);
    pump(&mut host, &mut client, now);
    host.take_events();
    drop(client);
    std::thread::sleep(Duration::from_millis(10));
    host.poll(now);
    assert_eq!(
        host.take_events(),
        vec![NetEvent::PeerLeft {
            peer_id: 2,
            name: "dan".into()
        }]
    );
}

#[test]
fn client_send_before_welcome_is_an_error() {
    let now = Instant::now();
    // Nothing listens on this port; the client stays unwelcomed.
    let probe = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = probe.local_addr().unwrap().to_string();
    drop(probe);
    let mut client = NetEndpoint::bind(NetRole::Client, &addr, "dan", now).unwrap();
    assert!(client.send_app("x", "y").is_err());
}
