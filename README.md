
# mittens 0.6.0
<img width="1920" height="745" alt="Screenshot_20260303_015535" src="https://github.com/user-attachments/assets/16d9656c-9df3-4a96-89bd-658d222e78d0" />

A hypermedia / web development inspired game engine specially made for social vr, vtubing, visual novels, css UI / spatial layout, and 3D character animation.


### Workspace crates

- `mittens-engine` 0.6.0 at the workspace root: Vulkan/OpenXR rendering, ECS,
  engine component materialization, and the Mittens-specific scripting host.
- `meow-meow-script` 0.6.0 in `crates/meow-meow-script`: host-neutral syntax,
  parser, runtime/session evaluator, configurable component/API catalogs, and
  the generic host protocol.
- `mittens-query` 0.6.0 in `crates/mittens-query`: CSS/MMQ parsing and
  host-neutral query-tree evaluation.

The dependency graph is acyclic: `mittens-engine` depends on both standalone
crates; neither standalone crate depends on the engine.

### Scripting split

`meow-meow-script` does not know about Mittens components. It provides the
language runtime, typed catalog declarations, stateful sessions, host-boundary
DTOs, callbacks, and the generic `Host` request/response contract.

`mittens-engine` embeds that runtime by registering the Mittens catalog: engine
component names, aliases, constructors, builder calls, properties, component
methods, and host APIs. Its Mittens host maps script component handles to ECS
components and implements the actual effects for emission, registration,
attachment, queries, methods, signals, and callbacks.

That split lets other hosts reuse `meow-meow-script` with their own component
catalogs and semantics, while Mittens keeps its engine-specific behavior in the
engine crate.


(see docs/meow_meow for an overview of .mms scripts)

## Running examples

- Run examples in release mode by default: `cargo run --release --example <name>`.
- Avoid debug example runs unless you specifically need debug-only diagnostics or faster compile iteration.
- Large `.glb` model assets are omitted from the crates.io package. Every example
  calls `mittens_engine::example_support::ensure_model_assets()` before scene
  setup; if `assets/models` contains no `.glb` files, it runs
  `scripts/download-model-assets.sh` to fetch the example model bundle from
  GitHub. Run the script manually to prefetch models. Set
  `MITTENS_MODEL_ASSET_BASE_URL` to override the download source.

## Windowing
+ uses winit to make a window and passes the RawDisplayHandle to renderer to render into the window
+ provides user input events and frame loop

## Universe
+ holds all the layers below,
+ and provides simple API to build component trees and add them to the world

### Universe API (common helpers)

The `engine::Universe` type is a convenience wrapper around `World + SystemWorld + VisualWorld + CommandQueue`.

In addition to `add(...)` and `attach(parent, child)`, it provides a few higher-level helpers for
prefab-style workflows and safe subtree removal:

- `attach_clone(parent, prefab_root) -> Result<ComponentId, String>`
  - Clones the component subtree rooted at `prefab_root` (fresh `ComponentId`s and fresh GUIDs) and attaches it under `parent`.
  - Clone is done via component `encode`/`decode` using `ComponentCodec` (no JSON round-trip).
  - Note: if any components contain references to other components (e.g. action targets stored inside component payloads), those references are currently copied as-is and may need a future fixup pass.

- `remove_child(parent, index) -> Result<ComponentId, &'static str>`
  - Detaches the child immediately and queues deletion of that child subtree via the command queue.
  - Deletion is applied when the command queue is processed (after systems tick), so systems/visuals can cleanly unregister.

- `remove_children(parent) -> Result<Vec<ComponentId>, &'static str>`
  - Detaches all direct children and queues deletion of each child subtree (applied on command processing).

Example (prefab clone):

```rust
use mittens_engine::engine;

let prefab_root: engine::ecs::ComponentId = /* detached prefab subtree root */;
let parent: engine::ecs::ComponentId = /* some TransformComponent in the live scene */;

let instance_root = universe.attach_clone(parent, prefab_root)?;
// GUID is stored on the component record:
let guid = universe.world.get_component_record(instance_root).unwrap().guid;
```

## (component) World
+ stores list of components and topology (parent / child relationship between components)
+ components can have subcomponents
+ specific types of components register with SystemWorld and have methods that also call SystemWorld
+ registration / removal and methods of components that affect SystemWorld go through a CommandQueue and get applied after systems.tick() in the update loop.

## SystemWorld
+ handles the behaviors of components
+ can have one system's method invoked and then defer to one or more other systems
+ can call methods on components (via CommandQueue)
    + calls to component methods are applied after all systems have run their tick() method.

#### RenderableSystem
+ keeps a queue of CPUMesh from RenderableComponent that need to be converted to GpuMesh and uploaded into the GPU.

## VisualWorld
+ stores a snapshot of GpuRenderables
+ and builds cache, sorted by material pipeline, mesh, and texture
  + when ever RenderableSystem or LightSystem (or TransformSystem if involving renderables, lights or cameras) updates. 
  
#### RenderAssets
+ converts `CPUMesh` into `GPUMesh`

## VulkanoRenderer 
+ displays data from VisualWorld through vulkan
+ TODO: make WgpuRenderer for web / webasm

### Render phases (render graph summary)

Rendering is recorded in a single dynamic-rendering scope, but split into explicit phases (a small “render graph”) built by `VisualWorld` and recorded by `VulkanoRenderer`.

Current phase order (high level):

1. **Background** (instanced, no depth write)
2. **Background occluded+lit** (instanced, depth write ON for self-occlusion)
   - Then the renderer clears depth so background never occludes the foreground.
3. **Opaque** (instanced, depth write ON)
4. **Cutout** (instanced, alpha-tested)
5. **Transparent single-layer** (instanced)
6. **Transparent multi-layer** (sorted back-to-front, drawn one-by-one for correct blending)

See [docs/render-phases.md](docs/render-phases.md) for details and the relevant code entry points.

# Components

See the [MMS component guide](docs/how_to/guide/components.md) for the exhaustive component catalog and current scripting support.

## Transforms

Transforms are central in Mittens: most component subtrees are rooted at a `TransformComponent`, and many engine systems interpret the component tree as “a scene graph of nested transforms + things attached under them”.

0. **Brief intro: `TransformComponent`**
  - Stores local TRS (translation / rotation / scale) and a cached `matrix_world`.
  - Local TRS is represented as a *model matrix* (`transform.model`), and the engine propagates it through the component tree to compute `matrix_world` for nested transforms.

1. **How transforms can be nested**
  - A `TransformComponent` can parent other `TransformComponent`s.
  - Nesting is defined by the ECS topology (parent/child relationships in the component tree).

2. **What that means for model vs `matrix_world` propagation**
  - Each transform has a local `model` matrix derived from its TRS.
  - World-space transforms are computed by multiplying ancestor models down the tree:
    - `matrix_world(child) = matrix_world(parent) * model(child)`
  - `TransformSystem` caches `matrix_world` on each `TransformComponent` and uses it as the source of truth for systems that need world-space.
  - Topology changes (Attach/Detach) can require recomputation even if local TRS didn’t change; the engine has a dedicated intent for that (`UpdateTransformWorld`).

3. **Which systems are affected by transforms**
  - `RenderableSystem` / `VisualWorld`: instance model matrices for renderables
  - `CameraSystem`: camera view/projection updates when parent transform changes
  - `LightSystem`: point light world-space position updates
  - `CollisionSystem`: collider world-space updates
  - `SkinnedMeshSystem`: joint world matrices / skinning matrices become dirty
  - `BvhSystem` + `RaycastSystem`: BVH refit and raycast correctness depends on world matrices
  - `OpenXRSystem`: XR devices/cameras often read/write world transforms
  - Editor gizmos: visual alignment + drag application depend on consistent `matrix_world`

4. **Which systems determine / write transform intents**
  - User code: calling `TransformComponent::{set_position,set_rotation_*,set_scale}` queues `UpdateTransform`
  - `InputSystem`: movement/controls update transforms via `UpdateTransform`
  - `OpenXRSystem`: device pose application uses `UpdateTransform`
  - `CollisionResponseSystem`: kinematic collision response integrates motion via `UpdateTransform`
  - `TransformGizmoSystem`: editor gestures call transform setters (which queue `UpdateTransform`)

4.1 **Transform propagation pipelines / transform operators**
  - `TransformSystem::transform_changed(...)` is the core propagation pipeline: it recomputes cached `matrix_world` for a transform subtree and pushes side effects to dependent systems.
  - `TransformFilterComponent` is a “filter-as-node” operator that changes what descendants inherit (e.g. inherit translation+rotation but *not* scale). It’s used heavily for editor/gizmo visuals.
  - For deeper notes/specs:
    - `TransformFilterComponent` motivation: `docs/analysis/gizmo-transform-propagation.md`
    - Gizmo coord spaces (Local/World): `docs/spec/editor-gizmo-coord-spaces.md`
    - Transform update flow and refit/rebuild behavior: `docs/analysis/refresh-transform.md`


# Signals

This engine uses an explicit **drain-point** signal model.

Instead of letting systems mutate everything immediately (and in arbitrary order), code emits **signals** into the per-frame queue, and the engine drains them in a consistent sequence.

Signal types:

- **Events** (`EventSignal`): facts/observations ("parent changed", "drag started", ...).
  - Dispatched to handlers (global handlers and/or scoped handlers rooted at a scope subtree).
  - Event handlers should be *observers*: they typically emit follow-up intents rather than directly performing large mutations.

- **Intents** (`IntentSignal` carrying an `IntentValue`): requests for side effects ("attach", "set transform", "remove subtree", ...).
  - Executed at drain points.
  - Can be scheduled for the future via `at_beat(...)` (timed intents sit in a holding-pen until due).

Execution order (inside `SystemWorld::process_signals`):

1. **Dispatch ready events** to handlers.
   - Any *events emitted by handlers* are **deferred to the next tick**.
   - Any *intents emitted by handlers* are queued for later execution.

2. **Promote timed intents** that have become due at the current beat.

3. **Execute ready intents**.
   - Intents may emit more intents; those will run later in the same tick (after queue draining), up to the `max_signals` cap.
   - Intents may also emit events, but (like handler-emitted events) those are deferred to the next tick.

Implementation detail: intent execution is split into two layers:

- `RxIntentExecutor`: “interpretation” intents that expand into follow-up work.
- `RxMutationExecutor`: low-level canonical mutations (register/update/remove, etc.).

Scoped handler lifecycle: systems can install handlers rooted at a component subtree (e.g. gizmos). When a subtree is removed, any scoped handlers rooted in that deleted subtree are removed automatically.

See the [MMS signal guide](docs/how_to/guide/signals.md) for the architecture, exhaustive signal catalog, and current scripting support.


# Building Widgets (Panels & Tools)

Complex editor UI (like the `inspector_panel`, `paint_panel`, or `world_panel`) follows a data-driven projection pattern.

### 1. State & Reducers
Panels define their own domain-specific state and a pure reducer function to handle events.
- **State**: e.g., `InspectorWorkspaceState` or `PaintState`.
- **Reducer**: `fn reduce_state(old: &State, event: &Event) -> State`.

### 2. Event Adapters
Raw engine events (clicks, drags, signal emissions) are converted into high-level domain events by "adapters" (often scoped signal handlers installed at the widget root).

### 3. Data Renderer System
The `DataRendererSystem` manages the lifecycle of projecting a list of data items into a live component subtree. It ensures that when data changes, the previous visual subtree is cleaned up and a fresh one is attached to the target slot.

### 4. RendererSpec
You define how each item in your data list should be rendered using a `RendererSpec<T>`:

- **RendererSpec::Mms**: Project data into an MMS component factory.
  ```rust
  RendererSpec::Mms {
      asset_path: "assets/components/item.mms",
      export_name: "my_item",
      to_args: |data| vec![Value::String(data.label.clone())],
  }
  ```
- **RendererSpec::Rust**: Build the component tree directly in Rust.
  ```rust
  RendererSpec::Rust {
      render_fn: Box::new(|world, emit, data| {
          let root = world.add_component(...);
          // ... build tree ...
          Ok(root)
      }),
  }
  ```

## Working with MMS Components

For simpler widgets or reusable UI elements, you can define factory functions in `.mms` scripts.

### Calling component methods from animation keyframes

Keyframe blocks run in the live world when the keyframe becomes due, so they can call methods on captured component handles directly.

```javascript
Clock.bpm(60) {}

let cube_t = T.position(0.0, 0.0, 0.0).scale(0.5, 0.5, 0.5) {
    Transition {
        duration_beats(0.85)
        ease_in_out_sine()
        replace_same_target()
    }
    R.cube() {
        C.rgba(0.90, 0.75, 0.30, 1.0)
    }
}

cube_t

Animation.looping() {
    Keyframe.at(0) {
        cube_t.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.5, 0.5, 0.5])
    }
    Keyframe.at(1) {
        cube_t.update_transform([0.0, 0.0, 0.0], [0.0, 3.14159 / 2, 0.0], [0.5, 0.5, 0.5])
    }
    Keyframe.at(2) {
        cube_t.update_transform([0.0, 0.0, 0.0], [0.0, 3.14159, 0.0], [0.5, 0.5, 0.5])
    }
    Keyframe.at(3) {
        cube_t.update_transform([0.0, 0.0, 0.0], [0.0, 3.14159 * 1.5, 0.0], [0.5, 0.5, 0.5])
    }
}
```

This is the simplest pattern for authoring animation-driven behavior in MMS: capture a live component handle with `let`, then mutate it from `Keyframe.at(...)` blocks.

### Playing and pausing an animation from MMS

Animation components are also live handles, so you can store them in a variable and call playback methods from signal handlers or other script logic.

```c
let anim = Animation.looping() {
    Keyframe.at(0) { cube_t.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.5, 0.5, 0.5]) }
    Keyframe.at(1) { cube_t.update_transform([0.0, 0.0, 0.0], [0.0, 3.14159 / 2, 0.0], [0.5, 0.5, 0.5]) }
    Keyframe.at(2) { cube_t.update_transform([0.0, 0.0, 0.0], [0.0, 3.14159, 0.0], [0.5, 0.5, 0.5]) }
    Keyframe.at(3) { cube_t.update_transform([0.0, 0.0, 0.0], [0.0, 3.14159 * 1.5, 0.0], [0.5, 0.5, 0.5]) }
}

anim

let pause_btn = T.position(-1.2, -1.2, 0.0).scale(0.35, 0.35, 0.35) {
    R.cube() {
        C.rgba(0.25, 0.55, 1.0, 1.0)
        Raycastable.enabled()
    }
}

let play_btn = T.position(1.2, -1.2, 0.0).scale(0.35, 0.35, 0.35) {
    R.cube() {
        C.rgba(0.30, 0.85, 0.45, 1.0)
        Raycastable.enabled()
    }
}

pause_btn
play_btn

on(pause_btn, "Click", fn(event) {
    anim.pause()
})

on(play_btn, "Click", fn(event) {
    anim.play()
})
```

See `examples/component-method-call.mms` for a complete runnable example.

### Reusable Buttons (`button.mms`)
The `assets/components/button.mms` file provides a standard button:

```javascript
import { button } from "assets/components/button.mms"

let my_btn = button("Click Me")
T.position(0, 0, 0) { my_btn }

// Attach signal handlers directly in MMS:
on(my_btn, "Click", fn(e) {
    print("Button was clicked!")
})
```

See `assets/components/` for more reusable primitives.


# REPL / CLI

There is a small stdin-driven REPL (processed on the main thread in `Universe::update()`) for inspecting the component tree.

## Commands

- `help` — print commands
- `ls` — list children of the current working component (or roots at `/`)
- `cd <name|index|guid|path>` — change working component
  - `cd /` goes to root
  - `cd ..` goes to parent
  - `cd /7v1:root/8v1:child` walks by `ComponentId` tokens and names
  - `cd <guid>` supports a global jump by GUID
- `pwd` — print a copy-pastable path for the current working component
- `cat [path]` — pretty-print JSON serialization of the subtree
  - `cat` with no args prints from the current working component
  - `cat /` prints the whole scene (all roots)
- `clear` / `cls` — clear the terminal

## Pipes

Pipes use `|` but they pipe *component objects* (ComponentIds), not strings.

- A trailing `|` prints an `ls`-style summary of the piped components.
  - Example: `cat / |`

### `grep`

`grep <pattern>` filters the piped components by matching against component properties (including `name`, `type`, `guid`, and encoded fields), and prints the full serialized value of any matching property.

- Example: `ls | grep color`
- Example: `cat /6v1:input | grep camera`


# Lifecycle

#### Frame loop:
```rust
// in engine::Universe:

/// Game/update step
  pub fn update(&mut self, _dt_sec: f32, _input: &InputState) {
      // each frame,
      // 1. Process input events (handled inside systems for now).
      // 2. Let systems call methods on components,
      //      for example, to update transforms or renderables, which
      //      will update VisualWorld can update draw_batches and give Renderer a snapshot
      self.systems.tick(&mut self.world, &mut self.visuals, _input);
      
      // Process commands after tick so any commands queued during tick are processed in the same frame
      self.systems.process_commands(&mut self.world, &mut self.visuals, &mut self.command_queue);
  }

  pub fn render(&mut self, renderer: &mut graphics::Renderer) {
      // Ensure VisualWorld contains only GPU-ready instances.
      self.systems
          .prepare_render(&mut self.world, &mut self.visuals, &mut self.render_assets, renderer);
      // TODO: rebuild inspector around component graph instead of entities.
      renderer.render_visual_world(&mut self.visuals)
              .expect("render failed");
  }
```


https://github.com/user-attachments/assets/ce4ac311-1087-4792-bec8-5dd012d848f2



## Profiling (flamegraph)

You can profile binaries (including examples) using `cargo-flamegraph` without adding any dependency to this project.

Prereqs:

- Install the Cargo subcommand: `cargo install cargo-flamegraph`
- Linux: install `perf` (Arch/EndeavourOS: `sudo pacman -S perf`)

Example (profile an optimized release build of an example with debug symbols and frame pointers):

```bash
CARGO_PROFILE_RELEASE_DEBUG=true \
  RUSTFLAGS="-C debuginfo=1 -C force-frame-pointers=yes" \
  cargo flamegraph --release --example vtuber-joints-example
```


# Credits:

Special thanks to [2gd4.me](https://2gd4.me) for designing font_system.png
 
