//! Per-frame context + legacy name: used to be a command queue.
//!
//! This is a transitional facade that stages signals locally and drains them into `RxWorld`
//! at explicit drain points.
//!
//! Important safety note:
//! - This type must not store raw pointers into `SystemWorld`/`RxWorld` owned alongside it
//!   (e.g. in `Universe`), because moving that owner would invalidate the pointer.
//! - Instead, we queue signals locally and drain them into `SystemWorld.rx` at explicit
//!   drain points.

use crate::engine::ecs::{ComponentId, EventSignal, IntentSignal, RxWorld, Signal, SignalEmitter};

pub struct CommandQueue {
    queued: Vec<Signal>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self { queued: Vec::new() }
    }

    /// Drain locally-queued signals into the target `RxWorld`.
    ///
    /// Returns the number of signals moved.
    pub fn drain_into_rx(&mut self, rx: &mut RxWorld) -> usize {
        if self.queued.is_empty() {
            return 0;
        }

        let drained = std::mem::take(&mut self.queued);
        let moved = drained.len();
        for env in drained {
            if let Some(event) = env.event {
                rx.push_event(env.scope, event);
                continue;
            }
            if let Some(intent) = env.intent {
                rx.push_intent(env.scope, intent);
                continue;
            }
        }
        moved
    }

    // Note: this type is intentionally minimal; emit `Signal` values directly via the
    // `SignalEmitter` impl instead of using command-style convenience methods.

    /// Flush used to apply queued commands; now it executes pending signals.
    pub fn flush(
        &mut self,
        world: &mut crate::engine::ecs::World,
        systems: &mut crate::engine::ecs::system::SystemWorld,
        visuals: &mut crate::engine::graphics::VisualWorld,
    ) {
        // Execute + dispatch any newly-pushed signals.
        let _ = systems.process_signals(world, visuals, self, 100_000);
    }
}

impl SignalEmitter for CommandQueue {
    fn push_event(&mut self, scope: ComponentId, event: EventSignal) {
        self.queued.push(Signal::event(scope, event));
    }

    fn push_intent(&mut self, scope: ComponentId, intent: IntentSignal) {
        self.queued.push(Signal::intent(scope, intent));
    }
}
