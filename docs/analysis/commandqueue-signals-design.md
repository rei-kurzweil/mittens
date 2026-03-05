# CommandQueue → Signals: callsite churn + signal shape

Date: 2026-03-04

This doc explains (1) which callsites we avoid touching by keeping the `CommandQueue` API as a facade while we migrate execution onto the signal stream, and (2) the design choice between a single “`CommandQueue` signal” payload vs introducing distinct typed signal variants for each former command.

## Goals

- Consolidate mutations onto one transport: signals.
- Preserve the existing “mutation barrier” structure (explicit flush/dispatch points) while we simplify.
- Avoid a repo-wide mechanical rewrite of callsites during the first iteration.
- Enable two dispatch modes:
  - **execute** (immediate) for mutations/actions
  - **observe** (handler dispatch) for scoped/global listeners

## Non-goals

- Perfect final semantics (ordering, “execute then observe” vs “observe then execute”, etc.) in the first iteration.
- A total elimination of `CommandQueue` as a type (that’s a later optional step).

## What “callsite churn” we’re avoiding

The refactor direction is explicitly: *keep callsites calling `queue_*` methods* and change the implementation underneath so that those methods emit signals.

### Quantification (current repo)

As of 2026-03-04, there are roughly:

- ~190 occurrences of `queue_*` calls across the repo (`src` is ~182 of those).
- ~122 references to the `CommandQueue` type in `src` (function parameters, fields, imports, etc.).

If we removed `CommandQueue` and switched callsites to “push typed signals directly”, we would be editing on the order of “a couple hundred” callsites immediately, plus whatever fallout comes from signature changes.

### Where the churn is concentrated

The highest-churn hotspots are:

- `src/engine/ecs/system/action_system.rs` (dozens of `queue_*` usages)
- `src/engine/ecs/component/*` registration/unregistration paths (`queue_register_*`, `queue_remove_*`)
- XR + render backend glue that uses `queue_family_index`/`queue_submit` style calls

This matters because those callsites are spread across many conceptual subsystems (render registration, topology updates, audio graph scheduling, XR frame sync). The “keep the facade” approach means those systems don’t all need to be rewritten at once.

### Concrete examples of what stays stable

These categories of callsites remain unchanged in the “facade” approach:

- **Component lifecycle hooks** keep calling `queue_register_*` / `queue_remove_*`.
  - Example cluster: `src/engine/ecs/component/transform.rs`, `renderable.rs`, `text.rs`, `raycast.rs`, etc.
- **ActionSystem** keeps calling `queue_update_transform`, `queue_remove_subtree`, `queue_set_text`, and various audio scheduling helpers.
- **Systems** that respond to physics/input/topology changes keep calling `queue_update_transform` and “topology refresh” helpers.
- **Graphics/XR backend** callsites keep calling `queue_*` methods for device/queue bookkeeping (these are not “ECS mutation” signals, but are still currently plumbed through the same queue type).

The only files that need to change for the first iteration are the “bridge” points:

- `CommandQueue` implementation (emit signals)
- the central dispatcher/executor in `SystemWorld` (execute immediate signals, then dispatch handlers)
- whichever systems used to consume commands in `flush()` (now consume signals)

## Two designs for “commands become signals”

There are two orthogonal choices here:

1) **Do callsites emit signals directly?**
2) **What shape are the emitted signals?**

This doc is primarily about (2). For (1), the current plan is “no”: callsites keep using `CommandQueue` during the first iteration.

### Option A — one `SignalValue::CommandQueue { command_name, params }`

This is the “stringly typed” bridge:

- A single signal variant (e.g. `SignalValue::CommandQueue`) contains:
  - `command_name: String`
  - `params: serde_json::Value` (or an array of JSON values)
- `SystemWorld` (or a dedicated executor system) matches on `command_name` and deserializes/reads params.

**Pros**

- Minimal surface-area change while migrating transport.
- Lets us move “execution” to the signal pipeline quickly.
- Avoids a large `enum` growth in `SignalValue` immediately.

**Cons**

- Weak typing: wrong param shapes become runtime errors.
- Harder refactors: renaming a command is a string change that the compiler cannot help with.
- Worse discoverability: IDE navigation/search is poorer than matching a typed enum variant.
- JSON is a non-trivial cost (allocations + parsing/format conversion) for high-frequency operations like transform updates.

**When Option A is acceptable**

- As a short-lived adapter (days/weeks), used to validate execution ordering / dispatch mechanics.
- For low-frequency “tooling-ish” commands.

**When Option A becomes painful**

- High-frequency operations (`UpdateTransform`, topology refresh) where JSON overhead matters.
- Any command that wants strong invariants (e.g. “a renderable registration must include X and Y”).

### Option B — typed signal variants per former command

Instead of one generic command packet, introduce typed variants, e.g.:

- `SignalValue::RegisterTransform { entity, initial: Transform }`
- `SignalValue::UpdateTransform { entity, translation, rotation, scale }`
- `SignalValue::SetText { entity, text }`
- `SignalValue::RemoveSubtree { root }`
- Audio scheduling variants (`ScheduleAudioOp { .. }`, etc.)

There are a few ways to organize these:

- **B1:** put each as a direct `SignalValue` variant.
- **B2:** add a nested enum, e.g. `SignalValue::Command(CommandSignal)`.
- **B3:** split into multiple enums by domain, e.g. `RenderCommand`, `TopologyCommand`, `AudioCommand`.

**Pros**

- Compile-time checking and exhaustiveness when executing.
- Easier refactors: renames and signature changes are compiler-driven.
- No JSON overhead; payload is native Rust types.
- Better ability to reason about “immediate execution” vs “observe only” per signal type.

**Cons**

- More up-front design work.
- Potential enum bloat (though a nested `CommandSignal` keeps `SignalValue` manageable).
- You still have to decide where “graphics/XR queue_*” calls belong (they might not belong in ECS signals at all).

### Key point: typed signals do *not* require changing the callsites

If the goal is “avoid editing hundreds of callsites right now”, typed signals are still compatible with that goal.

You can keep `CommandQueue::queue_update_transform(...)` etc. exactly as-is, but have those methods emit typed signals instead of the generic `command_name/params` signal.

That gives:

- The same callsite stability as Option A.
- Most of the long-term correctness/maintainability benefits of Option B.

## Recommendation

- Use Option A (`command_name` + JSON params) only as a very short-lived bridge if needed to unblock the dispatcher/executor refactor.
- Prefer Option B for anything that is:
  - high-frequency (transforms)
  - correctness-sensitive (topology changes)
  - likely to evolve (render registration)

Pragmatically: keep `CommandQueue` as the callsite facade, but move its internal emission to typed variants early, before the stringly layer spreads further.

## Migration sketch (low churn)

1) Keep callsites unchanged: `queue_*` methods stay.
2) Change `CommandQueue` to emit **typed** command-signal variants.
3) Implement `SystemWorld` dispatcher/executor to:
   - execute immediate signals (the direct/executor path)
   - then dispatch handler observers (the observe path)
4) Once stable, decide whether to:
   - keep `CommandQueue` forever as a convenience facade, or
   - gradually change specific subsystems to push signals directly and delete the corresponding `queue_*` methods.

## Open questions

- Should render/XR “queue_*” helpers remain in `CommandQueue`, or move to a different abstraction (they’re not ECS mutations)?
- For “execute vs observe”, do we want per-signal-type defaults (e.g. all `CommandSignal::*` are immediate by default)?
- Do we need stable ordering guarantees for “execute then observe”, and where should that be enforced (dispatcher vs executor vs handler registry)?

## Appendix: current `queue_*` inventory (high-signal subset)

These counts are from `rg -o "queue_[A-Za-z0-9_]+" src | sort | uniq -c | sort -nr`.

Most common callsites (excluding the `CommandQueue` impl itself):

- `queue_audio_graph_dirty` (11) — mostly ActionSystem + audio components
- `queue_topology_transform_refresh` (9) — ActionSystem
- `queue_family_index` (5+ across XR + renderer backends)
- `queue_update_transform` (multiple systems + `transform` component)
- `queue_remove_subtree` (ActionSystem + `Universe` + gizmo)

“One-off but wide” registration callsites (typically 1–2 each):

- `queue_register_transform`, `queue_register_renderable`, `queue_register_text`, `queue_register_raycast`, …
- `queue_make_active_camera` (camera components)
- `queue_register_openxr` (XR component)
- `queue_register_light`, `queue_register_emissive`, `queue_register_opacity`, …

To regenerate the full method list and counts locally:

```bash
rg -o "queue_[A-Za-z0-9_]+" src | sort | uniq -c | sort -nr
```

To see which files contain the most `queue_*` callsites:

```bash
rg -n "\bqueue_[A-Za-z0-9_]+\b" src | cut -d: -f1 | sort | uniq -c | sort -nr
```
