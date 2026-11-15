// Benchmark-only access to the unchanged production private codec.
fn bench_schema(pose: bool) -> &'static Schema {
    use std::sync::OnceLock;
    static POSE: OnceLock<Schema> = OnceLock::new();
    static SCALAR: OnceLock<Schema> = OnceLock::new();
    if pose {
        POSE.get_or_init(|| Schema::record(meow_meow_script::RecordType([
            ("rotation".into(), meow_meow_script::TypeTag::Vec4),
            ("scale".into(), meow_meow_script::TypeTag::Vec3),
            ("translation".into(), meow_meow_script::TypeTag::Vec3),
        ].into())).unwrap())
    } else {
        SCALAR.get_or_init(|| Schema::primitive(meow_meow_script::TypeTag::F32))
    }
}
pub fn bench_encode(channel: u64, sequence: usize, pose: bool, values: &[f32]) -> Vec<u8> {
    let mut bytes = vec![0; MAX_DATAGRAM];
    protocol::header(&mut bytes, protocol::SAMPLE, [1; 16], channel, sequence as u64 + 1).unwrap();
    let len = bench_schema(pose).encode_numeric(values, &mut bytes[protocol::HEADER..]).unwrap();
    bytes.truncate(protocol::HEADER + len);
    bytes
}
pub fn bench_decode(bytes: &[u8]) -> Option<(usize, u64)> {
    let packet = protocol::decode(bytes).ok()?;
    if packet.kind != protocol::SAMPLE { return None; }
    let field = packet.channel.checked_sub(1)? % 16;
    if field > 7 { return None; }
    let pose = field < 4;
    let mut storage = [0.; 10];
    let output = &mut storage[..if pose { 10 } else { 1 }];
    bench_schema(pose).decode_numeric(packet.payload, output).ok()?;
    let checksum = output.iter().fold(packet.channel, |sum, v| sum.wrapping_add(v.to_bits() as u64));
    Some((usize::try_from(packet.sequence.checked_sub(1)?).ok()?, checksum))
}
