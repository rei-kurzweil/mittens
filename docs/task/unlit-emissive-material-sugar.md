# Unlit and Emissive as sugar over material definitions

Status: investigation. Nothing here is implemented except the editor-panel
unblock noted at the end.

## Problem

`Unlit {}` and `Emissive` are marker components that select **built-in**
programs, outside the custom material-definition system:

- `Unlit` makes `MaterialSystem::resolve_surface` return
  `SurfaceSource::Unlit`, which maps to `MaterialHandle::UNLIT_MESH`
  (`toon-mesh.vert` + `unlit-mesh.frag`). The marker is checked before a
  `Material` child on the same node and hides any custom `Material` on an
  ancestor.
- `Emissive` sets `VisualInstance.emissive`. For built-in programs,
  `builtin_emission_variant` swaps `TOON_MESH` for `EMISSIVE_TOON_MESH` and so
  on. For a custom definition the program is unchanged, and
  `VisualWorld::is_emissive_instance` uses the definition's base capability
  (`MaterialEmission`), not the component. So on desktop, `Emissive` under a
  custom `Surface` material does nothing.

The Android renderer accepts custom material definitions only (AGENTS.md: no
legacy material pipelines or silent legacy fallbacks). That rules out both
markers by design, not as missing work:

- `Unlit` surfaces resolve to `UNLIT_MESH` and fail with "mobile renderer
  requires a custom Material(definition) on every surface".
- Any instance with `emissive != 0` is rejected, even under a custom material
  where desktop would ignore it. The two renderers disagree here.

## Existing pieces

- `MaterialBaseKind`: `Surface`, `ExtractedEmissive` (opaque static meshes
  whose fragment output is also their emitted radiance), `PlanarReflective`,
  `Billboard`, `AxialBillboard`.
- A custom fragment that ignores the light buffers is already unlit:
  `assets/shaders/ui-unlit.frag` (`ui_unlit_material()`), used by the Android
  scenes and the launcher.
- `MaterialDefinition.anime()` / `.toon()` show the pattern for one shared
  canonical definition per world.

## Proposal (to investigate)

Keep `Unlit` and `Emissive` as authoring sugar, but resolve them to material
definitions instead of built-in programs.

1. **Unlit**: `Unlit {}` resolves to a shared canonical custom definition
   (`MaterialBase()` + an unlit fragment, the same as `ui-unlit.frag`). It
   then follows ordinary nearest-material-wins rules and draws on both
   renderers. Optionally expose it as `MaterialDefinition.unlit()`.
2. **Emissive**: `Emissive` on, or above, a surface selects the
   `ExtractedEmissive` variant of the resolved definition (same fragment,
   different base), instead of a built-in emissive toon program. This makes
   `Emissive` work with custom materials on desktop. On Android it becomes an
   explicit capability check: support extracted emission once there is a bloom
   path (`docs/task/mittens-benchmark-android.md`), else reject with a clear
   error.

## Questions

- **Emissive scope.** `ExtractedEmissive` is documented for opaque static
  meshes. Skinned, cutout and transparent emitters need their own contract.
- **Two bases.** Switching the base per instance means one custom fragment
  compiled against two bases. Check whether the pipeline cache and the
  descriptor interface allow that without per-draw cost.
- **Emissive intensity.** `Emissive` intensity is per instance today. Decide
  whether it becomes a material input or stays per instance.
- **Rust-built UNLIT_MESH users.** Gestures, implicit surfaces, combined
  meshes and view bindings use `UNLIT_MESH` directly. They are separate from
  the component and stay built-in until migrated.
- **Desktop cost.** Moving `Unlit` users onto the custom pipeline should cost
  about the same with one shared definition. Measure an editor-panel-heavy
  scene.
- **Migration.** `Unlit` appears 38 times across 19 MMS files, mostly through
  `accordion({ unlit = true })`. `Emissive` is used by icons, the accordion
  toggle glow, signs and glTF import (black base color with emissive factor).

## Validation

- Desktop: `ui_lighting_tests`, the material resolution tests, and a
  bloom-scene comparison before and after for `Emissive` under custom
  materials.
- Android: `android/validation/renderer-smoke` (the sakura stage with
  EditorUI, the launcher dock) renders with no legacy-material rejections.

## Done so far (unblock)

`accordion({ unlit = true })` now attaches `shared_ui_unlit_material()` (one
shared `ui-unlit.frag` definition) instead of `Unlit {}`, unless the caller
passes `material`. Editor panels, info panels and settings panels therefore
draw on Android. Direct `Unlit {}` uses and `Emissive` are unchanged.
