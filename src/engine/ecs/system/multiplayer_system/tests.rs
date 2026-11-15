use super::endpoint::{MultiplayerEndpoint, MultiplayerEvent, PEER_TIMEOUT};
use super::protocol::{MAX_DATAGRAM, Message, Packet, PeerInfo, Pose, VERSION, snapshot_chunks};
use crate::engine::ecs::component::MultiplayerRole;
use std::time::{Duration, Instant};

fn pose(x: f32) -> Pose {
    Pose {
        p: [x, 1.0, -2.5],
        r: [0.0, 0.0, 0.0, 1.0],
        s: [1.0; 3],
    }
}

fn host_and_client(now: Instant) -> (MultiplayerEndpoint, MultiplayerEndpoint) {
    let host = MultiplayerEndpoint::bind(MultiplayerRole::Host, "127.0.0.1:0", "rei", now).unwrap();
    let addr = host.local_addr().unwrap().to_string();
    let client = MultiplayerEndpoint::bind(MultiplayerRole::Client, &addr, "dan", now).unwrap();
    (host, client)
}

/// Loopback delivery is immediate but not synchronous with `send_to`.
fn pump(host: &mut MultiplayerEndpoint, client: &mut MultiplayerEndpoint, now: Instant) {
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
    assert_eq!(
        host.take_events(),
        vec![MultiplayerEvent::Connected { peer_id: 1 }]
    );
    pump(&mut host, &mut client, now);
    assert_eq!(
        host.take_events(),
        vec![MultiplayerEvent::PeerJoined {
            peer_id: 2,
            name: "dan".into()
        }]
    );
    assert_eq!(
        client.take_events(),
        vec![MultiplayerEvent::Connected { peer_id: 2 }]
    );

    // Session snapshots carry only the roster; spatial telemetry uses typed channels.
    host.publish(1);
    client.publish(1);
    pump(&mut host, &mut client, now);
    assert_eq!(
        client.take_events(),
        vec![MultiplayerEvent::PeerJoined {
            peer_id: 1,
            name: "rei".into()
        }]
    );
    assert_eq!(client.local_peer_id(), Some(2));
}

#[test]
fn stale_roster_snapshots_are_dropped() {
    let now = Instant::now();
    let (mut host, mut client) = host_and_client(now);
    pump(&mut host, &mut client, now);
    host.publish(5);
    pump(&mut host, &mut client, now);
    client.take_events();
    let mut third = MultiplayerEndpoint::bind(
        MultiplayerRole::Client,
        &host.local_addr().unwrap().to_string(),
        "third",
        now,
    )
    .unwrap();
    pump(&mut host, &mut third, now);
    host.publish(4);
    pump(&mut host, &mut client, now);
    assert!(
        client.take_events().is_empty(),
        "old roster must not introduce third"
    );
    host.publish(6);
    pump(&mut host, &mut client, now);
    assert!(client.take_events().iter().any(|event| matches!(event,
        MultiplayerEvent::PeerJoined { name, .. } if name == "third")));
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
        vec![MultiplayerEvent::App {
            from: 2,
            name: "launch".into(),
            text: "{\"power\":2}".into()
        }]
    );
    assert_eq!(
        client.take_events(),
        vec![MultiplayerEvent::App {
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
        vec![MultiplayerEvent::PeerLeft {
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
        vec![MultiplayerEvent::PeerLeft {
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
    let mut client = MultiplayerEndpoint::bind(MultiplayerRole::Client, &addr, "dan", now).unwrap();
    assert!(client.send_app("x", "y").is_err());
}

#[test]
fn message_only_sessions_stay_alive_without_legacy_pose_replication() {
    let start = Instant::now();
    let (mut host, mut client) = host_and_client(start);
    pump(&mut host, &mut client, start);
    host.take_events();
    client.take_events();
    for tick in 1..=12 {
        let now = start + Duration::from_millis(tick * 500);
        client.publish(tick);
        host.publish(tick);
        pump(&mut host, &mut client, now);
        assert!(
            !host
                .take_events()
                .iter()
                .any(|e| matches!(e, MultiplayerEvent::PeerLeft { .. }))
        );
        assert!(
            !client
                .take_events()
                .iter()
                .any(|e| matches!(e, MultiplayerEvent::PeerLeft { .. }))
        );
    }
}
