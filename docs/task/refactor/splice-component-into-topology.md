# Splicing a component into existing topology

Date: 2026-03-20

Historical note: examples below that mention `TransformPipeline` / `TransformPipelineOutput` describe the removed authored wrapper/output topology. Current authored transform shaping uses `TransformForkTRS` as the root operator node, with downstream content attached directly under that fork.

This note proposes a small but high-value topology convenience API:

> Insert a new component **between an existing parent and child** without rebuilding the subtree.

Motivating example:
- a spawned glTF / VTuber armature already has stable transform component IDs,
- we may want to insert a `ControllerXRComponent` between `LowerArm` and `Wrist`,
- the wrist / hand / finger subtree should stay intact,
- `SkinnedMeshSystem` should keep seeing the same wrist/hand/finger transform IDs,
- only the topology edge changes from:

```text
LowerArm -> Wrist
```

to:

```text
LowerArm -> ControllerXR -> Wrist
```

No code changes are proposed yet; this doc is about the desired API and semantics.

---

## 1. Why this is useful

Today, if we want to place a component between two existing nodes, we effectively need to do a multi-step topology rewrite:

1. create the new component,
2. detach the child from its current parent,
3. attach the new component under the old parent,
4. attach the old child under the new component,
5. possibly initialize the inserted subtree correctly,
6. possibly emit the right parent/topology signals.

This is mechanically simple, but it is easy to get wrong because the behavior is split across:
- raw `World` graph mutation methods,
- `Universe` lifecycle helpers,
- signal emission / observer expectations,
- component `init()` behavior.

So this deserves a dedicated helper.

---

## 2. Current gap in the API

Today the low-level graph API gives us:
- `World::add_child(parent, child)`
- `World::set_parent(child, Some(parent))`
- `World::detach_from_parent(child)`

These are topology rewrites only.

They do **not** by themselves guarantee:
- `ParentChanged` event emission,
- subtree initialization behavior,
- special handling for “insert this new node between these two existing nodes”.

Separately, `Universe` gives us safer higher-level operations like:
- `Universe::attach(parent, child)`
- `Universe::add(root)`

Those are closer to the right lifecycle semantics, but there is still no single convenience method for:

```text
splice new_component between existing parent and existing child
```

---

## 3. Desired operation

Conceptually, we want something like:

```rust
splice_between(parent, child, inserted)
```

with the precondition that:
- `child` is currently a direct child of `parent`.

After the operation:
- `inserted` becomes a direct child of `parent`,
- `child` becomes a direct child of `inserted`,
- all descendants of `child` stay exactly where they were beneath `child`,
- the IDs of `child` and all its descendants remain unchanged.

So the operation rewires **one edge into two edges**:

```text
before: parent -> child
after:  parent -> inserted -> child
```

---

## 4. Why this matters for the VTuber armature case

For a spawned avatar skeleton, we often care about stable transform IDs because downstream systems already refer to those nodes:
- skin binding resolution,
- transform propagation,
- later semantic targeting or retargeting,
- debugging / inspection tools.

If we inserted a new node by rebuilding or cloning the wrist subtree, that would change IDs and complicate:
- `SkinnedMeshSystem` expectations,
- cached joint resolution,
- any stored references to named/selected bones.

By contrast, a splice operation preserves:
- the original wrist transform ID,
- the original hand/finger subtree,
- all existing descendants under the wrist.

That is exactly what we want for cases like:
- `LowerArm -> ControllerXR -> Wrist`
- `Hand -> TransformPipeline -> FingerRoot`
- future helper components inserted into imported skeletons.

---

## 5. Recommended API level

This should probably exist at the **`Universe` API level first**, not just raw `World`.

Reason:
- raw `World` handles graph mutation,
- `Universe` is where we want common topology+lifecycle operations to live,
- `Universe` is already the safer public path for attach/init behavior.

So the most useful public convenience is something like:

```rust
Universe::splice_between(parent, child, inserted)
```

Potentially with a lower-level helper underneath:

```rust
World::splice_between(parent, child, inserted)
```

But the important point is:
- the **publicly encouraged** API should probably be on `Universe`.

That matches the direction in `docs/task/refactor/private-world-api.md`.

---

## 6. Required semantics

A splice helper should do more than just graph rewiring.

## 6.1 Topology semantics

Required:
- validate all three IDs exist,
- validate `child` is currently parented directly under `parent`,
- reject `parent == child`, `parent == inserted`, `child == inserted`,
- reject cycles,
- after success, topology must be:
  - `parent.children` contains `inserted` (not `child`)
  - `inserted.children` contains `child`
  - `child.parent == Some(inserted)`

The operation should be treated as one conceptual topology mutation even if internally implemented via two reparent steps.

## 6.2 Lifecycle semantics

This is the important extra requirement from the prompt:

> the inserted component should be re-initialized so it can react to its new children.

That means the splice helper should not behave like a purely structural `set_parent` sequence.

Desired behavior:
- if the inserted component is not initialized yet, initialize it in its new position,
- if it is already initialized, allow a lifecycle hook that re-runs child-sensitive setup for the inserted node.

Why:
- some components only become meaningful once they have children,
- `ControllerXRComponent` is a good example of a component whose practical behavior depends on its child transform relationship,
- future topology-sensitive components may scan descendants or emit registration intents based on new subtree context.

So the splice convenience should explicitly support **“inserted node reacts to newly adopted child subtree”** semantics.

---

## 7. Important nuance: `init()` today is mostly one-shot

Today `World::init_component_tree(...)` is idempotent: it calls `Component::init(...)` only for nodes whose `initialized` flag is false.

That is good for normal scene bring-up, but it means a splice helper cannot simply rely on:
- “call `init_component_tree(inserted)` again”

if the inserted node was already initialized earlier.

So a proper splice design likely needs one of these explicit semantic choices.

### Option A: require inserted node to be unattached and uninitialized

The helper only accepts a freshly created node.

Pros:
- simplest,
- existing `init()` semantics remain enough.

Cons:
- less general,
- does not support reusing an already-live component.

### Option B: add a dedicated reattach/reparent lifecycle hook

For example, conceptually:

```rust
on_adopted_children(...)
on_topology_changed(...)
on_spliced_into_tree(...)
```

Pros:
- best semantic fit.

Cons:
- introduces lifecycle surface area.

### Option C: explicit re-init of the inserted node only

A special helper intentionally re-runs the inserted node’s setup logic after the splice.

Pros:
- matches the immediate need.

Cons:
- `init()` is no longer strictly one-shot for that path unless we define a separate method.

### Recommended direction

For correctness and clarity, the best long-term answer is probably:
- keep `init()` one-shot,
- add a distinct topology/lifecycle hook for “this component has been spliced into a live subtree and now has children”.

But for the immediate API design note, the key requirement is simply:
- the inserted component must get a chance to react to its new children/subtree.

---

## 8. Signal semantics

A splice changes topology in a way observers may care about.

At minimum, observers should be able to understand the effective structural changes:
- `child` changed parent from `parent` to `inserted`,
- `inserted` changed parent from `None` or previous parent to `parent`.

So a `Universe`-level splice helper should likely emit the same topology signals/events we would expect from equivalent safe attach operations.

The exact emission shape could be either:

### Option A: emit the two underlying `ParentChanged` facts
- one for `inserted`
- one for `child`

### Option B: emit those plus a higher-level splice fact

Conceptually:

```rust
TopologySpliced {
    parent,
    inserted,
    child,
}
```

For now, Option A is probably enough.

The key point is:
- splicing should not be a silent raw-graph mutation if we expect editor/tools/systems to observe topology changes.

---

## 9. Proposed public contract

A future convenience method should probably promise something like:

```rust
/// Insert `inserted` between `parent` and its current direct child `child`.
///
/// Before:
///   parent -> child
/// After:
///   parent -> inserted -> child
///
/// Guarantees:
/// - preserves `child` and descendant ComponentIds
/// - preserves the subtree rooted at `child`
/// - performs topology validation
/// - emits normal topology change signals/events
/// - gives `inserted` a lifecycle chance to react to its new child subtree
fn splice_between(parent: ComponentId, child: ComponentId, inserted: ComponentId)
```

That is the user-facing behavior we want, regardless of exact implementation details.

---

## 10. Expected implementation shape later

No code now, but the likely implementation later is:

1. Validate `child.parent == Some(parent)`.
2. Detach `child` from `parent`.
3. Attach `inserted` under `parent`.
4. Attach `child` under `inserted`.
5. Emit topology/signal notifications through the normal path.
6. Run inserted-node lifecycle reaction in the final topology shape.

The key detail is step 6:
- the inserted node should observe its **final** adopted child subtree, not an intermediate empty state.

So if a lifecycle callback exists, it should happen after both edges are in place.

---

## 11. Why this should be a convenience helper, not userland boilerplate

User code could always perform:
- `detach`
- `attach`
- `attach`
- maybe init
- maybe custom signal emission

But that pushes subtle invariants outward:
- ordering of operations,
- whether child IDs are preserved,
- whether events are emitted consistently,
- whether inserted component gets the right lifecycle callback,
- how partially-failed topology rewrites are handled.

This is exactly the kind of operation that should be centralized.

---

## 12. Relation to the current ControllerXR idea

This helper would make the previously discussed experimental topology possible:

```text
LowerArmTransform
  ControllerXR
    WristTransform
      ... hand subtree ...
```

via a splice of `ControllerXR` between `LowerArmTransform` and `WristTransform`.

That does **not** settle whether this is the final best semantic design for avatar hands.

But it would make that experiment practical while preserving:
- wrist/hand/finger IDs,
- the imported subtree,
- skinning expectations.

So this helper is useful regardless of whether the final avatar-hand API stays splice-based or evolves toward semantic armature targeting.

---

## 13. Open questions

1. Should the public API live only on `Universe`, or also on `World`?
2. Should the helper require `inserted` to be unattached and uninitialized, or support already-live nodes?
3. Should the lifecycle callback be:
   - a re-run of `init()`,
   - a new dedicated topology hook,
   - or a targeted explicit re-init helper?
4. Should splice emit only ordinary `ParentChanged` facts, or also a dedicated higher-level splice event?
5. Should splice be atomic from the signal/observer point of view, or is it acceptable for observers to see the two-step parent changes?
6. Should there also be a batch form for inserting the same helper pattern across multiple matched edges?

---

## 14. Recommended current stance

The immediate doc-level recommendation is:
- add a **convenience splice helper** for inserting one component between an existing parent and child,
- prefer exposing it on `Universe`,
- preserve the existing child subtree and IDs,
- ensure the inserted component gets a lifecycle opportunity to react to its newly adopted children,
- keep raw graph rewrites out of user/example code.

That gives us a clean way to experiment with topology edits like inserting `ControllerXRComponent` into an imported armature edge without forcing subtree rebuilds or ad hoc lifecycle handling.

---

## 15. Tree splice: the output-node problem (2026-03-23)

The sections above all treat the inserted value as a **single component**.  In practice,
the insertions we actually perform (body pipeline, hand smoothing pipeline, head splice)
are whole **trees**, not single components.  This surfaces a design problem that
`splice_between(parent, child, inserted_root)` cannot express cleanly.

### The core problem

When a tree is spliced in, the old child must be re-parented under a **specific node
within that tree** — not necessarily the root, and not necessarily a leaf.  The caller
must be able to nominate that output node.

Examples from the current codebase:

| Splice site | Inserted tree root | Output node (where old child lands) |
|---|---|---|
| Head bone | `splice_head` (plain TC) | same — root IS the output |
| Hand bone | `ControllerXRComponent` | `driven_t` → `TransformPipelineOutput` → `smoothed_t` |
| Body rotation | `TransformPipelineComponent` | `TransformPipelineOutputComponent` |

The head splice is degenerate (single node, root = output).  The others have a non-trivial
depth between root and output.  A generic tree splice therefore needs four endpoints:

```
splice_tree(parent, child, inserted_root, inserted_output)
```

where `inserted_output` is some descendant of `inserted_root` under which `child` is
re-parented.

### Options for nominating the output node

**A. Explicit 4th argument** — caller holds both IDs and passes both.  Most explicit,
no new component conventions required.  Works naturally with the programmatic assembly
already done in `AvatarControlSystem::try_init_splices`.

**B. Marker component** — a `SpliceOutputComponent` (or reuse
`TransformPipelineOutputComponent`) signals "this is where the old child goes".
`splice_tree` walks the inserted tree to find it.  Declarative, but couples the splice
API to a specific component convention; ambiguous if multiple outputs exist.

**C. Closure** — `splice_tree(parent, child, fn(root) -> output)` — caller resolves the
output node after the tree is constructed.  Flexible but harder to express declaratively
in MMS.

Option A is the most straightforward given current usage — the caller always has both IDs
in scope by the time they are splicing.

### The sideways-graft case is not a splice

The body pipeline attachment in `AvatarControlSystem` is often described as a splice,
but it does not fit the `parent → new_tree → child` shape:

- the pipeline root is attached as a **new child of AVC** (a sibling of `model_root`),
- then `model_root` is re-parented under the pipeline output.

The old parent (`AVC`) does not change; only `model_root`'s parent changes.  This is a
**sideways graft** — inserting a processing branch beside an existing node and then
redirecting the node into it — rather than an inline splice between two existing edges.
A `splice_tree` API should not try to express this case; it belongs to a separate
`graft` concept or remains handled by explicit `emit_attach` sequences.

### Current state

No `splice_tree` (or `splice_between`) helper exists yet.  All splice operations in
`AvatarControlSystem` and the examples are performed manually via sequences of
`emit_attach` / `emit.push_intent_now`.  The four-argument form above is the natural
next step once a helper is warranted.
