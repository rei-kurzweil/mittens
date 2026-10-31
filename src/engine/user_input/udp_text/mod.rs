//! Ordered UDP text/edit input for development terminals. It supplies ordinary
//! TextInput events, and never interprets text as MMS or holds gameplay keys.
mod host;
mod receiver;
pub use host::UdpTerminalHost;
mod sender;
#[cfg(test)]
mod tests;
use super::TextInputFrameEvent;
pub use receiver::UdpTextInputProvider;
pub use sender::UdpTextInputSender;
use serde::{Deserialize, Serialize};
use std::{io, net::UdpSocket};
pub const MAX_PACKET: usize = 1200;
const VERSION: u8 = 1;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "op",
    content = "text",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Edit {
    Insert(String),
    Backspace,
    Delete,
    Left,
    Right,
}
impl Edit {
    fn from_event(event: &TextInputFrameEvent) -> Self {
        match event {
            TextInputFrameEvent::InsertText(text) => Self::Insert(text.clone()),
            TextInputFrameEvent::Backspace => Self::Backspace,
            TextInputFrameEvent::DeleteForward => Self::Delete,
            TextInputFrameEvent::MoveCaretLeft => Self::Left,
            TextInputFrameEvent::MoveCaretRight => Self::Right,
        }
    }
    fn event(self) -> TextInputFrameEvent {
        match self {
            Self::Insert(text) => TextInputFrameEvent::InsertText(text),
            Self::Backspace => TextInputFrameEvent::Backspace,
            Self::Delete => TextInputFrameEvent::DeleteForward,
            Self::Left => TextInputFrameEvent::MoveCaretLeft,
            Self::Right => TextInputFrameEvent::MoveCaretRight,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    version: u8,
    session: uuid::Uuid,
    sequence: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    events: Vec<Edit>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    ack: bool,
}
impl Packet {
    fn new(session: uuid::Uuid, sequence: u64, events: Vec<Edit>) -> Self {
        Self {
            version: VERSION,
            session,
            sequence,
            events,
            ack: false,
        }
    }
    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_PACKET {
            return None;
        }
        let packet: Self = serde_json::from_slice(bytes).ok()?;
        if packet.version != VERSION
            || packet.events.len() > 16
            || packet
                .events
                .iter()
                .any(|event| matches!(event,Edit::Insert(text) if text.len()>512))
        {
            return None;
        }
        Some(packet)
    }
    fn send(&self, socket: &UdpSocket, peer: std::net::SocketAddr) -> io::Result<()> {
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_PACKET {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "UDP terminal packet exceeds 1200 bytes",
            ));
        }
        socket.send_to(&bytes, peer).map(|_| ())
    }
}
