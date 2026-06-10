use std::collections::HashMap;

use crate::engine::ecs::{ComponentId, World};

use super::{
    EventSignal, IntentSignal, Signal, SignalEmitter, SignalHandler, SignalKind, SignalWhen,
};

use super::signal_pipeline::SignalPipeline;

enum Handler {
    Fn {
        handler: SignalHandler,
        name: Option<String>,
    },
    Closure {
        handler: Box<dyn FnMut(&mut World, &mut dyn SignalEmitter, &Signal) + Send + Sync + 'static>,
        name: Option<String>,
    },
}

impl Handler {
    fn name(&self) -> Option<&str> {
        match self {
            Handler::Fn { name, .. } => name.as_deref(),
            Handler::Closure { name, .. } => name.as_deref(),
        }
    }
}

struct Emitter {
    intents: *mut Vec<Signal>,
    pending_intents: *mut Vec<Signal>,
    events_out: *mut Vec<Signal>,
}

impl SignalEmitter for Emitter {
    fn push_event(&mut self, scope: ComponentId, event: EventSignal) {
        // SAFETY: pointers refer to `RxWorld` storage for the duration of dispatch.
        unsafe {
            (*self.events_out).push(Signal::event(scope, event));
        }
    }

    fn push_intent(&mut self, scope: ComponentId, intent: IntentSignal) {
        // SAFETY: pointers refer to `RxWorld` storage for the duration of dispatch.
        unsafe {
            match intent.when {
                SignalWhen::Now => (*self.intents).push(Signal::intent(scope, intent)),
                SignalWhen::AtBeat(_) => {
                    (*self.pending_intents).push(Signal::intent(scope, intent));
                    sort_pending_intents_by_beat(&mut *self.pending_intents);
                }
            }
        }
    }
}

impl SignalEmitter for RxWorld {
    fn push_event(&mut self, scope: ComponentId, event: EventSignal) {
        RxWorld::push_event(self, scope, event);
    }

    fn push_intent(&mut self, scope: ComponentId, intent: IntentSignal) {
        RxWorld::push_intent(self, scope, intent);
    }
}

/// Stores a unified signal stream and dispatches signals to scope-rooted handlers.
///
/// Handlers are keyed by (signal kind, scope root). When dispatching a signal with `scope=S`,
/// handlers attached to `S` or any ancestor of `S` are invoked.
#[derive(Default)]
pub struct RxWorld {
    /// Ready event signals for this tick.
    ready_events: Vec<Signal>,

    /// Event signals produced while processing (handlers/executor). These are deferred until the
    /// next tick.
    deferred_events: Vec<Signal>,

    /// Ready intent signals for this tick.
    ready_intents: Vec<Signal>,

    /// Timed holding-pen for intent signals that should not run until a target transport beat.
    ///
    /// Invariant: this list is kept sorted by target beat ascending.
    pending_intents: Vec<Signal>,

    /// Scoped handlers keyed by (kind, scope_root).
    scoped_handlers: HashMap<SignalKind, HashMap<ComponentId, Vec<Handler>>>,

    /// Optional global handlers keyed by signal kind.
    ///
    /// These are useful for system-level observers (gesture/editor) that need to see events
    /// regardless of scope.
    global_handlers: HashMap<SignalKind, Vec<Handler>>,

    /// Component-targeted intent routing pipelines.
    ///
    /// Keyed by the component id the pipeline applies to.
    pipelines_by_component: HashMap<ComponentId, Vec<SignalPipeline>>,

    /// Reverse index for efficient removal: operator component id -> pipeline owner component id.
    pipeline_owner_by_operator: HashMap<ComponentId, ComponentId>,
}

impl std::fmt::Debug for RxWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RxWorld")
            .field("ready_events_len", &self.ready_events.len())
            .field("deferred_events_len", &self.deferred_events.len())
            .field("ready_intents_len", &self.ready_intents.len())
            .field("pending_intents_len", &self.pending_intents.len())
            .field("global_kinds", &self.global_handlers.len())
            .field("scoped_kinds", &self.scoped_handlers.len())
            .field(
                "pipeline_owner_by_operator_len",
                &self.pipeline_owner_by_operator.len(),
            )
            .field(
                "pipelines_by_component_len",
                &self.pipelines_by_component.len(),
            )
            .finish()
    }
}

impl RxWorld {
    pub(crate) fn pipelines_for_component(&self, component: ComponentId) -> &[SignalPipeline] {
        static EMPTY: [SignalPipeline; 0] = [];
        self.pipelines_by_component
            .get(&component)
            .map(|v| v.as_slice())
            .unwrap_or(&EMPTY)
    }

    pub(crate) fn register_pipeline(&mut self, owner: ComponentId, pipeline: SignalPipeline) {
        self.remove_pipelines_from_operator(pipeline.source_operator);

        self.pipeline_owner_by_operator
            .insert(pipeline.source_operator, owner);
        self.pipelines_by_component
            .entry(owner)
            .or_default()
            .push(pipeline);
    }

    pub(crate) fn remove_pipelines_from_operator(&mut self, operator: ComponentId) {
        let Some(owner) = self.pipeline_owner_by_operator.remove(&operator) else {
            return;
        };

        let Some(list) = self.pipelines_by_component.get_mut(&owner) else {
            return;
        };

        list.retain(|p| p.source_operator != operator);
        if list.is_empty() {
            self.pipelines_by_component.remove(&owner);
        }
    }

    pub fn push_event(&mut self, scope: ComponentId, event: EventSignal) {
        self.ready_events.push(Signal::event(scope, event));
    }

    pub fn push_intent(&mut self, scope: ComponentId, intent: IntentSignal) {
        match intent.when {
            SignalWhen::Now => self.ready_intents.push(Signal::intent(scope, intent)),
            SignalWhen::AtBeat(_) => {
                self.pending_intents.push(Signal::intent(scope, intent));
                sort_pending_intents_by_beat(&mut self.pending_intents);
            }
        }
    }

    /// Move any pending timed signals whose target beat is now due into the per-frame queue.
    ///
    /// Returns the number of promoted signals.
    pub fn promote_due_intents(&mut self, now_beat: f64) -> usize {
        if self.pending_intents.is_empty() {
            return 0;
        }

        let eps = 1e-9;
        let mut end = 0usize;
        while end < self.pending_intents.len() {
            let Some(intent) = self.pending_intents[end].intent.as_ref() else {
                end += 1;
                continue;
            };
            let SignalWhen::AtBeat(b) = intent.when else {
                end += 1;
                continue;
            };
            if b <= now_beat + eps {
                end += 1;
            } else {
                break;
            }
        }

        if end == 0 {
            return 0;
        }

        let due: Vec<Signal> = self.pending_intents.drain(..end).collect();
        let promoted = due.len();
        self.ready_intents.extend(due);
        promoted
    }

    /// Reset drain-point dispatch state for a new frame.
    ///
    /// In the current architecture, signals are typically drained once per frame in
    /// `SystemWorld::process_commands`, which also implicitly resets this cursor.
    pub fn begin_frame(&mut self) {
        if !self.deferred_events.is_empty() {
            self.ready_events
                .extend(std::mem::take(&mut self.deferred_events));
        }
    }

    /// Returns the current queued signals for this frame.
    ///
    /// This is intentionally read-only: signals are drained and dispatched later in
    /// `SystemWorld::process_commands`.
    pub fn drain_ready_events(&mut self) -> Vec<Signal> {
        std::mem::take(&mut self.ready_events)
    }

    pub fn drain_ready_intents(&mut self) -> Vec<Signal> {
        std::mem::take(&mut self.ready_intents)
    }

    pub fn has_ready_events(&self) -> bool {
        !self.ready_events.is_empty()
    }

    pub fn has_ready_intents(&self) -> bool {
        !self.ready_intents.is_empty()
    }

    pub fn requeue_ready_events(&mut self, mut events: Vec<Signal>) {
        if events.is_empty() {
            return;
        }
        self.ready_events.append(&mut events);
    }

    pub fn requeue_ready_intents(&mut self, mut intents: Vec<Signal>) {
        if intents.is_empty() {
            return;
        }
        self.ready_intents.append(&mut intents);
    }

    /// Add a scoped handler rooted at `scope_root`.
    ///
    /// Note: this is a function pointer (no captures). Use `add_handler_closure` internally
    /// when you need stateful handlers.
    pub fn add_handler(
        &mut self,
        kind: SignalKind,
        scope_root: ComponentId,
        handler: SignalHandler,
    ) {
        self.add_handler_named(kind, scope_root, None, handler);
    }

    /// Add a named scoped handler rooted at `scope_root`.
    pub fn add_handler_named(
        &mut self,
        kind: SignalKind,
        scope_root: ComponentId,
        name: Option<String>,
        handler: SignalHandler,
    ) {
        let list = self
            .scoped_handlers
            .entry(kind)
            .or_default()
            .entry(scope_root)
            .or_default();

        // Idempotent registration for function-pointer handlers.
        // This avoids duplicate dispatches if a system registers handlers multiple times
        // for the same (kind, scope_root).
        let handler_usize = handler as usize;
        if list.iter().any(|h| match h {
            Handler::Fn {
                handler: fp,
                name: existing_name,
            } => *fp as usize == handler_usize && *existing_name == name,
            _ => false,
        }) {
            return;
        }

        list.push(Handler::Fn { handler, name });
    }

    /// Add a scoped handler closure rooted at `scope_root`.
    pub fn add_handler_closure(
        &mut self,
        kind: SignalKind,
        scope_root: ComponentId,
        handler: impl FnMut(&mut World, &mut dyn SignalEmitter, &Signal) + Send + Sync + 'static,
    ) {
        self.add_handler_closure_named(kind, scope_root, None, handler);
    }

    /// Add a named scoped handler closure rooted at `scope_root`.
    pub fn add_handler_closure_named(
        &mut self,
        kind: SignalKind,
        scope_root: ComponentId,
        name: Option<String>,
        handler: impl FnMut(&mut World, &mut dyn SignalEmitter, &Signal) + Send + Sync + 'static,
    ) {
        self.scoped_handlers
            .entry(kind)
            .or_default()
            .entry(scope_root)
            .or_default()
            .push(Handler::Closure {
                handler: Box::new(handler),
                name,
            });
    }

    /// Add a global handler rooted at no scope.
    ///
    /// Note: this is a function pointer (no captures). Use `add_global_handler_closure`
    /// when you need stateful handlers.
    pub fn add_global_handler(&mut self, kind: SignalKind, handler: SignalHandler) {
        self.add_global_handler_named(kind, None, handler);
    }

    /// Add a named global handler rooted at no scope.
    pub fn add_global_handler_named(
        &mut self,
        kind: SignalKind,
        name: Option<String>,
        handler: SignalHandler,
    ) {
        let list = self.global_handlers.entry(kind).or_default();

        let handler_usize = handler as usize;
        if list.iter().any(|h| match h {
            Handler::Fn {
                handler: fp,
                name: existing_name,
            } => *fp as usize == handler_usize && *existing_name == name,
            _ => false,
        }) {
            return;
        }

        list.push(Handler::Fn { handler, name });
    }

    /// Add a global handler closure rooted at no scope.
    pub fn add_global_handler_closure(
        &mut self,
        kind: SignalKind,
        handler: impl FnMut(&mut World, &mut dyn SignalEmitter, &Signal) + Send + Sync + 'static,
    ) {
        self.add_global_handler_closure_named(kind, None, handler);
    }

    /// Add a named global handler closure rooted at no scope.
    pub fn add_global_handler_closure_named(
        &mut self,
        kind: SignalKind,
        name: Option<String>,
        handler: impl FnMut(&mut World, &mut dyn SignalEmitter, &Signal) + Send + Sync + 'static,
    ) {
        self.global_handlers
            .entry(kind)
            .or_default()
            .push(Handler::Closure {
                handler: Box::new(handler),
                name,
            });
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
            Handler::Fn {
                handler: fp,
                name: _,
            } => *fp as usize != handler as usize,
            Handler::Closure { .. } => true,
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

    /// Remove all *scoped* handlers rooted at `scope_root`.
    ///
    /// This is intended for component lifecycle cleanup: when a component (or subtree) is removed
    /// from the `World`, any handlers rooted at those component ids should be removed to avoid
    /// unbounded growth of the handler maps.
    pub fn remove_all_scoped_handlers_for_scope(&mut self, scope_root: ComponentId) -> usize {
        if self.scoped_handlers.is_empty() {
            return 0;
        }

        let mut removed = 0usize;

        // Clone keys to avoid borrowing issues while mutating the map.
        let kinds: Vec<SignalKind> = self.scoped_handlers.keys().copied().collect();
        for kind in kinds {
            let Some(by_scope) = self.scoped_handlers.get_mut(&kind) else {
                continue;
            };

            if let Some(list) = by_scope.remove(&scope_root) {
                removed += list.len();
            }

            if by_scope.is_empty() {
                self.scoped_handlers.remove(&kind);
            }
        }

        removed
    }

    pub fn remove_all_scoped_handlers_for_scopes(
        &mut self,
        scopes: impl IntoIterator<Item = ComponentId>,
    ) -> usize {
        let mut removed = 0usize;
        for scope in scopes {
            removed += self.remove_all_scoped_handlers_for_scope(scope);
        }
        removed
    }

    pub fn dispatch_event_handlers(&mut self, world: &mut World, env: &Signal) {
        let Some(event) = env.event.as_ref() else {
            return;
        };
        let kind = event.kind();

        let mut emitter = Emitter {
            intents: &mut self.ready_intents as *mut Vec<Signal>,
            pending_intents: &mut self.pending_intents as *mut Vec<Signal>,
            events_out: &mut self.deferred_events as *mut Vec<Signal>,
        };

        dispatch_global_kind(self, world, &mut emitter, SignalKind::Any, env);
        dispatch_global_kind(self, world, &mut emitter, kind, env);

        let scope_chain = compute_scope_chain(world, env.scope);
        for scope in scope_chain {
            dispatch_scoped_kind(self, world, &mut emitter, SignalKind::Any, scope, env);
            dispatch_scoped_kind(self, world, &mut emitter, kind, scope, env);
        }
    }
}

fn dispatch_global_kind(
    rx: &mut RxWorld,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    kind: SignalKind,
    env: &Signal,
) {
    let Some(list) = rx.global_handlers.get_mut(&kind) else {
        return;
    };

    for handler in list {
        match handler {
            Handler::Fn { handler: fp, .. } => fp(world, emit, env),
            Handler::Closure { handler: c, .. } => c(world, emit, env),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::IntentValue;
    use slotmap::KeyData;

    fn cid(ffi: u64) -> ComponentId {
        KeyData::from_ffi(ffi).into()
    }

    #[test]
    fn timed_signals_are_held_until_due() {
        let mut rx = RxWorld::default();

        rx.push_intent(cid(1), IntentSignal::at_beat(10.0, IntentValue::Noop));
        assert_eq!(rx.ready_intents.len(), 0);

        rx.push_intent_now(
            cid(1),
            IntentValue::Print {
                message: "hi".to_string(),
            },
        );
        assert_eq!(rx.ready_intents.len(), 1);

        assert_eq!(rx.promote_due_intents(0.0), 0);
        assert_eq!(rx.ready_intents.len(), 1);

        assert_eq!(rx.promote_due_intents(10.0), 1);
        assert_eq!(rx.ready_intents.len(), 2);

        // Drain should clear only ready intents.
        let drained = rx.drain_ready_intents();
        assert_eq!(drained.len(), 2);
        assert_eq!(rx.ready_intents.len(), 0);
    }
}

fn sort_pending_intents_by_beat(pending: &mut Vec<Signal>) {
    pending.sort_by(|a, b| {
        let ba = a
            .intent
            .as_ref()
            .and_then(|i| i.when.beat())
            .unwrap_or(f64::NEG_INFINITY);
        let bb = b
            .intent
            .as_ref()
            .and_then(|i| i.when.beat())
            .unwrap_or(f64::NEG_INFINITY);
        ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn dispatch_scoped_kind(
    rx: &mut RxWorld,
    world: &mut World,
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

    let router = world
        .children_of(scope)
        .iter()
        .find_map(|&ch| world.get_component_by_id_as::<crate::engine::ecs::component::SignalObserverRouterComponent>(ch));
    let (blacklist, whitelist) = if let Some(router) = router {
        (Some(router.blacklist.clone()), Some(router.whitelist.clone()))
    } else {
        (None, None)
    };

    for idx in 0..handlers.len() {
        let handler_ptr: *mut Handler = &mut handlers[idx];
        unsafe {
            if let Some(blacklist) = blacklist.as_ref() {
                if let Some(name) = (*handler_ptr).name() {
                    if blacklist.iter().any(|b| b == name) {
                        continue;
                    }
                    if let Some(whitelist) = whitelist.as_ref() {
                        if !whitelist.is_empty() && !whitelist.iter().any(|w| w == name) {
                            continue;
                        }
                    }
                }
            }

            match &mut *handler_ptr {
                Handler::Fn { handler: fp, .. } => fp(world, emitter, env),
                Handler::Closure { handler: f, .. } => f(world, emitter, env),
            }
        }
    }
}
