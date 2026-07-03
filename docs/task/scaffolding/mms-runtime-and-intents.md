# MMS Runtime, ComponentCodec, and Intents

This document traces the question: what should the MMS evaluator produce, how
does that relate to the existing intent system, and what happens to
`ComponentCodec`?

---

## The problem with `BuildCommand` as a separate type

In `mms-phase-1.md` §4, the stop-gap evaluator is described as producing a
`Vec<BuildCommand>` — a new type with variants like `CreateComponent`,
`SetProperty`, `CallMethod`, `Attach`. The main thread has a separate executor
for those.

The engine already has a pipeline for requesting side effects on the main
thread: `IntentValue` → `CommandQueue` → `RxIntentExecutor`. Introducing
`BuildCommand` as a second parallel system means two ways to mutate the world,
two executors to maintain, and no clear principle for which one to use when.

The right end state is: **MMS evaluation produces intents, and the single
intent pipeline is the only way to drive world mutations.**

---

## Why intents don't cover this today

Every existing `IntentValue` variant takes `Vec<ComponentId>`. They operate on
components that *already exist* in the world:

```
IntentValue::Attach { parents: Vec<ComponentId>, child: ComponentId }
IntentValue::RegisterTransform { component_ids: Vec<ComponentId> }
IntentValue::SetColor { component_ids: Vec<ComponentId>, rgba: [f32; 4] }
```

Component *creation* — `world.add_component_boxed_named(...)` — is not an
intent today. It happens as direct world mutation: in `Universe::add()`, in
`ComponentCodec::decode_subtree`, and in Rust example code. Component IDs are
only known *after* creation, so they can't be in the intent payload before
that.

This is the gap. MMS evaluation needs to describe "create a component of type X
with these properties, then attach it to Y" — but no `IntentValue` variant
exists for that because creation has never been driven through the intent
pipeline.

---

## The right fix: extend IntentValue with spawning

Add a new intent variant that carries a self-contained spawn description:

```rust
IntentValue::SpawnComponentTree {
    /// The root ComponentExpression to instantiate.
    /// Already parsed; evaluation (shortform expansion, value binding) happens
    /// on the main thread when the intent is executed.
    root: Box<ComponentExpression>,

    /// If Some, attach the spawned root as a child of this component.
    parent: Option<ComponentId>,
}
```

The name is `SpawnComponentTree`, not `SpawnComponentTree`. The intent executor
doesn't know or care whether the `ComponentExpression` came from parsing a
`.mms` file, from the REPL, or was constructed programmatically in Rust. It
is a general tree-spawning intent. Any thread can emit it; the executor runs
on the main thread at the next drain point.

The intent executor handles `SpawnComponentTree` by:
1. Walking the `ComponentExpression` tree depth-first.
2. For each node: call the component factory (by type name), apply named
   assignments and builder calls, insert into the world.
3. Wire parent/child topology.
4. Each newly-created component's `init()` fires, which emits its own
   `Register*` intents — those drain normally through the existing pipeline.

The evaluator thread does parsing and expression evaluation (resolving `let`
bindings, evaluating literals and arrays into `Value`s), and the result is a
`SpawnComponentTree` intent. Heavy computation (parsing) stays off the main
thread; topology mutation stays on it.

### Why carry ComponentExpression and not a pre-evaluated command list?

Because `ComponentExpression` *is* the right intermediate form — it's the
AST, and ASTs are the universal currency between parsing and execution. A
separate `BuildCommand` list is just a manually-serialized version of the same
tree. Carrying the AST in the intent and evaluating it on the main thread
keeps the evaluator thread's output minimal and well-typed.

---

## What this means for ComponentCodec

`ComponentCodec` currently does three things:

1. **Type registry** — maps type name strings to component constructors
   (`create_component`). This is needed both for decode and for
   `SpawnComponentTree` execution.
2. **Encode path** — walks the world tree, calls `component.encode()` on each
   node, produces `ComponentDataNode` → JSON.
3. **Decode path** — reads JSON → `ComponentDataNode` → direct world mutation.

Once MMS is the scene format:

- The **encode path** is replaced by `encode_mms()` per component +
  `MmsPrinter` tree walker (see `mms-phase-1.md` §5).
- The **decode path** is replaced by the MMS parser + `SpawnComponentTree` intent.
- The **type registry** is still needed — but it belongs in the intent
  executor (or a shared helper it calls), not in `ComponentCodec`.

`ComponentCodec` then has no remaining purpose and can be deleted. The JSON
format becomes a legacy artifact, supportable with a compatibility shim if
needed but no longer the primary format.

---

## What ComponentRegistry was and why it's not the right framing

The phase-1 plan introduced `ComponentRegistry` as a refactor of
`ComponentCodec::create_component` — a struct holding a `HashMap<type_name,
factory_fn>` shared between the codec and the evaluator. This was a reasonable
incremental step, but "ComponentRegistry as a standalone struct" is really just
a named container for something that should live *inside* the intent executor:
the ability to instantiate a component given a type name.

Once `ComponentCodec` goes away and spawning is driven by `SpawnComponentTree`, the
"registry" concept dissolves into:

- The `SpawnComponentTree` intent executor branch, which needs to call a factory by
  type name.
- That factory is a private helper on the intent executor (or a free function
  in the component module), not a publicly-named `ComponentRegistry` struct.

The important thing is not the name or the struct — it's that there is **one
canonical place** where `"transform"` → `Box<TransformComponent::new()>` is
defined, and that place is adjacent to where the intent executor handles
`SpawnComponentTree`.

---

## Queries

Queries (read operations — parent, children, component data) are a different
problem from mutations and are covered in `mms-world-topology-api.md`. Short
version: same-thread queries stay as direct synchronous `&World` access;
off-thread queries (future VM path) use oneshot-channel request/response
intents.

---

## The long-term picture

```
.mms file
    │
    ▼
MMS parser (worker thread)
    │  produces ComponentExpression tree
    ▼
MMS evaluator (worker thread)
    │  resolves let bindings, evaluates literals/arrays into Value
    │  shortform expansion
    ▼
IntentValue::SpawnComponentTree { root: ComponentExpression, parent: Option<ComponentId> }
    │
    ▼
CommandQueue  →  intent pipeline (main thread)
    │
    ▼
SpawnComponentTree executor
    │  walks ComponentExpression, calls factory by type name,
    │  applies named assignments + calls, wires topology
    ▼
World (new components instantiated)
    │
    ▼
Component::init() on each new component
    │  emits Register* intents
    ▼
Existing intent pipeline handles registration normally
```

Encoding (world → .mms) is the reverse:

```
World
    │
    ▼
ComponentCodec tree walker (temporary; eventually just Universe)
    │  calls component.encode_mms() on each node
    │  inserts child ComponentExpressions
    ▼
ComponentExpression tree
    │
    ▼
MmsPrinter  →  .mms source text
```

---

## Phase 1 implications

For phase 1, `BuildCommand` is still the simplest thing to build as a
stepping stone — it lets us test the evaluator in isolation before touching the
intent pipeline. But the design should be kept consciously temporary:

- `BuildCommand` should not acquire new callers or become load-bearing.
- The step-6 work in the checklist (`ComponentRegistry`) should be reframed:
  instead of creating a `ComponentRegistry` struct, just move
  `create_component` to wherever the `SpawnComponentTree` executor will eventually
  live, and note it as the future executor site.
- Before phase 1 is considered complete, add `IntentValue::SpawnComponentTree`
  and wire the evaluator to produce it instead of `Vec<BuildCommand>`. This is
  the right end state for the stop-gap evaluator, not an optimization for later.

The checklist (`mms-phase-1-checklist.md`) should be updated to reflect this:
step 6 changes from "extract ComponentRegistry" to "add SpawnComponentTree
intent + wire evaluator to produce it", and step 8 (encode_mms pilot) remains
unchanged.
