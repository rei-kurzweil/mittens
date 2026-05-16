# ComponentId references in MMS source

How components that hold pointers to other components (`ActionComponent`,
`IKChainComponent`, …) author, serialize, and resolve those pointers.

Shipped with the JSON → `to_mms_ast` migration on the `mittens` branch.

## The two surface forms

There are two durable ways to refer to another component in MMS:

| Form | Selector syntax | Resolves via | Use |
|---|---|---|---|
| Name selector | `"#hero"`, `"[name='hero']"`, `Type#hero`, … | `World::find_component` (mmq walk) | Readable diffs in human-authored scenes. Round-trips via the `name = "hero"` named-prop. |
| GUID selector | `"@uuid:8c4f3e72-...-e90a"`, `"[guid=8c4f...]"` | `World::component_id_by_guid` (O(1) hashmap) | Stable cross-process handle for any component, named or not. Emitted by dump for components passed as live handles. |

There is a third form authored in MMS — a live `Value::ComponentObject`
from `let x = T {}` or `query("…")` — but it collapses to a GUID selector
at registry-call time, so on the wire there are only the two above.

```meow_meow
let hero = T { name = "hero" }
Action.set_color("#hero", [1, 0, 0, 1])   // name selector
Action.set_color(hero, [1, 0, 0, 1])      // live handle → recorded as Guid(hero.guid)
Action.set_color("@uuid:8c4f...", [...])  // explicit guid selector
```

All three are accepted by `arg_component_ref`. The third literal form is
mostly produced by dump, not hand-typed.

## ComponentRef — the authoring metadata

Components that hold these references carry a `ComponentRef` alongside
each resolved ComponentId slot:

```rust
pub enum ComponentRef {
    Guid(uuid::Uuid),    // author wrote @uuid:..., OR passed a live handle
    Query(String),       // author wrote any other selector string
}
```

Defined in `src/engine/ecs/component/component_ref.rs`. Two consumers
today:

- **`ActionComponent`** — `target_sources: Vec<ComponentRef>`, one entry
  per ComponentId slot in `signal: IntentValue`, ordered by declaration
  order in the variant. `resolved: bool` flag.
- **`IKChainComponent`** — `target_source: Option<ComponentRef>` and
  `end_effector_source: Option<ComponentRef>` paired one-to-one with
  `target_id` / `end_effector_id`.

The point of carrying the source alongside the resolved id is **lossless
round-trip**: dump emits whatever the author wrote, not a re-derivation.
A `#hero` reference stays `#hero` on save; a live-handle reference
becomes `@uuid:<g>`; an `@uuid:` literal stays as written.

## Registry: parsing arguments into ComponentRef

`src/meow_meow/component_registry.rs` exposes three helpers:

```rust
fn arg_component_ref(world, args, i)      -> Result<ComponentRef>
fn arg_component_ref_vec(world, args, i)  -> Result<Vec<ComponentRef>>
fn resolve_component_ref(world, &ref)     -> Option<ComponentId>
```

`arg_component_ref` maps each argument shape:

- `Value::ComponentObject { id, .. }` — looks up the target's
  `node.guid` in `World` and returns `Guid(g)`. Live-handle authoring
  collapses to a guid here.
- `Value::String` / `Value::Identifier` starting `@uuid:<hex>` —
  pre-parses the uuid and returns `Guid(parsed)`. Pre-parsing saves a
  selector parse on every later resolution.
- Any other string / identifier — returns `Query(s)` verbatim.

`resolve_component_ref` is best-effort: returns `Some(id)` if the
referent already exists, `None` otherwise. Used by component apply_call
handlers that want to fill in the resolved id eagerly when possible.

## Resolution timing — when refs become ComponentIds

A `ComponentRef` resolves to a `ComponentId` in one of three places.

### 1. At registry-call time (eager)

When the apply_call handler can resolve immediately (referent is
already spawned), it stores the id directly. Both `Action.*` and
`IKChain.target`/`.end_effector` try this first.

### 2. AnimationSystem — for ActionComponent

Configured per-`AnimationComponent`:

```meow_meow
Animation.looping()                          // default: OnAttach
Animation.looping().resolve_targets("on_play")   // defer
```

- `OnAttach` (default): on the first tick that sees an animation,
  bulk-resolve every action under its keyframes once. Runtime ticks
  use the cached ids.
- `OnPlay`: lazy. Each Action push resolves just before firing, so
  forward refs (action authored before referent exists) and
  dynamically-spawned targets work.

The runtime form is always an `IntentValue` with the ComponentId slots
filled in; `ActionComponent.resolved` flips to `true` after a
successful pass. Errors are logged; broken actions skip but don't halt
the animation.

### 3. IKSystem — for IKChainComponent

`IKSystem::tick` calls `resolve_ik_chain_refs` before each
`tick_chain`. Same semantics as AnimationSystem's `OnPlay` path: null
target_id / end_effector_id slots get filled from their sources via
the same `guid_index` or selector walk. Idempotent — non-null slots
are left alone, so registry-resolved chains and runtime-wired chains
(AvatarControlSystem) both keep working.

There is no `OnAttach` toggle for IK; deferred-only matches the
existing "wired by system" behavior.

## Save → reload: how GUID references survive respawn

A `Guid(u)` reference is only useful at reload if the referent
component still has guid `u`. By default, every component mints a fresh
`uuid::Uuid::new_v4()` at construction, so a naive round-trip would
break every `@uuid:` selector.

The dump path closes this gap:

1. **Pre-pass.** `subtree_to_ce_ast` walks the subtree once and
   collects every uuid referenced via `ComponentRef::Guid(u)` by any
   ActionComponent.target_sources or IKChainComponent.{target_source,
   end_effector_source}.
2. **Emit.** When emitting each component CE, if the component's guid
   is in the referenced set, the body gets a
   `guid = "8c4f3e72-..."` named-prop.
3. **Restore.** `spawn_tree`'s `guid` named-prop intercept calls
   `World::set_component_guid(id, parsed)` to overwrite the freshly
   minted guid with the authored one (and rewires `guid_index`).

`name` is emitted independently when set, since `#name` selectors rely
on it; both `name` and `guid` can appear together on the same
component CE.

Unreferenced components don't get a `guid = ...` line, so typical
scene dumps stay clean.

## End-to-end example

Source (hand-authored):

```meow_meow
T {
  let hero = T { name = "hero" }
  Animation.looping() {
    Keyframe.at(0.0) {
      Action.set_color(hero, [1, 0, 0, 1])
      Action.set_color("#hero", [0, 1, 0, 1])
    }
  }
}
```

After save:

```meow_meow
T {
  T {
    name = "hero"
    guid = "8c4f3e72-1234-5678-9abc-def012345678"
  }
  Animation.looping() {
    Keyframe.at(0.0) {
      Action.set_color(["@uuid:8c4f3e72-1234-5678-9abc-def012345678"], [1.0, 0.0, 0.0, 1.0])
      Action.set_color(["#hero"], [0.0, 1.0, 0.0, 1.0])
    }
  }
}
```

Live-handle reference dumped as `@uuid:`; selector reference preserved
as-is; the target got `guid = "..."` because it's referenced by guid,
and kept `name = "hero"` because the author set one. Both refs
resolve to the same component on reload.

## Snapshot saves (not implemented)

The above is for **authored-scene saves** — human-edited `.mms` source
that round-trips losslessly via selector strings and survives renumbering
the slotmap arena. A second axis, **snapshot saves** (quicksave, replay,
undo/redo where the whole world including slotmap state is restored
verbatim), is a separate Phase 2 effort and not implemented. The
short version is that a raw `cid(0x<u64>)` literal would carry a
slotmap key directly — zero-walk reload at the cost of being unusable
for anything other than its originating process / slotmap state. There
is no plan to wire this until a real product constraint demands it.

## See also

- `docs/spec/component-query-selectors.md` — selector grammar shared by
  mmq and the future CSS surface.
