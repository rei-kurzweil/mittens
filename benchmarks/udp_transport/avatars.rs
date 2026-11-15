//! Steady-state avatar traffic viewed from one VR participant, not an ECS/IK benchmark.
use crate::{
    protocol::{Message, Packet, PeerInfo, VERSION},
    workload::{Event, Kind, Workload},
};
use std::time::Duration;

fn profiles(case: &str) -> &'static [bool] {
    match case {
        "vr2" => &[true, true],
        "vr4" => &[true, true, true, true],
        "mixed4" => &[true, true, false, false],
        _ => panic!("unknown avatar scenario: {case}; expected vr2, vr4, mixed4"),
    }
}
fn values(person: usize, field: usize, t: f32, tracked: bool) -> Vec<f32> {
    let phase = t + person as f32 * 0.37;
    if field >= 4 {
        return vec![if field < 6 {
            (1. - (phase.rem_euclid(2.8) - 0.88).abs() / 0.10).clamp(0., 1.)
        } else if tracked {
            1.
        } else {
            0.
        }];
    }
    let translation = match field {
        0 => [
            person as f32 * 2. + (phase * 0.8).sin() * 0.65,
            0.004,
            (phase * 0.6).sin() * 0.4,
        ],
        1 => [0., 1.58 + (phase * 1.2).sin() * 0.015, 0.],
        2 => [-0.35, -0.35 + (phase * 2.).sin() * 0.1, -0.25],
        3 => [0.35, -0.35 + (phase * 2. + 1.).sin() * 0.1, -0.25],
        _ => unreachable!(),
    };
    let yaw = (phase * 0.7).sin() * 0.12;
    [0., (yaw * 0.5).sin(), 0., (yaw * 0.5).cos()]
        .into_iter()
        .chain([1.; 3])
        .chain(translation)
        .collect()
}
pub fn workload(case: &str, seconds: f64, scale: usize) -> Workload {
    let people = profiles(case);
    // One local eye-sensor socket; one multiplexed session socket; one typed
    // socket shared by all remote publishers, as in the actual scene.
    let kinds = vec![Kind::Htc, Kind::Session, Kind::Avatar];
    let mut scheduled = Vec::new();
    for tick in 0..(seconds * 120. * scale as f64).floor() as usize {
        scheduled.push((
            Duration::from_secs_f64(tick as f64 / (120 * scale) as f64),
            0,
            0,
            0,
        ));
    }
    for tick in 0..(seconds * 20. * scale as f64).floor() as usize {
        let at = Duration::from_secs_f64(tick as f64 / (20 * scale) as f64);
        for (person, &tracked) in people.iter().enumerate().skip(1) {
            scheduled.push((at, 1, person, 0));
            for field in 0..8 {
                if !tracked && matches!(field, 2 | 3) {
                    continue;
                }
                scheduled.push((at, 2, person, field));
            }
        }
    }
    scheduled.sort();
    let events = scheduled
        .into_iter()
        .enumerate()
        .map(|(seq, (at, route, person, field))| {
            let kind = kinds[route];
            let bytes = match kind {
                Kind::Eye => unreachable!(),
                Kind::Htc => {
                    let mut bytes = vec![0; 84];
                    bytes[..3].copy_from_slice(&[b'M', 1, 0xcf]);
                    let t = at.as_secs_f32();
                    let yaw = (t * 1.3).sin() * 0.15;
                    for offset in [4, 20] {
                        for (i, value) in [0f32, (yaw * 0.5).sin(), 0., (yaw * 0.5).cos()]
                            .iter()
                            .enumerate()
                        {
                            bytes[offset + i * 4..offset + i * 4 + 4]
                                .copy_from_slice(&value.to_le_bytes());
                        }
                    }
                    for offset in [36, 48] {
                        bytes[offset..offset + 4]
                            .copy_from_slice(&(1. - values(0, 4, t, true)[0]).to_le_bytes());
                    }
                    bytes[68..72]
                        .copy_from_slice(&f32::from_bits(0x3f000000 | seq as u32).to_le_bytes());
                    bytes
                }
                Kind::Avatar => crate::udp_channels::bench_encode(
                    (person * 16 + field + 1) as u64,
                    seq,
                    field < 4,
                    &values(person, field, at.as_secs_f32(), people[person]),
                ),
                Kind::Session => Packet {
                    v: VERSION,
                    session: uuid::Uuid::from_u128(1),
                    seq: seq as u64,
                    msg: if seq % 4 == 0 {
                        Message::Snapshot {
                            tick: seq as u64,
                            peers: Some(
                                (0..people.len())
                                    .map(|id| PeerInfo {
                                        id: id as u32 + 1,
                                        name: format!("person{id}"),
                                    })
                                    .collect(),
                            ),
                            players: vec![],
                            entities: vec![],
                        }
                    } else {
                        Message::Heartbeat
                    },
                }
                .encode()
                .unwrap(),
            };
            Event {
                at,
                route,
                kind,
                bytes,
            }
        })
        .collect();
    Workload {
        kinds,
        events,
        duration: Duration::from_secs_f64(seconds),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn realistic_profiles_and_wire_roundtrip() {
        for (case, packets, avatar_packets, avatar_bytes) in [
            ("vr2", 300, 160, 9600),
            ("vr4", 660, 480, 28800),
            ("mixed4", 580, 400, 22560),
        ] {
            let work = workload(case, 1., 1);
            assert_eq!(work.events.len(), packets);
            let avatar: Vec<_> = work
                .events
                .iter()
                .filter(|e| matches!(e.kind, Kind::Avatar))
                .collect();
            assert_eq!(avatar.len(), avatar_packets);
            assert_eq!(
                avatar.iter().map(|e| e.bytes.len()).sum::<usize>(),
                avatar_bytes
            );
            for (seq, event) in work.events.iter().enumerate() {
                assert_eq!(
                    crate::workload::decode(event.kind, &event.bytes).unwrap().0,
                    seq
                );
            }
            // Two desktop publishers never send tracked hand-pose channels.
            if case == "mixed4" {
                for event in avatar {
                    let channel = u64::from_le_bytes(event.bytes[22..30].try_into().unwrap()) - 1;
                    assert!(channel / 16 < 2 || !matches!(channel % 16, 2 | 3));
                }
            }
        }
        assert_ne!(values(1, 0, 0., true), values(1, 0, 1., true));
        assert_eq!(values(2, 6, 0., false), vec![0.]);
        assert_eq!(values(1, 6, 0., true), vec![1.]);
    }
}
