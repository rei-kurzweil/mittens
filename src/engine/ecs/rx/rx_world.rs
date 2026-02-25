use std::collections::HashMap;

use crate::engine::ecs::{CommandQueue, ComponentId, World};

use super::{Signal, SignalHandler, SignalKind, SignalValue};

type HandlerFn = SignalHandler;

/// Stores a unified signal stream and dispatches signals to scope-rooted handlers.
///
/// Handlers are keyed by (signal kind, scope root). When dispatching a signal with `scope=S`,
/// handlers attached to `S` or any ancestor of `S` are invoked.
#[derive(Debug, Default)]
pub struct RxWorld {
    signals: Vec<Signal>,
    handlers: HashMap<SignalKind, HashMap<ComponentId, Vec<HandlerFn>>>,
}

impl RxWorld {
    pub fn push(&mut self, scope: ComponentId, value: impl Into<SignalValue>) {
        self.signals.push(Signal {
            scope,
            value: value.into(),
        });
    }

    /// Returns the current queued signals for this frame.
    ///
    /// This is intentionally read-only: signals are drained and dispatched later in
    /// `SystemWorld::process_commands`.
    pub fn signals(&self) -> &[Signal] {
        &self.signals
    }

    pub fn drain(&mut self) -> Vec<Signal> {
        std::mem::take(&mut self.signals)
    }

    pub fn add_handler(&mut self, kind: SignalKind, scope_root: ComponentId, handler: HandlerFn) {
        self.handlers
            .entry(kind)
            .or_default()
            .entry(scope_root)
            .or_default()
            .push(handler);
    }

    pub fn remove_handler(
        &mut self,
        kind: SignalKind,
        scope_root: ComponentId,
        handler: HandlerFn,
    ) -> bool {
        let Some(by_scope) = self.handlers.get_mut(&kind) else {
            return false;
        };
        let Some(list) = by_scope.get_mut(&scope_root) else {
            return false;
        };

        let before = list.len();
        list.retain(|&h| h as usize != handler as usize);
        let removed = list.len() != before;

        if list.is_empty() {
            by_scope.remove(&scope_root);
        }
        if by_scope.is_empty() {
            self.handlers.remove(&kind);
        }

        removed
    }

    pub fn dispatch_handlers(
        &mut self,
        world: &mut World,
        queue: &mut CommandQueue,
        env: &Signal,
    ) {
        let kind = env.kind();
        let scope_chain = compute_scope_chain(world, env.scope);
        for scope in scope_chain {
            dispatch_kind(self, world, queue, SignalKind::Any, scope, env);
            dispatch_kind(self, world, queue, kind, scope, env);
        }
    }
}

fn compute_scope_chain(world: &World, start: ComponentId) -> Vec<ComponentId> {
    let mut chain = Vec::new();
    let mut cursor = Some(start);
    while let Some(node) = cursor {
        chain.push(node);
        cursor = world.parent_of(node);
    }
    chain
}

fn dispatch_kind(
    rx: &mut RxWorld,
    world: &mut World,
    queue: &mut CommandQueue,
    kind: SignalKind,
    scope: ComponentId,
    env: &Signal,
) {
    let Some(by_scope) = rx.handlers.get(&kind) else {
        return;
    };
    let Some(handlers) = by_scope.get(&scope) else {
        return;
    };

    for handler in handlers.iter().copied() {
        handler(world, queue, env);
    }
}
