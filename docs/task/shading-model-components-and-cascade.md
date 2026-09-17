# Task: Unified Shading component, Anime default, and cascading settings

Status: agreed authoring direction; implementation pending.
Updated: 2026-09-08.

Partial implementation for the [first live-controls slice](anime-shading-panel-and-live-shader-inputs.md):
`Shading.anime()` and `.toon()`, one shared `ShadingComponent`, Anime parameter
builders, shade-strength live read/write, wrapper inheritance, and GLTF source
projection are available. `AnimeShading` remains a temporary compatibility name.
The default switch, other constructors, legacy removal, conflict diagnostics,
and complete reparent/removal invalidation below remain pending.

## Decisions

Use one `Shading` MMS component / `ShadingComponent` Rust component for built-in
surface shading. Anime becomes the default for ordinary mesh renderables with
no authored shading selection, including imported and skinned meshes.

The public constructors are:

- `Shading.anime()`
- `Shading.toon()`
- `Shading.unlit()`
- `Shading.refraction()`
- `Shading.rough_transmission()`

`Shading {}` means Anime with the generic Anime defaults. Built-in parameters
are builder calls on the selected model. `Shader` / `ShaderComponent` is reserved
for custom shading, using the same cascade and ownership rules. It is not a
second name for built-in selection. Exact custom-program registration and
constructor syntax remain design work under [Materials v2](epic/materials-v2.md).

This task supersedes the earlier separate `Toon`/`MeowToon` proposal and the
Toon-default decision in this file. It also defines the intended migration away
from separate `AnimeShading`, `Unlit`, `Refraction`, and `RoughTransmission`
authoring components. These are target APIs, not currently implemented syntax.

## Cascade and parameter ownership

One authored shading component with explicit builder settings must style many
descendant renderables. In particular, the existing intended shape
`AnimeShading.shade_strength(0.5) { ...many renderables... }` becomes
`Shading.anime().shade_strength(0.5) { ...many renderables... }`.

Resolve one model and its complete validated parameter state for each renderable:

1. An immediate shading-model child of the renderable wins over inherited shading.
2. Otherwise, use the nearest enclosing shading scope. A `Shading` or custom
   `Shader` wrapper is a scope for its descendants. A shading child attached to
   an ordinary container declares that container's descendant scope, including
   a shading child beneath a GLTF component for its generated primitives.
3. At each scope, more than one competing shading declaration is an authoring
   error, including competing built-in and custom declarations. Do not use child
   order to break ties. Diagnostics must identify the conflicting components.
4. With no authored declaration, use generic Anime defaults.

A local declaration replaces the whole inherited model and settings; it does
not merge individual parameters. For example, a local `Shading.anime()` resets
to generic Anime settings even beneath a customized Anime scope. Color, texture,
opacity, and other independent style resolution retain their separate contracts.

```mms
Shading.anime().shade_strength(0.5).rim_strength(0.38) {
    R.cube() {}                         // inherits both explicit settings
    T.position(2.0, 0.0, 0.0) {
        R.sphere() {}                   // inherits through ordinary ancestors
    }
    R.cube() {
        Shading.toon()                  // immediate child overrides Anime
    }
    Shading.unlit() {
        R.sphere() {}                   // nearest enclosing scope wins
        R.cube() { Shading.anime() }    // local generic Anime defaults
    }
    GLTF.new("assets/models/bisket.glb") {
        // Generated primitives inherit the outer Anime source and settings.
    }
}
```

The authored source owns mutable parameter state. Generated GLTF projections
must retain source identity rather than becoming independent stale copies.
Live edits update every renderable that resolves to that source, including
primitives created after the edit. Renderables with a more specific declaration
and unrelated instances must remain unchanged. A renderable's own shading child
also establishes its descendant scope, subject to nearer overrides.

Reparenting, source removal, and declaration replacement must invalidate
resolution so affected descendants resolve their current source again. Removing
an override restores the next enclosing source, or generic Anime if none exists.
Serialization must preserve authored declarations and builder values without
serializing generated projections as additional user declarations.

## Defaults and renderer behavior

Use the generic Anime defaults, not the Bisket-specific preset:

| Parameter | Default |
| --- | --- |
| Shade color | [0.72, 0.50, 0.54] |
| Shade strength | 0.30 |
| Shade threshold | 0.35 |
| Lit threshold | 0.55 |
| Rim color | [1.0, 0.85, 0.92] |
| Rim strength | 0.18 |
| Rim power | 4.0 |

Keep one canonical definition for component and GPU defaults. Preserve current
Anime normalization, including coupled threshold ordering, in construction,
live mutation, and readback. Reject parameters that do not belong to the selected
model; for example, `roughness` belongs to rough transmission, not refraction.

Resolve semantic shading before choosing static, skinned/cached-deformed,
transparent, cutout, or clipped pipelines. Do not expose these pipeline variants
as more authored shading components or replace every `TOON_MESH` token blindly.
Audit primitive defaults, imported primitives, and direct Rust scene construction.
Keep explicit Toon handles available internally and `Shading.toon()` available
to authors who want the previous appearance.

Anime currently ignores ambient light, light RGB, quantization, and emissive
inputs. The default migration must explicitly verify its intended lighting
appearance and preserve working `Emissive`, Unlit, and bloom behavior. Do not
silently change Anime's artistic lighting formula as part of a naming refactor.
`Shading.unlit()` preserves the existing Unlit contract. Emission remains a
separate concern for this slice; document and test its model interactions.

Grid, mirror, text, and other specialized outputs need explicit compatibility
rules; default Anime must not overwrite their required rendering contracts.
Apply the cascade to ordinary meshes, implicit-surface mesh outputs, and generated
renderables where compatible. Reject unsupported explicit combinations with a
useful diagnostic rather than silently selecting another model. Refraction and
rough transmission retain their optical validation and render-graph requirements
while gaining the same authoring cascade as the other built-ins.

## Migration and implementation

- [ ] Introduce one semantic model enum with typed model-specific parameter state,
      `ShadingComponent`, MMS constructors/builders, and serialization.
- [ ] Implement one cascade resolver with source identity and deterministic conflict
      errors; use it for ordinary meshes and GLTF-generated primitives.
- [ ] Implement live parameter mutation/readback and invalidation for changes to
      settings, declaration ownership, and tree structure.
- [ ] Switch ordinary static/skinned mesh defaults to Anime and verify specialized
      paths, emission, and Unlit behavior.
- [ ] Migrate `AnimeShading...` to `Shading.anime()...`, `Unlit` to
      `Shading.unlit()`, `Refraction...` to `Shading.refraction()...`, and
      `RoughTransmission...` to `Shading.rough_transmission()...` throughout Rust
      callers, MMS assets/examples, tests, and current authoring documentation.
- [ ] Remove the separate legacy component implementations and registrations once
      migration is complete. If a temporary compatibility alias is necessary, it
      must resolve to the same unified state/resolver and have a removal plan.
      New serialization emits only the unified API.
- [ ] Keep the left model in `shading-models.mms` explicitly `Shading.toon()`;
      bind the right model's panel to its authored `Shading.anime()` source.
- [ ] Integrate custom `Shader` declarations with the same resolver after defining
      their validated program/schema contract. Built-in migration does not depend
      on implementing arbitrary shader-file loading.

## Live panel integration

[Anime shading panel and live inputs](anime-shading-panel-and-live-shader-inputs.md)
tracks the sliders, effective-value readback, Reset, accordion lifecycle, and
bounded GPU resource retention. Its target is now `Shading.anime()`. Preserve the
Bisket preset for the right model rather than substituting generic defaults.

Parameter-only edits must reach GPU data without reloading models, rebuilding
geometry, or recompiling shaders. Bound descriptor/UBO retention during sustained
dragging and keep in-flight GPU resources valid. A general custom-shader API is
not required for these controls.

## Acceptance criteria

- Unconfigured ordinary static and skinned meshes render with generic Anime.
- One customized Anime wrapper styles multiple renderables through nested
  transforms and GLTF generation, including primitives created after an edit.
- Immediate-child overrides of each built-in model beat the wrapper; nested
  scopes, same-model resets, and conflict diagnostics follow the rules above.
- Source edits update inherited consumers without changing overridden or unrelated
  consumers. Reparenting and override removal recompute the correct result.
- Refraction and rough transmission inherit independently and preserve their
  distinct parameter validation and render phases.
- Unlit scenes retain their behavior after migration to `Shading.unlit()`.
- Serialization round trips preserve model, settings, and scope; generated
  projections are not emitted as authored declarations.
- The comparison remains Toon versus Anime, and its sliders update only the
  intended source with bounded GPU cache retention.
- Custom `Shader` uses the same precedence/conflict rules when implemented.

## Related documents

- [Materials v2](epic/materials-v2.md): renderer definitions, schemas, and ownership.
- [Transmission authoring contract](transmissive-materials-ecs-mms-authoring-contract.md):
  existing implementation; this task supersedes its separate-type/non-cascading
  authoring decisions for the planned migration.
- [Shader component draft](../draft/shader-component.md): V1 custom fragment
  material contract; this task establishes `Shader` as the custom-shading
  authoring name and its shared cascade rules.
