use super::protocol;
use super::*;
use meow_meow_script::{RecordType, TransportValue as V, TypeTag};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

fn vec3() -> Schema {
    Schema::primitive(TypeTag::Vec3)
}
fn pose_schema() -> Schema {
    Schema::record(RecordType(BTreeMap::from([
        ("translation".into(), TypeTag::Vec3),
        ("rotation".into(), TypeTag::Vec4),
        ("scale".into(), TypeTag::Vec3),
    ])))
    .unwrap()
}
fn socket() -> ChannelSocket {
    ChannelSocket::bind("127.0.0.1:0".parse().unwrap()).unwrap()
}
fn pump(a: &mut ChannelSocket, b: &mut ChannelSocket, now: Instant) {
    // Numeric loopback sends are synchronous; no sleeps or real-time timeout dependence.
    for _ in 0..3 {
        a.poll(now).unwrap();
        b.poll(now).unwrap();
    }
}
fn packet(kind: u8, epoch: [u8; 16], channel: u64, sequence: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0; protocol::HEADER + payload.len()];
    protocol::header(&mut out, kind, epoch, channel, sequence).unwrap();
    out[protocol::HEADER..].copy_from_slice(payload);
    out
}
fn declaration(epoch: [u8; 16], channel: u64, name: &str, schema: &Schema) -> Vec<u8> {
    let mut bytes = [0; MAX_DATAGRAM];
    protocol::header(&mut bytes, protocol::DECLARE, epoch, channel, 0).unwrap();
    let len = protocol::declaration(name, schema, &mut bytes).unwrap();
    bytes[..len].to_vec()
}

#[test]
fn numeric_codec_is_exact_fixed_width_and_canonical() {
    for (tag, len) in [
        (TypeTag::F32, 1),
        (TypeTag::Vec2, 2),
        (TypeTag::Vec3, 3),
        (TypeTag::Vec4, 4),
    ] {
        let schema = Schema::from_type(&V::TypeTag(tag)).unwrap();
        let input = [-0.0f32, f32::from_bits(1), f32::MAX, -32.00586];
        let mut bytes = [0; 16];
        assert_eq!(
            schema.encode_numeric(&input[..len], &mut bytes).unwrap(),
            len * 4
        );
        assert_eq!(&bytes[..4], &(-0.0f32).to_le_bytes());
        let mut output = [0.; 4];
        schema
            .decode_numeric(&bytes[..len * 4], &mut output[..len])
            .unwrap();
        assert_eq!(
            input[..len].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            output[..len]
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        );
    }
    let pose = pose_schema();
    assert_eq!(pose.fixed_size(), Some(40));
    let value = V::Table(vec![
        (
            "translation".into(),
            V::Array(vec![V::Number(1.), V::Number(2.), V::Number(3.)]),
        ),
        ("scale".into(), V::Array(vec![V::Number(1.); 3])),
        (
            "rotation".into(),
            V::Array(vec![
                V::Number(0.),
                V::Number(0.),
                V::Number(0.),
                V::Number(1.),
            ]),
        ),
    ]);
    let mut bytes = [0; 40];
    pose.encode_value(&value, &mut bytes).unwrap();
    let mut lanes = [0.; 10];
    pose.decode_numeric(&bytes, &mut lanes).unwrap();
    // Lexicographic field order: rotation, scale, translation.
    assert_eq!(lanes, [0., 0., 0., 1., 1., 1., 1., 1., 2., 3.]);
    let decoded = pose.decode_value(&bytes).unwrap();
    assert_eq!(pose.encode_value(&decoded, &mut [0; 40]).unwrap(), 40);
    assert!(Schema::from_type(&V::Table(vec![])).is_err());
}

#[test]
fn structured_strings_are_bounded_and_invalid_values_are_atomic() {
    let schema = Schema::record(RecordType(BTreeMap::from([
        ("label".into(), TypeTag::Str),
        ("value".into(), TypeTag::F32),
    ])))
    .unwrap();
    let value = V::Table(vec![
        ("value".into(), V::Number(2.5)),
        ("label".into(), V::String("温度".into())),
    ]);
    let mut bytes = [0; 100];
    let len = schema.encode_value(&value, &mut bytes).unwrap();
    assert_eq!(len, 12);
    assert!(schema.decode_value(&bytes[..len]).is_ok());
    assert!(schema.decode_value(&bytes[..len - 1]).is_err());
    assert!(schema.decode_value(&bytes[..len + 1]).is_err());
    let mut unchanged = [0xa5; 12];
    assert!(
        vec3()
            .encode_numeric(&[1., f32::NAN, 3.], &mut unchanged)
            .is_err()
    );
    assert_eq!(unchanged, [0xa5; 12]);
    for invalid in [
        V::Number(f64::INFINITY),
        V::Number(f64::MAX),
        V::String("wrong".into()),
    ] {
        assert!(
            Schema::primitive(TypeTag::F32)
                .encode_value(&invalid, &mut unchanged)
                .is_err()
        );
        assert_eq!(unchanged, [0xa5; 12]);
    }
    let duplicate = V::Table(vec![
        ("label".into(), V::String("a".into())),
        ("label".into(), V::String("b".into())),
    ]);
    assert!(schema.encode_value(&duplicate, &mut unchanged).is_err());
    assert!(
        Schema::primitive(TypeTag::Str)
            .decode_value(&[1, 0, 0xff])
            .is_err()
    );
}

#[test]
fn declaration_parser_rejects_truncation_unknown_tags_and_duplicate_fields() {
    let bytes = declaration([1; 16], 7, "pose", &pose_schema());
    let p = protocol::decode(&bytes).unwrap();
    assert_eq!(
        protocol::read_declaration(p.payload).unwrap(),
        ("pose".into(), pose_schema())
    );
    for end in 0..bytes.len() {
        assert!(
            protocol::decode(&bytes[..end])
                .and_then(|p| protocol::read_declaration(p.payload))
                .is_err()
        );
    }
    let mut invalid = bytes.clone();
    invalid[4] = 2;
    assert!(protocol::decode(&invalid).is_err());
    let mut oversized = bytes.clone();
    oversized.resize(MAX_DATAGRAM + 1, 0);
    assert!(protocol::decode(&oversized).is_err());
    assert!(protocol::read_declaration(&[1, b'x', 0, 255]).is_err());
    assert!(protocol::read_declaration(&[1, b'x', 1, 2, 1, b'a', 1, 1, b'a', 1]).is_err());
    // Deterministic malformed inputs must never panic or allocate unboundedly.
    for len in 0..=MAX_DATAGRAM {
        let bytes = vec![0xff; len];
        assert!(protocol::decode(&bytes).is_err());
        assert!(vec3().decode_value(&bytes).is_err());
    }
}

#[test]
fn loopback_multiplexes_channels_and_shares_receive_state() {
    let now = Instant::now();
    let mut a = socket();
    let mut b = socket();
    let pose_pub = a
        .publish(b.local_addr().unwrap(), "hand.pose", pose_schema())
        .unwrap();
    let temp_pub = a
        .publish(
            b.local_addr().unwrap(),
            "temperature",
            Schema::primitive(TypeTag::F32),
        )
        .unwrap();
    assert!(a.write_numeric(pose_pub, &[0.; 10]).is_err());
    let pose_sub = b
        .subscribe(a.local_addr().unwrap(), "hand.pose", pose_schema())
        .unwrap();
    let second = b
        .subscribe(a.local_addr().unwrap(), "hand.pose", pose_schema())
        .unwrap();
    let temp_sub = b
        .subscribe(
            a.local_addr().unwrap(),
            "temperature",
            Schema::primitive(TypeTag::F32),
        )
        .unwrap();
    pump(&mut a, &mut b, now);
    assert!(a.is_publication_ready(pose_pub));
    assert!(a.is_publication_ready(temp_pub));
    assert_eq!(b.channel(pose_sub), b.channel(second));
    assert_ne!(b.channel(pose_sub), b.channel(temp_sub));
    a.drain_events().for_each(drop);
    b.drain_events().for_each(drop);
    let values = [0., 0., 0., 1., 1., 1., 1., 2., 3., 4.];
    a.write_numeric(pose_pub, &values).unwrap();
    a.write_numeric(temp_pub, &[22.5]).unwrap();
    b.poll(now).unwrap();
    let sample = b.sample(pose_sub).unwrap();
    assert_eq!(sample.bytes.len(), 40);
    let mut output = [0.; 10];
    sample
        .schema
        .decode_numeric(sample.bytes, &mut output)
        .unwrap();
    assert_eq!(output, values);
    assert_eq!(
        b.sample(temp_sub)
            .unwrap()
            .schema
            .decode_value(b.sample(temp_sub).unwrap().bytes)
            .unwrap(),
        V::Number(22.5)
    );
    let events: Vec<_> = b.drain_events().collect();
    assert!(events.contains(&ChannelEvent::Sample(pose_sub)));
    assert!(events.contains(&ChannelEvent::Sample(second)));
    b.remove_subscription(pose_sub).unwrap();
    assert!(b.channel(second).is_some());
    assert!(b.channel(pose_sub).is_none());
    assert!(b.remove_subscription(pose_sub).is_err());
    a.remove_publication(pose_pub).unwrap();
    b.poll(now).unwrap();
    assert!(b.channel(second).is_none());
    assert!(b.channel(temp_sub).is_some());
    assert!(a.write_numeric(pose_pub, &values).is_err());
}

#[test]
fn subscriber_can_arrive_later_and_mismatches_never_receive_values() {
    let now = Instant::now();
    let mut a = socket();
    let mut b = socket();
    let p = a.publish(b.local_addr().unwrap(), "value", vec3()).unwrap();
    pump(&mut a, &mut b, now);
    assert!(!a.is_publication_ready(p));
    let wrong = b
        .subscribe(
            a.local_addr().unwrap(),
            "value",
            Schema::primitive(TypeTag::F32),
        )
        .unwrap();
    pump(&mut a, &mut b, now + Duration::from_millis(250));
    assert!(!a.is_publication_ready(p));
    assert!(b.channel(wrong).is_none());
    assert!(
        b.drain_events()
            .any(|e| e == ChannelEvent::SchemaMismatch(wrong))
    );
    b.remove_subscription(wrong).unwrap();
    let right = b
        .subscribe(a.local_addr().unwrap(), "value", vec3())
        .unwrap();
    pump(&mut a, &mut b, now + Duration::from_millis(500));
    assert!(a.is_publication_ready(p));
    assert!(b.channel(right).is_some());
}

#[test]
fn retries_recover_lost_declarations_and_timeouts_are_observable() {
    let now = Instant::now();
    let mut a = socket();
    let mut b = socket();
    let p = a.publish(b.local_addr().unwrap(), "value", vec3()).unwrap();
    let sub = b
        .subscribe(a.local_addr().unwrap(), "value", vec3())
        .unwrap();
    pump(&mut a, &mut b, now);
    a.drain_events().for_each(drop);
    b.drain_events().for_each(drop);
    b.poll(now + Duration::from_secs(4)).unwrap();
    assert!(b.channel(sub).is_none());
    assert!(b.drain_events().any(|e| e == ChannelEvent::Lost(sub)));
    a.poll(now + Duration::from_secs(4)).unwrap();
    assert!(
        a.drain_events()
            .any(|e| e == ChannelEvent::PublicationLost(p))
    );
    pump(&mut a, &mut b, now + Duration::from_secs(4));
    assert!(a.is_publication_ready(p));
    assert!(b.channel(sub).is_some());
}

#[test]
fn samples_coalesce_and_stale_wrong_sender_unknown_or_invalid_data_is_ignored() {
    let now = Instant::now();
    let source = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let from = source.local_addr().unwrap();
    let mut b = socket();
    let dest = b.local_addr().unwrap();
    let sub = b.subscribe(from, "position", vec3()).unwrap();
    let epoch = [7; 16];
    source
        .send_to(&declaration(epoch, 1, "position", &vec3()), dest)
        .unwrap();
    b.poll(now).unwrap();
    b.drain_events().for_each(drop);
    let mut payload = [0; 12];
    vec3().encode_numeric(&[1., 2., 3.], &mut payload).unwrap();
    for seq in [1, 3, 2, 3] {
        source
            .send_to(&packet(protocol::SAMPLE, epoch, 1, seq, &payload), dest)
            .unwrap();
    }
    source
        .send_to(&packet(protocol::SAMPLE, epoch, 2, 99, &payload), dest)
        .unwrap();
    let wrong = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    wrong
        .send_to(&packet(protocol::SAMPLE, epoch, 1, 99, &payload), dest)
        .unwrap();
    source
        .send_to(&packet(protocol::SAMPLE, epoch, 1, 4, &[0xff; 12]), dest)
        .unwrap();
    source.send_to(&vec![0; MAX_DATAGRAM + 400], dest).unwrap();
    b.poll(now).unwrap();
    assert_eq!(b.sample(sub).unwrap().sequence, 3);
    assert_eq!(b.stats.stale_samples, 2);
    assert_eq!(b.stats.unknown_samples, 2);
    assert_eq!(b.stats.invalid_packets, 2);
    assert_eq!(
        b.drain_events()
            .filter(|e| *e == ChannelEvent::Sample(sub))
            .count(),
        1
    );
}

#[test]
fn restart_epochs_and_withdrawals_prevent_delayed_channel_resurrection() {
    let now = Instant::now();
    let source = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut b = socket();
    let sub = b
        .subscribe(source.local_addr().unwrap(), "position", vec3())
        .unwrap();
    let dest = b.local_addr().unwrap();
    let old = declaration([1; 16], 1, "position", &vec3());
    source.send_to(&old, dest).unwrap();
    b.poll(now).unwrap();
    let old_handle = b.channel(sub).unwrap();
    let new = declaration([2; 16], 1, "position", &vec3());
    source.send_to(&new, dest).unwrap();
    b.poll(now).unwrap();
    let new_handle = b.channel(sub).unwrap();
    assert_ne!(old_handle, new_handle);
    source.send_to(&old, dest).unwrap();
    b.poll(now).unwrap();
    assert_eq!(b.channel(sub), Some(new_handle));
    source
        .send_to(&packet(protocol::WITHDRAW, [2; 16], 1, 0, &[]), dest)
        .unwrap();
    b.poll(now).unwrap();
    assert!(b.channel(sub).is_none());
    source.send_to(&new, dest).unwrap();
    b.poll(now).unwrap();
    assert!(b.channel(sub).is_none());
}

#[test]
fn sequence_floor_survives_timeout_and_subscription_recreation() {
    let now = Instant::now();
    let source = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let from = source.local_addr().unwrap();
    let mut receiver = socket();
    let address = receiver.local_addr().unwrap();
    let sub = receiver.subscribe(from, "position", vec3()).unwrap();
    let declare = declaration([1; 16], 3, "position", &vec3());
    let mut payload = [0; 12];
    vec3().encode_numeric(&[1., 2., 3.], &mut payload).unwrap();
    source.send_to(&declare, address).unwrap();
    source
        .send_to(&packet(protocol::SAMPLE, [1; 16], 3, 10, &payload), address)
        .unwrap();
    receiver.poll(now).unwrap();
    assert_eq!(receiver.sample(sub).unwrap().sequence, 10);
    receiver.poll(now + Duration::from_secs(4)).unwrap();
    assert!(receiver.channel(sub).is_none());
    source.send_to(&declare, address).unwrap();
    source
        .send_to(&packet(protocol::SAMPLE, [1; 16], 3, 9, &payload), address)
        .unwrap();
    receiver.poll(now + Duration::from_secs(4)).unwrap();
    assert!(receiver.channel(sub).is_some());
    assert!(receiver.sample(sub).is_none());
    receiver.remove_subscription(sub).unwrap();
    let sub = receiver.subscribe(from, "position", vec3()).unwrap();
    source.send_to(&declare, address).unwrap();
    source
        .send_to(&packet(protocol::SAMPLE, [1; 16], 3, 10, &payload), address)
        .unwrap();
    receiver.poll(now + Duration::from_secs(4)).unwrap();
    assert!(receiver.sample(sub).is_none());
    source
        .send_to(&packet(protocol::SAMPLE, [1; 16], 3, 11, &payload), address)
        .unwrap();
    receiver.poll(now + Duration::from_secs(4)).unwrap();
    assert_eq!(receiver.sample(sub).unwrap().sequence, 11);
}

#[test]
fn local_limits_and_declaration_budget_fail_before_registration() {
    let mut socket = socket();
    let destination = "127.0.0.1:7010".parse().unwrap();
    let huge = Schema::record(RecordType(
        (0..32)
            .map(|i| (format!("{i:02}{}", "x".repeat(90)), TypeTag::F32))
            .collect(),
    ))
    .unwrap();
    assert!(socket.publish(destination, "huge", huge.clone()).is_err());
    assert!(socket.subscribe(destination, "huge", huge).is_err());
    assert!(socket.publish(destination, "", vec3()).is_err());
    for i in 0..64 {
        socket
            .publish(destination, &format!("channel{i}"), vec3())
            .unwrap();
    }
    assert!(socket.publish(destination, "overflow", vec3()).is_err());
    socket.subscribe(destination, "channel", vec3()).unwrap();
    assert!(
        socket
            .subscribe(destination, "channel", Schema::primitive(TypeTag::F32))
            .is_err()
    );
}

#[test]
fn withdrawal_before_declaration_is_tombstoned() {
    let now = Instant::now();
    let source = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut receiver = socket();
    let sub = receiver
        .subscribe(source.local_addr().unwrap(), "position", vec3())
        .unwrap();
    let to = receiver.local_addr().unwrap();
    source
        .send_to(&packet(protocol::WITHDRAW, [3; 16], 7, 0, &[]), to)
        .unwrap();
    source
        .send_to(&declaration([3; 16], 7, "position", &vec3()), to)
        .unwrap();
    receiver.poll(now).unwrap();
    assert!(receiver.channel(sub).is_none());
    assert_eq!(receiver.stats.invalid_packets, 0);
}

#[test]
fn receive_and_event_work_is_bounded() {
    let now = Instant::now();
    let source = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut receiver = socket();
    let sub = receiver
        .subscribe(source.local_addr().unwrap(), "position", vec3())
        .unwrap();
    let to = receiver.local_addr().unwrap();
    source
        .send_to(&declaration([1; 16], 1, "position", &vec3()), to)
        .unwrap();
    receiver.poll(now).unwrap();
    // At most one Sample notification per poll, and an undrained event queue
    // never grows past its cap. Data remains queryable even if events overflow.
    let mut payload = [0; 12];
    vec3().encode_numeric(&[1., 2., 3.], &mut payload).unwrap();
    for seq in 1..=300 {
        source
            .send_to(&packet(protocol::SAMPLE, [1; 16], 1, seq, &payload), to)
            .unwrap();
        receiver.poll(now).unwrap();
    }
    assert_eq!(receiver.sample(sub).unwrap().sequence, 300);
    assert_eq!(receiver.drain_events().count(), 256);
    assert_eq!(receiver.stats.dropped_events, 45);
    assert!(receiver.stats.received_packets >= 301);
}

/// Measurement only: no timing assertions. The standalone harness optimizes
/// this module without rebuilding the renderer; see the accompanying script.
#[test]
#[ignore]
fn codec_measurement() {
    use std::hint::black_box;
    fn measure<const N: usize>(name: &str, schema: Schema, values: [f32; N])
    where
        [f32; N]: serde::Serialize + serde::de::DeserializeOwned,
    {
        let iterations = 200_000;
        let json = serde_json::to_vec(&values).unwrap();
        let mut json_out = Vec::with_capacity(256);
        let mut bytes = [0; 128];
        let len = N * 4;
        let mut output = [0.; N];
        let start = Instant::now();
        for _ in 0..iterations {
            schema
                .encode_numeric(black_box(&values), black_box(&mut bytes))
                .unwrap();
            black_box(&bytes[..len]);
        }
        let binary_encode = start.elapsed();
        let start = Instant::now();
        for _ in 0..iterations {
            schema
                .decode_numeric(black_box(&bytes[..len]), black_box(&mut output))
                .unwrap();
            black_box(&output);
        }
        let binary_decode = start.elapsed();
        let start = Instant::now();
        for _ in 0..iterations {
            json_out.clear();
            serde_json::to_writer(black_box(&mut json_out), black_box(&values)).unwrap();
            black_box(&json_out);
        }
        let json_encode = start.elapsed();
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(serde_json::from_slice::<[f32; N]>(black_box(&json)).unwrap());
        }
        let json_decode = start.elapsed();
        let ns = |d: Duration| d.as_nanos() as f64 / iterations as f64;
        println!(
            "{name}: binary_bytes={len}, json_array_bytes={}, binary_encode_ns={:.1}, binary_decode_ns={:.1}, json_reused_encode_ns={:.1}, json_fixed_array_decode_ns={:.1}",
            json.len(),
            ns(binary_encode),
            ns(binary_decode),
            ns(json_encode),
            ns(json_decode)
        );
    }
    measure("vec3", vec3(), [12.345678, -9.876543, 0.00123456]);
    measure(
        "vec4",
        Schema::primitive(TypeTag::Vec4),
        [0.1234567, -0.7654321, 0.3333333, 0.9238795],
    );
    measure(
        "TRS",
        pose_schema(),
        [
            0.1234567, -0.7654321, 0.3333333, 0.9238795, 1., 1., 1., 12.345678, -9.876543,
            0.00123456,
        ],
    );
    // Matrix-sized storage via four vec4 fields; no new MMS mat4 tag is implied.
    let matrix = Schema::record(RecordType(
        (0..4)
            .map(|i| (format!("column{i}"), TypeTag::Vec4))
            .collect(),
    ))
    .unwrap();
    measure(
        "four-vec4-record",
        matrix,
        [
            0.1234567, 0.7654321, 0., 0., -0.7654321, 0.1234567, 0., 0., 0., 0., 1., 0., 12.345678,
            -9.876543, 0.00123456, 1.,
        ],
    );
    let metadata = Schema::record(RecordType(BTreeMap::from([
        ("label".into(), TypeTag::Str),
        ("weight".into(), TypeTag::F32),
    ])))
    .unwrap();
    let value = V::Table(vec![
        ("label".into(), V::String("left-hand".into())),
        ("weight".into(), V::Number(0.75)),
    ]);
    let json_value = serde_json::json!({"label": "left-hand", "weight": 0.75});
    let json = serde_json::to_vec(&json_value).unwrap();
    let iterations = 200_000;
    let mut bytes = [0; 128];
    let len = metadata.encode_value(&value, &mut bytes).unwrap();
    let start = Instant::now();
    for _ in 0..iterations {
        metadata
            .encode_value(black_box(&value), black_box(&mut bytes))
            .unwrap();
    }
    let encode = start.elapsed();
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(metadata.decode_value(black_box(&bytes[..len])).unwrap());
    }
    let decode = start.elapsed();
    let mut json_out = Vec::with_capacity(128);
    let start = Instant::now();
    for _ in 0..iterations {
        json_out.clear();
        serde_json::to_writer(black_box(&mut json_out), black_box(&json_value)).unwrap();
        black_box(&json_out);
    }
    let json_encode = start.elapsed();
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(serde_json::from_slice::<serde_json::Value>(black_box(&json)).unwrap());
    }
    let json_decode = start.elapsed();
    let ns = |d: Duration| d.as_nanos() as f64 / iterations as f64;
    println!(
        "mixed-record: binary_bytes={len}, json_object_bytes={}, binary_transport_encode_ns={:.1}, binary_transport_decode_ns={:.1}, json_value_reused_encode_ns={:.1}, json_value_decode_ns={:.1}",
        json.len(),
        ns(encode),
        ns(decode),
        ns(json_encode),
        ns(json_decode)
    );
}
