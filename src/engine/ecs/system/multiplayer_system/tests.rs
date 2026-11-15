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

#[test]
fn roster_admission_preserves_unicode_and_keeps_admitted_peers_alive() {
    for (name, expect_rejection) in [
        ("🙂".into(), false),
        ("a".repeat(64), true),
        ("\"".repeat(64), true),
        ("🙂".repeat(16), true),
    ] {
        let start = Instant::now();
        let mut host =
            MultiplayerEndpoint::bind(MultiplayerRole::Host, "127.0.0.1:0", &name, start).unwrap();
        let addr = host.local_addr().unwrap().to_string();
        let mut clients: Vec<_> = (0..16)
            .map(|_| {
                MultiplayerEndpoint::bind(MultiplayerRole::Client, &addr, &name, start).unwrap()
            })
            .collect();
        for client in &mut clients {
            pump(&mut host, client, start);
        }
        let admitted = clients
            .iter()
            .filter(|c| c.local_peer_id().is_some())
            .count();
        assert!(admitted > 0);
        assert_eq!(admitted < 16, expect_rejection);
        let events = host.take_events();
        assert_eq!(
            events.iter().any(|e| matches!(
                e,
                MultiplayerEvent::Error {
                    phase: "admission",
                    ..
                }
            )),
            expect_rejection
        );
        assert!(
            events
                .iter()
                .filter_map(
                    |e| if let MultiplayerEvent::PeerJoined { name: actual, .. } = e {
                        Some(actual)
                    } else {
                        None
                    }
                )
                .all(|actual| actual == &name)
        );
        for tick in 1..=10 {
            let now = start + Duration::from_secs(tick);
            for client in &mut clients {
                client.publish(tick);
                client.poll(now);
            }
            host.poll(now);
            host.publish(tick);
            for client in &mut clients {
                client.poll(now);
            }
            assert_eq!(
                clients
                    .iter()
                    .filter(|c| c.local_peer_id().is_some())
                    .count(),
                admitted
            );
            assert!(
                !host
                    .take_events()
                    .iter()
                    .any(|e| matches!(e, MultiplayerEvent::PeerLeft { .. }))
            );
        }
    }
}

#[test]
fn roster_budget_checks_escaping_and_worst_case_envelope() {
    use super::protocol::roster_fits;
    for name in ["a".repeat(64), "\"".repeat(64), "👩🏽‍💻".repeat(4)] {
        let mut peers = vec![PeerInfo {
            id: 1,
            name: name.clone(),
        }];
        assert!(roster_fits(peers.clone()));
        while roster_fits(peers.clone()) {
            peers.push(PeerInfo {
                id: peers.len() as u32 + 1,
                name: name.clone(),
            });
        }
        peers.pop();
        let chunks = snapshot_chunks(u64::MAX, peers.clone(), Vec::new(), Vec::new());
        assert_eq!(chunks.len(), 1);
        assert!(
            Packet {
                v: VERSION,
                session: uuid::Uuid::nil(),
                seq: u64::MAX,
                msg: chunks[0].clone()
            }
            .encode()
            .is_ok()
        );
        peers.push(PeerInfo {
            id: peers.len() as u32 + 1,
            name,
        });
        assert!(!roster_fits(peers));
    }
}

#[test]
fn session_rate_validation_and_minimum_rate_liveness() {
    use super::MultiplayerSystem;
    use crate::engine::ecs::component::MultiplayerSessionComponent;
    use crate::engine::ecs::{CommandQueue, World};
    for (rate, expected) in [
        (0.2, None),
        (1., Some(1.)),
        (200., Some(120.)),
        (f32::NAN, Some(20.)),
        (0., Some(20.)),
    ] {
        let mut world = World::default();
        let mut config = MultiplayerSessionComponent::host("127.0.0.1:0");
        config.tick_hz = rate;
        let id = world.add_component(config);
        let mut system = MultiplayerSystem::default();
        let mut queue = CommandQueue::new();
        system.register_session(&mut world, &mut queue, id);
        assert_eq!(
            system.session.as_ref().map(|s| s.send_interval_sec),
            expected.map(|hz| 1. / hz)
        );
    }
    let mut host_world = World::default();
    let mut client_world = World::default();
    let mut config = MultiplayerSessionComponent::host("127.0.0.1:0");
    config.tick_hz = 1.;
    let host_id = host_world.add_component(config);
    let mut host = MultiplayerSystem::default();
    let mut client = MultiplayerSystem::default();
    let mut queue = CommandQueue::new();
    host.register_session(&mut host_world, &mut queue, host_id);
    let address = host
        .session
        .as_ref()
        .unwrap()
        .endpoint
        .local_addr()
        .unwrap();
    let mut config = MultiplayerSessionComponent::join(address.to_string());
    config.tick_hz = 1.;
    let client_id = client_world.add_component(config);
    client.register_session(&mut client_world, &mut queue, client_id);
    let start = Instant::now();
    for tick in 0..10 {
        let now = start + Duration::from_secs(tick);
        client.tick_at(&mut client_world, &mut queue, 1., now);
        host.tick_at(&mut host_world, &mut queue, 1., now);
        client.tick_at(&mut client_world, &mut queue, 0., now);
        assert_eq!(
            client_world
                .get_component_by_id_as::<MultiplayerSessionComponent>(client_id)
                .unwrap()
                .peer_id,
            2
        );
    }
    // An unrelated peer departs: the client's own identity must survive.
    let now = start + Duration::from_secs(10);
    let mut third =
        MultiplayerEndpoint::bind(MultiplayerRole::Client, &address.to_string(), "third", now)
            .unwrap();
    third.poll(now);
    host.tick_at(&mut host_world, &mut queue, 1., now);
    client.tick_at(&mut client_world, &mut queue, 0., now);
    drop(third);
    host.tick_at(&mut host_world, &mut queue, 1., now);
    client.tick_at(&mut client_world, &mut queue, 0., now);
    assert_eq!(
        client_world
            .get_component_by_id_as::<MultiplayerSessionComponent>(client_id)
            .unwrap()
            .peer_id,
        2
    );
    // Host Bye is projected even without a special local-disconnected event.
    host.session = None;
    client.tick_at(&mut client_world, &mut queue, 0., now);
    assert_eq!(
        client_world
            .get_component_by_id_as::<MultiplayerSessionComponent>(client_id)
            .unwrap()
            .peer_id,
        0
    );
    host_world
        .get_component_by_id_as_mut::<MultiplayerSessionComponent>(host_id)
        .unwrap()
        .addr = address.to_string();
    host.register_session(&mut host_world, &mut queue, host_id);
    let now = now + Duration::from_secs(1);
    client.tick_at(&mut client_world, &mut queue, 1., now);
    host.tick_at(&mut host_world, &mut queue, 1., now);
    client.tick_at(&mut client_world, &mut queue, 0., now);
    assert_eq!(
        client_world
            .get_component_by_id_as::<MultiplayerSessionComponent>(client_id)
            .unwrap()
            .peer_id,
        2
    );
    // No host polls for longer than the timeout: project disconnected again.
    client.tick_at(
        &mut client_world,
        &mut queue,
        0.,
        now + PEER_TIMEOUT + Duration::from_secs(1),
    );
    assert_eq!(
        client_world
            .get_component_by_id_as::<MultiplayerSessionComponent>(client_id)
            .unwrap()
            .peer_id,
        0
    );
}
