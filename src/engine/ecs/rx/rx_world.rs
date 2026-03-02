use std::collections::HashMap;

use crate::engine::ecs::{CommandQueue, ComponentId, World};

use super::{Signal, SignalEmitter, SignalHandler, SignalKind, SignalValue};

enum Handler {
    Fn(SignalHandler),
    Closure(
        Box<
            dyn FnMut(&mut World, &mut CommandQueue, &mut dyn SignalEmitter, &Signal)
                + Send
                + Sync
                + 'static,
        >,
    ),
}

struct Emitter {
    signals: *mut Vec<Signal>,
}

impl SignalEmitter for Emitter {
    fn push(&mut self, scope: ComponentId, value: SignalValue) {
        // SAFETY: `signals` points at `RxWorld.signals` for the duration of a dispatch.
        unsafe {
            (*self.signals).push(Signal { scope, value });
        }
    }
}

/// Stores a unified signal stream and dispatches signals to scope-rooted handlers.
///
/// Handlers are keyed by (signal kind, scope root). When dispatching a signal with `scope=S`,
/// handlers attached to `S` or any ancestor of `S` are invoked.
#[derive(Default)]
pub struct RxWorld {
    signals: Vec<Signal>,

    /// Cursor for immediate-mode dispatch. Signals before this index have already had handlers run.
    dispatched_cursor: usize,

    /// Global handlers run for every matching signal regardless of scope.
    global_handlers: HashMap<SignalKind, Vec<Handler>>,

    /// Scoped handlers keyed by (kind, scope_root).
    scoped_handlers: HashMap<SignalKind, HashMap<ComponentId, Vec<Handler>>>,
}

impl std::fmt::Debug for RxWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RxWorld")
            .field("signals_len", &self.signals.len())
            .field("dispatched_cursor", &self.dispatched_cursor)
            .field("global_kinds", &self.global_handlers.len())
            .field("scoped_kinds", &self.scoped_handlers.len())
            .finish()
    }
}

impl RxWorld {
    pub fn push(&mut self, scope: ComponentId, value: impl Into<SignalValue>) {
        self.signals.push(Signal {
            scope,
            value: value.into(),
        });
    }

    /// Reset immediate-mode dispatch state for a new frame.
    ///
    /// In the current architecture, signals are typically drained once per frame in
    /// `SystemWorld::process_commands`, which also implicitly resets this cursor.
    pub fn begin_frame(&mut self) {
        self.dispatched_cursor = 0;
    }

    /// Returns the current queued signals for this frame.
    ///
    /// This is intentionally read-only: signals are drained and dispatched later in
    /// `SystemWorld::process_commands`.
    pub fn signals(&self) -> &[Signal] {
        &self.signals
    }

    pub fn drain(&mut self) -> Vec<Signal> {
        self.dispatched_cursor = 0;
        std::mem::take(&mut self.signals)
    }

    /// Add a scoped handler rooted at `scope_root`.
    ///
    /// Note: this is a function pointer (no captures). Use `add_handler_closure` internally
    /// when you need stateful handlers.
    pub fn add_handler(&mut self, kind: SignalKind, scope_root: ComponentId, handler: SignalHandler) {
        self.scoped_handlers
            .entry(kind)
            .or_default()
            .entry(scope_root)
            .or_default()
            .push(Handler::Fn(handler));
    }

    /// Add a scoped handler closure rooted at `scope_root`.
    pub fn add_handler_closure(
        &mut self,
        kind: SignalKind,
        scope_root: ComponentId,
        handler: impl FnMut(&mut World, &mut CommandQueue, &mut dyn SignalEmitter, &Signal)
            + Send
            + Sync
            + 'static,
    ) {
        self.scoped_handlers
            .entry(kind)
            .or_default()
            .entry(scope_root)
            .or_default()
            .push(Handler::Closure(Box::new(handler)));
    }

    /// Add a global handler (runs regardless of scope).
    pub fn add_global_handler(&mut self, kind: SignalKind, handler: SignalHandler) {
        self.global_handlers
            .entry(kind)
            .or_default()
            .push(Handler::Fn(handler));
    }

    /// Add a global handler closure (runs regardless of scope).
    pub fn add_global_handler_closure(
        &mut self,
        kind: SignalKind,
        handler: impl FnMut(&mut World, &mut CommandQueue, &mut dyn SignalEmitter, &Signal)
            + Send
            + Sync
            + 'static,
    ) {
        self.global_handlers
            .entry(kind)
            .or_default()
            .push(Handler::Closure(Box::new(handler)));
    }

    pub fn remove_handler(
        &mut self,
        kind: SignalKind,
        scope_root: ComponentId,
        handler: SignalHandler,
    ) -> bool {
        let Some(by_scope) = self.scoped_handlers.get_mut(&kind) else {
            return false;
        };
        let Some(list) = by_scope.get_mut(&scope_root) else {
            return false;
        };

        let before = list.len();
        list.retain(|h| match h {
            Handler::Fn(fp) => *fp as usize != handler as usize,
            Handler::Closure(_) => true,
        });
        let removed = list.len() != before;

        if list.is_empty() {
            by_scope.remove(&scope_root);
        }
        if by_scope.is_empty() {
            self.scoped_handlers.remove(&kind);
        }

        removed
    }

    pub fn remove_global_handler(&mut self, kind: SignalKind, handler: SignalHandler) -> bool {
        let Some(list) = self.global_handlers.get_mut(&kind) else {
            return false;
        };

        let before = list.len();
        list.retain(|h| match h {
            Handler::Fn(fp) => *fp as usize != handler as usize,
            Handler::Closure(_) => true,
        });
        let removed = list.len() != before;

        if list.is_empty() {
            self.global_handlers.remove(&kind);
        }

        removed
    }

    /// Dispatch all signals pushed since the last dispatch.
    ///
    /// This supports immediate-mode signal graphs: you can call this multiple times per frame
    /// at explicit points (e.g. after RayCastSystem runs) so downstream systems can react
    /// without scanning `rx.signals()`.
    ///
    /// Returns the number of signals dispatched.
    pub fn dispatch_new_signals(
        &mut self,
        world: &mut World,
        queue: &mut CommandQueue,
        max_signals: usize,
    ) -> usize {
        let mut dispatched = 0usize;
        while self.dispatched_cursor < self.signals.len() {
            if dispatched >= max_signals {
                break;
            }
            let i = self.dispatched_cursor;
            self.dispatched_cursor += 1;
            dispatched += 1;

            // Clone the signal so handlers are free to push more signals.
            let env = self.signals[i].clone();
            self.dispatch_handlers(world, queue, &env);
        }
        dispatched
    }

    pub fn dispatch_handlers(&mut self, world: &mut World, queue: &mut CommandQueue, env: &Signal) {
        let kind = env.kind();

        let mut emitter = Emitter {
            signals: &mut self.signals as *mut Vec<Signal>,
        };

        // Global handlers (regardless of scope).
        dispatch_global_kind(self, world, queue, &mut emitter, SignalKind::Any, env);
        dispatch_global_kind(self, world, queue, &mut emitter, kind, env);

        let scope_chain = compute_scope_chain(world, env.scope);
        for scope in scope_chain {
            dispatch_scoped_kind(self, world, queue, &mut emitter, SignalKind::Any, scope, env);
            dispatch_scoped_kind(self, world, queue, &mut emitter, kind, scope, env);
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

fn dispatch_global_kind(
    rx: &mut RxWorld,
    world: &mut World,
    queue: &mut CommandQueue,
    emitter: &mut dyn SignalEmitter,
    kind: SignalKind,
    env: &Signal,
) {
    let Some(handlers) = rx.global_handlers.get_mut(&kind) else {
        return;
    };

    // Index-based iteration to avoid borrow issues if a handler mutates handler lists.
    for idx in 0..handlers.len() {
        // SAFETY: idx is in-bounds for current length snapshot.
        let handler_ptr: *mut Handler = &mut handlers[idx];
        // SAFETY: we only use this pointer for the duration of the call.
        unsafe {
            match &mut *handler_ptr {
                Handler::Fn(fp) => fp(world, queue, emitter, env),
                Handler::Closure(f) => f(world, queue, emitter, env),
            }
        }
    }
}

fn dispatch_scoped_kind(
    rx: &mut RxWorld,
    world: &mut World,
    queue: &mut CommandQueue,
    emitter: &mut dyn SignalEmitter,
    kind: SignalKind,
    scope: ComponentId,
    env: &Signal,
) {
    let Some(by_scope) = rx.scoped_handlers.get_mut(&kind) else {
        return;
    };
    let Some(handlers) = by_scope.get_mut(&scope) else {
        return;
    };

    for idx in 0..handlers.len() {
        let handler_ptr: *mut Handler = &mut handlers[idx];
        unsafe {
            match &mut *handler_ptr {
                Handler::Fn(fp) => fp(world, queue, emitter, env),
                Handler::Closure(f) => f(world, queue, emitter, env),
            }
        }
    }
}
