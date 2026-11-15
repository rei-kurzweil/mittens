//! Typed UDP channels, independent of ECS and scene policy.
//!
//! This first slice provides the wire codec and native endpoint. MMS socket
//! values and subscription-owned transform bindings build on this boundary.
mod endpoint;
mod payload;
mod protocol;
mod schema;

pub use endpoint::{
    ChannelEvent, ChannelId, ChannelSocket, ChannelStats, PublicationId, Sample, SubscriptionId,
};
pub use schema::{ChannelError, Schema};

pub const MAX_DATAGRAM: usize = 1200;

#[cfg(test)]
mod tests;
