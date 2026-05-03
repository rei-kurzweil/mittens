# Component emit lifecycle & multi-emit cloning

Date: 2026-04-29

Status: investigation. Concludes with a v1 rule and a v2 direction. No code changes
yet.

Companion to:
- [emission-and-component-value-model.md](emission-and-component-value-model.md) —
  *where* a CE emits.
- [emission-policy-options.md](emission-policy-options.md) — Option A/B emission rule.
- [object-world.md](object-world.md) — env/heap structure.
- [../spec/host-call-api.md](../spec/host-call-api.md) — Register/Attach protocol.

---

## The question

After the Register/Attach split landed and the `pending` bookkeeping list was dropped,
ObjectWorld is just `env + heap`. That simplification is fine for the case the example
exercises (`let x = Text {}` → `T { x }` → `x.set_text(...)`). But it leaves an open
question:

```mms
let badge = Text { "hi" }

T.position(0,0,0) { badge }
T.position(1,0,0) { badge }   # what happens here?
```

`badge` is a single `Value::ComponentObject { id, .. }`. Today the body splice path
calls `HostCallKind::Attach { parent, child: id }` on the *same* engine `ComponentId`
twice. The engine's `add_child` will either reparent the existing subtree (moving it
out from under the first `T`) or fail — neither matches what a script author expects.

A reader's intuition for `let badge = ...; T { badge }; T { badge }` is "two copies of
the badge, one under each parent." We don't have implicit clone semantics yet.

---

## Current behaviour (v1, after pending removal)

Lifecycle of a `let`-bound CE:

1. `let badge = Text { "hi" }` — evaluator issues `HostCallKind::Register(ce)`. Host
   calls `spawn_tree_uninitialized` and replies with `ComponentId`. Evaluator stores
   `Value::ComponentObject { id, component_type: "Text" }` in `env`. Subtree exists in
   the engine `World` with **no parent** and **uninitialized**.
2. First reference `T { badge }` — CE-body sees `Value::ComponentObject(id)` in the
   body and emits `CeChild::Attach(id)` in the materialized parent. When the parent
   is spawned, `spawn_tree`'s child loop calls `world.add_child(parent_id, id)` for
   that child. The top-level emit of `T { badge }` then triggers
   `HostCallKind::Attach { parent: None, child: parent_id }`, which runs
   `init_component_tree` over the whole subtree (including `badge`).
3. Second reference `T { badge }` — same path. `world.add_child(other_parent, id)`
   tries to re-parent the *already attached* subtree. Result is wrong (silent move,
   or error) regardless of which.

There is also the **statement-position bare reference**:

```mms
let badge = Text { "hi" }
badge       # statement
```

…which emits the `ComponentObject` as a root. Today this calls
`HostCallKind::Attach { parent: None, child: id }`. If `badge` was already attached
under another parent in a prior statement, this path is also broken for the same
reason.

### Identity considered

`Value::ComponentObject { id, .. }` is *the* engine handle. Method dispatch
(`badge.set_text("...")`) routes by `id`. If we silently clone on the second
emission, method calls on `badge` only affect *one* of the spawned copies (the
original). Authors expecting "set_text updates both badges" would be surprised.
Conversely if they expect "badge is a prefab; each emit makes its own", they want
clones.

Both expectations are reasonable. The language has to pick.

---

## Design space for multi-emit

### Option 1: keep one-shot semantics, error on re-emit (v1 rule)

Track on the engine side (or via a small evaluator-side set keyed by `ComponentId`)
whether a registered subtree has already been attached. Second attach → evaluator
error: *"component `badge` already attached; bind a new CE if you want a copy"*.

- Pros: predictable, no surprise about identity. Method dispatch stays sane.
- Cons: doesn't match author intuition for templates. Forces verbose
  `let mk_badge = fn() { Text { "hi" } }` workaround.
- Cost: trivial — a `HashSet<ComponentId>` of "claimed" ids. Or piggy-back on
  `World::parent_of(id).is_some()` since an attached subtree always has a parent.

### Option 2: implicit clone on every reference (prefab semantics)

Treat a `let x = CE` as binding a *prefab*. Every emission deep-copies the registered
subtree to a fresh subtree and attaches that.

The engine already supports this: `IntentValue::AttachClone { parents, prefab_root }`
encodes the subtree by id (via `ComponentCodec`) and decodes a fresh copy under each
parent.

Open issue: what does `badge.set_text("Paused")` do once there are N copies?
- 2a: keep methods bound to the *prefab* `id`. Method calls mutate the prefab; copies
  are unaffected (they were decoded snapshots). Surprising — author wrote
  `badge.set_text` after spawning two badges and saw nothing change.
- 2b: methods broadcast to all copies. Requires the evaluator (or engine) to remember
  the spawned-copy fanout. New bookkeeping; reintroduces something pending-shaped.
- 2c: `ComponentObject` becomes a *prefab handle* and the only way to talk to the
  spawned copy is via the value returned from the spawn (something like
  `let inst = T { badge }` binds the parent's handle, and indexing into its children
  is the only path to the copy). Cleaner conceptually, more verbose.

### Option 3: first reference is a move, subsequent references are clones

`T { badge }; T { badge }` — first attaches the registered subtree as-is, second
deep-clones via `AttachClone`. Method dispatch keeps targeting the original (the
first emission "consumes" the prefab's identity).

- Pros: matches the common pattern of `let x = Text {}; T { x }` (no clone) plus the
  occasional template use.
- Cons: ordering-dependent. Reordering statements changes whether something is the
  original or a copy. Hard to reason about.

### Option 4: explicit clone keyword

`T { badge }; T { clone(badge) }` — author-driven. v1 stays one-shot; `clone(...)`
desugars to `IntentValue::AttachClone`.

- Pros: zero ambiguity. Identity stays simple. Engine support already exists.
- Cons: more keystrokes; doesn't match `let mk = fn() { ... }` expectation that "use
  the binding multiple times = multiple instances."

---

## Recommendation

**v1**: Option 1 — one-shot. Evaluator detects re-attach via
`world.parent_of(id).is_some()` (or a "claimed" set on ObjectWorld) at the
`HostCallKind::Attach` site and surfaces an error. Cheap, prevents the silent-bug
case the dropped `pending` list used to gesture at, and keeps method dispatch
trivially correct.

**v1.5 (optional polish)**: Option 4 — add `clone(x)` builtin that lowers to
`AttachClone`. Pure ergonomic addition; doesn't change the rule.

**v2**: revisit Option 2 once we have a real use case. The right call between 2a/2b/2c
needs a concrete script driving it (e.g. an inventory grid where each slot is a
template). Premature without that.

---

## What changes if we adopt v1 Option 1

- ObjectWorld stays `env + heap`. No bookkeeping resurrection.
- The check lives at the Attach host-call site, which already has a live `&mut World`.
  The evaluator does not need to track attachment state itself.
- Error message points at the `let` name when possible (the evaluator knows the
  binding name at the point of emission via the source AST node, even if
  `Value::ComponentObject` itself doesn't carry it).

## What changes if we adopt v2 (later)

- `MaterializedCE` and `CeChild::Attach` need a discriminator: "splice this exact
  subtree" vs "clone-decode this prefab." Likely a new `CeChild::Clone(ComponentId)`
  alongside `Attach`.
- Method-dispatch story (2a/2b/2c) has to be picked.
- Likely interacts with module export semantics — modules naturally produce prefabs
  used in many places.

---

## Out of scope

- Function-call-as-template (`fn mk_badge() { Text { "hi" } }`) — already gives fresh
  CE per call; not affected by this analysis.
- Component scopes / `ComponentObject.scope` — separate v3 concern.
- `ComponentHandle` (id + guid + type) — orthogonal refactor.
