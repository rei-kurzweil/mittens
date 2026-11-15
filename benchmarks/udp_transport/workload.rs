//! Pre-encoded packets are identical in both modes. No benchmark wire envelope.
use crate::{
    eye,
    protocol::{Message, Packet, PeerInfo, Pose, VERSION},
};
use std::time::Duration;

#[derive(Clone, Copy)]
pub enum Kind {
    Eye,
    Htc,
    Session,
    Avatar,
}
pub struct Event {
    pub at: Duration,
    pub route: usize,
    pub kind: Kind,
    pub bytes: Vec<u8>,
}
pub struct Workload {
    pub kinds: Vec<Kind>,
    pub events: Vec<Event>,
    pub duration: Duration,
}

fn osc_string(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
    out.push(0);
    while out.len() % 4 != 0 {
        out.push(0);
    }
}
pub(crate) fn packet(kind: Kind, sequence: usize, poses: bool) -> Vec<u8> {
    match kind {
        Kind::Eye => {
            let mut out = Vec::new();
            osc_string(&mut out, "/tracking/eye/LeftRightPitchYaw");
            osc_string(&mut out, ",ffff");
            // A valid finite angle also carries the benchmark sample identity
            // in its mantissa. No bytes are added to the OSC packet.
            for value in [
                2.0f32,
                -3.0,
                2.1,
                f32::from_bits(0x3f000000 | sequence as u32),
            ] {
                out.extend_from_slice(&value.to_be_bytes());
            }
            out
        }
        Kind::Htc | Kind::Avatar => unreachable!("avatar packets have their own generator"),
        Kind::Session => {
            let pose = Pose {
                p: [1.0, 1.58, 2.0],
                r: [0., 0., 0., 1.],
                s: [1.; 3],
            };
            let msg = if poses || sequence % 4 == 0 {
                Message::Snapshot {
                    tick: sequence as u64,
                    peers: Some(vec![
                        PeerInfo {
                            id: 1,
                            name: "rei".into(),
                        },
                        PeerInfo {
                            id: 2,
                            name: "dan".into(),
                        },
                    ]),
                    players: if poses {
                        vec![(1, pose), (2, pose)]
                    } else {
                        vec![]
                    },
                    entities: if poses {
                        vec![("ball".into(), pose)]
                    } else {
                        vec![]
                    },
                }
            } else {
                Message::Heartbeat
            };
            Packet {
                v: VERSION,
                session: uuid::Uuid::from_u128(1),
                seq: sequence as u64,
                msg,
            }
            .encode()
            .unwrap()
        }
    }
}
impl Workload {
    pub fn new(pairs: usize, seconds: f64, scale: usize, poses: bool) -> Self {
        let duration = Duration::from_secs_f64(seconds);
        let mut kinds = vec![Kind::Eye; pairs];
        kinds.extend(vec![Kind::Session; pairs]);
        let mut schedule = Vec::new();
        for (route, kind) in kinds.iter().copied().enumerate() {
            let rate = (match kind {
                Kind::Eye | Kind::Htc => 120,
                Kind::Session | Kind::Avatar => 20,
            }) * scale;
            for tick in 0..(seconds * rate as f64).floor() as usize {
                schedule.push((
                    Duration::from_secs_f64(tick as f64 / rate as f64),
                    route,
                    kind,
                ));
            }
        }
        schedule.sort_by_key(|(at, route, _)| (*at, *route));
        assert!(
            !schedule.is_empty() && schedule.len() < 0x800000,
            "packet count outside benchmark identity capacity"
        );
        let events = schedule
            .into_iter()
            .enumerate()
            .map(|(seq, (at, route, kind))| Event {
                at,
                route,
                kind,
                bytes: packet(kind, seq, poses),
            })
            .collect();
        Self {
            kinds,
            events,
            duration,
        }
    }
}
pub fn decode(kind: Kind, bytes: &[u8]) -> Option<(usize, u64)> {
    match kind {
        Kind::Eye => {
            let decoded = eye::decode_osc(bytes)?;
            let checksum = decoded
                .1?
                .into_iter()
                .chain(decoded.2?)
                .fold(0u64, |sum, v| sum.wrapping_add(v.to_bits() as u64));
            let bits =
                u32::from_be_bytes(bytes.get(bytes.len().checked_sub(4)?..)?.try_into().ok()?);
            Some(((bits & 0x7fffff) as usize, checksum))
        }
        Kind::Htc => {
            let (left, right) = eye::decode_htc(bytes)?;
            let checksum = left
                .look?
                .into_iter()
                .chain(right.look?)
                .chain([left.openness?, right.openness?])
                .fold(0u64, |sum, value| sum.wrapping_add(value.to_bits() as u64));
            let bits = u32::from_le_bytes(bytes.get(68..72)?.try_into().ok()?);
            Some(((bits & 0x7fffff) as usize, checksum))
        }
        Kind::Avatar => crate::udp_channels::bench_decode(bytes),
        Kind::Session => {
            let decoded = Packet::decode(bytes)?;
            let sequence = usize::try_from(decoded.seq).ok()?;
            let checksum = match decoded.msg {
                Message::Snapshot {
                    peers,
                    players,
                    entities,
                    ..
                } => (peers.map_or(0, |p| p.len()) + players.len() + entities.len()) as u64,
                Message::Heartbeat => 1,
                _ => return None,
            };
            Some((sequence, checksum))
        }
    }
}
