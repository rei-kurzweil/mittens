# MMS Reply Channel, ObjectWorld, and MMQ Status

Date: 2026-04-26

This task note records the current implementation status of:

- the MMS live reply channel
- the handle shape for live spawned components
- `ObjectWorld` as the evaluator-side runtime container
- the current query backend
- a pragmatic MMQ-first MVP plan

It is intentionally status-first. Several older docs still describe some of this as
entirely planned; this file reflects the code currently in the repo.

---

## 1. Current status: live reply channel

There is already a narrow live reply-channel implementation for MMS.

What exists now:

- The evaluator protocol supports `EvalResponse::HostCall { id, kind }` and
  `EvalRequest::HostCallResult { id, value }`.
- `HostCallKind` currently only has `Spawn(MaterializedCE)`.
- `HostValue` currently only has `ComponentId(ComponentId)` and `Null`.
- `MeowMeowRunner::eval_with_world(...)` services `HostCall::Spawn`, calls
  `component_registry::spawn_tree(...)`, and replies with the spawned root
  `ComponentId`.
- In that path, `let x = T { ... }` upgrades from `Value::ComponentExpr(...)` to
  `Value::ComponentObject(id)`.

What does not exist yet:

- GUID in the reply payload
- a query HostCall surface (`Query`, `QueryAll`, etc.)
- method-call dispatch on `ComponentObject`
- emit-context stack for nested emits
- `ObjectWorld` as the actual evaluator environment container
- tracking/release of pending unattached component objects

So Phase 6 is not “unimplemented”; it is partially implemented in a narrow form.

---

## 2. Current identity model

Today the live MMS handle only carries the engine slotmap key:

```rust
Value::ComponentObject(ComponentId)
```

That is enough for immediate mutation/query work inside one runtime session, but it is
not enough for:

- stable debug display
- serialization / diagnostics
- comparing a live handle to externally-authored references
- future editor/session protocols where the user may want both the fast runtime id and
  the stable GUID

The engine already has both pieces:

- `ComponentId` is the slotmap key
- `ComponentNode.guid` is the stable UUID
- `World` maintains a `guid_index: HashMap<Uuid, ComponentId>`

---

## 3. Recommended handle shape

The next step should be to introduce an explicit live handle type and stop treating the
slotmap key as the whole handle.

Recommended shape:

```rust
pub struct ComponentHandle {
    pub id: crate::engine::ecs::ComponentId,
    pub guid: uuid::Uuid,
    pub component_type: String,
}

pub enum Value {
    // ...
    ComponentObject(ComponentHandle),
}
```

Recommended host reply shape:

```rust
pub enum HostValue {
    ComponentHandle(ComponentHandle),
    ComponentHandles(Vec<ComponentHandle>),
    Null,
}
```

Why this shape:

- `id` stays the fast runtime lookup key for direct world operations
- `guid` is immediately available for logging, debugging, persistence bridges, and
  future re-resolution when a stale `id` must be validated or repaired
- `component_type` gives MMS enough runtime type information to dispatch methods on the
  handle without first asking the host "what kind of node is this?"
- it avoids bolting “also fetch guid” onto every future query call separately

What should not happen:

- replacing `ComponentId` with GUID everywhere in runtime code
- storing only GUID in the live evaluator handle and re-resolving through the world on
  every method/query call

That would throw away the main benefit of the reply channel, which is to get the actual
runtime handle once and reuse it cheaply.

`component_type` does not need to be a Rust type id. For the current MMS needs, the
canonical MMS/registry component name is enough (`"T"`, `"Transform"`, `"R"`, etc., as
long as one canonical spelling is chosen for dispatch).

This is a runtime typing requirement even if the static type system stays conservative:

- static type in MMS can remain `ComponentObject` for now
- future static refinement can become `ComponentObject<T>`
- runtime method dispatch should still have access to the concrete root node type now

---

## 4. Recommended ObjectWorld shape

`ObjectWorld` exists in `src/meow_meow/object.rs`, but it is not yet wired into the
evaluator. The evaluator still uses a plain `HashMap<String, Value>` as `Env`.

The useful next-step shape is:

```rust
pub struct ObjectWorld {
    env: HashMap<String, Value>,
    heap: Heap,
    pending: Vec<ComponentHandle>,
}
```

And then:

```rust
impl ObjectWorld {
    pub fn bind(&mut self, name: impl Into<String>, value: Value) { ... }
    pub fn lookup(&self, name: &str) -> Option<&Value> { ... }

    pub fn track_component(&mut self, handle: ComponentHandle) { ... }
    pub fn release_component_by_id(&mut self, id: ComponentId) { ... }
    pub fn is_pending_id(&self, id: ComponentId) -> bool { ... }
    pub fn pending_components(&self) -> &[ComponentHandle] { ... }
}
```

Using `Vec<ComponentHandle>` instead of `Vec<ComponentId>` keeps the pending set aligned
with the actual live value model.

---

## 5. Recommended ObjectWorld / host data flow

### 5.1 Spawn / bind

1. MMS evaluates `let hero = T { ... }`
2. evaluator materializes the CE
3. evaluator emits `HostCall::Spawn(ce)`
4. host spawns the tree and obtains:
   - root `ComponentId`
   - root GUID
5. host replies with `HostValue::ComponentHandle(handle)`
6. evaluator binds `hero = Value::ComponentObject(handle)`
7. evaluator records the handle in `ObjectWorld.pending`

### 5.2 Emit / attach

1. MMS evaluates `emit(hero)` or bare `hero` in expression-statement position
2. evaluator sees `Value::ComponentObject(handle)`
3. evaluator emits an attach/add-root intent using `handle.id`
4. on success, evaluator removes the handle from `ObjectWorld.pending`

This release step is not implemented today because the `ComponentObject` emit path is not
implemented yet.

### 5.3 Evaluate first, attach second

The missing rule that should be made explicit is:

> evaluating a component expression in the live path always produces a root
> `ComponentHandle`; emission only decides whether that already-created root is attached
> into the world topology now, later, or never.

Concretely:

1. `let hero = T { R { C {} } }`
   - spawn/evaluate returns one live root handle for `T`
   - `hero` stores that root handle
   - `R` and `C` are live engine nodes too, but they are not separately bound into the
     MMS env
2. bare `T { ... }` in statement position
   - spawn/evaluate still returns a root handle
   - the evaluator immediately emits/attaches that handle
   - if no variable captures the value, the handle is transient and can be discarded once
     attach succeeds
3. `return T { ... }`, `[T { ... }]`, `f(T { ... })`
   - same root-handle creation rule
   - no auto-attach unless the resulting `ComponentObject` later reaches emit position

This avoids an awkward split where "captured component expressions become objects" but
"emitted component expressions are topology only." There should be one runtime value model:

- evaluation creates a live root handle
- emission attaches that handle
- subtree access to evaluated children happens through query/navigation from the root

The host therefore does not need to return a whole handle tree for `T { R { C {} } }`.
Returning the root handle is enough because the child handles are recoverable through
subtree query once query HostCalls exist.

### 5.4 Query

For world or subtree queries, the host-call shape should return full handles, not just ids:

```rust
HostCallKind::Query { root: Option<ComponentId>, query: QueryRequest }
HostCallKind::QueryAll { root: Option<ComponentId>, query: QueryRequest }
```

Where:

```rust
pub enum QueryRequest {
    Mmq(String),
    Css(String),
}
```

Recommended v1 behavior:

- world query: `root: None`
- subtree query: `root: Some(component_handle.id)`
- single result: `HostValue::ComponentHandle(...)` or `Null`
- multi result: `HostValue::ComponentHandles(...)`

### 5.5 Why not expose `ObjectWorld` to the host

`ObjectWorld` should remain the evaluator-side runtime container only.

The host should only see:

- `HostCallKind`
- `HostValue`
- intents emitted back out

That keeps the worker/main-thread boundary explicit and avoids accidental leakage of
evaluator internals into engine code.

---

## 6. Current query backend status

There are currently two different query layers in the repo.

### 6.1 Live world query in use today

The live engine path is ad hoc:

- `World::find_component(root, selector)`
- `World::find_all_components(root, selector)`
- `Universe::{find_component, find_all_components}` wrap those via query intents/replies

Actual supported selector behavior today is narrow:

- only `[name='...']` / `[name="..."]` in the `World` helper
- DFS preorder traversal under `root`
- no type selector
- no descendant / child combinator semantics beyond full subtree traversal

### 6.2 `src/query` backend

`src/query` is WIP and not integrated into `World` / `Universe`.

Current state:

- `src/query/css/parser.rs` can parse a small CSS-like subset into `QueryAst`
- `src/query/mmq/parser.rs` is still a stub
- `src/query/evaluator.rs` matches compound selectors on a node, but does not implement
  actual combinator semantics for `Child` / `Descendant`
- no adapter from `World` to `QueryTreeAdapter`
- no live call site currently uses `QueryEvaluator`

So “CSS query support exists” is only true at the parser layer. It is not true as an
end-to-end world query feature.

---

## 7. MMQ-first MVP

The original CSS-first direction is heavier than what the current immediate needs require.
For an MMS-first engine workflow, a small MMQ subset is a better first target.

Recommended MMQ v1 grammar:

```text
query        := segment+
segment      := type_selector name_suffix?
type_selector := IDENT               // examples: T, R, C, Transform
name_suffix  := '#' IDENT_OR_STRING  // example: T#hero
```

Examples:

- `T` → any transform in the scoped subtree
- `R` → any renderable
- `T#hero` → transform whose component name is `hero`
- `Transform#avatar_root` → full-name spelling, same semantics

Recommended v1 semantics:

- `find_component(root, "T")`
  - first preorder DFS match by component type
- `find_component(root, "T#hero")`
  - first preorder DFS match where type and name both match
- `find_all_components(...)`
  - all matches in preorder DFS

This is enough to unblock:

- simple subtree navigation from MMS
- glTF / imported hierarchy targeting by authored names
- basic method dispatch on live handles
- a clean path for `component."T"` and `component."T#name"` style subtree lookup

---

## 8. What is missing for that MMQ MVP

### In `src/query`

- implement `src/query/mmq/parser.rs`
- decide whether MMQ parses into the existing `QueryAst` or into a separate `MmqAst`
- integrate `QueryEvaluator` with real combinator semantics or keep MMQ v1 flat enough to
  avoid combinators initially
- add a `WorldQueryAdapter` implementing `QueryTreeAdapter` for ECS `World`

### In the evaluator

- add query HostCall variants
- add `HostValue::ComponentHandle` / `ComponentHandles`
- add subtree query support from `Value::ComponentObject(handle)`

### In the live world API

- replace the current ad hoc name-only parsing in `World::find_component` and
  `find_all_components`
- route `Universe` query intents through the shared query backend

---

## 9. Recommended implementation order

1. Freeze the lifecycle rule in docs: evaluation returns a root `ComponentHandle`; emit
   attaches/releases it.
2. Introduce `ComponentHandle { id, guid, component_type }` and change the reply path to
   return it.
3. Update `Value::ComponentObject` and `ObjectWorld.pending` to use `ComponentHandle`.
4. Add `ComponentObject` emit/release behavior for already-spawned handles.
5. Implement MMQ parser for `T` / `T#name`.
6. Add a `World` adapter for `QueryTreeAdapter`.
7. Route `World::find_component` / `find_all_components` through MMQ first.
8. Add query HostCalls for MMS world/subtree lookup.
9. Add method-call dispatch keyed by `handle.component_type`.
10. Revisit static typing later (`ComponentObject<T>`), after runtime handle typing and
   MMQ v1 are working end to end.
11. Revisit CSS parser/evaluator integration after MMQ v1 is working end to end.

This gives an MVP that is coherent, testable, and aligned with immediate MMS needs,
without pretending the CSS parser alone is already the query system.
