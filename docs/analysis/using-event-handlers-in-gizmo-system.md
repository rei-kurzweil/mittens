# Using event handlers in GizmoSystem (vs scanning `frame_events`)

## Why this doc exists

Right now, `TransformGizmoSystem::tick_with_queue` scans `rx.frame_events()` to find:

- `ParentChanged` (rebind gizmo target)
- `RayIntersected` (build a temporary lookup for ray direction)
- `DragStart` / `DragMove` / `DragEnd` (apply TRS updates)

See the three loops in [src/engine/ecs/system/gizmo_system.rs](../../src/engine/ecs/system/gizmo_system.rs#L606).

This doc explains what `frame_events` is doing for us, and how we could instead make GizmoSystem consume drag events via handlers (more in line with the drain-point architecture).

## What `rx.frame_events()` actually is

`RxWorld::frame_events` is a *per-frame log of events that were actually dispatched to handlers*.

- It is cleared in `RxWorld::begin_frame`.
- Every time `dispatch_event_handlers` runs for a ready event, it appends `env.clone()` into the log.

Implementation details:
- [src/engine/ecs/rx/rx_world.rs](../../src/engine/ecs/rx/rx_world.rs#L147-L189)

Important subtlety:
- `frame_events` records **dispatched** events.
- Events emitted *by handlers* go to `deferred_events` (next tick), not `ready_events`.

That “event→event is next tick” rule is enforced by the emitter used during handler dispatch:
- `Emitter::push_event` writes into `events_out`, which is wired to `RxWorld::deferred_events`.
- `RxWorld::begin_frame` moves `deferred_events` into `ready_events`.

See:
- [src/engine/ecs/rx/rx_world.rs](../../src/engine/ecs/rx/rx_world.rs#L14-L60)
- [src/engine/ecs/rx/rx_world.rs](../../src/engine/ecs/rx/rx_world.rs#L129-L146)
- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L133-L140)

## Why GizmoSystem can “see” Drag events today

Even though handlers can only emit events for the next tick, **GestureSystem doesn’t emit drag events from its RayIntersected handler**.

Instead, GestureSystem:

1. Installs a drain-point handler to *cache* the best `RayIntersected` hit (immediate-mode cache).
2. In its own `tick_with_rx`, converts cached hits + input into `DragStart`/`DragMove`/`DragEnd` by directly pushing events into `RxWorld`.

That makes drag events “ready events” for the *next* drain point (which happens immediately after GestureSystem ticks).

Relevant bits:
- GestureSystem caches ray hits via a handler in [src/engine/ecs/system/gesture_system.rs](../../src/engine/ecs/system/gesture_system.rs#L53-L95)
- The world tick order drains signals right after raycast, then right after gesture, then runs gizmos: [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L1148-L1187)

So by the time GizmoSystem runs, the drag events have already been dispatched and are present in `frame_events`.

## What scanning `frame_events` buys us

### Pros

- Zero handler installation for GizmoSystem.
- No extra persistent state for correlating events.
- Easy to correlate “earlier this frame” data like `RayIntersected` with later drag events.

### Cons

- It’s effectively an implicit “event bus replay” mechanism.
- Gizmo logic becomes order-dependent on what earlier drains happened in the same frame.
- It scales poorly if many systems start doing “scan the frame log” patterns.

## Can GizmoSystem just use handlers for Drag events?

Yes, conceptually.

Given the current tick order, the most natural handler-based approach is:

- GizmoSystem installs global handlers for `DragStart`, `DragMove`, `DragEnd` (and optionally `ParentChanged`).
- Those handlers run during the `process_signals` call **immediately after GestureSystem emits the drag events**.
- The handler can mutate components and emit intents (e.g. `IntentValue::UpdateTransform`) which will run later in the same drain loop.

This is consistent with the core semantics documented in `process_signals`:
- Intents emitted by event handlers run in the same tick.
- Events emitted by event handlers run next tick.

See [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L133-L140).

### Practical complication: handlers need to be installable without `&mut self`

`RxWorld` handler registration stores either:

- a function pointer (`SignalHandler`), or
- a `Send + Sync + 'static` closure

If GizmoSystem wants handlers without threading/`Arc<Mutex<...>>` overhead, prefer function pointers and keep all needed state inside the ECS (components), not inside the system struct.

That’s already mostly true:
- `TransformGizmoComponent` stores `active_raycaster` and `debug_drag_plane_root`.

So a handler can:
- resolve whether the drag target is a gizmo handle (`resolve_gizmo_op_for_renderable`)
- update `TransformGizmoComponent` state
- apply transforms by calling `TransformComponent::set_position/set_rotation_quat/set_scale`

No per-system captured state required.

### Practical complication: the debug plane needs the ray direction

Historically GizmoSystem built a lookup from `RayIntersected` to find the ray direction at drag start.

If we switch to handlers, we have three options:

1) Keep scanning `frame_events` only for `RayIntersected` correlation

- Handlers handle `Drag*`.
- A small bit of per-frame scanning remains to bridge missing context.

2) Extend `EventSignal::DragStart` to include ray information

We now do this: `DragStart` includes `ray_dir_world: [f32; 3]`.

That makes gizmo handling self-contained: a DragStart handler can spawn the debug plane
immediately without also observing `RayIntersected`.

This is likely the cleanest architecture because it makes drag events self-contained.

3) Cache ray direction inside a component

For example, store a “last pointer ray” on the raycaster entity each frame. Then DragStart handler can read it.

This increases state surface area and requires keeping that component updated.

## Suggested path (incremental, low-risk)

1. Add `TransformGizmoSystem::install_handlers(&mut self, rx: &mut RxWorld)` called from the same setup section that installs gesture/editor handlers.
2. Install global handlers for:
   - `SignalKind::DragStart`
   - `SignalKind::DragMove`
   - `SignalKind::DragEnd`
   - (optional) `SignalKind::ParentChanged`
3. Move the `Drag*` handling logic from `tick_with_queue` into handler functions.
4. Decide what to do about ray direction:
   - either keep the `frame_events` ray lookup temporarily, or
   - extend `EventSignal::DragStart` so the handler has the data it needs.

At the end of that path, `tick_with_queue` can likely shrink to either:

- nothing (purely event-driven gizmos), or
- only “non-event” per-frame maintenance.

## Notes on drain-point ordering

The reason this works at all is the explicit drains in the frame loop:

- raycast ticks → drain events (RayIntersected dispatched)
- gesture ticks → drain events (Drag* dispatched)
- gizmo ticks → drain intents/events

See [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L1148-L1187).

If we remove the Gizmo tick entirely, we should ensure there remains a drain+flush after gesture so gizmo-produced intents apply this frame. Today there is no flush immediately after gesture, but there *is* a later flush after the gizmo drain; so a no-op gizmo tick can still keep that structure intact.
