
# MMS signal guide

This is the canonical doc for the engine’s “signals-first” layer.

The doc is intentionally split into three parts:

1) Current design / goal (what to rely on)
2) Unsettled decisions (what’s still in flux)
3) Current status (what is implemented today)

## Current design / goal

### What a signal is

A `Signal` is a typed message with:

- `scope: ComponentId` — where it “happened” (used for subtree-scoped dispatch)
- `event: Option<EventSignal>` — fact/observation payload (routed to handlers)
- `intent: Option<IntentSignal>` — side-effect request payload (executed at drain points)

Timing (`SignalWhen`) lives on `IntentSignal` (not events).

`SignalWhen::AtBeat(b)` means the signal is held until transport beat $\ge b$.

### Intent/event model replaces the older action/event split

The canonical model is now:

- `IntentSignal` = side-effect request
- `EventSignal` = observed fact

Older docs may refer to an “action” layer or `ActionMethod` as if it were the public conceptual API. That is historical terminology only.

`ActionComponent` and `ActionSystem` have been removed. Deferred animation is
authored only with executable `Keyframe.at(...) { ... }` blocks that capture
live component handles.

This is a breaking API and authoring change in `mittens-engine` 0.7. Existing
Rust code must call component methods or emit intents directly; existing MMS
must replace `Action.*` children with method calls inside keyframe blocks.

### Every intent has one recipient

An intent dispatch addresses exactly one `ComponentId`. Recipient fields use
the singular name `component_id`; topology intents use `parent` and `child`.
If a caller needs to affect several components it emits several intents in
deterministic order.

`Signal.scope` remains the origin and routing context. It is not an execution
recipient and pipeline routing may rewrite the intent recipient without
changing the signal scope. Semantic collections such as selection entries,
visualization `scope_roots`, HTTP headers, and query results remain vectors.

### One stream, explicit drain points

The core invariant is:

- Signals are **emitted** freely during systems/ticks.
- Signals are **executed and observed** only at explicit **drain points**.

Drain points are implemented by `SystemWorld::process_signals(...)`.

At a high level, each drain point does:

1. Move locally-staged signals into the main bus.
2. Promote due timed signals (`AtBeat`) into the ready queue.
3. For each ready signal (up to a cap):
   - run the engine’s execution stages
   - then dispatch handlers as observers

### Two-stage intent execution

Intent execution is intentionally split into two layers:

- **Intent interpretation stage**: `rx::RxIntentExecutor`
  - Runs for high-level `IntentValue`s that expand into follow-up intents/events.
  - Emits follow-up work via `SignalEmitter`.

- **Default executor stage**: `SystemWorld::execute_intent_signal(...)`
  - Applies canonical engine side effects (register/remove/update, system registration, etc).

After execution, `RxWorld` dispatches handlers for observation.

Design goal: handlers should be observers/emitters, not “the place where mutations happen”.

#### Guideline: where intent logic lives

- If an intent can be fulfilled with a **small amount of code** and is **not system-specific** (e.g. topology helpers like attach/detach/remove), implement it directly in the **IntentExecutor**.
- If fulfilling an intent is more than a few lines, or clearly belongs to a system, the **IntentExecutor should still “own” fulfilling the intent**, but it should delegate to the appropriate system.
  - Example: an intent that affects rendering should delegate to Renderable/Texture/Visual systems.
  - Example: an intent that affects physics should delegate to Collision/CollisionResponse systems.

### Scoped dispatch

Handlers are registered at `(SignalKind, scope_root)`.

When a signal with `scope = S` is dispatched, the engine walks ancestry:

`S, parent(S), parent(parent(S)), ...`

and invokes any handlers registered at any of those nodes.

This gives you “subscribe to a subtree” semantics without global filtering.

Important clarification:

- this is **ancestor-bubbling only**
- a parent can observe child-scoped events
- a child cannot observe parent-scoped events just by registering a scoped handler

So the current runtime does **not** provide a second propagation mode such as
"child listens to parent events".

If a component needs to react to an upstream event and expose a component-local semantic
event (for example `ScrollingComponent` projecting ancestor `DragMove` into a local
`Scrolling` event), the current model is:

- register a handler at the upstream scope
- map the upstream event in handler code
- emit a new event scoped to the component that owns the behavior

See [docs/draft/event-signal-pipelines.md](../../draft/event-signal-pipelines.md) for the draft
proposal to formalize that pattern as an event routing/projection layer.

Example: listen for topology changes in a subtree:

```rust
use cat_engine::engine::ecs;

fn on_parent_changed(
  _world: &mut ecs::World,
  _emit: &mut dyn ecs::SignalEmitter,
  signal: &ecs::Signal,
) {
  let Some(ecs::EventSignal::ParentChanged { child, old_parent, new_parent }) = signal.event.as_ref() else {
    return;
  };
  println!("child={child:?} old={old_parent:?} new={new_parent:?}");
}

fn setup(universe: &mut cat_engine::engine::Universe, scope_root: ecs::ComponentId) {
  universe.add_signal_handler(ecs::SignalKind::ParentChanged, scope_root, on_parent_changed);
}
```

### Scheduling: can attach/detach/remove be timed?

Yes: signals carry `when`, and `RxWorld` supports a holding pen for `SignalWhen::AtBeat`.

Practical semantics (important): timing delays *eligibility*; resolution happens at execution time.
So if you schedule something structural like `Attach` / `Detach` / `RemoveSubtree`:

- It executes at the due drain point.
- It is best-effort with respect to world state at that time.
  - If the referenced `ComponentId`s no longer exist, the operation should effectively no-op.
  - If topology has changed, the operation applies to the current topology.

Design constraint / goal (not fully enforced yet):

- Only **intent-ish** operations should be scheduled.
- Facts/events (e.g. `ParentChanged`) should not be scheduled.
- Low-level internal registrations (e.g. `RegisterRenderable`) should not be scheduled.

### Subtree deletion: no `*Immediate`

Subtree deletion is represented by `IntentValue::RemoveSubtree { target: Vec<ComponentId> }`.

There is no `RemoveSubtreeImmediate` variant. Deletion happens at drain points via the default
executor, which:

- detaches the root (if still attached) and emits `ParentChanged`
- performs best-effort system teardown (renderables/collision/etc)
- removes the component subtree from `World`

This keeps “when does deletion happen?” aligned with drain points and avoids duplicated API
surface area.

## Unsettled decisions

- **Type shape**: keep one flat `IntentValue` enum that mixes user intents and internal mutations, or split into explicit `UserIntent`/`EcsMutation` enums?
- **Scheduling policy**: should we hard-forbid `AtBeat` for low-level internal ops and events at the type level?
- **Failure semantics**: when a scheduled signal references missing components, should we (a) silently no-op, (b) return an error somewhere, or (c) emit a structured failure event?
- **Re-entrancy**: if handlers emit more signals, do they run in the same drain point or always at the next one? What are the budgets/caps per stage?
- **Ordering guarantees**: do we need a single total order across “intent vs events”, or is staged ordering sufficient? Should signals carry a `seq: u64`?
- **Global handlers**: keep global handlers in `RxWorld`, or require explicit scope roots only?
- **Handler API**: keep public handlers as `fn` pointers, or move to `HandlerId` + boxed closures for ergonomics?
- **Where intent logic lives**: how far do we push `RxIntentExecutor` vs keeping a legacy interpreter layer around system-owned mutations?

## Current status (2026-07-19)

- `SignalWhen::{Now, AtBeat}` exists and timed signals are held pending until `ClockSystem` beat is due.
- Drain-point execution lives in `SystemWorld::process_signals(...)`.
- `CommandQueue` is a transitional per-frame staging emitter (no raw pointers); it drains into `SystemWorld.rx` at drain points.
- `RemoveSubtreeImmediate` is gone; `RemoveSubtree { target }` is the one subtree deletion action.
- `SetTextImmediate` is gone; `SetText` executes at drain points and rebuilds the glyph subtree.
- `RxIntentExecutor` interprets high-level `IntentValue`s and the mutation
  executor applies canonical side effects.

## MMS exposure labels and limitations

The catalogs below describe the implementation as it exists, not proposed syntax. Event labels are **observable with payload**, **observable with partial payload**, **observable with `null`**, or **unavailable**. Intent labels are **available through a live method/builtin**, **indirectly emitted by component lifecycle**, or **engine-only**.

Three limitations are especially easy to miss:

- `LayoutRootSizeAvailable` exists in the engine but is not accepted by MMS `on(...)`.
- Many accepted event names dispatch correctly but `event_arg_value` converts their handler argument to `null`.
- Partial payloads intentionally omit engine fields: `DataEvent` omits its component payload; XR events omit `source_component`; and `TextInputChanged` omits `component_id`.

All examples are syntax-checked. `parse-only` examples for internal or unsupported signals demonstrate the closest real MMS relationship; they do not claim that MMS can construct an engine signal directly.

## Event catalog

### Runtime and routing

#### `FrameTick`
<!-- catalog:signal source="FrameTick" kind="event" mms="observable-payload" -->
**Event.** A rendered frame began; the payload reports elapsed seconds. The runtime and routing subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "FrameTick", fn(event) { print(event) })
scope
```

#### `KeyDown`, `KeyPress`, and `KeyUp`
<!-- catalog:signal source="KeyDown" kind="event" mms="observable-payload" -->
<!-- catalog:signal source="KeyPress" kind="event" mms="observable-payload" -->
<!-- catalog:signal source="KeyUp" kind="event" mms="observable-payload" -->
**Events.** Gameplay keyboard events are available scene-wide through `on_global` without an
`Input` component. An initial press emits `KeyDown` followed by `KeyPress`; each OS repeat emits
only `KeyPress`; release emits only `KeyUp`. `KeyPress` is not committed text input, and movement
should use held state plus `FrameTick` rather than repeat frequency. These events are suppressed
while a text input owns keyboard focus. Losing focus releases keys previously delivered to the
scene. Escape remains a window-level exit shortcut, so its callback is not guaranteed to run.

The callback receives exactly `{ code, key }`. `code` is the layout-independent physical-key name
(for example `"KeyW"`, `"ArrowUp"`, or `"ShiftLeft"`) or `null` for an unidentified physical key.
`key` preserves the logical, layout-dependent value and case, or is a named value such as
`"Enter"`, `"Dead"`, or `"Unidentified"`; it is not guaranteed to be committed text.

```mms parse-only
on_global("KeyDown", fn(event) {
    print("down code=" + event.code + " key=" + event.key)
})
on_global("KeyPress", fn(event) { print("press " + event.key) })
on_global("KeyUp", fn(event) { print("up " + event.key) })
```

#### `DataEvent`
<!-- catalog:signal source="DataEvent" kind="event" mms="observable-partial-payload" -->
**Event.** User code emitted a named cross-subtree data event. The runtime and routing subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "DataEvent", fn(event) { print(event) })
scope
```

### Topology and transforms

#### `ParentChanged`
<!-- catalog:signal source="ParentChanged" kind="event" mms="observable-null" -->
**Event.** A component was attached, detached, or reparented. The topology and transforms subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "ParentChanged", fn(event) { print(event) })
scope
```

### Rendering and assets

#### `GltfInitialized`
<!-- catalog:signal source="GltfInitialized" kind="event" mms="observable-payload" -->
**Event.** A glTF asset finished spawning and its runtime nodes can be queried. The rendering and assets subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "GLTFInitialized", fn(event) { print(event) })
scope
```

### Interaction and physics

#### `RayIntersected`
<!-- catalog:signal source="RayIntersected" kind="event" mms="observable-null" -->
**Event.** A raycast hit a renderable. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "RayIntersected", fn(event) { print(event) })
scope
```

#### `CollisionStarted`
<!-- catalog:signal source="CollisionStarted" kind="event" mms="observable-null" -->
**Event.** Two collision objects began overlapping. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "CollisionStarted", fn(event) { print(event) })
scope
```

#### `CollisionEnded`
<!-- catalog:signal source="CollisionEnded" kind="event" mms="observable-null" -->
**Event.** Two collision objects stopped overlapping. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "CollisionEnded", fn(event) { print(event) })
scope
```

#### `MountStarted`
<!-- catalog:signal source="MountStarted" kind="event" mms="observable-payload" -->
**Event.** `AttachmentSystem` committed a Rider-to-Mountable relationship. The
payload contains live `rider` and `mountable` component handles. The event is
scoped to the `Mountable`, and MMS `on(...)` accepts it.
```mms parse-only
let vehicle = Mountable {}
on(vehicle, "MountStarted", fn(event) {
    print(event.rider, event.mountable)
})
vehicle
```

#### `MountEnded`
<!-- catalog:signal source="MountEnded" kind="event" mms="observable-payload" -->
**Event.** `AttachmentSystem` ended or unwound a Rider-to-Mountable
relationship. The payload contains live `rider` and `mountable` component
handles. The event is scoped to the `Mountable`, and MMS `on(...)` accepts it.
```mms parse-only
let vehicle = Mountable {}
on(vehicle, "MountEnded", fn(event) {
    print(event.rider, event.mountable)
})
vehicle
```

#### `DragStart`
<!-- catalog:signal source="DragStart" kind="event" mms="observable-null" -->
**Event.** A gesture crossed into the dragging state. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "DragStart", fn(event) { print(event) })
scope
```

#### `DragMove`
<!-- catalog:signal source="DragMove" kind="event" mms="observable-null" -->
**Event.** An active drag moved during the current tick. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "DragMove", fn(event) { print(event) })
scope
```

#### `DragEnd`
<!-- catalog:signal source="DragEnd" kind="event" mms="observable-null" -->
**Event.** An active drag ended. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "DragEnd", fn(event) { print(event) })
scope
```

#### `GrabStart`
<!-- catalog:signal source="GrabStart" kind="event" mms="observable-null" -->
**Event.** A pointer attached an eligible `Grabbable` target. MMS accepts this
event name; its structured engine payload is not yet exposed to MMS, so the
handler currently receives `null`.
```mms parse-only
let scope = Transform {}
on(scope, "GrabStart", fn(event) { print(event) })
scope
```

#### `GrabEnd`
<!-- catalog:signal source="GrabEnd" kind="event" mms="observable-null" -->
**Event.** A pointer released its attached `Grabbable` target. MMS accepts this
event name; its structured engine payload is not yet exposed to MMS, so the
handler currently receives `null`.
```mms parse-only
let scope = Transform {}
on(scope, "GrabEnd", fn(event) { print(event) })
scope
```

#### `Click`
<!-- catalog:signal source="Click" kind="event" mms="observable-null" -->
**Event.** A drag ended within the click displacement threshold. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "Click", fn(event) { print(event) })
scope
```

#### `ToggleChanged`
<!-- catalog:signal source="ToggleChanged" kind="event" mms="observable-payload" -->
**Event.** A `Toggle` changed value. The payload contains the live toggle component and its boolean `value`.
```mms parse-only
let scope = Transform {}
on(scope, "ToggleChanged", fn(event) { print(event.value) })
scope
```

#### `SliderChanged`
<!-- catalog:signal source="SliderChanged" kind="event" mms="observable-payload" -->
**Event.** A `Slider` received a user or emitting programmatic value change. The payload contains the live slider component and its normalized numeric `value`.
```mms parse-only
let slider = Slider.range(0.0, 1.0)
on(slider, "SliderChanged", fn(event) { print(event.value) })
slider
```

#### `SliderCommitted`
<!-- catalog:signal source="SliderCommitted" kind="event" mms="observable-payload" -->
**Event.** A slider drag completed. The payload contains the live slider component and its final normalized numeric `value`.
```mms parse-only
let slider = Slider.range(0.0, 1.0)
on(slider, "SliderCommitted", fn(event) { print(event.value) })
slider
```

#### `SelectionChanged`
<!-- catalog:signal source="SelectionChanged" kind="event" mms="observable-null" -->
**Event.** The complete state of a selection scope changed. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "SelectionChanged", fn(event) { print(event) })
scope
```

#### `SelectionAdded`
<!-- catalog:signal source="SelectionAdded" kind="event" mms="observable-null" -->
**Event.** An entry was added to a selection scope. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "SelectionAdded", fn(event) { print(event) })
scope
```

#### `SelectionRemoved`
<!-- catalog:signal source="SelectionRemoved" kind="event" mms="observable-null" -->
**Event.** An entry was removed from a selection scope. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "SelectionRemoved", fn(event) { print(event) })
scope
```

#### `SelectionCleared`
<!-- catalog:signal source="SelectionCleared" kind="event" mms="observable-null" -->
**Event.** All entries were removed from a selection scope. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "SelectionCleared", fn(event) { print(event) })
scope
```

### Text and layout

#### `Scrolling`
<!-- catalog:signal source="Scrolling" kind="event" mms="observable-null" -->
**Event.** A scrolling component consumed drag motion and changed offset. The text and layout subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "Scrolling", fn(event) { print(event) })
scope
```

#### `TextInputFocusChanged`
<!-- catalog:signal source="TextInputFocusChanged" kind="event" mms="observable-null" -->
**Event.** The focused text-input component changed. The text and layout subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "TextInputFocusChanged", fn(event) { print(event) })
scope
```

#### `TextInputChanged`
<!-- catalog:signal source="TextInputChanged" kind="event" mms="observable-partial-payload" -->
**Event.** Text or caret state changed in a text input. The text and layout subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "TextInputChanged", fn(event) { print(event) })
scope
```

#### `LayoutRootSizeAvailable`
<!-- catalog:signal source="LayoutRootSizeAvailable" kind="event" mms="unavailable" -->
**Event.** Layout completed and computed root dimensions became available. The text and layout subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` rejects this name. Constructing a layout root is the closest real relationship. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
LayoutRoot { Text { "content" } }
```

### XR

#### `XrButtonDown`
<!-- catalog:signal source="XrButtonDown" kind="event" mms="observable-partial-payload" -->
**Event.** An XR button crossed into the pressed state. The xr subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "XrButtonDown", fn(event) { print(event) })
scope
```

#### `XrButtonUp`
<!-- catalog:signal source="XrButtonUp" kind="event" mms="observable-partial-payload" -->
**Event.** An XR button crossed into the released state. The xr subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "XrButtonUp", fn(event) { print(event) })
scope
```

#### `XrButtonChanged`
<!-- catalog:signal source="XrButtonChanged" kind="event" mms="observable-partial-payload" -->
**Event.** An XR button analog value changed. The xr subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "XrButtonChanged", fn(event) { print(event) })
scope
```

#### `XrAxisChanged`
<!-- catalog:signal source="XrAxisChanged" kind="event" mms="observable-partial-payload" -->
**Event.** An XR two-axis control changed. The xr subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "XrAxisChanged", fn(event) { print(event) })
scope
```

#### `XrEyeTrackingUpdated`
<!-- catalog:signal source="XrEyeTrackingUpdated" kind="event" mms="observable-payload" -->
**Event.** The generic XR eye-tracking source published a new sample. The MMS
payload contains optional `combined_look`, `left_look`, `right_look`, and
`combined_openness` values.
```mms parse-only
let scope = Transform {}
on(scope, "XrEyeTrackingUpdated", fn(event) { print(event.combined_look) })
scope
```

#### `XrEyeTrackingHtcUpdated`
<!-- catalog:signal source="XrEyeTrackingHtcUpdated" kind="event" mms="observable-payload" -->
**Event.** The HTC eye-tracking source published a new sample. The MMS payload
contains `left` and `right` tables with optional look, position, openness, and
pupil-diameter fields.
```mms parse-only
let scope = Transform {}
on(scope, "XrEyeTrackingHtcUpdated", fn(event) { print(event.left.look) })
scope
```

### HTTP

#### `HttpRequest`
<!-- catalog:signal source="HttpRequest" kind="event" mms="observable-payload" -->
**Event.** An enabled HTTP server accepted a request. The http subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "HttpRequest", fn(event) { print(event) })
scope
```

#### `HttpResponse`
<!-- catalog:signal source="HttpResponse" kind="event" mms="observable-payload" -->
**Event.** An HTTP client request completed successfully. The http subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "HttpResponse", fn(event) { print(event) })
scope
```

#### `HttpError`
<!-- catalog:signal source="HttpError" kind="event" mms="observable-payload" -->
**Event.** An HTTP client or server operation failed. The http subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/signals/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "HttpError", fn(event) { print(event) })
scope
```

## Intent catalog

### Runtime and routing

#### `Noop`
<!-- catalog:signal source="Noop" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `Noop` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `SpawnComponentTree`
<!-- catalog:signal source="SpawnComponentTree" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `SpawnComponentTree` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `Print`
<!-- catalog:signal source="Print" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `Print` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
print("intent example")
```

#### `ReplExec`
<!-- catalog:signal source="ReplExec" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ReplExec` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRouter`
<!-- catalog:signal source="RegisterRouter" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRouter` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterHttpServer`
<!-- catalog:signal source="RegisterHttpServer" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterHttpServer` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterHttpClient`
<!-- catalog:signal source="RegisterHttpClient" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterHttpClient` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterInput`
<!-- catalog:signal source="RegisterInput" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterInput` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTransparentCutout`
<!-- catalog:signal source="RegisterTransparentCutout" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTransparentCutout` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterEditor`
<!-- catalog:signal source="RegisterEditor" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterEditor` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterEditorUI`
<!-- catalog:signal source="RegisterEditorUI" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Registers an authored `EditorUI` and materializes its configured shared panel workspace.
```mms parse-only
EditorUI {}
```

#### `RegisterSignalRouteUpward`
<!-- catalog:signal source="RegisterSignalRouteUpward" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterSignalRouteUpward` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveSignalRouteUpward`
<!-- catalog:signal source="RemoveSignalRouteUpward" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveSignalRouteUpward` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Topology and transforms

#### `SetPosition`
<!-- catalog:signal source="SetPosition" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `SetPosition` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `LookAt`
<!-- catalog:signal source="LookAt" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `LookAt` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let node = Transform {}
node.look_at([0, 0, -1])
```

#### `Attach`
<!-- catalog:signal source="Attach" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `Attach` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `QueryFindComponent`
<!-- catalog:signal source="QueryFindComponent" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `QueryFindComponent` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `QueryFindAllComponents`
<!-- catalog:signal source="QueryFindAllComponents" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `QueryFindAllComponents` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AttachClone`
<!-- catalog:signal source="AttachClone" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `AttachClone` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `Detach`
<!-- catalog:signal source="Detach" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `Detach` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `RemoveChild`
<!-- catalog:signal source="RemoveChild" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveChild` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveChildren`
<!-- catalog:signal source="RemoveChildren" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveChildren` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveSubtree`
<!-- catalog:signal source="RemoveSubtree" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `RemoveSubtree` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `RegisterTransform`
<!-- catalog:signal source="RegisterTransform" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTransform` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `UpdateTransformWorld`
<!-- catalog:signal source="UpdateTransformWorld" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `UpdateTransformWorld` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `UpdateTransform`
<!-- catalog:signal source="UpdateTransform" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `UpdateTransform` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `SetTransformTrs`
<!-- catalog:signal source="SetTransformTrs" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Transfers one complete copied TRS value into a transform in explicitly selected local or world space. World-space values are converted through the target's effective parent basis when the intent executes; conversion failure leaves the target unchanged. MMS currently emits this through `transform.world.trs(value)`, while the local `transform.trs(value)` path retains the established `UpdateTransform` intent. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS method registry](../../../src/scripting/component_method_registry.rs).
```mms parse-only
let source = Transform {}
let target = Transform {}
let pose = source.world.trs()
target.world.trs(pose)
```

#### `RemoveTransform`
<!-- catalog:signal source="RemoveTransform" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveTransform` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTransformGizmo`
<!-- catalog:signal source="RegisterTransformGizmo" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTransformGizmo` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Rendering and assets

#### `SetColor`
<!-- catalog:signal source="SetColor" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `SetColor` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `SetEmissiveIntensity`
<!-- catalog:signal source="SetEmissiveIntensity" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `SetEmissiveIntensity` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `GLTFArmatureVisible`
<!-- catalog:signal source="GLTFArmatureVisible" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `GLTFArmatureVisible` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRenderable`
<!-- catalog:signal source="RegisterRenderable" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRenderable` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveRenderable`
<!-- catalog:signal source="RemoveRenderable" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveRenderable` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterStencilClip`
<!-- catalog:signal source="RegisterStencilClip" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterStencilClip` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `UnregisterStencilClip`
<!-- catalog:signal source="UnregisterStencilClip" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `UnregisterStencilClip` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterCamera3d`
<!-- catalog:signal source="RegisterCamera3d" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterCamera3d` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterCamera2d`
<!-- catalog:signal source="RegisterCamera2d" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterCamera2d` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `MakeActiveCamera`
<!-- catalog:signal source="MakeActiveCamera" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `MakeActiveCamera` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Camera3D {}
```

#### `RegisterUv`
<!-- catalog:signal source="RegisterUv" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterUv` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterLight`
<!-- catalog:signal source="RegisterLight" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterLight` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterColor`
<!-- catalog:signal source="RegisterColor" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterColor` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterOpacity`
<!-- catalog:signal source="RegisterOpacity" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterOpacity` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterBackgroundColor`
<!-- catalog:signal source="RegisterBackgroundColor" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterBackgroundColor` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRendererSettings`
<!-- catalog:signal source="RegisterRendererSettings" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRendererSettings` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRenderGraph`
<!-- catalog:signal source="RegisterRenderGraph" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRenderGraph` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAmbientLight`
<!-- catalog:signal source="RegisterAmbientLight" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAmbientLight` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterEmissive`
<!-- catalog:signal source="RegisterEmissive" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterEmissive` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterLightQuantization`
<!-- catalog:signal source="RegisterLightQuantization" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterLightQuantization` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterToonOutline`
<!-- catalog:signal source="RegisterToonOutline" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Registers a `ToonOutline` modifier and propagates its effective parameters to matching renderables and source-linked GLTF projections. User MMS authors the component rather than this enum variant. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
ToonOutline.width(0.012) {}
```

#### `RegisterGLTF`
<!-- catalog:signal source="RegisterGLTF" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterGLTF` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterNormalVis`
<!-- catalog:signal source="RegisterNormalVis" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterNormalVis` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Interaction and physics

#### `SelectionSet`
<!-- catalog:signal source="SelectionSet" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SelectionSet` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ToggleSet`
<!-- catalog:signal source="ToggleSet" kind="intent" mms="live-api" -->
**Intent — Available through engine UI synchronization.** Sets one or more `Toggle` values, updates their active highlight, and emits `ToggleChanged` only when the value changes.
```mms parse-only
Toggle.off()
```

#### `SliderSet`
<!-- catalog:signal source="SliderSet" kind="intent" mms="live-api" -->
**Intent — Available through live slider methods.** Normalizes a slider value, moves its thumb, and optionally emits `SliderChanged`. `set_value(...)` emits while `sync_value(...)` is silent.
```mms parse-only
let slider = Slider.range(0.0, 1.0)
slider.sync_value(0.5)
slider
```

#### `RegisterSlider`
<!-- catalog:signal source="RegisterSlider" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Creates the stable native track/thumb mounts, attaches authored visuals or defaults, and makes their renderables interactive.
```mms parse-only
Slider.range(0.0, 1.0)
```

#### `CollisionVisualizationSet`
<!-- catalog:signal source="CollisionVisualizationSet" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Adds, updates, or removes an EditorUI-owned collider visualization request.
```mms parse-only
EditorUI {}
```

#### `SpringBoneVisualizationSet`
<!-- catalog:signal source="SpringBoneVisualizationSet" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Adds, updates, or removes an EditorUI-owned spring-bone visualization request.
```mms parse-only
EditorUI {}
```

#### `RequestRaycast`
<!-- catalog:signal source="RequestRaycast" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `RequestRaycast` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `RegisterCollision`
<!-- catalog:signal source="RegisterCollision" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterCollision` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveCollision`
<!-- catalog:signal source="RemoveCollision" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveCollision` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterCollisionResponse`
<!-- catalog:signal source="RegisterCollisionResponse" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterCollisionResponse` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveCollisionResponse`
<!-- catalog:signal source="RemoveCollisionResponse" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveCollisionResponse` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRaycast`
<!-- catalog:signal source="RegisterRaycast" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRaycast` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRaycastable`
<!-- catalog:signal source="RegisterRaycastable" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRaycastable` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterPointer`
<!-- catalog:signal source="RegisterPointer" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterPointer` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveRaycast`
<!-- catalog:signal source="RemoveRaycast" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveRaycast` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveRaycastable`
<!-- catalog:signal source="RemoveRaycastable" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveRaycastable` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Text and layout

#### `SetText`
<!-- catalog:signal source="SetText" kind="intent" mms="action" -->
**Intent — Available through a live method/builtin.** Requests the `SetText` operation. It is emitted by a supported live method or engine code and consumed at a drain point. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Transform {}
target
```

#### `SetLayoutAvailableWidth`
<!-- catalog:signal source="SetLayoutAvailableWidth" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SetLayoutAvailableWidth` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `SetLayoutAvailableHeight`
<!-- catalog:signal source="SetLayoutAvailableHeight" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SetLayoutAvailableHeight` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `SetLayoutInspect`
<!-- catalog:signal source="SetLayoutInspect" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SetLayoutInspect` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterScrolling`
<!-- catalog:signal source="RegisterScrolling" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterScrolling` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTexture`
<!-- catalog:signal source="RegisterTexture" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTexture` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTextureFiltering`
<!-- catalog:signal source="RegisterTextureFiltering" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTextureFiltering` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterText`
<!-- catalog:signal source="RegisterText" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterText` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTextInput`
<!-- catalog:signal source="RegisterTextInput" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTextInput` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputSetFocus`
<!-- catalog:signal source="TextInputSetFocus" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputSetFocus` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputClearFocus`
<!-- catalog:signal source="TextInputClearFocus" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputClearFocus` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputInsertText`
<!-- catalog:signal source="TextInputInsertText" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputInsertText` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputBackspace`
<!-- catalog:signal source="TextInputBackspace" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputBackspace` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputDeleteForward`
<!-- catalog:signal source="TextInputDeleteForward" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputDeleteForward` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputMoveCaret`
<!-- catalog:signal source="TextInputMoveCaret" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputMoveCaret` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputMoveCaretTo`
<!-- catalog:signal source="TextInputMoveCaretTo" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputMoveCaretTo` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Animation, avatar, and poses

#### `InitializePoseCapture`
<!-- catalog:signal source="InitializePoseCapture" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `InitializePoseCapture` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `PoseCapture`
<!-- catalog:signal source="PoseCapture" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `PoseCapture` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `PoseApply`
<!-- catalog:signal source="PoseApply" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `PoseApply` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
PoseCapturePose.new("idle")
```

#### `PoseReset`
<!-- catalog:signal source="PoseReset" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `PoseReset` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAvatarControl`
<!-- catalog:signal source="RegisterAvatarControl" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAvatarControl` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAvatarBodyYaw`
<!-- catalog:signal source="RegisterAvatarBodyYaw" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAvatarBodyYaw` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterIkChain`
<!-- catalog:signal source="RegisterIkChain" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterIkChain` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterSecondaryMotion`
<!-- catalog:signal source="RegisterSecondaryMotion" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterSecondaryMotion` operation. `SecondaryMotion`, `SpringBone`, and `SpringJoint` creation and initialization emit this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting component and executes at an explicit drain point. The secondary-motion system retains root, child, joint, GLTF, and resolved-transform ownership so its frame tick performs no graph discovery. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [component lifecycle](../../../src/engine/ecs/component/secondary_motion.rs).
```mms parse-only
SecondaryMotion {}
```

#### `SecondaryMotionConfigurationChanged`
<!-- catalog:signal source="SecondaryMotionConfigurationChanged" kind="intent" mms="component-lifecycle" -->
**Intent — Emitted by engine/editor mutation paths.** Announces an in-place `SpringBone` or `SpringJoint` field edit. The mutation executor uses retained reverse ownership to rebind only the affected chain. Builder calls made before component initialization require no notification. Direct unsignaled mutation is outside the runtime contract. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs) and [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs).

#### `RegisterJointRetargetBasis`
<!-- catalog:signal source="RegisterJointRetargetBasis" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Registers an authored `JointRetargetBasis` source with the retained cache and resolves it only within its owning GLTF armature. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [component lifecycle](../../../src/engine/ecs/component/joint_retarget_basis.rs).

#### `RegisterHumanoidBoneMap`
<!-- catalog:signal source="RegisterHumanoidBoneMap" kind="intent" mms="component-lifecycle" -->

**Intent — Indirectly emitted by component lifecycle.** Registers and resolves a GLTF-owned
humanoid map without frame polling.

#### `UnregisterHumanoidBoneMap`
<!-- catalog:signal source="UnregisterHumanoidBoneMap" kind="intent" mms="component-lifecycle" -->

**Intent — Indirectly emitted by component lifecycle.** Removes the authored source and invalidates
its retained report.

#### `HumanoidBoneMapGltfInitialized`
<!-- catalog:signal source="HumanoidBoneMapGltfInitialized" kind="intent" mms="component-lifecycle" -->

**Intent — Indirectly emitted from `GltfInitialized`.** Re-resolves only the affected GLTF report.

#### `HumanoidBoneMapTopologyChanged`
<!-- catalog:signal source="HumanoidBoneMapTopologyChanged" kind="intent" mms="component-lifecycle" -->

**Intent — Indirectly emitted from relevant parent changes.** Refreshes a retained report for late
camera anchors or armature topology changes.

#### `RetargetBasisConfigurationChanged`
<!-- catalog:signal source="RetargetBasisConfigurationChanged" kind="intent" mms="component-lifecycle" -->
**Intent — Emitted by future engine/editor mutation paths.** Replaces a source definition atomically through the retained system. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs) and [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs).

#### `JointRetargetBasisGltfInitialized`
<!-- catalog:signal source="JointRetargetBasisGltfInitialized" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by GLTF lifecycle.** Resolves only retained basis sources waiting on the initialized GLTF. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs) and [retained runtime](../../../src/engine/ecs/system/joint_basis_retargeting_system.rs).

#### `UnregisterJointRetargetBasis`
<!-- catalog:signal source="UnregisterJointRetargetBasis" kind="intent" mms="component-lifecycle" -->
**Intent — Emitted by component teardown.** Removes a definition source and republishes a surviving non-conflicting definition for its target when possible. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs) and [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs).

#### `SecondaryMotionTopologyChanged`
<!-- catalog:signal source="SecondaryMotionTopologyChanged" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by topology lifecycle.** The global `ParentChanged` handler targets the affected retained root, chain, joint configuration, or imported transform. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs) and [retained runtime](../../../src/engine/ecs/system/secondary_motion_system.rs).

#### `SecondaryMotionGltfInitialized`
<!-- catalog:signal source="SecondaryMotionGltfInitialized" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by GLTF lifecycle.** Retries only roots retained under the initialized or respawned GLTF. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs) and [retained runtime](../../../src/engine/ecs/system/secondary_motion_system.rs).

#### `UnregisterSecondaryMotion`
<!-- catalog:signal source="UnregisterSecondaryMotion" kind="intent" mms="component-lifecycle" -->
**Intent — Emitted by component teardown.** Removes retained ownership and binding state for affected roots, chains, joint configurations, GLTFs, or imported transforms. The current subtree-removal coordinator also invokes the same cleanup directly. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs) and [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs).

#### `ResetSecondaryMotion`
<!-- catalog:signal source="ResetSecondaryMotion" kind="intent" mms="component-lifecycle" -->
**Intent — Emitted by explicit engine reset paths.** Rebinds the targeted chain, root, or GLTF-owned roots without enabling frame-time discovery. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs) and [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs).

#### `RegisterAnimation`
<!-- catalog:signal source="RegisterAnimation" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAnimation` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `SetAnimationState`
<!-- catalog:signal source="SetAnimationState" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SetAnimationState` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let animation = Animation {}
animation.play()
```

#### `RegisterKeyframe`
<!-- catalog:signal source="RegisterKeyframe" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterKeyframe` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Audio and timing

#### `AudioGraphRebuild`
<!-- catalog:signal source="AudioGraphRebuild" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `AudioGraphRebuild` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AudioLowPassSetCutoffHz`
<!-- catalog:signal source="AudioLowPassSetCutoffHz" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `AudioLowPassSetCutoffHz` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AudioBandPassSetCenterHz`
<!-- catalog:signal source="AudioBandPassSetCenterHz" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `AudioBandPassSetCenterHz` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `OscillatorSetEnabled`
<!-- catalog:signal source="OscillatorSetEnabled" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `OscillatorSetEnabled` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `OscillatorSetPitch`
<!-- catalog:signal source="OscillatorSetPitch" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `OscillatorSetPitch` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `OscillatorScheduleSetPitch`
<!-- catalog:signal source="OscillatorScheduleSetPitch" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `OscillatorScheduleSetPitch` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AudioSchedulePlay`
<!-- catalog:signal source="AudioSchedulePlay" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `AudioSchedulePlay` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
AudioOscillator.sin()
```

#### `RegisterAudioOutput`
<!-- catalog:signal source="RegisterAudioOutput" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAudioOutput` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AudioGraphDirtyImmediate`
<!-- catalog:signal source="AudioGraphDirtyImmediate" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `AudioGraphDirtyImmediate` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAudioOscillator`
<!-- catalog:signal source="RegisterAudioOscillator" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAudioOscillator` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAudioClip`
<!-- catalog:signal source="RegisterAudioClip" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAudioClip` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAudioBufferSize`
<!-- catalog:signal source="RegisterAudioBufferSize" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAudioBufferSize` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterClock`
<!-- catalog:signal source="RegisterClock" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterClock` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioOp`
<!-- catalog:signal source="ScheduleAudioOp" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioOp` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioGraphSwap`
<!-- catalog:signal source="ScheduleAudioGraphSwap" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioGraphSwap` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioPitchSetHz`
<!-- catalog:signal source="ScheduleAudioPitchSetHz" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioPitchSetHz` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioOscillatorEnabled`
<!-- catalog:signal source="ScheduleAudioOscillatorEnabled" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioOscillatorEnabled` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioGainSet`
<!-- catalog:signal source="ScheduleAudioGainSet" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioGainSet` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### XR

#### `RetryXrRuntime`
<!-- catalog:signal source="RetryXrRuntime" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `RetryXrRuntime` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterXr`
<!-- catalog:signal source="RegisterXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterInputXr`
<!-- catalog:signal source="RegisterInputXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterInputXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterControllerXr`
<!-- catalog:signal source="RegisterControllerXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterControllerXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterInputXrGamepad`
<!-- catalog:signal source="RegisterInputXrGamepad" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterInputXrGamepad` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveInputXr`
<!-- catalog:signal source="RemoveInputXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveInputXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveControllerXr`
<!-- catalog:signal source="RemoveControllerXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveControllerXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveInputXrGamepad`
<!-- catalog:signal source="RemoveInputXrGamepad" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveInputXrGamepad` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### HTTP

#### `HttpClientRequest`
<!-- catalog:signal source="HttpClientRequest" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `HttpClientRequest` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
HttpClient {}
```

#### `HttpServerReply`
<!-- catalog:signal source="HttpServerReply" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `HttpServerReply` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/signals/signal.rs), [intent interpretation](../../../src/engine/ecs/signals/intent_executor.rs), [mutation execution](../../../src/engine/ecs/signals/mutation_executor.rs), and [MMS component registry](../../../src/scripting/component_registry.rs).
```mms parse-only
HttpServer {}
```
