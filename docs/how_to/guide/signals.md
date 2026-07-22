
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

Implementation note:

- `ActionComponent` / `ActionSystem` may still appear in code as legacy plumbing reused by intent execution
- but the architectural model to document and build on is **intent vs event**, not **action vs event**

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
- Intent execution is in transition:
  - `RxIntentExecutor` exists and currently reuses some legacy `ActionSystem` interpretation logic for many high-level `IntentValue`s.
    - The default intent executor (`execute_intent_signal`) applies canonical side effects.

## MMS exposure labels and limitations

The catalogs below describe the implementation as it exists, not proposed syntax. Event labels are **observable with payload**, **observable with partial payload**, **observable with `null`**, or **unavailable**. Intent labels are **directly authorable through `Action`**, **available through a live method/builtin**, **indirectly emitted by component lifecycle**, or **engine-only**.

Three limitations are especially easy to miss:

- `LayoutRootSizeAvailable` exists in the engine but is not accepted by MMS `on(...)`.
- Many accepted event names dispatch correctly but `event_arg_value` converts their handler argument to `null`.
- Partial payloads intentionally omit engine fields: `DataEvent` omits its component payload; XR events omit `source_component`; and `TextInputChanged` omits `component_id`.

All examples are syntax-checked. `parse-only` examples for internal or unsupported signals demonstrate the closest real MMS relationship; they do not claim that MMS can construct an engine signal directly.

## Event catalog

### Runtime and routing

#### `FrameTick`
<!-- catalog:signal source="FrameTick" kind="event" mms="observable-payload" -->
**Event.** A rendered frame began; the payload reports elapsed seconds. The runtime and routing subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "FrameTick", fn(event) { print(event) })
scope
```

#### `DataEvent`
<!-- catalog:signal source="DataEvent" kind="event" mms="observable-partial-payload" -->
**Event.** User code emitted a named cross-subtree data event. The runtime and routing subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "DataEvent", fn(event) { print(event) })
scope
```

### Topology and transforms

#### `ParentChanged`
<!-- catalog:signal source="ParentChanged" kind="event" mms="observable-null" -->
**Event.** A component was attached, detached, or reparented. The topology and transforms subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "ParentChanged", fn(event) { print(event) })
scope
```

### Rendering and assets

#### `GltfInitialized`
<!-- catalog:signal source="GltfInitialized" kind="event" mms="observable-payload" -->
**Event.** A glTF asset finished spawning and its runtime nodes can be queried. The rendering and assets subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "GLTFInitialized", fn(event) { print(event) })
scope
```

### Interaction and physics

#### `RayIntersected`
<!-- catalog:signal source="RayIntersected" kind="event" mms="observable-null" -->
**Event.** A raycast hit a renderable. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "RayIntersected", fn(event) { print(event) })
scope
```

#### `CollisionStarted`
<!-- catalog:signal source="CollisionStarted" kind="event" mms="observable-null" -->
**Event.** Two collision objects began overlapping. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "CollisionStarted", fn(event) { print(event) })
scope
```

#### `CollisionEnded`
<!-- catalog:signal source="CollisionEnded" kind="event" mms="observable-null" -->
**Event.** Two collision objects stopped overlapping. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "CollisionEnded", fn(event) { print(event) })
scope
```

#### `DragStart`
<!-- catalog:signal source="DragStart" kind="event" mms="observable-null" -->
**Event.** A gesture crossed into the dragging state. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "DragStart", fn(event) { print(event) })
scope
```

#### `DragMove`
<!-- catalog:signal source="DragMove" kind="event" mms="observable-null" -->
**Event.** An active drag moved during the current tick. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "DragMove", fn(event) { print(event) })
scope
```

#### `DragEnd`
<!-- catalog:signal source="DragEnd" kind="event" mms="observable-null" -->
**Event.** An active drag ended. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "DragEnd", fn(event) { print(event) })
scope
```

#### `Click`
<!-- catalog:signal source="Click" kind="event" mms="observable-null" -->
**Event.** A drag ended within the click displacement threshold. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
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

#### `SelectionChanged`
<!-- catalog:signal source="SelectionChanged" kind="event" mms="observable-null" -->
**Event.** The complete state of a selection scope changed. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "SelectionChanged", fn(event) { print(event) })
scope
```

#### `SelectionAdded`
<!-- catalog:signal source="SelectionAdded" kind="event" mms="observable-null" -->
**Event.** An entry was added to a selection scope. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "SelectionAdded", fn(event) { print(event) })
scope
```

#### `SelectionRemoved`
<!-- catalog:signal source="SelectionRemoved" kind="event" mms="observable-null" -->
**Event.** An entry was removed from a selection scope. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "SelectionRemoved", fn(event) { print(event) })
scope
```

#### `SelectionCleared`
<!-- catalog:signal source="SelectionCleared" kind="event" mms="observable-null" -->
**Event.** All entries were removed from a selection scope. The interaction and physics subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "SelectionCleared", fn(event) { print(event) })
scope
```

### Text and layout

#### `Scrolling`
<!-- catalog:signal source="Scrolling" kind="event" mms="observable-null" -->
**Event.** A scrolling component consumed drag motion and changed offset. The text and layout subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "Scrolling", fn(event) { print(event) })
scope
```

#### `TextInputFocusChanged`
<!-- catalog:signal source="TextInputFocusChanged" kind="event" mms="observable-null" -->
**Event.** The focused text-input component changed. The text and layout subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with `null`**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "TextInputFocusChanged", fn(event) { print(event) })
scope
```

#### `TextInputChanged`
<!-- catalog:signal source="TextInputChanged" kind="event" mms="observable-partial-payload" -->
**Event.** Text or caret state changed in a text input. The text and layout subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "TextInputChanged", fn(event) { print(event) })
scope
```

#### `LayoutRootSizeAvailable`
<!-- catalog:signal source="LayoutRootSizeAvailable" kind="event" mms="unavailable" -->
**Event.** Layout completed and computed root dimensions became available. The text and layout subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` rejects this name. Constructing a layout root is the closest real relationship. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
LayoutRoot { Text { "content" } }
```

### XR

#### `XrButtonDown`
<!-- catalog:signal source="XrButtonDown" kind="event" mms="observable-partial-payload" -->
**Event.** An XR button crossed into the pressed state. The xr subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "XrButtonDown", fn(event) { print(event) })
scope
```

#### `XrButtonUp`
<!-- catalog:signal source="XrButtonUp" kind="event" mms="observable-partial-payload" -->
**Event.** An XR button crossed into the released state. The xr subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "XrButtonUp", fn(event) { print(event) })
scope
```

#### `XrButtonChanged`
<!-- catalog:signal source="XrButtonChanged" kind="event" mms="observable-partial-payload" -->
**Event.** An XR button analog value changed. The xr subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "XrButtonChanged", fn(event) { print(event) })
scope
```

#### `XrAxisChanged`
<!-- catalog:signal source="XrAxisChanged" kind="event" mms="observable-partial-payload" -->
**Event.** An XR two-axis control changed. The xr subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with partial payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "XrAxisChanged", fn(event) { print(event) })
scope
```

### HTTP

#### `HttpRequest`
<!-- catalog:signal source="HttpRequest" kind="event" mms="observable-payload" -->
**Event.** An enabled HTTP server accepted a request. The http subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "HttpRequest", fn(event) { print(event) })
scope
```

#### `HttpResponse`
<!-- catalog:signal source="HttpResponse" kind="event" mms="observable-payload" -->
**Event.** An HTTP client request completed successfully. The http subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "HttpResponse", fn(event) { print(event) })
scope
```

#### `HttpError`
<!-- catalog:signal source="HttpError" kind="event" mms="observable-payload" -->
**Event.** An HTTP client or server operation failed. The http subsystem produces it; scoped RX handlers consume it at a signal drain point after execution stages. Events are immediate observations, bubble from the signal scope to ancestor handler scopes, and do not carry `SignalWhen`. Related components are the producers or scopes named by the variant and its subsystem. MMS `on(...)` accepts this event. Handler exposure: **Observable with payload**. Sources: [event definition](../../../src/engine/ecs/rx/signal.rs), [MMS handler names](../../../src/scripting/world_evaluator.rs), and [payload conversion](../../../src/scripting/runner.rs).
```mms parse-only
let scope = Transform {}
on(scope, "HttpError", fn(event) { print(event) })
scope
```

## Intent catalog

### Runtime and routing

#### `Noop`
<!-- catalog:signal source="Noop" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `Noop` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Action.noop()
```

#### `SpawnComponentTree`
<!-- catalog:signal source="SpawnComponentTree" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `SpawnComponentTree` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `Print`
<!-- catalog:signal source="Print" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `Print` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Action.print("intent example")
```

#### `ReplExec`
<!-- catalog:signal source="ReplExec" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ReplExec` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRouter`
<!-- catalog:signal source="RegisterRouter" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRouter` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterHttpServer`
<!-- catalog:signal source="RegisterHttpServer" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterHttpServer` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterHttpClient`
<!-- catalog:signal source="RegisterHttpClient" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterHttpClient` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterInput`
<!-- catalog:signal source="RegisterInput" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterInput` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTransparentCutout`
<!-- catalog:signal source="RegisterTransparentCutout" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTransparentCutout` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterEditor`
<!-- catalog:signal source="RegisterEditor" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterEditor` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterEditorUI`
<!-- catalog:signal source="RegisterEditorUI" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Registers an authored `EditorUI` and materializes its configured shared panel workspace.
```mms parse-only
EditorUI {}
```

#### `RegisterAction`
<!-- catalog:signal source="RegisterAction" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAction` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterSignalRouteUpward`
<!-- catalog:signal source="RegisterSignalRouteUpward" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterSignalRouteUpward` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveSignalRouteUpward`
<!-- catalog:signal source="RemoveSignalRouteUpward" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveSignalRouteUpward` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Topology and transforms

#### `SetPosition`
<!-- catalog:signal source="SetPosition" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `SetPosition` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `LookAt`
<!-- catalog:signal source="LookAt" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `LookAt` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let node = Transform {}
node.look_at([0, 0, -1])
```

#### `Attach`
<!-- catalog:signal source="Attach" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `Attach` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `QueryFindComponent`
<!-- catalog:signal source="QueryFindComponent" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `QueryFindComponent` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `QueryFindAllComponents`
<!-- catalog:signal source="QueryFindAllComponents" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `QueryFindAllComponents` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AttachClone`
<!-- catalog:signal source="AttachClone" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `AttachClone` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `Detach`
<!-- catalog:signal source="Detach" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `Detach` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `RemoveChild`
<!-- catalog:signal source="RemoveChild" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveChild` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveChildren`
<!-- catalog:signal source="RemoveChildren" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveChildren` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveSubtree`
<!-- catalog:signal source="RemoveSubtree" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `RemoveSubtree` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `RegisterTransform`
<!-- catalog:signal source="RegisterTransform" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTransform` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `UpdateTransformWorld`
<!-- catalog:signal source="UpdateTransformWorld" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `UpdateTransformWorld` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `UpdateTransform`
<!-- catalog:signal source="UpdateTransform" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `UpdateTransform` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `RemoveTransform`
<!-- catalog:signal source="RemoveTransform" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveTransform` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTransformGizmo`
<!-- catalog:signal source="RegisterTransformGizmo" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTransformGizmo` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Rendering and assets

#### `SetColor`
<!-- catalog:signal source="SetColor" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `SetColor` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `SetEmissiveIntensity`
<!-- catalog:signal source="SetEmissiveIntensity" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `SetEmissiveIntensity` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `GLTFArmatureVisible`
<!-- catalog:signal source="GLTFArmatureVisible" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `GLTFArmatureVisible` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRenderable`
<!-- catalog:signal source="RegisterRenderable" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRenderable` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveRenderable`
<!-- catalog:signal source="RemoveRenderable" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveRenderable` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterStencilClip`
<!-- catalog:signal source="RegisterStencilClip" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterStencilClip` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `UnregisterStencilClip`
<!-- catalog:signal source="UnregisterStencilClip" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `UnregisterStencilClip` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterCamera3d`
<!-- catalog:signal source="RegisterCamera3d" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterCamera3d` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterCamera2d`
<!-- catalog:signal source="RegisterCamera2d" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterCamera2d` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `MakeActiveCamera`
<!-- catalog:signal source="MakeActiveCamera" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `MakeActiveCamera` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Camera3D {}
```

#### `RegisterUv`
<!-- catalog:signal source="RegisterUv" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterUv` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterLight`
<!-- catalog:signal source="RegisterLight" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterLight` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterColor`
<!-- catalog:signal source="RegisterColor" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterColor` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterOpacity`
<!-- catalog:signal source="RegisterOpacity" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterOpacity` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterBackgroundColor`
<!-- catalog:signal source="RegisterBackgroundColor" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterBackgroundColor` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRendererSettings`
<!-- catalog:signal source="RegisterRendererSettings" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRendererSettings` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRenderGraph`
<!-- catalog:signal source="RegisterRenderGraph" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRenderGraph` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAmbientLight`
<!-- catalog:signal source="RegisterAmbientLight" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAmbientLight` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterEmissive`
<!-- catalog:signal source="RegisterEmissive" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterEmissive` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterLightQuantization`
<!-- catalog:signal source="RegisterLightQuantization" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterLightQuantization` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterGLTF`
<!-- catalog:signal source="RegisterGLTF" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterGLTF` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterNormalVis`
<!-- catalog:signal source="RegisterNormalVis" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterNormalVis` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Interaction and physics

#### `SelectionSet`
<!-- catalog:signal source="SelectionSet" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SelectionSet` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ToggleSet`
<!-- catalog:signal source="ToggleSet" kind="intent" mms="live-api" -->
**Intent — Available through engine UI synchronization.** Sets one or more `Toggle` values, updates their active highlight, and emits `ToggleChanged` only when the value changes.
```mms parse-only
Toggle.off()
```

#### `CollisionVisualizationSet`
<!-- catalog:signal source="CollisionVisualizationSet" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Adds, updates, or removes an EditorUI-owned collider visualization request.
```mms parse-only
EditorUI {}
```

#### `RequestRaycast`
<!-- catalog:signal source="RequestRaycast" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `RequestRaycast` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `RegisterCollision`
<!-- catalog:signal source="RegisterCollision" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterCollision` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveCollision`
<!-- catalog:signal source="RemoveCollision" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveCollision` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterCollisionResponse`
<!-- catalog:signal source="RegisterCollisionResponse" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterCollisionResponse` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveCollisionResponse`
<!-- catalog:signal source="RemoveCollisionResponse" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveCollisionResponse` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRaycast`
<!-- catalog:signal source="RegisterRaycast" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRaycast` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterRaycastable`
<!-- catalog:signal source="RegisterRaycastable" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterRaycastable` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterPointer`
<!-- catalog:signal source="RegisterPointer" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterPointer` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveRaycast`
<!-- catalog:signal source="RemoveRaycast" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveRaycast` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveRaycastable`
<!-- catalog:signal source="RemoveRaycastable" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveRaycastable` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Text and layout

#### `SetText`
<!-- catalog:signal source="SetText" kind="intent" mms="action" -->
**Intent — Directly authorable through `Action`.** Requests the `SetText` operation. An `Action` constructor authors this intent; the action system resolves targets and the intent executor performs it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let target = Transform {}
Action.noop()
target
```

#### `SetLayoutAvailableWidth`
<!-- catalog:signal source="SetLayoutAvailableWidth" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SetLayoutAvailableWidth` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `SetLayoutAvailableHeight`
<!-- catalog:signal source="SetLayoutAvailableHeight" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SetLayoutAvailableHeight` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `SetLayoutInspect`
<!-- catalog:signal source="SetLayoutInspect" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SetLayoutInspect` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterScrolling`
<!-- catalog:signal source="RegisterScrolling" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterScrolling` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTexture`
<!-- catalog:signal source="RegisterTexture" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTexture` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTextureFiltering`
<!-- catalog:signal source="RegisterTextureFiltering" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTextureFiltering` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterText`
<!-- catalog:signal source="RegisterText" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterText` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterTextInput`
<!-- catalog:signal source="RegisterTextInput" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterTextInput` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputSetFocus`
<!-- catalog:signal source="TextInputSetFocus" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputSetFocus` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputClearFocus`
<!-- catalog:signal source="TextInputClearFocus" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputClearFocus` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputInsertText`
<!-- catalog:signal source="TextInputInsertText" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputInsertText` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputBackspace`
<!-- catalog:signal source="TextInputBackspace" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputBackspace` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputDeleteForward`
<!-- catalog:signal source="TextInputDeleteForward" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputDeleteForward` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputMoveCaret`
<!-- catalog:signal source="TextInputMoveCaret" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputMoveCaret` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `TextInputMoveCaretTo`
<!-- catalog:signal source="TextInputMoveCaretTo" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `TextInputMoveCaretTo` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Animation, avatar, and poses

#### `InitializePoseCapture`
<!-- catalog:signal source="InitializePoseCapture" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `InitializePoseCapture` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `PoseCapture`
<!-- catalog:signal source="PoseCapture" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `PoseCapture` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `PoseApply`
<!-- catalog:signal source="PoseApply" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `PoseApply` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
PoseCapturePose.new("idle")
```

#### `PoseReset`
<!-- catalog:signal source="PoseReset" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `PoseReset` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAvatarControl`
<!-- catalog:signal source="RegisterAvatarControl" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAvatarControl` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAvatarBodyYaw`
<!-- catalog:signal source="RegisterAvatarBodyYaw" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAvatarBodyYaw` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterIkChain`
<!-- catalog:signal source="RegisterIkChain" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterIkChain` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterSecondaryMotion`
<!-- catalog:signal source="RegisterSecondaryMotion" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterSecondaryMotion` operation. `SecondaryMotion`, `SpringBone`, and `SpringJoint` creation and initialization emit this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting component and executes at an explicit drain point. The secondary-motion system retains root, child, joint, GLTF, and resolved-transform ownership so its frame tick performs no graph discovery. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [component lifecycle](../../../src/engine/ecs/component/secondary_motion.rs).
```mms parse-only
SecondaryMotion {}
```

#### `SecondaryMotionConfigurationChanged`
<!-- catalog:signal source="SecondaryMotionConfigurationChanged" kind="intent" mms="component-lifecycle" -->
**Intent — Emitted by engine/editor mutation paths.** Announces an in-place `SpringBone` or `SpringJoint` field edit. The mutation executor uses retained reverse ownership to rebind only the affected chain. Builder calls made before component initialization require no notification. Direct unsignaled mutation is outside the runtime contract. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs) and [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs).

#### `SecondaryMotionTopologyChanged`
<!-- catalog:signal source="SecondaryMotionTopologyChanged" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by topology lifecycle.** The global `ParentChanged` handler targets the affected retained root, chain, joint configuration, or imported transform. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs) and [retained runtime](../../../src/engine/ecs/system/secondary_motion_system.rs).

#### `SecondaryMotionGltfInitialized`
<!-- catalog:signal source="SecondaryMotionGltfInitialized" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by GLTF lifecycle.** Retries only roots retained under the initialized or respawned GLTF. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs) and [retained runtime](../../../src/engine/ecs/system/secondary_motion_system.rs).

#### `UnregisterSecondaryMotion`
<!-- catalog:signal source="UnregisterSecondaryMotion" kind="intent" mms="component-lifecycle" -->
**Intent — Emitted by component teardown.** Removes retained ownership and binding state for affected roots, chains, joint configurations, GLTFs, or imported transforms. The current subtree-removal coordinator also invokes the same cleanup directly. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs) and [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs).

#### `ResetSecondaryMotion`
<!-- catalog:signal source="ResetSecondaryMotion" kind="intent" mms="component-lifecycle" -->
**Intent — Emitted by explicit engine reset paths.** Rebinds the targeted chain, root, or GLTF-owned roots without enabling frame-time discovery. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs) and [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs).

#### `RegisterAnimation`
<!-- catalog:signal source="RegisterAnimation" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAnimation` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `SetAnimationState`
<!-- catalog:signal source="SetAnimationState" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `SetAnimationState` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
let animation = Animation {}
animation.play()
```

#### `RegisterKeyframe`
<!-- catalog:signal source="RegisterKeyframe" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterKeyframe` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### Audio and timing

#### `AudioGraphRebuild`
<!-- catalog:signal source="AudioGraphRebuild" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `AudioGraphRebuild` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AudioLowPassSetCutoffHz`
<!-- catalog:signal source="AudioLowPassSetCutoffHz" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `AudioLowPassSetCutoffHz` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AudioBandPassSetCenterHz`
<!-- catalog:signal source="AudioBandPassSetCenterHz" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `AudioBandPassSetCenterHz` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `OscillatorSetEnabled`
<!-- catalog:signal source="OscillatorSetEnabled" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `OscillatorSetEnabled` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `OscillatorSetPitch`
<!-- catalog:signal source="OscillatorSetPitch" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `OscillatorSetPitch` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `OscillatorScheduleSetPitch`
<!-- catalog:signal source="OscillatorScheduleSetPitch" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `OscillatorScheduleSetPitch` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AudioSchedulePlay`
<!-- catalog:signal source="AudioSchedulePlay" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `AudioSchedulePlay` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
AudioOscillator.sin()
```

#### `RegisterAudioOutput`
<!-- catalog:signal source="RegisterAudioOutput" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAudioOutput` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `AudioGraphDirtyImmediate`
<!-- catalog:signal source="AudioGraphDirtyImmediate" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `AudioGraphDirtyImmediate` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAudioOscillator`
<!-- catalog:signal source="RegisterAudioOscillator" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAudioOscillator` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAudioClip`
<!-- catalog:signal source="RegisterAudioClip" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAudioClip` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterAudioBufferSize`
<!-- catalog:signal source="RegisterAudioBufferSize" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterAudioBufferSize` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterClock`
<!-- catalog:signal source="RegisterClock" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterClock` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioOp`
<!-- catalog:signal source="ScheduleAudioOp" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioOp` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioGraphSwap`
<!-- catalog:signal source="ScheduleAudioGraphSwap" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioGraphSwap` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioPitchSetHz`
<!-- catalog:signal source="ScheduleAudioPitchSetHz" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioPitchSetHz` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioOscillatorEnabled`
<!-- catalog:signal source="ScheduleAudioOscillatorEnabled" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioOscillatorEnabled` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `ScheduleAudioGainSet`
<!-- catalog:signal source="ScheduleAudioGainSet" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `ScheduleAudioGainSet` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### XR

#### `RetryXrRuntime`
<!-- catalog:signal source="RetryXrRuntime" kind="intent" mms="engine-only" -->
**Intent — Engine-only.** Requests the `RetryXrRuntime` operation. Only engine code emits this internal operation; MMS has no direct constructor, method, or builtin for it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterXr`
<!-- catalog:signal source="RegisterXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterInputXr`
<!-- catalog:signal source="RegisterInputXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterInputXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterControllerXr`
<!-- catalog:signal source="RegisterControllerXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterControllerXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RegisterInputXrGamepad`
<!-- catalog:signal source="RegisterInputXrGamepad" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RegisterInputXrGamepad` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveInputXr`
<!-- catalog:signal source="RemoveInputXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveInputXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveControllerXr`
<!-- catalog:signal source="RemoveControllerXr" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveControllerXr` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

#### `RemoveInputXrGamepad`
<!-- catalog:signal source="RemoveInputXrGamepad" kind="intent" mms="component-lifecycle" -->
**Intent — Indirectly emitted by component lifecycle.** Requests the `RemoveInputXrGamepad` operation. Component creation, initialization, teardown, or topology work emits this intent indirectly; user MMS does not author the enum variant. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transform {}
```

### HTTP

#### `HttpClientRequest`
<!-- catalog:signal source="HttpClientRequest" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `HttpClientRequest` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
HttpClient {}
```

#### `HttpServerReply`
<!-- catalog:signal source="HttpServerReply" kind="intent" mms="live-api" -->
**Intent — Available through a live method/builtin.** Requests the `HttpServerReply` operation. A live component method or evaluator builtin requests this intent; the RX/default executor or owning system consumes it. It is scoped to the requesting/affected component and executes at an explicit drain point; `AtBeat` delays eligibility when the producer supplies timed metadata. Related components and systems are the targets named by the variant; see executor matching for exact effects. Sources: [intent definition](../../../src/engine/ecs/rx/signal.rs), [intent interpretation](../../../src/engine/ecs/rx/intent_executor.rs), [mutation execution](../../../src/engine/ecs/rx/mutation_executor.rs), and [MMS action registry](../../../src/scripting/component_registry.rs).
```mms parse-only
HttpServer {}
```
