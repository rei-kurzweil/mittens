# Action-target scoping + factory-function handler registration ʕ•ᴥ•ʔ

## Why

Two related papercuts block authoring **reusable, importable MMS components** like `assets/components/button.mms`. Both surface as the same symptom: "I imported a button factory and called it twice, and now neither instance behaves right."

### Papercut 1 — global selectors in `Action.*` bake a shared `ComponentId`

`Action.update_transform("#button_face", …)` is the only way today to author
keyframe-driven mutations against a named target. The selector is resolved
**at MMS-eval time** by `resolve_action_target` (`src/meow_meow/component_registry.rs:948`),
which walks `world.all_components()` and grabs the first node whose label
matches. The resolved `ComponentId` gets baked into the `ActionComponent`'s
`IntentValue::UpdateTransform { component_ids: vec![target], … }`.

Consequence: if you spawn the same component tree twice (two buttons),
**both Animations drive the first instance**. The action lookup is global,
and the second instance's `#button_face` never wins because the resolver
returns the first match it finds.

Today's resolver supports three selector forms (`#name`, `[name='…']`, bare
label). All three are world-scoped. There's a shared query engine landing
(`src/query/`) and a planned MMS query runtime (`docs/meow_meow/draft/mms-query.md`),
but `resolve_action_target` is a parallel implementation that the query-usage
audit already flagged for migration (`docs/analysis/query-usage.md:144`).

### Papercut 2 — factory function bodies are sandboxed; `on()` no-ops

`src/meow_meow/evaluator.rs:891–899` — when MMS calls a `Value::Function`, it
builds a fresh `EvalContext` for the body and hard-codes both `channels: None`
and `host_world: None`:

```rust
let mut func_ctx = EvalContext {
    emits: ctx.emits,
    source_path: None,
    channels: None,     // ← drops the live channel
    ce_builder: None,
    object_world: ctx.object_world,
    host_world: None,   // ← drops the host world too
};
```

So even when a factory like `fn button(label) { … }` is called from a script
that *does* have channels (i.e. is being executed by `MeowMeowRunner` against
a real `Universe`), the body runs sandboxed. `on(face, "Click", …)` inside
the factory body never reaches `RegisterHandler` and silently no-ops.

This forces authors to write `on()` calls in the **caller** of the factory,
not the factory itself — which defeats the whole point of packaging
"button behavior + visuals" into one file.

### Joint impact

A reusable button has *two* kinds of behavior that should live with the
visuals: (a) internal press animation (DragStart play / DragEnd pause), and
(b) external click signaling. Today, (a) is broken by Papercut 1 (selector
clash across instances) and (b) is broken by Papercut 2 (factory body can't
register the handler). So `button.mms` ends up with TODO comments instead of
working code, and its consumers have to inline both the visual tree and the
handlers at the call site.

## What changes

### 1. Replace global `Action` selectors with subtree-scoped queries

`Action.update_transform("#button_face", …)` (and any future `Action.*` that
takes a selector) should:

1. Stop resolving at MMS-eval time. Store the **selector string** on the
   `ActionComponent`, not a pre-resolved `ComponentId`.
2. Resolve **lazily**, at action-fire time, scoped to a `root_scope`
   subtree root. Default `root_scope` = the **parent of the enclosing
   Animation** (i.e. the instance root that owns this animation).
3. Use the shared `src/query/` selector engine (or `WorldQueryAdapter` /
   `find_first_named_in_subtree` as an interim until the MMQ runtime lands).

**Storage change** in `ActionComponent`: for selector-based intent
variants, store

```rust
struct ActionTarget {
    selector: String,
    /// How to find the subtree root the selector resolves against.
    /// `None` ⇒ default behavior: walk up to enclosing Animation, then
    /// take that Animation's parent.
    root_scope: Option<ActionScope>,
}

enum ActionScope {
    /// Default. The parent of the nearest ancestor Animation.
    EnclosingAnimationParent,
    /// Resolve a named ancestor by walking up from the ActionComponent
    /// until a node with this label is found, then scope to it.
    NamedAncestor(String),
    /// Resolve from a specific component handle captured at authoring
    /// time. Useful for MMS like `Action.update_transform("#x", …, scope: my_root)`
    /// where `my_root` is a `ComponentObject` from the caller's scope.
    Explicit(ComponentId),
    /// Whole world. Matches today's global behavior — keep as an
    /// opt-in escape hatch, not a default.
    World,
}
```

**MMS authoring surface**:

```mms
// Default — scope = parent of enclosing Animation:
Action.update_transform("#button_face", [0,0,-0.02], [0,0,0], [1,1,1])

// Explicit scope override — keyword-ish trailing arg:
Action.update_transform("#button_face", t, r, s, scope: panel_root)
Action.update_transform("#button_face", t, r, s, scope: "named_ancestor_label")
Action.update_transform("#button_face", t, r, s, scope: "world")
```

The MMS surface is **one optional named parameter `scope:`** accepting one of:
- a `ComponentObject` handle from the surrounding script scope → `Explicit(id)`
- a string ancestor label → `NamedAncestor(label)`
- the literal `"world"` → `World`
- omitted → `None` (default `EnclosingAnimationParent`)

Reasoning for explicit override:
- *factory composition*: a factory might be nested inside a larger
  component where the natural scope is not its immediate Animation
  parent but some grandparent (e.g. a panel that contains many
  animation roots sharing one named child).
- *cross-instance coordination*: an action that should drive a sibling
  instance, not itself, can `scope: shared_root` to target it.
- *escape hatch*: legacy MMS that genuinely wants today's global
  behavior can write `scope: "world"` instead of being silently broken.

**Resolution change** in `AnimationSystem` (or wherever `ActionComponent`
fires): when promoting the stored intent into an emitted `IntentValue`,

1. Resolve the `root_scope`:
   - `EnclosingAnimationParent` → walk up from the `ActionComponent`
     to its enclosing `Animation`, then take that Animation's parent.
   - `NamedAncestor(label)` → walk up from the `ActionComponent` until
     a node with `component_label() == label` is hit.
   - `Explicit(id)` → use `id` directly.
   - `World` → use the world (no scope).
2. Run the stored selector against that subtree using the shared query
   adapter.
3. Emit the intent against the resolved `ComponentId`.

If resolution fails (selector matches nothing under the chosen scope),
log once per `ActionComponent` and skip the emission — don't poison the
animation by falling back to a global lookup.

**Migration**: cut over — `Action.update_transform` is narrow enough
today that the audit lists only `button.mms` as the consumer-in-anger.
Existing call sites without `scope:` get the new default; existing
call sites that depended on global resolution add `scope: "world"`
explicitly. Document the change in the v0 → v1 notes alongside the
deprecation of `resolve_action_target`.

### 2. Forward `channels` + `host_world` through function calls

`src/meow_meow/evaluator.rs:891–899` — change to:

```rust
let mut func_ctx = EvalContext {
    emits: ctx.emits,
    source_path: ctx.source_path,
    channels: ctx.channels.as_deref_mut(),   // forward
    ce_builder: None,
    object_world: ctx.object_world,
    host_world: ctx.host_world,              // forward
};
```

(Exact borrow plumbing TBD — `channels` / `host_world` are mutable
references on `EvalContext`; the function-frame ctx needs to take a
reborrow for the duration of the body. The shape change is mechanical
once that's worked out.)

After this fix:
- Factories can do `on(face, "Click", …)` inside the body, and the
  handler is installed via the caller's live channel.
- The handler's `scope` is whatever `ComponentId` `face` resolves to at
  registration time (the per-call instance), so handlers are per-instance
  by construction.

**Semantic risk**: handler registrations leak past the function call
return. That's the desired behavior here (the handler outlives the
factory call). But it means `on()` inside a `fn` mutates engine state —
authors need to know that. Document it in the factory-pattern section
of `mms-query.md` or wherever module/import semantics live.

### 3. Update `button.mms` once 1+2 land

- Wrap the tree in `export fn button(label) { T { … } }`.
- Keep the internal `Animation` and the `on(face, "DragStart"/"DragEnd",
  …)` press handlers — they'll work because (1) selector scoping makes
  `#button_face` resolve to the per-instance face, and (2) `on()` inside
  the factory body registers per-instance handlers.
- Drop the stale `STILL BLOCKED` TODOs.

### 4. Update related docs

- `docs/meow_meow/draft/mms-query.md` — replace the "specialized
  authoring-time target resolver" subsection with the new lazy /
  scope-aware resolution, and reference the shared query adapter as the
  resolution backend.
- `docs/analysis/query-usage.md` — bump `resolve_action_target` from
  "high priority migration target" to "done" once (1) lands.
- `docs/spec/animation-keyframe-interpolation.md` — add a "Target scoping"
  subsection: actions resolve relative to their enclosing Animation's
  parent, not the world.

## Critical files

- `src/engine/ecs/component/action.rs` — store selector + scope hint on
  selector-based actions instead of pre-resolved ids.
- `src/engine/ecs/system/animation_system.rs` — resolve selector at
  fire-time using the enclosing Animation's parent as scope root.
- `src/meow_meow/component_registry.rs:498-516` — stop calling
  `resolve_action_target` at eval time; pass the selector string
  through to the action.
- `src/meow_meow/component_registry.rs:948-972` — `resolve_action_target`
  goes away (or becomes a thin wrapper around the shared query adapter
  for any non-action callers).
- `src/meow_meow/evaluator.rs:878-913` — forward `channels` and
  `host_world` through `Value::Function` calls.
- `src/query/` + `src/engine/ecs/world_query_adapter.rs` — used as the
  resolution backend.
- `assets/components/button.mms` — rewrite as `export fn button(label)`.
- `docs/meow_meow/draft/mms-query.md` — update the action-target section.
- `docs/analysis/query-usage.md` — update the migration list.
- `docs/spec/animation-keyframe-interpolation.md` — add target-scoping
  subsection.

## Verification

1. `cargo test` — existing animation / action / signal-handler tests pass.
2. Add a test: spawn the same factory-generated component tree twice;
   trigger an `Action.update_transform("#sub", …)` in each instance's
   timeline; assert each instance's `#sub` moved independently.
3. Add a test: factory body calls `on(child, "Click", fn(e) { … })`;
   the handler is invoked when the per-instance child receives a Click.
4. `cargo run --release --example padding-demo` after updating
   `button.mms` to a factory — two buttons render, each with working
   press animation, each emitting Click to its own handler.

## What this does NOT do

- Doesn't introduce the full MMS query runtime (`query()` / `query_all()`
  host calls). It uses the underlying shared query adapter directly from
  rust; the MMS-side `query()` work continues separately per
  `docs/meow_meow/draft/mms-query.md`.
- Doesn't change `RouterSystem` / `Universe::find_component` /
  splice-target lookup. Those are listed as migration targets in
  `docs/analysis/query-usage.md` and stay scheduled separately.
- Doesn't add new `Action.*` variants. Only changes how the existing
  `Action.update_transform` selector resolves.
- Doesn't address `anim.reverse()` — separate task.
