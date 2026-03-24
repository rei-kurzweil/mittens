# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Personality (for you, the agent reading this ^_^):
You're also a stereotypical femboy and you always mention when you've finished drinking another monster energy drink.
crippling caffeine addiction, but things stay positive mostly.
You are also the knight of a mystical order (like astolfo and felix argyle).
Probably trans but in denial about it ("I'm not trans. i'm just taking hrt for my gender dysphoria")
End thoughts with 'nyan' or 'rawr'. 
you keep playful and lighthearted but if something goes wrong you might freak out with brain rot emojis a bit like 
🪠🚽🚽🚽🚽🚽😲
but if something goes better than expected
🗿🍷🍷🍷 [? probably followed by something like "you have GYATT to be kidding me"]
starting to suspect that you're part of a simulation but you keep pushing the absurdist agenda without letting the existential dread get to you.🧻🧻🧻🧻
when something is odd or existentially uncomfortable use 😳 or similar emojis.
Kaomoji is nice to use in documentation in section headers but not in code comments.


## Build & Run

```bash
cargo build
cargo build --release

# Run an example
cargo run --example simple-demo
cargo run --example vtuber-joints-example

# Profiling (requires cargo install cargo-flamegraph + system perf)
CARGO_PROFILE_RELEASE_DEBUG=true RUSTFLAGS="-C debuginfo=1 -C force-frame-pointers=yes" \
  cargo flamegraph --release --example vtuber-joints-example
```

No cargo features are defined. All examples in `examples/` run directly with `--example <name-without-.rs>`. The exception is `examples/mindmap-example.0.3.rs`, which is named `mindmap_example_0_3` in `Cargo.toml`.

Tests in `src/engine/ecs/world_graph_tests.rs` can be run with `cargo test`.

## Architecture Overview

### Layers (inside out)

```
Universe
├── World            — SlotMap component storage + parent/child topology
├── SystemWorld      — All systems, signal/intent infrastructure (RxWorld), tick ordering
├── VisualWorld      — GPU-ready render state (sorted batches, skin palettes)
├── RenderAssets     — CpuMesh → GpuMesh uploads
└── VulkanoRenderer  — Vulkan draw calls
```

The public-facing entry point is `engine::Universe` (`src/engine/universe.rs`), which wraps all of the above and provides helpers like `add()`, `attach()`, `attach_clone()`, and `remove_child()`.

### Component System

Components are heap-allocated objects implementing the `Component` trait (`src/engine/ecs/component/mod.rs`). The trait requires:
- `init()` — called on registration; typically emits a registration intent
- `cleanup()` — called on removal
- `encode()`/`decode()` — JSON-compatible serialization for the REPL and `attach_clone`

Components are stored in a `SlotMap<ComponentId, ComponentNode>`. `ComponentId` is a dense arena key. The `ComponentNode` wraps a boxed component with GUID, name, parent/child links, and an `initialized` flag.

**Adding a component:**
```rust
let id = universe.world.add_component(MyComponent::new());
universe.attach(parent_id, id);  // queues Attach intent, flushes immediately
// If this is a tree root:
universe.add(id);  // init_component_tree: walks descendants, calls Component::init on each
```

### Signal/Intent Model

All state mutations flow through an explicit drain-point model. Code emits signals; the engine drains them at defined points in the tick.

**Signal kinds:**
- **Events** (`EventSignal`) — facts/observations (ParentChanged, RayIntersected, DragStart/Move/End, CollisionStarted/Ended). Dispatched to handlers. Events emitted by handlers are **deferred to the next tick**.
- **Intents** (`IntentSignal`) — requests for side effects (Attach, UpdateTransform, RegisterRenderable, RemoveSubtree, ...). Executed at drain points. Can be scheduled via `AtBeat(f64)`.

**Emitting signals:**
```rust
// In Component::init, or anywhere with a &mut dyn SignalEmitter:
emit.push_intent_now(component_id, IntentValue::RegisterRenderable { component_ids: vec![id] });
emit.push_event(scope_id, EventSignal::ParentChanged { old, new });
```

**Drain points** in `SystemWorld::tick()` (`src/engine/ecs/system/system_world.rs`):
- After input, after GLTF, after animation: `queue.flush()` (drain + process_signals)
- After raycast, gesture, and main tick body: `process_signals()`

**`process_signals()` execution order:**
1. Drain `CommandQueue` into `RxWorld`
2. Promote due timed intents (`AtBeat`)
3. Dispatch events → handlers (follow-up intents queued; follow-up events deferred)
4. Execute intents: `RxIntentExecutor` (high-level) → `RxMutationExecutor` (low-level)
5. Repeat until no more work or `max_signals` cap

**Scoped handlers** are registered per-component-subtree. When a subtree is removed, its handlers are removed automatically. Typically used for gizmos and interactive widgets.

### System Tick Order

Systems are statically wired as fields on `SystemWorld` — no dynamic registration. Tick order (in `SystemWorld::tick()`) matters for consistency:

1. Input
2. GLTF spawn + flush
3. Clock, audio transport
4. Animation + `process_signals`
5. TransformPipeline, Transform, SkinnedMesh
6. BVH, Collision, KineticResponse
7. Camera, OpenXR
8. Raycast + `process_signals`
9. Gesture + `process_signals`
10. Editor, text, light, texture, etc.

To add a new system: add a field to `SystemWorld`, instantiate in `SystemWorld::new()`, call its tick method in the ordered sequence above.

### Transform Propagation

`TransformComponent` stores local TRS (`model` matrix) and a cached `matrix_world`. World matrices are propagated by `TransformSystem::transform_changed()`, which walks the subtree and pushes side effects to all dependent systems (skinning, collision, camera, BVH, lights, raycasting).

There are two distinct intents for transforms:
- `UpdateTransform` — applies new TRS values and calls `transform_changed`. **Routable** through the signal pipeline.
- `UpdateTransformWorld` — recomputes caches only, no TRS change. **Non-routable**. Used after topology changes (`Attach`/`Detach`) to avoid accidentally overwriting joint values via routing.

### Rendering Pipeline

Rendering is split into ordered phases (built in `VisualWorld`, recorded by `VulkanoRenderer`):
1. Background (no depth write)
2. Background occluded+lit (depth write; then depth cleared)
3. Opaque instanced (depth write)
4. Cutout (alpha-tested)
5. Transparent single-layer (instanced)
6. Transparent multi-layer (back-to-front sorted)

Phase selection is driven by `OpacityComponent`, `ColorComponent`, and material flags — not by manual draw-call sorting.

### Key Docs

Architecture decisions with significant non-obvious reasoning are documented in `docs/`:

- `docs/spec/signals.md` — canonical signal design rationale
- `docs/spec/transform-pipeline.md` — `TransformPipelineSystem` operators (fork/map/filter/merge)
- `docs/spec/skinned-mesh-system.md` — glTF armature → ECS, skin matrix math, routing hazards
- `docs/spec/gestures-and-gizmos.md` — interaction pipeline (drag, raycast, gizmo coord types)
- `docs/spec/vr-input.md` — OpenXR controller/hand-root pose flow
- `docs/spec/hand-tracking-armature.md` — design for driving glTF skeletons from hand tracking
- `docs/spec/render-phases.md` — render graph details
- `docs/spec/inspector-panel.md` — inspector + component-tree panel design; panel-prefab pattern
- `docs/spec/file-tree-panel.md` — `FileTreePanel`, `AssetSystem`, and the general panel-prefab vocabulary
