# MMS component migration checklist

Tracking the cutover from `Component::encode/decode` (JSON-based) to
`Component::to_mms_ast` (MMS ComponentExpression AST). The codec was deleted
in the same change set — see `src/meow_meow/component_registry.rs` for the
new pipeline:

```
live subtree ──► to_mms_ast (per component)
            ──► subtree_to_ce_ast (recursive)
            ──► ce_ast_to_materialized      ──► spawn_tree  (attach_clone / load)
            ──► unparser::unparse_component ──► .mms text   (save / dump)
```

Components that are not yet migrated still compile and still spawn —
`component_registry::resolve_type_name` accepts the snake_case name returned
by `Component::name()` and routes it to the canonical PascalCase variant.
What they lose, until migrated, is **internal state on save/clone**: the
default `to_mms_ast` emits an empty CE (no constructors, no body), so a
saved-then-loaded copy is reset to whatever `create_component` produces
with no args.

## Done

- [x] `TransformComponent` — `Transform.position().rotation_quat().scale()` (lossless)
- [x] `ColorComponent` — `Color.rgba()`
- [x] `RenderableComponent` — primitive ctor by `base_mesh`
      (`cube` / `sphere` / `triangle` / `square` / `tetrahedron` / `circle2d`).
      Dynamic meshes registered via `RenderAssets` are NOT round-tripable
      (the handle is allocation-order, not stable) — they emit bare `Renderable`.
- [x] `TextComponent` — text as positional string in body. `wrap_at`,
      `word_wrap`, `word_wrap_tokens` are not yet emitted.
- [x] `LayoutComponent` — `LayoutRoot.width().height().unit_scale()`
- [x] `StyleComponent` — **stub only** (`Style {}`). Style has ~50 fields;
      proper migration needs builders for each. Most authored Style ends up
      in MMS source today, so the stub is acceptable until we round-trip
      live-edited Style.
- [x] `OpacityComponent` — `Opacity.opacity(x)` + optional `.multiple_layers()`
- [x] `EmissiveComponent` — `Emissive.on()` / `Emissive.off()` + optional `.intensity(x)`
- [x] `AmbientLightComponent` — `AmbientLight.rgb(r,g,b)`
- [x] `DirectionalLightComponent` — `DirectionalLight.intensity(x).color(r,g,b)`
- [x] `PointLightComponent` — `PointLight.intensity(x).distance(x).color(r,g,b)`
- [x] `GLTFComponent` — `GLTF.new(uri)` + optional `.with_visualized_transforms(true)`

## Components still using default `to_mms_ast`

These all override `encode`/`decode` today (dead code — codec is gone) but
have no `to_mms_ast`. When the World panel's Save button writes the scene,
their internal state is lost on reload.

Priority is roughly "what's likely to be live-edited and need round-trip":

### High value

- [x] `TextureComponent` — `Texture.with_uri/from_dds/render_image(...)` (handle source is runtime-only, emits unresolved)
- [x] `Camera3DComponent` — `Camera3D.target(...).fov(...).near(...).far(...)`
- [x] `Camera2DComponent` — `Camera2D.target(...)` (registry entry added)
- [x] `CameraXRComponent` — `CameraXR.on/off()` + optional `.target("window")`
- [x] `OpenXRComponent` — `OpenXR.on/off()`
- [x] `ControllerXRComponent` — `ControllerXR.new(enabled, hand, pose)`
- [x] `InputXRComponent` — `InputXR.on/off()`
- [x] `AnimationComponent` — `Animation.playing/looping/paused()`
- [x] `KeyframeComponent` — `Keyframe.at(beat)`
- [x] `InputComponent` — `Input.speed(x)`
- [x] `InputTransformModeComponent` — `InputTransformMode.forward_y/forward_z()` + optional `.roll_axis_y()` / `.fps_rotation()`
- [x] `EditorComponent` — `Editor.translation_space(...).rotation_space(...)` (panel positions still lost)
- [x] `BackgroundComponent` — `Background {}` + optional `.occlusion_and_lighting()` / `.ray_casting()`
- [x] `BackgroundColorComponent` — `BackgroundColor {}` (marker)
- [x] `OverlayComponent` — `Overlay {}` (marker, never had encode/decode)
- [x] `RaycastableComponent` — `Raycastable.enabled/disabled/drag_only/click_only()` + optional `.pointer_events("pass_through")`
- [x] `SelectableComponent` — `Selectable.on/off()`
- [x] `HtmlElementComponent` — element-type ctor (`div`, `span`, `h1`–`h6`, `header`, `footer`, `main`, `nav`, `aside`, `section`, `article`, `body`, `p`); rarer element types fall back to bare `HtmlElement {}` until apply_call vocab is expanded

### Medium value

- [x] `TransparentCutoutComponent` — `TransparentCutout {}` (enabled) or `.disabled()`
- [x] `TextureFilteringComponent` — `TextureFiltering.linear/nearest/nearest_magnification()`
- [x] `EmissivePassComponent` — `EmissivePass {}` (marker)
- [x] `BloomComponent` — `Bloom.enabled().intensity().radius_ndc().emissive_scale().half_res()` + optional `.output_texture()`
- [x] `BlurPassComponent` — `BlurPass.enabled().radius_ndc().half_res()`
- [x] `RenderGraphComponent` — `RenderGraph.on/off()`
- [x] `LightQuantizationComponent` — `LightQuantization.steps(x)`
- [x] `NormalVisualisationComponent` — `NormalVis.thickness(x)`
- [x] `UVComponent` — `UV {}` chained with `.uv(u, v)` per vertex
- [x] `ScrollingComponent` — `Scrolling.new(viewport, content)` (runtime drag/track state intentionally not serialized)
- [x] `ClockComponent` — `Clock.bpm(x)`
- [ ] `ActionComponent` — has multiple shapes (print, update_transform); complex
- [x] `RouterComponent` — `Router.target("name").ignore(["a","b"])`
- [x] `TransitionComponent` — `Transition.enabled().duration_beats().<easing>().capture_from_current().<replace_policy>()`
- [x] `TextShadowComponent` — `TextShadow.rgba([…]).scale(x).offset([x,y,z])`
- [x] `RendererSettingsComponent` — `RendererSettings {}` or `.msaa_off()` + optional `.window_size(w, h)`

### Low value (transient / runtime-only / not user-authored)

- [x] `RendererStatsComponent` — `RendererStats {}` with builder chain
- [x] `RaycastComponent` — `Raycast.continuous/event_driven()` + `.max_distance(x)`
- [x] `PointerComponent` — `Pointer {}` or `.disabled()`
- [x] `RaycastableShapeComponent` — `RaycastableShape.<shape>()` enum ctor
- [x] `BoundsComponent` — `Bounds.aabb([…], […])`
- [x] `MeshComponent` — `Mesh.new(key)`
- [x] `CollisionComponent` — `Collision.static/kinematic/rigged()`
- [x] `CollisionShapeComponent` — `CollisionShape.cube([h,h,h])` / `.sphere(r)`
- [x] `GravityComponent` — `Gravity {}` with `.enabled().coefficient()`
- [x] `KineticResponseComponent` — `KineticResponse.slide/push()` with builder chain
- [x] `SkinnedMeshComponent` — `SkinnedMesh.new(skin_index)` (skin_id is runtime)
- [x] `GestureCoordTypeComponent` — `GestureCoordType.world_plane/screen_space_1d_slider()`
- [x] `SignalRouteUpwardComponent` — `SignalRouteUpward.new(intent_kind, parent_type)`
- [x] `StencilClipComponent` — `StencilClip {}` + optional `.stencil_ref(n)`
- [x] `AvatarBodyYawComponent` — `AvatarBodyYaw {}` with builder chain
- [x] `AvatarControlComponent` — `AvatarControl {}` with bone-name builder chain
- [x] `WorldPanelComponent` — `WorldPanel {}` (marker; auto-spawned by editor)
- [x] `InspectorPanelComponent` — `InspectorPanel {}` (marker; auto-spawned by editor)
- [x] `IKChainComponent` — `IKChain.<solver>(...)` + `.weight(x)` (target/end_effector are runtime-wired)
- [ ] All `Audio*` components (output, oscillator, gain, mix, limiter,
      buffer size, low/high/band-pass filter) — deferred per AudioNode consolidation plan
- [x] `MusicNoteComponent` — `MusicNote.<pitch>(octave, duration)` + optional `.velocity(x)`
- [x] All transform-pipeline operators (markers via default `to_mms_ast`,
      `TransformSampleAncestor.skip(n)` for the only one with state)
- [x] `QuatTemporalFilterComponent` — `QuatTemporalFilter.smoothing_factor(x)`
- [x] `Vector3TemporalFilterComponent` — `Vector3TemporalFilter.smoothing_factor(x)`
- [x] `QuatExtractYawComponent` — `QuatExtractYaw {}` (marker via default)
- [x] `QuatYawFollowComponent` — `QuatYawFollow.new(threshold, rate)` with builder chain
- [x] `TransformGizmoComponent` — `TransformGizmo {}.scale(x)`
- [x] `TransformGizmoTranslate/Rotate/Scale` — `.x/y/z()` axis ctor

## Process for migrating one component

For each component with state worth round-tripping:

1. Read the current `encode`/`decode` impl in `src/engine/ecs/component/<name>.rs`.
2. Check `component_registry::create_component` and `apply_call` for an
   existing builder vocabulary. If the registry can't reconstruct the
   component's state from named ctors + builder calls, **add the missing
   builders to `apply_call` first** (or to `apply_transform_builder` for
   transforms).
3. Replace `encode`/`decode` with `to_mms_ast` returning a
   `ComponentExpression`. Use the helpers in
   `src/engine/ecs/component/mod.rs::ce_helpers` (`ce`, `ce_call`,
   `with_call`, `num`, `nums`, `s`, `b`, `ident`, `array`).
4. Drop any `use serde_json::*` once both functions are gone.
5. Sanity-check by save → load round-trip (World panel Save button +
   `cargo run -- load <file>`).

## Trait-level cleanup (after all migrations land)

- [ ] Remove `fn encode` and `fn decode` defaults from the `Component`
      trait (`src/engine/ecs/component/mod.rs`).
- [ ] Drop `serde_json` from `Cargo.toml` if no other callers remain.

## Out-of-band future work

These are not per-component migrations, but are tracked here so they're
not forgotten.

- [ ] **Expose save / load to MMS** — at the moment scene save/load lives
      in rust (`std::fs::write` + `MeowMeowRunner::eval_with_world_at_path`).
      We want MMS-callable host functions:
      - `mms.save(root, "scene.mms")` — calls `subtree_to_ce_ast` +
        `unparser::unparse_component` and writes the result.
      - `mms.load("scene.mms")` — evaluates the file in the current world.
      - `mms.serialize(root)` — returns the MMS source string in-memory
        (so a script can post it to a server, diff it, etc).
      The hook points are `HostCallKind` in `src/meow_meow/evaluator.rs`
      and the matcher in `MeowMeowRunner::eval_with_world_at_path`.

- [ ] **Stable mesh identity for `RenderableComponent`** — today
      `base_mesh: CpuMeshHandle(u32)` is allocation-order. To round-trip
      a renderable that uses a dynamic mesh, the registration site needs
      to give meshes a stable string name and `to_mms_ast` should emit
      `Renderable.mesh("<name>")`.

- [ ] **Style fields** — every Style field needs either a `setter(value)`
      builder in `apply_call` or a `field = value` named-assignment hook
      in `apply_named_assignment`. Mirroring the CSS-ish vocabulary the
      layout system already understands.

- [ ] **`ComponentNode.name` and `.classes`** — currently `subtree_to_ce_ast`
      drops these. The MMS form is `Name = "value"` / `class = "..."` inside
      a component body. Emit them in the body block before children.
