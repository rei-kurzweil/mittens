use super::*;

#[test]
fn shared_udp_lease_budget_oversize_reply_and_diagnostics() {
    let udp = UdpSystem::default();
    let clone = udp.clone();
    assert!(udp.shares_transport_with(&clone));
    let endpoint = udp.bind("127.0.0.1:0", 8, "test").unwrap();
    let address = endpoint.local_addr().unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .unwrap();
    sender.send_to(b"oversized", address).unwrap();
    sender.send_to(b"ok", address).unwrap();
    let first = clone
        .poll(&endpoint, 1, |_, _| panic!("oversize packet delivered"))
        .unwrap();
    assert_eq!(first.oversized, 1);
    assert!(first.exhausted);
    let second = udp
        .poll(&endpoint, 2, |from, bytes| {
            assert_eq!(bytes, b"ok");
            endpoint.send_to(b"ack", from).unwrap();
            assert!(clone.poll(&endpoint, 1, |_, _| {}).unwrap().deferred);
        })
        .unwrap();
    assert_eq!(second.delivered, 1);
    assert!(second.drained);
    let mut reply = [0; 8];
    assert_eq!(sender.recv_from(&mut reply).unwrap().0, 3);
    assert_eq!(&reply[..3], b"ack");
    let stats = endpoint.stats();
    assert_eq!(stats.received_packets, 2);
    assert_eq!(stats.oversized_packets, 1);
    assert_eq!(stats.sent_packets, 1);
    assert_eq!(stats.deferred_polls, 1);
    assert_eq!(udp.diagnostics().len(), 1);
    assert!(UdpSystem::default().poll(&endpoint, 1, |_, _| {}).is_err());
    let lease = endpoint.clone();
    drop(endpoint);
    assert!(UdpSocket::bind(address).is_err());
    drop(lease);
    assert!(udp.diagnostics().is_empty());
    assert!(UdpSocket::bind(address).is_ok());
}

#[test]
fn shared_udp_endpoint_budgets_do_not_starve_other_sockets() {
    let udp = UdpSystem::default();
    let busy = udp.bind("127.0.0.1:0", 16, "busy").unwrap();
    let other = udp.bind("127.0.0.1:0", 16, "other").unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    for _ in 0..3 {
        sender.send_to(b"a", busy.local_addr().unwrap()).unwrap();
    }
    sender.send_to(b"b", other.local_addr().unwrap()).unwrap();
    assert_eq!(
        udp.poll(&busy, 2, |_, bytes| assert_eq!(bytes, b"a"))
            .unwrap()
            .delivered,
        2
    );
    assert_eq!(
        udp.poll(&other, 2, |_, bytes| assert_eq!(bytes, b"b"))
            .unwrap()
            .delivered,
        1
    );
    assert_eq!(
        udp.poll(&busy, 2, |_, bytes| assert_eq!(bytes, b"a"))
            .unwrap()
            .delivered,
        1
    );
    assert!(busy.send_to(&[0; 17], other.local_addr().unwrap()).is_err());
    assert_eq!(busy.stats().send_errors, 1);
}

#[test]
fn shared_udp_concurrent_poll_defers_without_consuming_a_packet() {
    use std::sync::Barrier;
    let udp = UdpSystem::default();
    let a = udp.bind("127.0.0.1:0", 16, "a").unwrap();
    let b = udp.bind("127.0.0.1:0", 16, "b").unwrap();
    a.send_to(b"first", a.local_addr().unwrap()).unwrap();
    a.send_to(b"second", b.local_addr().unwrap()).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let background_udp = udp.clone();
    let background_barrier = Arc::clone(&barrier);
    let thread = std::thread::spawn(move || {
        background_udp
            .poll(&a, 1, |_, _| {
                background_barrier.wait();
                background_barrier.wait();
            })
            .unwrap()
    });
    barrier.wait();
    let deferred = udp
        .poll(&b, 1, |_, _| panic!("busy scratch must defer"))
        .unwrap();
    assert!(deferred.deferred);
    barrier.wait();
    assert_eq!(thread.join().unwrap().delivered, 1);
    assert_eq!(
        udp.poll(&b, 1, |_, bytes| assert_eq!(bytes, b"second"))
            .unwrap()
            .delivered,
        1
    );
}

#[test]
fn shared_udp_ipv6_when_loopback_is_available() {
    let udp = UdpSystem::default();
    let Ok(a) = udp.bind("[::1]:0", 16, "ipv6-a") else {
        return;
    };
    let b = udp.bind("[::1]:0", 16, "ipv6-b").unwrap();
    a.send_to(b"ipv6", b.local_addr().unwrap()).unwrap();
    assert_eq!(
        udp.poll(&b, 1, |from, bytes| {
            assert!(from.is_ipv6());
            assert_eq!(bytes, b"ipv6");
        })
        .unwrap()
        .delivered,
        1
    );
}
