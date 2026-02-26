# cat engine「0.4」

<img width="498" height="400" alt="Screenshot_20260106_094219" src="https://github.com/user-attachments/assets/83c00897-aa61-4520-8756-cd7263289800" />


small game engine `[obstensively]` for making cats,
using vulkan instanced rendering and several layers to describe game objects:

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

+ TransformComponent
  + lets position anything in space (and rotate and scale it)
  + affects children:
    + RenderableComponent
    + Camera2DComponent
    + Camera3DComponent
    + PointLightComponent
    + CollisionComponent
  + affected by parents:
    + InputComponent (recieves transform input from InputComponent)

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
  + Influences which transparency **draw phase** an instance uses:
    + Instances are treated as transparent if `opacity < 0.999` **or** `color.a < 0.999`.
      + (Note: texture alpha is not currently considered for pass selection.)
    + Transparent instances with `multiple_layers=false` go through the **transparent single-layer** instanced phase.
    + Transparent instances with `multiple_layers=true` go through the **transparent multi-layer** sorted phase.
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
    + Opt-in kinematic collision response for a collider.
    + **Policy:** collision detection/queries still work without this; collision signals still emit. This component only controls *automatic movement* in response to overlaps.
    + **Topology requirement:** attach as a direct child of a `CollisionComponent` (which itself should be a direct child of a `TransformComponent`).

Example topology:

```rust
TransformComponent {
  CollisionComponent::KINEMATIC() {
    CollisionShapeComponent { ... }
    KineticResponseComponent::push() { ... }
  }
    RenderableComponent { ... }
}

GravityComponent {
  TransformComponent {
    CollisionComponent::KINEMATIC() {
      CollisionShapeComponent { ... }
      KineticResponseComponent::push() { ... }
    }
  }
}
```

  + **Modes**
    + `slide` (`KineticResponseComponent::slide()`)
      + Classic kinematic “push out of statics” behavior.
      + Each tick, if overlapping static colliders, pushes the transform out along the minimum-penetration axis (AABB).
      + Good for camera rigs and players sliding along level geometry.
    + `push` (`KineticResponseComponent::push()`)
      + “Pushable” behavior.
      + Accumulates a runtime velocity away from overlapping **non-static** colliders, integrates it every tick, and still resolves overlaps against static colliders.
      + Includes a simple horizontal bounce on static side-wall contacts (X/Z velocity reflection) so bodies don’t just stick while being corrected.

  + **Tuning fields (encode/decode keys shown)**
    + `enabled: bool` — master toggle.
    + `mode: "slide" | "push"`
    + `max_iterations: u32` — max static push-out iterations per tick.
    + `push_out_epsilon: f32` — tiny extra separation to reduce jitter at exact contact.
    + `push_strength: f32` — strength of push-mode acceleration from non-static overlaps.
      + Builder: `with_push_strength(f32)`
    + `max_speed: f32` — clamp on push-mode speed (world units/sec).
    + `friction: f32` — per-second velocity damping applied every tick in push-mode.
      + Off by default (`0.0`).
      + Builder: `with_friction(f32)`
    + `friction_y: f32` — per-second damping applied to **Y velocity only**, and only when resolving a **vertical (Y-axis) static overlap** (e.g. floor/roof contact).
      + Off by default (`0.0`).
      + Builder: `with_friction_y(f32)`

  + **Runtime state**
    + `velocity: [f32; 3]` is runtime-only (not serialized).

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


# Actions

Actions are data-driven “commands” stored in the component graph as `ActionComponent`s.
They are typically executed by the `AnimationSystem` when a `KeyframeComponent` fires, but can also be executed directly via `ActionSystem`.

## ActionComponent schema

An `ActionComponent` encodes to a small JSON-ish record:

- `target: [u64, ...]` — list of component ids (slotmap FFI ids)
- `method: "..."` — action method string
- `params: [ ... ]` — method-specific parameters

## Supported actions

Common topology/scene actions:

- `set_color(target, rgba)` (`method = "set_color"`)
- `set_text(target, text)` (`method = "set_text"`)
- `set_position(target, x, y, z)` (`method = "set_position"`)

- `attach(parent, child)` (`method = "attach"`)
- `detach(targets)` (`method = "detach"`)
- `remove_subtree(targets)` (`method = "remove_subtree"`)

Prefab + child removal helpers (mirror the `Universe` helpers):

- `attach_clone(parent, prefab_root)` (`method = "attach_clone"`)
  - Clones the prefab subtree and attaches the cloned root under each target parent.

- `remove_child(parent, index)` (`method = "remove_child"`)
  - Detaches the selected child immediately and queues deletion of that subtree.
  - `index` is based on the current `children_of(parent)` order; if you want a stable index, avoid attaching other “marker” children under the same parent.

- `remove_children(parent)` (`method = "remove_children"`)
  - Detaches + queues deletion for all direct children.

Audio actions:

- `audio_graph_rebuild(targets)` (`method = "audio_graph_rebuild"`)
- `audio_low_pass_set_cutoff_hz(targets, cutoff_hz)` (`method = "audio_low_pass_set_cutoff_hz"`)
- `audio_band_pass_set_center_hz(targets, center_hz)` (`method = "audio_band_pass_set_center_hz"`)

Oscillator/music actions:

- `oscillator_set_enabled(targets, enabled)` (`method = "oscillator_set_enabled"`)
- `oscillator_set_pitch(targets, frequency_hz)` (`method = "oscillator_set_pitch"`)
- `oscillator_schedule_set_pitch(targets, beat_offset, frequency_hz)` (`method = "oscillator_schedule_set_pitch"`)
- `oscillator_schedule_set_note(targets, beat_offset, pitch, octave)` (`method = "oscillator_schedule_set_note"`)
- `oscillator_schedule_music_note(targets, beat_offset, note)` (`method = "oscillator_schedule_music_note"`)
- `music_set_note(targets, note)` (`method = "music_set_note"`)


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


Credits:

Special thanks to [2gd4.me](https://2gd4.me) for designing font_system.png
 