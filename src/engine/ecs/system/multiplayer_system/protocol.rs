//! Versioned wire format. JSON keeps the first slice debuggable with tcpdump;
//! the envelope is the stable boundary, so a binary/delta encoding can replace
//! the body without changing session semantics.
use serde::{Deserialize, Serialize};

// Version 2 adds explicit control heartbeats for sessions without pose replication.
pub const VERSION: u8 = 2;
/// Complete UDP payload budget, envelope included (see the UDP survey doc).
pub const MAX_DATAGRAM: usize = 1200;
pub const HOST_PEER_ID: u32 = 1;

/// World-space pose. Peers exchange world poses, never ECS component ids.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub p: [f32; 3],
    pub r: [f32; 4],
    pub s: [f32; 3],
}

impl Pose {
    pub fn is_finite(&self) -> bool {
        self.p
            .iter()
            .chain(self.r.iter())
            .chain(self.s.iter())
            .all(|v| v.is_finite())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    /// Client -> host, repeated until welcomed.
    Hello {
        name: String,
    },
    /// Host -> client; assigns the client's peer id.
    Welcome {
        peer_id: u32,
    },
    /// Client -> host: the client's own player pose.
    PlayerPose {
        pose: Pose,
    },
    /// Host -> clients. A tick may span several chunks; each chunk is
    /// independently applicable. Only the first chunk carries the roster.
    Snapshot {
        tick: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        peers: Option<Vec<PeerInfo>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        players: Vec<(u32, Pose)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        entities: Vec<(String, Pose)>,
    },
    /// Application message. Clients send to the host; the host broadcasts.
    /// Unreliable for now (see `Channel`).
    App {
        from: u32,
        name: String,
        text: String,
    },
    /// Keep the control session alive when telemetry uses separate typed channels.
    Heartbeat,
    Bye,
}

/// Delivery class. Only `Unreliable` exists today; reliable/ordered delivery
/// (sequence + ACK + resend, as in the terminal UDP prototype) is the next
/// step for spawn/despawn and gameplay intents.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Unreliable,
    // ReliableOrdered,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub v: u8,
    pub session: uuid::Uuid,
    pub seq: u64,
    pub msg: Message,
}

impl Packet {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_DATAGRAM {
            return Err(format!(
                "multiplayer packet is {} bytes, exceeds {MAX_DATAGRAM}",
                bytes.len()
            ));
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_DATAGRAM {
            return None;
        }
        let packet: Self = serde_json::from_slice(bytes).ok()?;
        (packet.v == VERSION).then_some(packet)
    }
}

/// Split one host tick into datagram-sized snapshot chunks.
pub fn snapshot_chunks(
    tick: u64,
    peers: Vec<PeerInfo>,
    players: Vec<(u32, Pose)>,
    entities: Vec<(String, Pose)>,
) -> Vec<Message> {
    // Worst-case envelope around an empty chunk, plus the `"peers":`,
    // `"players":[]`, and `"entities":[]` keys omitted while empty.
    let envelope = json_len(&Packet {
        v: VERSION,
        session: uuid::Uuid::nil(),
        seq: u64::MAX,
        msg: Message::Snapshot {
            tick,
            peers: None,
            players: Vec::new(),
            entities: Vec::new(),
        },
    }) + r#","peers":,"players":[],"entities":[]"#.len();
    let budget = MAX_DATAGRAM.saturating_sub(envelope);
    let mut chunks = Vec::new();
    let mut first = Some(peers);
    let mut current_players = Vec::new();
    let mut current_entities = Vec::new();
    let mut used = first.as_ref().map_or(0, json_len);
    let mut flush = |players: &mut Vec<_>, entities: &mut Vec<_>, peers: Option<Vec<PeerInfo>>| {
        chunks.push(Message::Snapshot {
            tick,
            peers,
            players: std::mem::take(players),
            entities: std::mem::take(entities),
        });
    };
    for player in players {
        let len = json_len(&player);
        if used + len > budget && (!current_players.is_empty() || first.is_some()) {
            flush(&mut current_players, &mut current_entities, first.take());
            used = 0;
        }
        used += len;
        current_players.push(player);
    }
    for entity in entities {
        let len = json_len(&entity);
        if used + len > budget
            && (!current_players.is_empty() || !current_entities.is_empty() || first.is_some())
        {
            flush(&mut current_players, &mut current_entities, first.take());
            used = 0;
        }
        used += len;
        current_entities.push(entity);
    }
    if first.is_some() || !current_players.is_empty() || !current_entities.is_empty() {
        flush(&mut current_players, &mut current_entities, first.take());
    }
    chunks
}

/// Serialized size plus a separator; oversize values land in their own chunk
/// and are rejected by `Packet::encode`.
fn json_len<T: Serialize>(value: &T) -> usize {
    serde_json::to_vec(value).map_or(MAX_DATAGRAM, |bytes| bytes.len() + 1)
}
