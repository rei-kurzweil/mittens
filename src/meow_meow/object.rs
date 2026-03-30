use std::collections::HashMap;

use crate::engine::ecs::ComponentId;

// ---------------------------------------------------------------------------
// Runtime values
// ---------------------------------------------------------------------------

/// Runtime value representation for Meow Meow evaluation.
///
/// Primitive values live inline. Heap-allocated MMS objects are addressed by
/// `ObjectId`. Engine-side components created but not yet emitted/attached are
/// addressed by `ComponentId` (the engine's own stable key).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),

    /// A live engine component created but not yet attached to any parent.
    ///
    /// Holds the engine-side `ComponentId`. When this value appears in
    /// statement position (bare identifier, free-standing expression), the
    /// evaluator emits it to the engine (sends `SpawnComponentTree` /
    /// `Attach` intent to the main thread).
    ComponentObject(ComponentId),

    /// Heap-allocated MMS object (map / record / instance).
    Object(ObjectId),

    /// Symbolic identifier value (e.g. positional flags like `QUAD_2D`).
    ///
    /// Kept distinct from `String` to preserve intent when passing
    /// enum-variant-like bare identifiers to host-defined component constructors.
    Identifier(String),

    /// Unresolved component expression (pre-Phase-6 placeholder).
    /// Captured from a `let x = ComponentType { ... }` binding or expression.
    ComponentExpr(Box<crate::meow_meow::ast::ComponentExpression>),

    /// A closure: params + body AST + captured environment snapshot.
    Function {
        params: Vec<String>,
        body: crate::meow_meow::ast::BlockStatement,
        captured_env: HashMap<String, Value>,
    },

    /// A loaded module: named exports + ordered sequence of root CE emits.
    Module {
        named: HashMap<String, Value>,
        sequence: Vec<crate::meow_meow::ast::ComponentExpression>,
    },
}

// ---------------------------------------------------------------------------
// MMS heap objects
// ---------------------------------------------------------------------------

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ObjectId(u32);

impl ObjectId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Object {
    /// Simple string-keyed map.
    Map(HashMap<String, Value>),
}

#[derive(Debug, Default)]
pub struct Heap {
    objects: Vec<Object>,
}

impl Heap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc(&mut self, object: Object) -> ObjectId {
        let id = ObjectId(
            self.objects.len().try_into().expect("too many heap objects"),
        );
        self.objects.push(object);
        id
    }

    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(id.0 as usize)
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(id.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ObjectWorld — the MMS worker thread's evaluated object layer
// ---------------------------------------------------------------------------

/// The scripting-side runtime container. Lives on the MMS worker thread.
///
/// Holds the variable environment (scope), the MMS-side heap, and the set of
/// `ComponentObject`s that have been created in the engine but not yet emitted
/// (attached to the world or to a parent).
///
/// Communication with the engine goes through intent channels owned by the
/// evaluator — `ObjectWorld` itself never sends intents directly.
///
/// See `docs/meow_meow/analysis/object-world.md` for the full design.
#[derive(Debug, Default)]
pub struct ObjectWorld {
    /// Flat variable environment (v1: no scope chain yet).
    env: HashMap<String, Value>,
    /// MMS-side heap for map/record objects.
    heap: Heap,
    /// ComponentIds of components created but not yet attached/emitted.
    pending: Vec<ComponentId>,
}

impl ObjectWorld {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Variable environment ---

    /// Bind a name to a value in the current scope.
    pub fn bind(&mut self, name: impl Into<String>, value: Value) {
        self.env.insert(name.into(), value);
    }

    /// Look up a name in the current scope.
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.env.get(name)
    }

    // --- ComponentObject tracking ---

    /// Record a `ComponentId` as pending (created, not yet emitted/attached).
    pub fn track_component(&mut self, id: ComponentId) {
        if !self.pending.contains(&id) {
            self.pending.push(id);
        }
    }

    /// Remove a `ComponentId` from the pending list (it has been emitted or attached).
    pub fn release_component(&mut self, id: ComponentId) {
        self.pending.retain(|&p| p != id);
    }

    /// Returns `true` if the given component has been created but not yet emitted.
    pub fn is_pending(&self, id: ComponentId) -> bool {
        self.pending.contains(&id)
    }

    /// All currently pending (created, unattached) component IDs.
    pub fn pending_components(&self) -> &[ComponentId] {
        &self.pending
    }

    // --- Heap access ---

    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.heap
    }
}
