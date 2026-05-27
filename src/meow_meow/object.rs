use std::collections::HashMap;

use crate::engine::ecs::ComponentId;

// ---------------------------------------------------------------------------
// Materialized component expression
// ---------------------------------------------------------------------------

/// A fully-evaluated component expression: all values are concrete `Value`s,
/// all control flow has been expanded, all constructor args evaluated.
///
/// Produced by the evaluator when a `ComponentExpression` AST node is
/// evaluated. Passed to `spawn_tree` in the registry — no MMS expression
/// evaluation needed on the main thread.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedCE {
    /// Component type name (short or full, e.g. `"T"` / `"Transform"`).
    pub component_type: String,
    /// First constructor call method, e.g. `"position"` from `T.position(...)`.
    pub ctor_method: Option<String>,
    /// First constructor call args, evaluated.
    pub ctor_args: Vec<Value>,
    /// Remaining chained constructor calls + body builder calls, in source order.
    /// e.g. `.scale(...)` after `.position(...)`, plus `fps_rotation()` in the body.
    pub calls: Vec<(String, Vec<Value>)>,
    /// Named property assignments from the body, e.g. `intensity = 0.9`.
    pub named: Vec<(String, Value)>,
    /// String-type positional content (e.g. `Text { "hello " + name }`).
    pub positionals: Vec<Value>,
    /// Child component trees, in source order. Each entry is either a CE to
    /// spawn fresh, or a pre-Registered `ComponentId` to splice in.
    pub children: Vec<CeChild>,
}

/// A child slot inside a `MaterializedCE`.
///
/// `Spawn` is the normal case (the body produced a fresh CE). `Attach` is used
/// when the body referenced a `Value::ComponentObject` — a previously
/// `Register`ed component — that should be attached as a child rather than
/// re-created.
#[derive(Debug, Clone, PartialEq)]
pub enum CeChild {
    Spawn(MaterializedCE),
    Attach(ComponentId),
}

// ---------------------------------------------------------------------------
// Runtime values
// ---------------------------------------------------------------------------

/// Runtime value representation for Meow Meow evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    /// Numeric value tagged with a source-level unit suffix (e.g. `50%`,
    /// `20gu`, `30deg`). Produced by `Expression::Dimension`. Consumers
    /// such as the Style setters use this to disambiguate `Percent` vs
    /// `GlyphUnits` at the boundary between MMS values and engine types.
    Dimension { value: f64, unit: crate::meow_meow::token::Unit },
    String(String),
    Array(Vec<Value>),

    /// A live engine component (already spawned). Holds the engine-side
    /// `ComponentId` and the MMS component type name (e.g. `"Anim"`, `"T"`).
    /// Produced when `let x = CE` is evaluated with a live reply channel
    /// (`eval_with_world`). The `component_type` drives method dispatch.
    ComponentObject { id: ComponentId, component_type: String },

    /// Heap-allocated MMS object (map / record / instance).
    Object(ObjectId),

    /// Symbolic identifier value (e.g. enum-like flags passed to constructors).
    Identifier(String),

    /// A fully-evaluated component expression ready to spawn.
    /// Produced whenever a `ComponentExpression` AST node is evaluated.
    ComponentExpr(Box<MaterializedCE>),

    /// A closure: params + body AST + captured environment snapshot.
    Function {
        params: Vec<String>,
        body: crate::meow_meow::ast::BlockStatement,
        captured_env: HashMap<String, Value>,
    },

    /// A loaded module: named exports + ordered sequence of root CE emits.
    Module {
        named: HashMap<String, Value>,
        sequence: Vec<MaterializedCE>,
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

/// What a scope frame can do at its boundary.
///
/// - `Block` — fully transparent: read & reassign walk past it. Used for plain
///   blocks, loops, if-bodies, and CE bodies (CE bodies are not write-barriered;
///   children can mutate parent CE locals if they choose to).
/// - `Function` — hard barrier: read & reassign both stop. Seeded with a closure's
///   `captured_env`; the function body cannot see the caller's locals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Block,
    Function,
}

#[derive(Debug, Default)]
struct Frame {
    kind_or_root: Option<FrameKind>,
    bindings: HashMap<String, Value>,
}

impl Frame {
    fn new(kind: FrameKind) -> Self {
        Self { kind_or_root: Some(kind), bindings: HashMap::new() }
    }
    fn root() -> Self {
        Self { kind_or_root: None, bindings: HashMap::new() }
    }
    fn is_function_barrier(&self) -> bool {
        matches!(self.kind_or_root, Some(FrameKind::Function))
    }
}

/// The scripting-side runtime container. Lives on the MMS worker thread.
///
/// Holds the lexical scope chain (a stack of frames) and the MMS-side heap.
/// Communication with the engine goes through intent channels owned by the
/// evaluator — `ObjectWorld` itself never sends intents directly.
///
/// See `docs/meow_meow/spec/env-heap-object-world.md` for the full design and
/// `docs/meow_meow/task/frame-stack-object-world.md` for the migration plan.
#[derive(Debug)]
pub struct ObjectWorld {
    frames: Vec<Frame>,
    heap: Heap,
}

impl Default for ObjectWorld {
    fn default() -> Self {
        Self {
            frames: vec![Frame::root()],
            heap: Heap::new(),
        }
    }
}

impl ObjectWorld {
    /// New `ObjectWorld` with one root frame already pushed.
    pub fn new() -> Self {
        Self::default()
    }

    // --- Frame management ---

    pub fn push_frame(&mut self, kind: FrameKind) {
        self.frames.push(Frame::new(kind));
    }

    /// Push a function frame seeded with the closure's captured environment.
    /// This is a hard barrier: lookups inside the function will not see past it.
    pub fn push_function_frame(&mut self, captured: HashMap<String, Value>) {
        self.frames.push(Frame {
            kind_or_root: Some(FrameKind::Function),
            bindings: captured,
        });
    }

    pub fn pop_frame(&mut self) {
        // Never pop the root frame.
        if self.frames.len() > 1 {
            self.frames.pop();
        }
    }

    pub fn frame_depth(&self) -> usize {
        self.frames.len()
    }

    // --- Variable environment ---

    /// Bind a name in the top (innermost) frame. Shadows outer bindings.
    pub fn bind(&mut self, name: impl Into<String>, value: Value) {
        let top = self.frames.last_mut().expect("ObjectWorld: no frames");
        top.bindings.insert(name.into(), value);
    }

    /// Look up a name. Walks frames from innermost outward; stops at a
    /// `Function` barrier (function bodies cannot see the caller's locals).
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        for frame in self.frames.iter().rev() {
            if let Some(v) = frame.bindings.get(name) {
                return Some(v);
            }
            if frame.is_function_barrier() {
                return None;
            }
        }
        None
    }

    /// Whether `name` is reachable from the current scope (same walk as `lookup`).
    pub fn has(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Reassign an existing binding. Walks frames inward-to-outward looking for
    /// the declaring frame; stops at a `Function` barrier.
    ///
    /// Errors:
    /// - name not declared anywhere reachable
    /// - name declared only beyond the function barrier (caller's locals)
    pub fn reassign(&mut self, name: &str, value: Value) -> Result<(), String> {
        for frame in self.frames.iter_mut().rev() {
            if frame.bindings.contains_key(name) {
                frame.bindings.insert(name.to_string(), value);
                return Ok(());
            }
            if matches!(frame.kind_or_root, Some(FrameKind::Function)) {
                return Err(format!(
                    "cannot reassign '{}' from inside function (only its captured snapshot is visible)",
                    name
                ));
            }
        }
        Err(format!("reassignment: '{}' is not defined", name))
    }

    /// Flatten all frames visible from the current point into a single map for
    /// closure capture. Walks innermost outward, **including** the first
    /// `Function` barrier's frame (so a closure created inside a function sees
    /// that function's captured snapshot), then stops. Inner names shadow outer.
    pub fn snapshot_visible(&self) -> HashMap<String, Value> {
        let mut out: HashMap<String, Value> = HashMap::new();
        for frame in self.frames.iter().rev() {
            for (k, v) in &frame.bindings {
                out.entry(k.clone()).or_insert_with(|| v.clone());
            }
            if frame.is_function_barrier() {
                break;
            }
        }
        out
    }

    // --- Heap access ---

    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.heap
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn n(x: f64) -> Value {
        Value::Number(x)
    }

    #[test]
    fn root_frame_bind_and_lookup() {
        let mut ow = ObjectWorld::new();
        ow.bind("x", n(1.0));
        assert_eq!(ow.lookup("x"), Some(&n(1.0)));
        assert!(ow.has("x"));
        assert!(!ow.has("y"));
    }

    #[test]
    fn block_frame_is_transparent_for_read_and_write() {
        let mut ow = ObjectWorld::new();
        ow.bind("x", n(1.0));
        ow.push_frame(FrameKind::Block);
        // Read sees outer
        assert_eq!(ow.lookup("x"), Some(&n(1.0)));
        // Reassign writes through to outer
        ow.reassign("x", n(2.0)).unwrap();
        ow.pop_frame();
        assert_eq!(ow.lookup("x"), Some(&n(2.0)));
    }

    #[test]
    fn block_frame_local_let_does_not_leak() {
        let mut ow = ObjectWorld::new();
        ow.push_frame(FrameKind::Block);
        ow.bind("local", n(42.0));
        assert_eq!(ow.lookup("local"), Some(&n(42.0)));
        ow.pop_frame();
        assert_eq!(ow.lookup("local"), None);
    }

    #[test]
    fn function_frame_blocks_read_of_caller_locals() {
        let mut ow = ObjectWorld::new();
        ow.bind("caller_var", n(1.0));
        let mut captured = HashMap::new();
        captured.insert("captured_var".to_string(), n(99.0));
        ow.push_function_frame(captured);
        assert_eq!(ow.lookup("captured_var"), Some(&n(99.0)));
        assert_eq!(ow.lookup("caller_var"), None);
    }

    #[test]
    fn function_frame_blocks_reassign_of_caller_locals() {
        let mut ow = ObjectWorld::new();
        ow.bind("caller_var", n(1.0));
        ow.push_function_frame(HashMap::new());
        let err = ow.reassign("caller_var", n(2.0)).unwrap_err();
        assert!(err.contains("inside function"), "got: {}", err);
    }

    #[test]
    fn reassign_undefined_errors() {
        let mut ow = ObjectWorld::new();
        let err = ow.reassign("nope", n(1.0)).unwrap_err();
        assert!(err.contains("not defined"), "got: {}", err);
    }

    #[test]
    fn nested_block_reassign_walks_to_declaring_frame() {
        let mut ow = ObjectWorld::new();
        ow.bind("sum", n(0.0));
        ow.push_frame(FrameKind::Block);
        ow.push_frame(FrameKind::Block);
        ow.reassign("sum", n(6.0)).unwrap();
        ow.pop_frame();
        ow.pop_frame();
        assert_eq!(ow.lookup("sum"), Some(&n(6.0)));
    }

    #[test]
    fn snapshot_visible_flattens_with_inner_shadowing() {
        let mut ow = ObjectWorld::new();
        ow.bind("a", n(1.0));
        ow.bind("b", n(2.0));
        ow.push_frame(FrameKind::Block);
        ow.bind("b", n(20.0)); // shadows
        ow.bind("c", n(3.0));
        let snap = ow.snapshot_visible();
        assert_eq!(snap.get("a"), Some(&n(1.0)));
        assert_eq!(snap.get("b"), Some(&n(20.0))); // inner wins
        assert_eq!(snap.get("c"), Some(&n(3.0)));
    }

    #[test]
    fn snapshot_visible_stops_at_function_barrier() {
        let mut ow = ObjectWorld::new();
        ow.bind("caller", n(1.0));
        let mut captured = HashMap::new();
        captured.insert("cap".to_string(), n(2.0));
        ow.push_function_frame(captured);
        ow.bind("inner", n(3.0));
        let snap = ow.snapshot_visible();
        assert_eq!(snap.get("inner"), Some(&n(3.0)));
        assert_eq!(snap.get("cap"), Some(&n(2.0)));
        assert_eq!(snap.get("caller"), None);
    }

    #[test]
    fn pop_frame_does_not_pop_root() {
        let mut ow = ObjectWorld::new();
        let depth0 = ow.frame_depth();
        ow.pop_frame();
        ow.pop_frame();
        assert_eq!(ow.frame_depth(), depth0);
    }

    #[test]
    fn bind_in_inner_frame_shadows_outer_for_lookup() {
        let mut ow = ObjectWorld::new();
        ow.bind("x", n(1.0));
        ow.push_frame(FrameKind::Block);
        ow.bind("x", n(2.0));
        assert_eq!(ow.lookup("x"), Some(&n(2.0)));
        ow.pop_frame();
        assert_eq!(ow.lookup("x"), Some(&n(1.0)));
    }
}
