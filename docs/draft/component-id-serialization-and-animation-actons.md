# ComponentId Serialization and Animation Actions

Draft / design discussion. Not yet a plan, not yet code.

## Context

We're finishing the JSON → `to_mms_ast` migration on `mittens`. Every
component except the `Audio*` family (deferred per
`docs/task/audio-clip-terminology-and-effect-consolidation.md`) and
`ActionComponent` has been migrated. Action is the remaining holdout
because it stores a full `IntentValue` enum and most variants carry
`Vec<ComponentId>` references to other components — and we don't have a
clean story yet for how a ComponentId reference lives in authored MMS
source, in the heap, and across save/load.

The original JSON codec for Action serialized raw slotmap u64s for the
ComponentId vecs. That's not portable across runs (indices change) and
the corresponding MMS-side registration path doesn't exist for most
variants anyway. So we have two gaps to close at once: the **vocabulary
gap** (Action's apply_call only knows `print` and `update_transform`)
and the **reference gap** (how do you author / serialize a target
ComponentId).

## The fundamental tension

A ComponentId reference has two desirable properties that pull in
opposite directions:

- **Speed at composition / runtime.** Animation-heavy scenes (VN, vtuber
  stream overlays, particle-ish UI) might fire hundreds or thousands of
  `Action.*(target, ...)` per tick. The target lookup needs to be
  pointer-cheap — a slotmap deref, not a CSS-selector walk.
- **Portability across save / load.** Slotmap keys are dense arena
  indices. They survive within a single process if the slotmap state is
  preserved, but they do **not** survive a "save scene as text, edit it
  by hand, re-evaluate from scratch" round-trip. For that you need a
  stable handle: a string name resolved via the query system.

Today's mechanism handles each side in isolation: `let x = T {}` gives
you a fast `Value::ComponentObject { id, .. }` in
`ObjectWorld::heap`; `resolve_action_target("#name")` gives you the
portable form. They don't compose, and neither has a full
serialization story.

## Two reference shapes — what they cost

### Selector (`"#hero"`, `[name=hero]`, bare `"hero"`)

- **Resolution cost:** one walk through the world's component slotmap
  (`all_components().find(...)`) or, for `[name=...]`, a query-system
  parse + cached eval.
- **Pays the cost when:** at *spawn time* (registry runs
  `resolve_action_target` once when constructing the Action), and again
  every time we re-spawn (clone, load).
- **Does not pay** at tick time. Once stored in `IntentValue::SetColor {
  component_ids: vec![<resolved id>], .. }`, subsequent ticks use the
  slotmap key directly.
- **Round-trips through MMS source:** yes. The selector string is just a
  string.

### Live id (`Value::ComponentObject { id, component_type }`)

- **Resolution cost:** zero. The slotmap key was already produced by the
  evaluator when `let x = T {}` was bound.
- **Pays no cost** at spawn or at tick.
- **Composes cheaply in MMS source:** `let hero = T {}` once, then
  `Action.set_color(hero, …)` a thousand times in a `for` loop pays
  zero lookup cost per call.
- **Does not round-trip through MMS source today:** there is no AST
  literal form for a ComponentId. The unparser cannot emit one.

The earlier proposal — "selectors are the canonical wire form, dump
always rewrites live ids back to `#name`" — sounded clean but
**bottlenecks composition.** Even if per-tick cost stays zero (because
the resolved id is cached in the IntentValue), every spawn of an
animation-heavy subtree replays selector resolution for every Action
inside it. A 1000-action loop pays 1000 selector walks at spawn time.
Pre-resolution helps but the design feel is wrong — we're forcing the
fast composition path through the slow portable form.

## We already have stable cross-process ids: GUIDs

Every `ComponentNode` gets `guid: uuid::Uuid` (`uuid::v4()`) at
construction in `src/engine/ecs/component/mod.rs:210, 226`. The
`World` keeps a `guid_index: HashMap<Uuid, ComponentId>` for O(1)
reverse lookup (`src/engine/ecs/mod.rs:64`) and exposes
`component_id_by_guid` (line 74).

The query system **already has a `SimpleSelector::Guid(String)`
variant** in its AST (`src/query/ast.rs:33`) and a
`tree.matches_guid` evaluator hook (`src/query/evaluator.rs:18, 81`).
Two pieces are missing:

- Neither selector parser (`src/query/css/parser.rs`,
  `src/query/mmq/parser.rs`) emits `Guid` — there's no text syntax
  for it yet.
- `WorldQueryAdapter::matches_guid`
  (`src/engine/ecs/world_query_adapter.rs`) is using the trait's
  default-false impl; it doesn't hit `guid_index`.

These are both small wires. Adding a `[guid=...]` (or `@uuid:...`)
arm to the mmq parser and one method override on the adapter unlocks
GUID-based selectors end to end.

## How does the dump know what to write for a ComponentId?

This is the question I underspecified. Dump must emit *some* stable
label that re-resolves to the same component on load. Three options:

### A. Manual names only — loud error on unnamed save

Author writes `name = "hero"` on anything referenced by an Action.
Save errors with a clear message if a referenced target is unnamed.

Pros: clean signal, no hidden state. Cons: annoying for
prototyping; awkward for components that exist only as Action
targets, not as human-referenced things; ugly errors during normal
iteration.

### B. Auto-generated synthetic names

At save time, walk the world, find every ComponentId referenced by
an id-holding component (Action.signal vecs, IKChain.target_id,
etc.). For any referent that lacks a `node.name`, assign one
(`__auto_3`, `__act_target_3`, etc.). Generated names need a stable
derivation rule (e.g. derived from the GUID's first 8 chars) so
diffs are sane.

Pros: zero author burden. Cons: pollutes the `node.name` namespace
which is also used for human-authored selectors and the inspector
display label; ugly synthetic strings in diffs; needs collision
avoidance with human-chosen names.

### C. GUID selector fallback — **recommended**

Dump policy: prefer `node.name` if set, otherwise emit
`"[guid=<uuid>]"`. Uses the GUIDs the engine is already minting per
component; no synthetic name pollution; no save errors; round-trips
losslessly even for components the author never named.

The GUID-selector parse cost is the same as any other `[name=x]`
selector (one query parse, one cached eval), and once
`WorldQueryAdapter::matches_guid` is wired it's an O(1) hashmap hit
— actually *faster* than name-based selectors (which scan
`all_components()`).

Why not always use GUIDs? Human-authored source benefits from
readable names (`#hero` is nicer to diff than
`[guid=8c4f...e90a]`), and a name written deliberately by the
author should survive across save/reload — that's the contract the
name field exists to carry.

So Phase 1 prefers names when present, GUIDs when not. Both fall
through the same `arg_target` helper on the eval side; both are
strings on the dump side.

## SlotMap keys can serialize too

`ComponentId` is declared with `slotmap::new_key_type!` in
`src/engine/ecs/mod.rs:15-18`. Under the hood that's a
`slotmap::KeyData` — two `u32`s (a dense index and a version
counter) packed into a `u64`. Public conversions:

```rust
let raw: u64 = id.data().as_ffi();
let id: ComponentId = ComponentId::from(KeyData::from_ffi(raw));
```

That gives us a wire form for live ids that is:
- **Stable within a process / single save→load cycle** if the slotmap
  state is preserved across the boundary.
- **Cheap to encode/decode** — one `u64`, or a printable string like
  `"cid:0x00000003_00000002"` for human-readable text formats.
- **Not stable** if the scene is re-spawned from scratch, because the
  slotmap will reissue indices in whatever order the new spawn walk
  uses.

So slotmap-key serialization is useful for **snapshot-style saves**
(undo/redo, save-game state, replays) where the engine restores the
whole world including slotmap state. It's not useful for
**authored-scene saves** (human-edited `.mms` source, asset pipeline,
diffs) — and it's not the right answer for **cross-process portable
references** either, because GUIDs already fill that role and the
selector machinery already supports them.

The shape that falls out:

| Save kind | Reference form | When |
|---|---|---|
| Authored scene (`.mms` source) | `"#name"` or `"[guid=...]"` selector | Default. Human-edited, diff-friendly, cross-process stable. |
| Snapshot / quicksave / replay | Slotmap key `u64` literal | Optional fast path. Reloads zero-walk; only valid intra-process or when slotmap state is restored. |

Slotmap-key serialization stops being "the long-term answer" and
becomes "the optional fast path for snapshot reloads." Phase 2 may or
may not need it depending on whether quicksave latency actually
matters.

## Two-phase plan

### Phase 1 — ship with `mittens` 0.5 (selectors only)

Smallest design that closes the vocabulary gap and doesn't paint us
into a corner.

1. **`arg_target(world, args, i) -> ComponentId`** — one new helper in
   `component_registry.rs`. Accepts either:
   - `Value::String(s)` → `resolve_action_target(world, s)`
   - `Value::Identifier(s)` → same
   - `Value::ComponentObject { id, .. }` → returns `*id` directly

   This is the **runtime escape hatch**. Phase 1 authored source uses
   string selectors (the only form the parser/unparser know about), but
   anyone composing via `let x = T {}` in MMS code already gets the
   fast path for free because `Value::ComponentObject` flows through.

2. **`arg_target_vec(world, args, i)`** — same for
   `Vec<ComponentId>` payloads (`SetColor`, `Attach`, …).

3. **Action vocab mirror** in `apply_call` for the variants the JSON
   codec covered: `print`, `set_color`, `set_text`, `set_position`,
   `attach`, `detach`, `update_transform`, plus `noop` /
   `remove_subtree` / etc. Each uses `arg_target` / `arg_target_vec`
   for any ComponentId fields, primitive helpers for the rest.

4. **`Action::to_mms_ast` emits selectors only.** Requires
   `Component::to_mms_ast` to gain a `&World` parameter (so it can
   reverse-lookup `node.name` / `node.guid` for each ComponentId).
   One-line trait signature change; every existing impl gets an unused
   `_world` argument. Or: keep `to_mms_ast(&self)` for the no-context
   case, add `to_mms_ast_with_world(&self, &World) -> CE` for
   components that need it and have the dump path prefer it. Either
   works.

5. **Selector-fallback wiring** (the part this design wouldn't be
   complete without):
   - Add a `[guid=<uuid>]` arm to the mmq selector parser so it can
     produce `SimpleSelector::Guid`.
   - Implement `WorldQueryAdapter::matches_guid` to hit
     `world.guid_index`.
   - Dump policy in `to_mms_ast`: prefer `node.name` when set,
     otherwise emit `"[guid=<uuid>]"`. Never errors on save, never
     requires the author to pre-name anything.

6. **Same change unlocks `IKChainComponent`** (`target_id`,
   `end_effector_id`) — currently uses a `ComponentId::null()` sentinel
   because the existing registry can't accept references. After Phase
   1, IKChain authoring becomes
   `IKChain.two_bone_ik([0,1,0], false).target("#hand_target")` or the
   handle form.

7. **What we explicitly don't do in Phase 1:**
   - No AST literal for ComponentId.
   - No slotmap-key serialization.
   - No snapshot save format.
   - No removal of the existing selector-based `resolve_action_target`
     path (Phase 2 is additive).

### Phase 2 — post-mittens, only if snapshot/replay latency matters

Phase 1 already gives us authored-scene portability (via name/guid
selectors) and in-session composition speed (via the
`Value::ComponentObject` path). The one thing it doesn't give us is
**zero-walk reloads after a process restart**. That only matters if
quicksave / replay / undo-redo latency is a real product
constraint — which we'll know better once we've actually built
animation-heavy scenes.

If we do need it, Phase 2 looks roughly like:

1. **AST literal for raw slotmap key** — pick one:
   - `cid(0x00000003_00000002)` — function-call-looking, parser-cheap.
   - `#0x3:2` — terse, reuses the `#` selector punctuator.
   - `@3v2` — Discord-like handle syntax.

   The parser produces `Expression::ComponentId(u64)`. Used only by
   snapshot saves — never by hand-written source, never by
   authored-scene saves.

2. **`Value::ComponentObject` gets two producers**: existing
   `let x = T {}` path (live, runtime-only), and AST literal eval
   (deserialized from a snapshot). Both flow through the same Value
   variant — call sites don't need to care.

3. **Snapshot save format**: scene tree + slotmap-state alongside the
   `.mms` text (or as a sidecar binary). On load: restore slotmap
   state first, then `ce_ast_to_materialized` + `spawn_tree` reuses
   the existing indices, and any `cid(...)` literals resolve to the
   right components without a walk.

4. **Author choice per save call**: `mms.save("scene.mms")` uses
   selectors. `mms.snapshot("save01.snap")` uses raw keys + slotmap
   state. Same scene tree, different reference form.

5. **GUID selectors don't go away** — they remain the right
   cross-process / human-edited form. Slotmap keys are a strict
   performance optimization for the snapshot/replay axis.

## Why this split is the right shape

- **Phase 1 ships Action migration with the vocabulary it actually
  needs** (matches what the JSON codec used to do, plus
  `update_transform`), so the mittens 0.5 release can drop the JSON
  codec entirely. No more dead encode/decode methods anywhere in
  `src/engine/ecs/component/`.

- **In-session composition is fast after Phase 1, but only during the
  session that authored it.** `let x = T {}` with
  `Action.set_color(x, ...)` in a loop pays zero selector walks at
  composition time (evaluator hands the bound `Value::ComponentObject`
  straight through `arg_target`) and zero per-tick (the resolved id
  lives in the `IntentValue`). The slow selector path only runs when
  the author explicitly writes a string.

- **Save → reload in Phase 1 does still pay selector cost.** Dump goes
  through `subtree_to_ce_ast`, which walks the live world tree — it
  has no record of the let-binding or the loop that produced those
  Actions. It sees 1000 ActionComponents each with a resolved id,
  rewrites each as `Action.set_color("#hero", ...)`, and on the next
  load each one re-runs the selector lookup. Fine for a scene spawned
  once. Fine for hot-reload dev. Not fine for quicksave / replay where
  reload latency matters.

- **Phase 2 closes the reload-time gap.** Snapshot saves carry the
  slotmap state alongside the scene, so the AST literal for
  ComponentId resolves directly to the same key on load — no walks at
  spawn time even after a process restart. Authored / human-edited
  scene saves keep using selectors because they're stable across
  edits.

- **Phase 2 is purely additive.** New AST node, new Value producer,
  new save format. Nothing in Phase 1 needs to be undone.

- **The `arg_target` helper from Phase 1 doesn't need to change** in
  Phase 2; it already handles `Value::ComponentObject`, which is what
  the new AST literal evaluates to.

## Syntax decisions (locked in)

Two surfaces, two syntaxes:

- **mmq selector** — `@uuid:<hex>` — terse, top-of-keyboard, scans
  cleanly inside a `query()` call or as an `arg_target` string.
  Example: `Action.set_color("@uuid:8c4f3e72-...-e90a", [1,0,0,1])`.
  Implemented as a new arm in `parse_compound_selector` producing
  `SimpleSelector::Guid(String)`.

- **css selector** — `[guid=<uuid>]` — fits the existing CSS attribute
  selector grammar. Example: `[guid=8c4f3e72-...-e90a]`. **No
  parser change needed** — the css parser already produces
  `SimpleSelector::Attribute(AttributeSelector { name: "guid",
  value: "..." })`. The wiring is purely on the evaluator side: when
  `WorldQueryAdapter::matches_attribute` sees `attribute.name ==
  "guid"`, it dispatches to the same `guid_index` lookup that
  `matches_guid` uses.

Both forms hit the same `O(1)` `guid_index` lookup at evaluation
time. The mmq form is preferred for dump output (terser, easier to
scan in `.mms` files). The css form remains available for callers
who want the CSS-shaped query surface.

Dump policy (final): `to_mms_ast` for any ComponentId reference emits
`"#<name>"` if `node.name` is non-empty, otherwise
`"@uuid:<node.guid>"`. Never errors, never requires the author to
pre-name anything, always round-trips losslessly cross-process.

## Phase 1 implementation checklist

Designed to be picked up in a fresh context. Files listed in
suggested edit order.

### 1 — GUID selector wiring (foundation; do first)

- [ ] **`src/query/mmq/parser.rs`** — add an `Some('@')` arm to
      `parse_compound_selector` (next to `Some('#')` at line 138).
      Consume `@`, then literal `uuid:`, then a uuid-shaped
      identifier (hex + hyphens). Emit `SimpleSelector::Guid(uuid)`.
- [ ] **`src/query/evaluator.rs`** — already dispatches `Guid` →
      `matches_guid` (line 81). No change needed.
- [ ] **`src/engine/ecs/world_query_adapter.rs`** — implement
      `matches_guid(&self, node, guid_str)` by parsing
      `guid_str` as `uuid::Uuid` and comparing to
      `world.get_component_record(node).map(|n| n.guid)`.
      Also extend `matches_attribute` to dispatch on
      `attribute.name == "guid"` to the same path (css surface).
- [ ] **`src/query/mmq/parser.rs::tests`** — round-trip test for
      `@uuid:<sample-uuid>`.
- [ ] **`src/query/evaluator.rs::tests`** (or a new ECS-level test)
      — end-to-end: spawn a component with a known GUID, look it up
      via `world.find_component(root, "@uuid:<guid>")` and via
      `world.find_component(root, "[guid=<guid>]")`. Both succeed.

### 2 — `arg_target` / `arg_target_vec` helpers

- [ ] **`src/meow_meow/component_registry.rs`** — new helpers below
      the existing `arg_str_vec` (around line 455):
      ```rust
      fn arg_target(world: &World, args: &[Value], i: usize)
          -> Result<ComponentId, String>;
      fn arg_target_vec(world: &World, args: &[Value], i: usize)
          -> Result<Vec<ComponentId>, String>;
      ```
      `arg_target` accepts `Value::ComponentObject { id, .. }`
      (return id), `Value::String(s)` /
      `Value::Identifier(s)` (call `resolve_action_target`).
      `arg_target_vec` accepts `Value::Array` of either form.

### 3 — `Component::to_mms_ast` gets `&World`

Pick one of these two paths and apply throughout:

**Path A — change the trait signature (recommended for
long-term cleanliness):**

- [ ] **`src/engine/ecs/component/mod.rs`** — change the trait
      method from `fn to_mms_ast(&self) -> CE` to
      `fn to_mms_ast(&self, world: &World) -> CE`. Update the
      default impl.
- [ ] Every existing override (~30 files in
      `src/engine/ecs/component/`) — add `_world: &World` to
      the signature. Most ignore it; this is mechanical.
- [ ] **`src/meow_meow/component_registry.rs::subtree_to_ce_ast`**
      (around line 254) — pass `world` to each `to_mms_ast` call.

**Path B — add an alternate method, no trait change to existing
impls:**

- [ ] **`src/engine/ecs/component/mod.rs`** — add
      `fn to_mms_ast_with_world(&self, world: &World) -> CE`
      with default impl calling `self.to_mms_ast()`.
- [ ] **`src/meow_meow/component_registry.rs::subtree_to_ce_ast`**
      — prefer `to_mms_ast_with_world`.
- [ ] Only Action and IKChain override the new method.

Path A is one churn pass with no long-term cost. Path B avoids the
churn now but leaves a dual-method footprint. Decide based on whether
the churn is acceptable to bundle with this work.

### 4 — Helper for "id → selector string" used by dump

- [ ] **`src/meow_meow/component_registry.rs`** — new helper:
      ```rust
      pub(crate) fn id_to_selector_string(world: &World, id: ComponentId)
          -> String;
      ```
      Logic: look up `node.name`. If non-empty, return
      `format!("#{name}")`. Otherwise return
      `format!("@uuid:{}", node.guid)`.
- [ ] Companion helper for vec form:
      `fn ids_to_selector_array_expr(world: &World, ids: &[ComponentId])
          -> Expression` returning `Expression::Array(...)`.

### 5 — Action vocabulary mirror

For each `IntentValue` variant that's worth authoring (not just
runtime-internal), add an `apply_call` arm in `create_component`
under `"Action" => ...` and a matching arm in
`Action::to_mms_ast`. Start with the variants the JSON codec covered:

- [ ] `print(message)` — keep existing
- [ ] `update_transform(target, t, r, s)` — keep existing,
      retarget through `arg_target`
- [ ] `set_color(target | [targets], rgba)` — uses `arg_target` or
      `arg_target_vec`
- [ ] `set_text(target | [targets], text)`
- [ ] `set_position(target | [targets], position)`
- [ ] `attach(parents, child)` — two `arg_target` slots
- [ ] `attach_clone(parents, child)` — same
- [ ] `detach([targets])`
- [ ] `remove_subtree([targets])`
- [ ] `request_raycast([targets])`
- [ ] `noop` — `Action.noop()`
- [ ] (defer audio/oscillator variants pending the AudioNode
      consolidation in
      `docs/task/audio-clip-terminology-and-effect-consolidation.md`)

Each registry arm pairs with an emitter arm in `Action::to_mms_ast`.
Round-trip test per variant (selector input → dump → re-parse →
matching IntentValue), following the pattern already established in
`src/meow_meow/tests.rs::roundtrip_*`.

### 6 — IKChain follow-up (same machinery)

- [ ] **`src/engine/ecs/component/ik_chain.rs::to_mms_ast`** —
      emit `target(<selector>)` and `end_effector(<selector>)`
      builder calls. Currently target/end_effector are silently
      dropped via `ComponentId::null()` sentinels.
- [ ] **`src/meow_meow/component_registry.rs`** — under `IKChain`
      apply_call, accept `target(...)` and `end_effector(...)`
      via `arg_target`. Replace the sentinel-based construction
      with the real ids.
- [ ] Round-trip test for an IKChain with both fields set.

### 7 — Drop the dead JSON codec

After Action lands:

- [ ] **`src/engine/ecs/component/mod.rs`** — remove the default
      `encode` / `decode` methods from the `Component` trait.
      Confirm zero callers remain.
- [ ] **`Cargo.toml`** — drop `serde_json` if no other consumers.
      Grep first.
- [ ] **`docs/task/mms-component-migration-checklist.md`** — flip
      Action's checkbox and the trait-cleanup checkboxes.

### 8 — Docs

- [ ] Promote this draft (`docs/draft/`) to a spec
      (`docs/spec/component-id-references.md`) once Phase 1 lands,
      with the Phase 2 section trimmed to "see this draft for
      historical context."
- [ ] Add a one-paragraph note to
      `docs/spec/inspector-panel.md` (or wherever target selectors
      are documented) explaining the `@uuid:` form for mmq.

## Open questions

- **`to_mms_ast` signature.** Add `&World` everywhere (one churn pass,
  cleaner long-term) or add a second optional method
  (`to_mms_ast_with_world`)? The first is honest about Action / IKChain
  needing world context; the second avoids touching ~30 trivial impls.

- **`arg_target_vec` source form.** Selector vec is just
  `["#a", "#b", "#c"]`. Live-handle vec via `[a, b, c]` works because
  evaluator already supports mixed Value arrays. Mixed
  selector-and-handle in the same vec is the awkward case — probably
  fine to allow (each element evaluated independently).

- **Selector-vs-handle priority for `eval_expr_stmt` in CE bodies.**
  Today, a `ComponentObject` appearing as a body statement is treated
  as `CeChild::Attach`. We don't want `Action.set_color(hero, ...)`
  inside a `T {}` body to splice `hero` as a child of T. Verify the
  existing path distinguishes "argument position" from "body statement
  position" cleanly. (I think it does — args are inside a `Call`
  expression, body statements are `Statement::Expression` directly.)

- **Snapshot save scope.** Does Phase 2 need to serialize
  `ObjectWorld::heap` too (the let-bindings), or only the
  scene + slotmap state? If a saved scene has a script with live
  references to bound names, the heap matters. If saves are
  scene-only, it doesn't.

- **Should the `node.name` reverse lookup be cached?** Save time is
  not a hot path; probably unnecessary. But worth noting in case
  someone dumps mid-tick (REPL, hot reload).

## Status

This is a draft to align on before any implementation. The mittens
branch already has all non-Action / non-Audio migrations landed and
green. Action is the last gate before the dead JSON codec can be
removed from `Component::encode` / `Component::decode` trait defaults.
