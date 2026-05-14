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

## Components still using default `to_mms_ast`

These all override `encode`/`decode` today (dead code — codec is gone) but
have no `to_mms_ast`. When the World panel's Save button writes the scene,
their internal state is lost on reload.

Priority is roughly "what's likely to be live-edited and need round-trip":

### High value

- [ ] `OpacityComponent` — needs `apply_call` for `opacity()` + `multiple_layers()`
- [ ] `EmissiveComponent` — needs `apply_call` for `intensity()`
- [ ] `AmbientLightComponent` — `AmbientLight.rgb()` already in registry; just add `to_mms_ast`
- [ ] `DirectionalLightComponent` — registry has `intensity()` + `color()` builders
- [ ] `PointLightComponent` — registry has `intensity()` / `distance()` / `color()` builders
- [ ] `GLTFComponent` — registry has `.new(uri)`; add `to_mms_ast`
- [ ] `TextureComponent` — registry has multiple ctors (`uri`, `from_png`, `from_dds`, `render_image`); pick from runtime state
- [ ] `Camera3DComponent`, `Camera2DComponent`, `CameraXRComponent`
- [ ] `OpenXRComponent`, `ControllerXRComponent`, `InputXRComponent`
- [ ] `AnimationComponent` (state — playing/paused/looping)
- [ ] `KeyframeComponent` (beat)
- [ ] `InputComponent` (speed)
- [ ] `InputTransformModeComponent`
- [ ] `EditorComponent` — but the editor's own subtree is excluded from save
- [ ] `BackgroundComponent`, `OverlayComponent`, `BackgroundColorComponent`
- [ ] `RaycastableComponent`, `SelectableComponent`
- [ ] `HtmlElementComponent` — many element-type variants

### Medium value

- [ ] `TransparentCutoutComponent`
- [ ] `TextureFilteringComponent`
- [ ] `EmissivePassComponent`, `BloomComponent`, `BlurPassComponent`
- [ ] `RenderGraphComponent`
- [ ] `LightQuantizationComponent`
- [ ] `NormalVisualisationComponent`
- [ ] `UVComponent`
- [ ] `ScrollingComponent`
- [ ] `ClockComponent` (bpm)
- [ ] `ActionComponent` — has multiple shapes (print, update_transform); complex
- [ ] `RouterComponent`
- [ ] `TransitionComponent`
- [ ] `TextShadowComponent`
- [ ] `RendererSettingsComponent`

### Low value (transient / runtime-only / not user-authored)

- [ ] `RendererStatsComponent`
- [ ] `RaycastComponent`, `PointerComponent`
- [ ] `RaycastableShapeComponent`
- [ ] `BoundsComponent`, `MeshComponent`
- [ ] `CollisionComponent`, `CollisionShapeComponent`, `GravityComponent`,
      `KineticResponseComponent` — physics state typically not authored in MMS
- [ ] `SkinnedMeshComponent`
- [ ] `GestureCoordTypeComponent`
- [ ] `SignalRouteUpwardComponent`
- [ ] `StencilClipComponent`
- [ ] `AvatarBodyYawComponent`, `AvatarControlComponent`
- [ ] `WorldPanelComponent`, `InspectorPanelComponent` — editor UI; excluded from save
- [ ] `IKChainComponent`
- [ ] All `Audio*` components (output, oscillator, gain, mix, limiter,
      buffer size, low/high/band-pass filter)
- [ ] `MusicNoteComponent`
- [ ] All transform-pipeline operators (`TransformPipeline`,
      `TransformForkTRS`, `TransformMap{Translation,Rotation,Scale}`,
      `TransformMergeTRS`, `TransformPipelineOutput`, `TransformDrop`,
      `TransformSampleAncestor`)
- [ ] `QuatTemporalFilterComponent`, `Vector3TemporalFilterComponent`,
      `QuatExtractYawComponent`, `QuatYawFollowComponent`
- [ ] `TransformGizmoComponent` + its `Translate`/`Rotate`/`Scale` variants

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
