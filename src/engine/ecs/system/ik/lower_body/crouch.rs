//! Rest-based posture goals. No gait state, joint writes, or collision ownership.
use super::{FootGoal, support::Support};

#[derive(Debug, Clone, Copy)]
pub(super) struct RestGoal {
    pub foot: FootGoal,
    pub support: Option<Support>,
    pub penetration: f32,
}

pub(super) fn evaluate(base: FootGoal, support: Option<Support>, enabled: bool) -> RestGoal {
    let penetration = support
        .map(|s| (s.height - base.sole[1]).max(0.0))
        .unwrap_or(0.0);
    let mut foot = base;
    if enabled {
        foot.sole[1] += penetration;
    }
    RestGoal {
        foot,
        support,
        penetration,
    }
}
