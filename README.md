# cat-engine 0.5 "mittens"

<img width="1920" height="745" alt="Screenshot_20260303_015535" src="https://github.com/user-attachments/assets/16d9656c-9df3-4a96-89bd-658d222e78d0" />

small game engine `[obstensively]` for making cats,
using vulkan instanced rendering and several layers to describe game objects:

## Running examples

- Run examples in release mode by default: `cargo run --release --example <name>`.
- Avoid debug example runs unless you specifically need debug-only diagnostics or faster compile iteration.

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
use cat_engine::engine;

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

#### TextureSystem
#### LightSystem

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

## Transforms

Transforms are central in cat-engine: most component subtrees are rooted at a `TransformComponent`, and many engine systems interpret the component tree as “a scene graph of nested transforms + things attached under them”.

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
  - `KineticResponseSystem`: kinematic collision response integrates motion via `UpdateTransform`
  - `TransformGizmoSystem`: editor gestures call transform setters (which queue `UpdateTransform`)

4.1 **Transform propagation pipelines / transform operators**
  - `TransformSystem::transform_changed(...)` is the core propagation pipeline: it recomputes cached `matrix_world` for a transform subtree and pushes side effects to dependent systems.
  - `TransformFilterComponent` is a “filter-as-node” operator that changes what descendants inherit (e.g. inherit translation+rotation but *not* scale). It’s used heavily for editor/gizmo visuals.
  - For deeper notes/specs:
    - `TransformFilterComponent` motivation: `docs/analysis/gizmo-transform-propagation.md`
    - Gizmo coord spaces (Local/World): `docs/spec/editor-gizmo-coord-spaces.md`
    - Transform update flow and refit/rebuild behavior: `docs/analysis/refresh-transform.md`

+ TransformComponent
  + lets position anything in space (translation/rotation/scale)
  + can be nested to build scene graphs; see “Transforms” above for propagation + affected systems

+ RenderableComponent
  + Several built-in RenderableComponents are available as special constructors on the impl.
  + If you need lower level control over the mesh or material, you can create a `CPUMesh` and `MaterialHandle` and pass them to the `RenderableComponent::new()` constructor.
  + Meshes are uploaded to the GPU via the RenderableSystem and stored in RenderAssets.
  + Materials are pre-defined pipelines stored in `graphics::primitives::MaterialHandle`.
  + TODO: make separate material and geometry components

+ InputComponent
  + Recieves keyboard or other input sources and passes that info to relevant child components
  + TODO: set up key mappings and movement / transform modes beyond the defaults.
+ InputTransformModeComponent
  + Configures how an InputComponent affects the TransformComponent child.
  + construct with `forward_z()` or `forward_y()` 
    + to change which axis is forward(useful for both 3D or 2D games)
  + `with_roll_axis_y()` to remap roll keys to yaw
  + `with_fps_rotation()` to use FPS-style mouse rotation 
  +
+ Camera2DComponent
  + simple orthographic camera for 2D rendering
  + add to TransformComponent to use that transform's model matrix for the camera

+ Camera3DComponent
  + add to TransformComponent to use that transform's model matrix for the camera
  + add to TransformComponent and add that TransformComponent to an InputComponent to control the camera with the keyboard.

+ CameraXRComponent
  + stereoscopic camera for OpenXR rendering
  + can be parented to TransformComponent to transform both eyes at once
  + must be used with OpenXRComponent to get proper view/projection matrices from the XR
```
// input example (pseudo code)
InputComponent {
    TransformComponent {
        Camera3DComponent { }
    }
    InputTransformModeComponent::forward_z().with_fps_rotation()
}
```

+ ColorComponent
  + Per-instance RGBA tint.
  + Routed into the instanced vertex buffer, so it does not split draw batches.
  + Useful for quick “team color” / debug visualization without creating new materials.

+ OpacityComponent
  + Per-instance opacity multiplier (separate from `ColorComponent` alpha).
  + Routed into the instanced vertex buffer as `i_opacity` and multiplied into the fragment alpha.
  + Like color, opacity can be inherited from ancestors (so you can set it once on a parent and affect all children).
  + Influences which virtual render pass (render phase) an instance uses:
    + Instances are treated as transparent if `opacity < 0.999` **or** `color.a < 0.999`.
      + (Note: texture alpha is not currently considered for pass selection.)
    + If it is not transparent, it goes through the **opaque** instanced phase.
    + Transparent instances with `multiple_layers=false` go through the **transparent single-layer** instanced phase.
    + Transparent instances with `multiple_layers=true` go through the **transparent multi-layer** sorted phase.
    + This does not control the **cutout** or **background** phases; those are driven by other instance flags/components.
  + Usage:
    + `OpacityComponent::new().with_opacity(0.5)`
    + `OpacityComponent::new().with_opacity(0.5).with_multiple_layers()` when it must blend correctly with other transparent surfaces.

+ UVComponent
  + Supplies UVs for a mesh so shaders can sample textures.

+ TextureComponent
  + References a texture by `uri` (e.g. `"assets/images/cat-face-neutral.png"`).
  + Loaded/decoded via the `image` crate and uploaded to the GPU.
  + Textures are deduplicated by `uri` (multiple components can share the same GPU texture).
  + Texture affects batching: draw calls are grouped by (material, mesh, texture).

+ GLTFComponent
  + Loads a glTF 2.0 model from a URI (e.g. `"assets/models/cat.glb"`).
  + Creates child components for each mesh in the glTF file.
  + Materials are mapped to built-in `MaterialHandle` pipelines where possible.
  + Textures are loaded and deduplicated via TextureComponent.

+ PointLightComponent
  + Adds a point light to the scene (fed to the shader via an SSBO).

+ CollisionComponent
  + adds parent transform as a collision object
  + types supported
    + STATIC     // does not move. only interacts with other CollisionComponents
    + KINEMATIC  // can move in response to collisions
    + RIGGED     // for cameras and players and npcs and stuff

  + CollisionShapeComponent
    + Defines the collision shape for this collider (attach as a child of the `CollisionComponent`).

  + (see `GravityComponent` below) gravity is inherited from ancestors.

  + KineticResponseComponent
    + Opt-in kinematic collision response (automatic movement/push-out in response to overlaps).
    + **Topology requirement:** attach as a direct child of a `CollisionComponent`.
    + Modes and tuning fields are documented in: [docs/spec/kinetic-response.md](docs/spec/kinetic-response.md)

+ GravityComponent
  + Gravity field component.
  + Any `KineticResponseComponent` nested under a `GravityComponent` will have gravity applied.
  + Can live anywhere in the scene graph and affect an entire subtree.
  + If multiple `GravityComponent`s are in the ancestor chain, the nearest enabled one wins.
  + Fields:
    + `enabled: bool`
    + `coefficient: f32` — multiplier applied to the system gravity (e.g. `1.0` earth, `0.0` none).

+ OpenXRComponent
  + adds OpenXR support to the universe
  + handles session, frame loop, and input events from XR runtime


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

See [docs/signals.md](docs/signals.md) for the deeper rationale.


# Building Widgets

This engine’s “widgets” (gizmos, editor handles, debug UI-in-world) are usually built as **component subtrees** plus **scoped signal handlers**.

At a high level:

- A widget is a small subtree of components that contains renderable geometry (things you can see) and `RaycastableComponent` markers (things you can click/drag).
- Interaction comes in as signals (`RayIntersected`, `DragStart`, `DragMove`, `DragEnd`). Systems install scoped handlers rooted at the widget subtree, so the widget can respond to events happening on any of its descendants.

## Transform gizmo (example widget)

The transform gizmo is a reference implementation of this pattern:

- `TransformGizmoComponent` is attached under a target `TransformComponent`.
- On init, `TransformGizmoSystem` spawns a visual subtree (rotate rings, translate arrows) and marks the clickable parts as raycastable.
- During a drag, the gizmo figures out “what operation is this?” by walking up ancestry from the hit renderable and looking for handle marker components:
  - `TransformGizmoTranslateComponent { axis }`
  - `TransformGizmoRotateComponent { axis }`
  - `TransformGizmoScaleComponent { axis }`

## GestureCoordType (how a handle interprets motion)

Some handles need different coordinate mappings. This is controlled by attaching a `GestureCoordTypeComponent` somewhere in the ancestry of the clicked handle renderable:

- `GestureCoordType::WorldPlane`
  - Use world-space hit-point deltas (good for translation along an axis, with the gesture system providing a stable drag plane).
- `GestureCoordType::ScreenSpace1DSlider`
  - Use screen-space deltas (good for rotation rings, where you want “drag anywhere” behavior).

The up-to-date, code-matching interaction pipeline docs are:

- [docs/spec/gestures-and-gizmos.md](docs/spec/gestures-and-gizmos.md)
- [docs/refactor/gesture-screen-distance.md](docs/refactor/gesture-screen-distance.md)

## Building your own widget

Typical steps:

1. Create a root marker component for the widget (stores runtime state like “active pointer”, “drag start value”, etc.).
2. Spawn a visual subtree under that root, including raycastable renderables.
3. Install scoped handlers for `DragStart/DragMove/DragEnd` rooted at the widget.
4. In handlers, mutate the target component(s) directly (or emit intents if you need the changes to flow through the drain-point signal model).


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
 