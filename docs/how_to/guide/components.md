# MMS component guide

This is the canonical catalog of concrete engine components and their current Meow Meow Script (MMS) exposure. The Rust implementation is authoritative; this guide deliberately links to it instead of duplicating builder and field APIs that change frequently.

MMS status means:

- **Directly constructible** — the component's canonical MMS name is in `SUPPORTED_COMPONENT_NAMES`.
- **Available through an alias** — MMS constructs this Rust type under a different public name.
- **Engine-only** — runtime systems may create or use it, but MMS cannot construct it directly. Its example shows the closest supported relationship and is not proposed syntax.

Every `mms parse-only` fence is syntax-checked by documentation tests. Fences marked `mms runnable` are also evaluated in an isolated world with render assets.

For common composition patterns, see the [MMS language guide](../../../crates/meow-meow-script/docs/guide/language.md). For signal semantics and the exhaustive signal catalog, see the [MMS signal guide](signals.md).

## Transforms and scene graph

### `BoundsComponent`
<!-- catalog:component source="BoundsComponent" mms="direct" names="Bounds" -->
Carries bounds state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `Bounds`. Sources: [Rust implementation](../../../src/engine/ecs/component/bounds.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Bounds {}
```

### `FitBoundsComponent`
<!-- catalog:component source="FitBoundsComponent" mms="direct" names="FitBounds" -->
Carries fit bounds state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `FitBounds`. Sources: [Rust implementation](../../../src/engine/ecs/component/fit_bounds.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
FitBounds {}
```

### `TransformApplyInverseLocalComponent`
<!-- catalog:component source="TransformApplyInverseLocalComponent" mms="direct" names="TransformApplyInverseLocal" -->
Reads the authored local matrix of an explicitly referenced transform and applies its complete
inverse to the inherited transform stream. Only children nested beneath the operator receive the
result; the referenced source is not mutated. **Directly constructible** as
`TransformApplyInverseLocal`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_apply_inverse_local.rs)
and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformApplyInverseLocal.source("#camera_anchor") {}
```

### `TransformCameraSpecificComponent`
<!-- catalog:component source="TransformCameraSpecificComponent" mms="direct" names="TransformCameraSpecific" -->
Carries transform camera specific state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Camera/XR systems; registration intents and XR button/axis events are relevant.
**Directly constructible** as `TransformCameraSpecific`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_camera_specific.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformCameraSpecific {}
```

### `TransformComponent`
<!-- catalog:component source="TransformComponent" mms="direct" names="Transform" -->
Stores local translation, rotation, and scale and the derived world transform used by scene traversal. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `Transform`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms runnable
Transform {}
```

`transform.local_bounds()` returns `{ min = [x, y, z], max = [x, y, z] }`
for descendant renderable geometry in that transform's local frame. Nested
transforms are included; the queried root's own position, rotation, and scale
are excluded. It returns `null` for an empty subtree or while any GLTF import
or renderable's cached bounds are still pending, rather than a partial box.
These are cached mesh bounds, not exact bounds of a skinned animation pose.

Keep the model in a dedicated transform if effects, debug visuals, or other
geometry should not count toward its bounds. Since imported GLTF nodes are
attached to their transform anchor, query that anchor rather than the GLTF
component itself. For example, poll until ready and calculate a front-facing
attachment offset once:

```mms parse-only
let model_box = model_root.local_bounds()
if model_box {
    let front_z = model_box["min"][2] - 0.10
}
```

### `TransformDropComponent`
<!-- catalog:component source="TransformDropComponent" mms="direct" names="TransformDrop" -->
Carries transform drop state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformDrop`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_pipeline.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformDrop {}
```

### `TransformForkTRSComponent`
<!-- catalog:component source="TransformForkTRSComponent" mms="direct" names="TransformForkTRS" -->
Carries transform fork t r s state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformForkTRS`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_pipeline.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformForkTRS {}
```

### `TransformGizmoComponent`
<!-- catalog:component source="TransformGizmoComponent" mms="direct" names="TransformGizmo" -->
Carries transform gizmo state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformGizmo`. Sources: [Rust implementation](../../../src/engine/ecs/component/gizmo.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformGizmo {}
```

### `TransformGizmoRotateComponent`
<!-- catalog:component source="TransformGizmoRotateComponent" mms="direct" names="TransformGizmoRotate" -->
Carries transform gizmo rotate state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformGizmoRotate`. Sources: [Rust implementation](../../../src/engine/ecs/component/gizmo.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformGizmoRotate {}
```

### `TransformGizmoScaleComponent`
<!-- catalog:component source="TransformGizmoScaleComponent" mms="direct" names="TransformGizmoScale" -->
Carries transform gizmo scale state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformGizmoScale`. Sources: [Rust implementation](../../../src/engine/ecs/component/gizmo.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformGizmoScale {}
```

### `TransformGizmoTranslateComponent`
<!-- catalog:component source="TransformGizmoTranslateComponent" mms="direct" names="TransformGizmoTranslate" -->
Carries transform gizmo translate state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformGizmoTranslate`. Sources: [Rust implementation](../../../src/engine/ecs/component/gizmo.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformGizmoTranslate {}
```

### `TransformGizmoTranslatePlaneComponent`
<!-- catalog:component source="TransformGizmoTranslatePlaneComponent" mms="direct" names="TransformGizmoTranslatePlane" -->
Marks a planar translation handle and its locked axis. **Directly constructible** as
`TransformGizmoTranslatePlane`. Sources: [Rust implementation](../../../src/engine/ecs/component/gizmo.rs)
and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformGizmoTranslatePlane.xy() {}
```

### `TransformMapRotationComponent`
<!-- catalog:component source="TransformMapRotationComponent" mms="direct" names="TransformMapRotation" -->
Carries transform map rotation state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformMapRotation`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_pipeline_map.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformMapRotation {}
```

### `TransformMapScaleComponent`
<!-- catalog:component source="TransformMapScaleComponent" mms="direct" names="TransformMapScale" -->
Carries transform map scale state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformMapScale`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_pipeline_map.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformMapScale {}
```

### `TransformMapTranslationComponent`
<!-- catalog:component source="TransformMapTranslationComponent" mms="direct" names="TransformMapTranslation" -->
Carries transform map translation state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformMapTranslation`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_pipeline_map.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformMapTranslation {}
```

### `TransformMergeTRSComponent`
<!-- catalog:component source="TransformMergeTRSComponent" mms="direct" names="TransformMergeTRS" -->
Carries transform merge t r s state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformMergeTRS`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_pipeline.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformMergeTRS {}
```

### `TransformParentComponent`
<!-- catalog:component source="TransformParentComponent" mms="direct" names="TransformParent" -->
Carries transform parent state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformParent`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_parent.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformParent {}
```

### `TransformSampleAncestorComponent`
<!-- catalog:component source="TransformSampleAncestorComponent" mms="direct" names="TransformSampleAncestor" -->
Carries transform sample ancestor state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Transform and bounds systems; transform update/removal intents and `ParentChanged` are relevant.
**Directly constructible** as `TransformSampleAncestor`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_pipeline.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransformSampleAncestor {}
```

## Rendering and appearance

### `BackgroundColorComponent`
<!-- catalog:component source="BackgroundColorComponent" mms="direct" names="BackgroundColor" -->
Carries background color state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `BackgroundColor`. Sources: [Rust implementation](../../../src/engine/ecs/component/background_color.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
BackgroundColor {}
```

### `BackgroundComponent`
<!-- catalog:component source="BackgroundComponent" mms="direct" names="Background" -->
Carries background state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `Background`. Sources: [Rust implementation](../../../src/engine/ecs/component/background.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Background {}
```

### `BloomComponent`
<!-- catalog:component source="BloomComponent" mms="direct" names="Bloom" -->
Carries bloom state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `Bloom`. Sources: [Rust implementation](../../../src/engine/ecs/component/bloom.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Bloom {}
```

### `BlurPassComponent`
<!-- catalog:component source="BlurPassComponent" mms="direct" names="BlurPass" -->
Carries blur pass state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `BlurPass`. Sources: [Rust implementation](../../../src/engine/ecs/component/blur_pass.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
BlurPass {}
```

### `ColorComponent`
<!-- catalog:component source="ColorComponent" mms="direct" names="Color" -->
Carries color state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `Color`. Sources: [Rust implementation](../../../src/engine/ecs/component/color.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Color {}
```

### `EmissiveComponent`
<!-- catalog:component source="EmissiveComponent" mms="direct" names="Emissive" -->
Carries emissive state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `Emissive`. Sources: [Rust implementation](../../../src/engine/ecs/component/emissive.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Emissive {}
```

### `EmissivePassComponent`
<!-- catalog:component source="EmissivePassComponent" mms="direct" names="EmissivePass" -->
Carries emissive pass state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `EmissivePass`. Sources: [Rust implementation](../../../src/engine/ecs/component/emissive_pass.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
EmissivePass {}
```

### `GLTFComponent`
<!-- catalog:component source="GLTFComponent" mms="direct" names="GLTF" -->
Carries gltf state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `GLTF`. Sources: [Rust implementation](../../../src/engine/ecs/component/gltf.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
GLTF {}
```

### `MeshComponent`
<!-- catalog:component source="MeshComponent" mms="direct" names="Mesh" -->
Carries mesh state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `Mesh`. Sources: [Rust implementation](../../../src/engine/ecs/component/mesh.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Mesh {}
```

### `MirrorComponent`
<!-- catalog:component source="MirrorComponent" mms="direct" names="Mirror" -->
Carries mirror state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `Mirror`. Sources: [Rust implementation](../../../src/engine/ecs/component/mirror.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Mirror {}
```

### `NormalVisualisationComponent`
<!-- catalog:component source="NormalVisualisationComponent" mms="direct" names="NormalVis" -->
Carries normal visualisation state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Normal Visualisation engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `NormalVis`. Sources: [Rust implementation](../../../src/engine/ecs/component/normal_visualisation.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
NormalVis {}
```

### `OpacityComponent`
<!-- catalog:component source="OpacityComponent" mms="direct" names="Opacity" -->
Carries opacity state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `Opacity`. Sources: [Rust implementation](../../../src/engine/ecs/component/opacity.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Opacity {}
```

### `OverlayComponent`
<!-- catalog:component source="OverlayComponent" mms="direct" names="Overlay" -->
Carries overlay state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Overlay engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Overlay`. Sources: [Rust implementation](../../../src/engine/ecs/component/overlay.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Overlay {}
```

### `RenderGraphComponent`
<!-- catalog:component source="RenderGraphComponent" mms="direct" names="RenderGraph" -->
Carries render graph state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `RenderGraph`. Sources: [Rust implementation](../../../src/engine/ecs/component/render_graph.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
RenderGraph {}
```

### `RenderableComponent`
<!-- catalog:component source="RenderableComponent" mms="direct" names="Renderable" -->
Describes drawable geometry and material state consumed by the renderable system. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `Renderable`. Sources: [Rust implementation](../../../src/engine/ecs/component/renderable.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Renderable.cube()
```

### `RefractionComponent`
<!-- catalog:component source="RefractionComponent" mms="direct" names="Refraction" -->
Selects sharp screen-space refraction for its immediate parent renderable. `Color.rgba` supplies
the transmission tint and compositing alpha. IOR must be at least `1.0`; thickness and strength are
non-negative; edge fade is measured in normalized viewport coordinates and lies in `0.0..=0.5`.
**Directly constructible** as `Refraction`. Sources: [Rust implementation](../../../src/engine/ecs/component/transmission.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Renderable.cube() {
    Color.rgba(0.85, 0.95, 1.0, 0.75)
    Refraction.ior(1.45).thickness(0.08).strength(1.0).edge_fade(0.02)
}
```

### `RoughTransmissionComponent`
<!-- catalog:component source="RoughTransmissionComponent" mms="direct" names="RoughTransmission" -->
Selects filtered screen-space transmission for its immediate parent renderable. It shares the
sharp-refraction inputs and adds roughness in `0.0..=1.0`; `Color.rgba` supplies tint and alpha.
Attach at most one `Refraction` or `RoughTransmission` component to a renderable.
**Directly constructible** as `RoughTransmission`. Sources: [Rust implementation](../../../src/engine/ecs/component/transmission.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Renderable.cube() {
    Color.rgba(0.85, 0.95, 1.0, 0.75)
    RoughTransmission.ior(1.45).thickness(0.08).strength(1.0).edge_fade(0.02).roughness(0.4)
}
```

### `RendererSettingsComponent`
<!-- catalog:component source="RendererSettingsComponent" mms="direct" names="RendererSettings" -->
Carries renderer settings state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `RendererSettings`. Sources: [Rust implementation](../../../src/engine/ecs/component/renderer_settings.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
RendererSettings {}
```

### `RendererStatsComponent`
<!-- catalog:component source="RendererStatsComponent" mms="direct" names="RendererStats" -->
Carries renderer stats state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `RendererStats`. Sources: [Rust implementation](../../../src/engine/ecs/component/renderer_stats.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
RendererStats {}
```

### `StencilClipComponent`
<!-- catalog:component source="StencilClipComponent" mms="direct" names="StencilClip" -->
Carries stencil clip state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `StencilClip`. Sources: [Rust implementation](../../../src/engine/ecs/component/stencil_clip.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
StencilClip {}
```

### `TextureComponent`
<!-- catalog:component source="TextureComponent" mms="direct" names="Texture" -->
Carries texture state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `Texture`. Sources: [Rust implementation](../../../src/engine/ecs/component/texture.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Texture {}
```

### `TextureFilteringComponent`
<!-- catalog:component source="TextureFilteringComponent" mms="direct" names="TextureFiltering" -->
Carries texture filtering state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `TextureFiltering`. Sources: [Rust implementation](../../../src/engine/ecs/component/texture_filtering.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TextureFiltering {}
```

### `TransparentCutoutComponent`
<!-- catalog:component source="TransparentCutoutComponent" mms="direct" names="TransparentCutout" -->
Carries transparent cutout state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Transparent Cutout engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `TransparentCutout`. Sources: [Rust implementation](../../../src/engine/ecs/component/transparent_cutout.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TransparentCutout {}
```

### `UVComponent`
<!-- catalog:component source="UVComponent" mms="direct" names="UV" -->
Carries u v state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `UV`. Sources: [Rust implementation](../../../src/engine/ecs/component/uv.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
UV {}
```

## Lighting

### `ShadingComponent`
<!-- catalog:component source="ShadingComponent" mms="direct" names="Shading,AnimeShading" -->

Selects Anime or Toon shading for descendant meshes, with immediate local shading
children overriding inherited settings. `Shading {}` and `Shading.anime()` use
generic Anime defaults; `Shading.toon()` explicitly selects Toon. The wider
default-material migration and other built-in constructors are still pending.
`AnimeShading` is a temporary compatibility constructor for the same component.
Sources: [Rust implementation](../../../src/engine/ecs/component/anime_shading.rs)
and [live panel task](../../task/anime-shading-panel-and-live-shader-inputs.md).

```mms parse-only
Shading.anime().shade_strength(0.5).rim_strength(0.38) {
    R.cube() {}
    R.sphere() { Shading.toon() }
}
```

Anime builders accept `shade_color`, `shade_strength`, `shade_threshold`,
`lit_threshold`, `rim_color`, `rim_strength`, and `rim_power`. A retained Anime
reference supports getter/setter pairs for `shade_strength`, `shade_threshold`,
`lit_threshold`, `rim_strength`, and `rim_power` (for example,
`set_shade_strength(value)` and `get_shade_strength()`). Setters use builder
normalization and update source-linked GLTF primitives; getters return effective
state immediately. These live methods reject a Toon target. Live color inputs
remain planned.

### `ToonOutlineComponent`
<!-- catalog:component source="ToonOutlineComponent" mms="direct" names="ToonOutline" -->

Adds a batched inverted-hull outline to a renderable or to descendant renderables. When attached to
a `GLTF`, the importer projects the modifier onto each generated primitive. `width` is expressed in
world-space engine units and `color` is RGBA. Sources: [Rust implementation](../../../src/engine/ecs/component/toon_outline.rs), [MMS registry](../../../src/scripting/component_registry.rs), and [implementation notes](../../task/toon-outline-component-and-shared-deformation.md).

```mms parse-only
ToonOutline.width(0.012).color([0.015, 0.008, 0.025, 1.0]) {
    R.cube() {}
}
```

### `LightQuantizationComponent`
<!-- catalog:component source="LightQuantizationComponent" mms="direct" names="LightQuantization" -->
Carries light quantization state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `LightQuantization`. Sources: [Rust implementation](../../../src/engine/ecs/component/light_quantization.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
LightQuantization {}
```

## Input, interaction, and selection

### `GestureCoordTypeComponent`
<!-- catalog:component source="GestureCoordTypeComponent" mms="direct" names="GestureCoordType" -->
Carries gesture coord type state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Raycast, pointer, and gesture systems; `RayIntersected`, `DragStart`, `DragMove`, `DragEnd`, and `Click` are relevant.
**Directly constructible** as `GestureCoordType`. Sources: [Rust implementation](../../../src/engine/ecs/component/gesture_coord_type.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
GestureCoordType {}
```

### `InputComponent`
<!-- catalog:component source="InputComponent" mms="direct" names="Input" -->
Drives a direct child Transform from desktop input. `enabled` is the master pose-driver gate;
`translation_enabled` independently gates WASD/R/F locomotion, and `rotation_enabled`
independently gates mouse, arrow, and Q/E rotation. Live scripts can call
`set_translation_enabled(bool)` and `set_rotation_enabled(bool)` without disabling the other
channel. The compatibility `InputTransformMode.rotation_disabled()` setting remains an additional
rotation gate.
**Directly constructible** as `Input`. Sources: [Rust implementation](../../../src/engine/ecs/component/input.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Input.speed(2.0).translation_enabled(true).rotation_enabled(true) {}
```

### `InputTransformModeComponent`
<!-- catalog:component source="InputTransformModeComponent" mms="direct" names="InputTransformMode" -->
Carries input transform mode state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Input Transform Mode engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `InputTransformMode`. Sources: [Rust implementation](../../../src/engine/ecs/component/input_transform_mode.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
InputTransformMode {}
```

### `InputXRComponent`
<!-- catalog:component source="InputXRComponent" mms="direct" names="InputXR" -->
Carries input xr state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Camera/XR systems; registration intents and XR button/axis events are relevant.
**Directly constructible** as `InputXR`; `InputVR` is also accepted as a compatibility alias. Sources: [Rust implementation](../../../src/engine/ecs/component/input_xr.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
InputXR {}
```

### `InputXRGamepadComponent`
<!-- catalog:component source="InputXRGamepadComponent" mms="direct" names="InputXRGamepad" -->
Carries input xr gamepad state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Camera/XR systems; registration intents and XR button/axis events are relevant.
**Directly constructible** as `InputXRGamepad`; compatibility aliases are `InputXrGamepad`, `InputVRGamepad`, and `InputVrGamepad`. Sources: [Rust implementation](../../../src/engine/ecs/component/input_xr_gamepad.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
InputXRGamepad {}
```

### `OptionComponent`
<!-- catalog:component source="OptionComponent" mms="direct" names="Option" -->
Carries option state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Option engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Option`. Sources: [Rust implementation](../../../src/engine/ecs/component/option.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Option {}
```

### `PointerComponent`
<!-- catalog:component source="PointerComponent" mms="direct" names="Pointer" -->
Carries pointer state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Raycast, pointer, and gesture systems; `RayIntersected`, `DragStart`, `DragMove`, `DragEnd`, and `Click` are relevant.
**Directly constructible** as `Pointer`. Sources: [Rust implementation](../../../src/engine/ecs/component/pointer.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Pointer.min_grab_distance(0.05) {}
Pointer.debug_enable(true) {}
```

Controller/hand pointers default to 0.05 m grab clearance. Desktop-camera and XR-head
pointers default to 0.75 m. `min_grab_distance` overrides that value per pointer.
`debug_enable(true)` is the generic per-pointer diagnostics switch. Its first diagnostic is a
visualization of the active start-ray drag-mapping surface; the surface exists only for that
pointer's active drag and is removed when the gesture ends.

### `GrabbableComponent`
<!-- catalog:component source="GrabbableComponent" mms="direct" names="Grabbable" -->

`Grabbable`, `Grabbable.on()`, and `Grabbable.parent()` mark transforms for attachment-style
grabbing. XR grip and desktop left mouse temporarily reparent the resolved target beneath the
pointer-driving transform while preserving world pose; release restores the original parent.
**Directly constructible** as `Grabbable`. Sources: [Rust implementation](../../../src/engine/ecs/component/grabbable.rs) and [MMS registry](../../../src/scripting/component_registry.rs).

```mms parse-only
T { Grabbable {} }
```

### `RiderComponent`
<!-- catalog:component source="RiderComponent" mms="direct" names="Rider" -->

`Rider` declares the alignment anchor, movement root, and automatic locomotion input for a
participant that can enter a `Mountable`. References accept selectors or live component objects.
The attachment system preserves XR tracking while suppressing only the referenced built-in
locomotion mapping during a mount.
**Directly constructible** as `Rider`. Sources: [Rust implementation](../../../src/engine/ecs/component/rider.rs) and [MMS registry](../../../src/scripting/component_registry.rs).

```mms parse-only
Rider
    .anchor("[name='rider_anchor']")
    .movement_root("[name='locomotion_root']")
    .input("[name='pedestrian_input']") {}
```

### `MountableComponent`
<!-- catalog:component source="MountableComponent" mms="direct" names="Mountable" -->

`Mountable` declares a single-seat destination with an entry zone, a mounted alignment anchor,
and a dismount anchor. Grip activation mounts only when the pointer-associated Rider's anchor is
inside the entry zone. An enabled Mountable supplies its owner with a runtime-only raycast marker
when one is not already authored.
**Directly constructible** as `Mountable`. Sources: [Rust implementation](../../../src/engine/ecs/component/mountable.rs) and [MMS registry](../../../src/scripting/component_registry.rs).

```mms parse-only
Mountable
    .entry_zone("[name='entry_zone']")
    .mount_anchor("[name='seat']")
    .dismount_anchor("[name='exit']")
    .on_grip() {}
```

### `DraggableComponent`
<!-- catalog:component source="DraggableComponent" mms="direct" names="Draggable" -->

`Draggable` and `Draggable.on()` move the marker's owning transform. `Draggable.parent()` moves
the next transform above that owner. `Draggable.target(ref)` instead requires one explicit
transform, where `ref` may be a selector, a live component object, or `@uuid:` GUID. Selectors are
local by default; prefix them with `../` to climb authoring scopes or `/` to search world roots.

Explicit resolution is strict: missing, ambiguous, or non-transform targets do not fall back to
the owner or parent. A live cached target remains sticky, selector targets may bind a replacement
after deletion, and GUID targets never retarget. The resolved target is captured at `DragStart`
and cannot switch before that gesture's `DragEnd`. All target modes chain with
`.plane("object" | "camera")` or two authored world axes.
**Directly constructible** as `Draggable`; canonical serialization writes the owner, parent, or
explicit target constructor before an optional chained plane call. Sources: [Rust implementation](../../../src/engine/ecs/component/draggable.rs) and [MMS registry](../../../src/scripting/component_registry.rs).

```mms parse-only
T { Draggable.plane("camera") {} }
T { Draggable.target("../#panel_root").plane("camera") {} }
```

### `RayCastComponent`
<!-- catalog:component source="RayCastComponent" mms="direct" names="Raycast" -->
Carries ray cast state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Raycast, pointer, and gesture systems; `RayIntersected`, `DragStart`, `DragMove`, `DragEnd`, and `Click` are relevant.
**Directly constructible** as `Raycast`. Sources: [Rust implementation](../../../src/engine/ecs/component/raycast.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Raycast {}
```

### `RaycastableComponent`
<!-- catalog:component source="RaycastableComponent" mms="direct" names="Raycastable" -->
Carries raycastable state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Raycast, pointer, and gesture systems; `RayIntersected`, `DragStart`, `DragMove`, `DragEnd`, and `Click` are relevant.
**Directly constructible** as `Raycastable`. Sources: [Rust implementation](../../../src/engine/ecs/component/raycastable.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Raycastable {}
Raycastable.drag_continuation("captured").drag_mapping("start_ray_plane") {}
```

Drag continuation accepts `"auto"`, `"require_target_contact"`, or `"captured"`. Drag mapping
accepts `"auto"`, `"contact_hit"`, or `"start_ray_plane"`. The two policies are independent.
`"auto"` preserves legacy inference: controller `Draggable` targets follow captured controller
translation, ordinary desktop drags use a captured start-ray plane under the default gesture
setting, and ordinary spatial targets require contact and use the current hit point.

### `RaycastableShapeComponent`
<!-- catalog:component source="RaycastableShapeComponent" mms="direct" names="RaycastableShape" -->
Carries raycastable shape state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Raycast, pointer, and gesture systems; `RayIntersected`, `DragStart`, `DragMove`, `DragEnd`, and `Click` are relevant.
**Directly constructible** as `RaycastableShape`. Sources: [Rust implementation](../../../src/engine/ecs/component/raycastable_shape.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
RaycastableShape {}
```

### `SelectableComponent`
<!-- catalog:component source="SelectableComponent" mms="direct" names="Selectable" -->
Carries selectable state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Selection system; `SelectionChanged`, `SelectionAdded`, `SelectionRemoved`, and `SelectionCleared` are relevant.
**Directly constructible** as `Selectable`. Sources: [Rust implementation](../../../src/engine/ecs/component/selectable.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Selectable {}
```

### `SelectionComponent`
<!-- catalog:component source="SelectionComponent" mms="direct" names="Selection" -->
Carries selection state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Selection system; `SelectionChanged`, `SelectionAdded`, `SelectionRemoved`, and `SelectionCleared` are relevant.
**Directly constructible** as `Selection`. Sources: [Rust implementation](../../../src/engine/ecs/component/selection.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Selection {}
```

### `SelectionStyleStateComponent`
<!-- catalog:component source="SelectionStyleStateComponent" mms="engine-only" names="" -->
Internal selection-system state used to restore visual styles after deselection. Use it when a tree needs this state or behavior. Selection system; `SelectionChanged`, `SelectionAdded`, `SelectionRemoved`, and `SelectionCleared` are relevant.
**Engine-only.** Direct MMS construction is unavailable; constructing `Selection` is the closest public way to make the selection system create and use this state. Sources: [Rust implementation](../../../src/engine/ecs/system/selection_system.rs).
```mms parse-only
Selection {}
```

### `ToggleComponent`
<!-- catalog:component source="ToggleComponent" mms="direct" names="Toggle" -->
Carries an independent boolean UI value. Clicking its styled owner flips the value, updates the standard active highlight, and emits `ToggleChanged`; `ToggleSet` synchronizes it programmatically.
**Directly constructible** as `Toggle.on()` or `Toggle.off()`. Sources: [Rust implementation](../../../src/engine/ecs/component/toggle.rs), [toggle system](../../../src/engine/ecs/system/toggle_system.rs), and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Toggle.on()
```

### `SliderComponent`
<!-- catalog:component source="SliderComponent" mms="direct" names="Slider" -->
Carries a finite horizontal numeric range, optional step, current value, and disabled state. `SliderSystem` owns pointer/keyboard interaction and the stable track and thumb mounts; authored component trees may supply the graphics.
**Directly constructible** as `Slider`, including builder calls such as `range`, `step`, `value`, `width`, `track`, and `thumb`. Live methods expose `value()`, `set_value(...)`, silent `sync_value(...)`, `track_mount()`, and `thumb_mount()`. Sources: [Rust implementation](../../../src/engine/ecs/component/slider.rs), [slider system](../../../src/engine/ecs/system/slider_system.rs), and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Slider.range(0.0, 1.0).step(0.1).value(0.5)
```

### `TextInputComponent`
<!-- catalog:component source="TextInputComponent" mms="direct" names="TextInput" -->
Carries text input state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Text-input system; focus/edit intents and `TextInputFocusChanged`/`TextInputChanged` are relevant.
**Directly constructible** as `TextInput`. Sources: [Rust implementation](../../../src/engine/ecs/component/text_input.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TextInput {}
```

### `TextInputGlyphHitComponent`
<!-- catalog:component source="TextInputGlyphHitComponent" mms="engine-only" names="" -->
Internal glyph hit target created by the text-input system for caret placement. Use it when a tree needs this state or behavior. Text-input system; focus/edit intents and `TextInputFocusChanged`/`TextInputChanged` are relevant.
**Engine-only.** Direct MMS construction is unavailable; constructing `TextInput` is the closest public way to make the text-input system create glyph hit targets. Sources: [Rust implementation](../../../src/engine/ecs/component/text_input.rs).
```mms parse-only
TextInput { "editable" }
```

## Cameras and XR

### `Camera2DComponent`
<!-- catalog:component source="Camera2DComponent" mms="direct" names="Camera2D" -->
Carries camera2 d state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Camera/XR systems; registration intents and XR button/axis events are relevant.
**Directly constructible** as `Camera2D`. Sources: [Rust implementation](../../../src/engine/ecs/component/camera_2d.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Camera2D {}
```

### `Camera3DComponent`
<!-- catalog:component source="Camera3DComponent" mms="direct" names="Camera3D" -->
Carries camera3 d state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Camera/XR systems; registration intents and XR button/axis events are relevant.
**Directly constructible** as `Camera3D`. Sources: [Rust implementation](../../../src/engine/ecs/component/camera_3d.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Camera3D {}
```

### `CameraXRComponent`
<!-- catalog:component source="CameraXRComponent" mms="direct" names="CameraXR" -->
Carries camera xr state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Camera/XR systems; registration intents and XR button/axis events are relevant.
**Directly constructible** as `CameraXR`. Sources: [Rust implementation](../../../src/engine/ecs/component/camera_xr.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
CameraXR {}
```

### `ControllerXRComponent`
<!-- catalog:component source="ControllerXRComponent" mms="alias" names="XRHand" -->
Carries controller xr state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Camera/XR systems; registration intents and XR button/axis events are relevant.
**Available through an alias.** Construct this Rust type as `XRHand`; compatibility aliases are `XrHand`, `VRHand`, and `VrHand`. Sources: [Rust implementation](../../../src/engine/ecs/component/controller_xr.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
XRHand.new(true, "Left", "Aim").laser()
```

`.laser()` adds one runtime-only, noninteractive cyan direction laser along local `-Z`.
Avatar alignment is intentionally separate: author `JointRetargetBasis` beneath the avatar GLTF
and a `RestAttachment` around the pointer content.

### `XrComponent`
<!-- catalog:component source="XrComponent" mms="direct" names="XR" -->
Carries xr state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Camera/XR systems; registration intents and XR button/axis events are relevant.
**Directly constructible** as `XR`. Sources: [Rust implementation](../../../src/engine/ecs/component/xr.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
XR {}
```

## Physics and motion

### `CollisionComponent`
<!-- catalog:component source="CollisionComponent" mms="direct" names="Collision" -->
Carries collision state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Collision systems; registration/removal intents plus `CollisionStarted` and `CollisionEnded` are relevant.
**Directly constructible** as `Collision`. Sources: [Rust implementation](../../../src/engine/ecs/component/collision.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Collision {}
```

### `CollisionResponseComponent`
<!-- catalog:component source="CollisionResponseComponent" mms="direct" names="CollisionResponse" -->
Carries collision response state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Collision systems; registration/removal intents plus `CollisionStarted` and `CollisionEnded` are relevant.
**Directly constructible** as `CollisionResponse`. Sources: [Rust implementation](../../../src/engine/ecs/component/collision_response.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
CollisionResponse {}
```

### `CollisionShapeComponent`
<!-- catalog:component source="CollisionShapeComponent" mms="direct" names="CollisionShape" -->
Carries collision shape state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Collision systems; registration/removal intents plus `CollisionStarted` and `CollisionEnded` are relevant.
**Directly constructible** as `CollisionShape`. Sources: [Rust implementation](../../../src/engine/ecs/component/collision_shape.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
CollisionShape {}
```

### `GravityComponent`
<!-- catalog:component source="GravityComponent" mms="direct" names="Gravity" -->
Carries gravity state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Gravity engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Gravity`. Sources: [Rust implementation](../../../src/engine/ecs/component/gravity.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Gravity {}
```

### `SecondaryMotionComponent`
<!-- catalog:component source="SecondaryMotionComponent" mms="direct" names="SecondaryMotion" -->
Carries secondary motion state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Secondary Motion engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `SecondaryMotion`. Sources: [Rust implementation](../../../src/engine/ecs/component/secondary_motion.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
SecondaryMotion {}
```

### `JointRetargetBasisComponent`
<!-- catalog:component source="JointRetargetBasisComponent" mms="direct" names="JointRetargetBasis" -->
Declares a canonical two-axis rest basis for one joint in the nearest ancestor GLTF armature. All five references resolve uniquely within that imported armature; runtime matrices and diagnostics are retained by `JointBasisRetargetingSystem`.
**Directly constructible** as `JointRetargetBasis`. Sources: [Rust implementation](../../../src/engine/ecs/component/joint_retarget_basis.rs), [retained runtime](../../../src/engine/ecs/system/joint_basis_retargeting_system.rs), and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
JointRetargetBasis.new("#hand", "#middle1", "#middle3", "#little1", "#index1")
```

### `HumanoidBoneMapComponent`
<!-- catalog:component source="HumanoidBoneMapComponent" mms="direct" names="HumanoidBoneMap" -->

**Directly constructible** as `HumanoidBoneMap`. It declares GLTF-owned semantic humanoid slots;
Auto is enabled by default, explicit references and absences take precedence, and runtime reports
are retained outside authored serialization.

```mms
HumanoidBoneMap.new()
  .slot("left_hand", "[name='J_Bip_L_Hand']")
  .absent("neck")
  .automap_disable()
```

### `RestAttachmentComponent`
<!-- catalog:component source="RestAttachmentComponent" mms="direct" names="RestAttachment" -->
Declares an imported target's immutable rest transform relative to an imported anchor. Resolution
is scoped to the owning GLTF supplied by the consumer and is independent from basis correction.
**Directly constructible** as `RestAttachment`. Sources: [Rust implementation](../../../src/engine/ecs/component/rest_attachment.rs), [runtime resolution](../../../src/engine/ecs/system/rest_attachment.rs), and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
RestAttachment.new("#hand", "#middle3")
```

### `SpringBoneComponent`
<!-- catalog:component source="SpringBoneComponent" mms="direct" names="SpringBone" -->
Carries spring bone state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `SpringBone`. Sources: [Rust implementation](../../../src/engine/ecs/component/secondary_motion.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
SpringBone.from_root("[name='tail']").virtual_end_length_ratio(1.0)
```

### `SpringCollidersComponent`
<!-- catalog:component source="SpringCollidersComponent" mms="direct" names="SpringColliders" -->
Groups explicitly authored spring collider configurations within a glTF instance.
**Directly constructible** as `SpringColliders`. Sources: [Rust implementation](../../../src/engine/ecs/component/secondary_motion.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
SpringColliders {}
```

### `SpringColliderComponent`
<!-- catalog:component source="SpringColliderComponent" mms="direct" names="SpringCollider" -->
Defines one or more spherical secondary-motion colliders at explicit glTF node targets.
**Directly constructible** as `SpringCollider`. Sources: [Rust implementation](../../../src/engine/ecs/component/secondary_motion.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
SpringCollider.sphere("[name='J_Bip_C_Hips']", 0.11)
```

### `SpringJointComponent`
<!-- catalog:component source="SpringJointComponent" mms="direct" names="SpringJoint" -->
Carries spring joint state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Spring Joint engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `SpringJoint`. Sources: [Rust implementation](../../../src/engine/ecs/component/secondary_motion.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
SpringJoint {}
```

## Layout, text, and HTML

### `HtmlElementComponent`
<!-- catalog:component source="HtmlElementComponent" mms="direct" names="HtmlElement" -->
Carries html element state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Html Element engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `HtmlElement`. Sources: [Rust implementation](../../../src/engine/ecs/component/html_element.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
HtmlElement {}
```

### `InspectLayoutComponent`
<!-- catalog:component source="InspectLayoutComponent" mms="direct" names="InspectLayout" -->
Carries inspect layout state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Layout/scrolling systems; layout registration, `LayoutRootSizeAvailable`, drag, and `Scrolling` signals are relevant.
**Directly constructible** as `InspectLayout`. Sources: [Rust implementation](../../../src/engine/ecs/component/inspect_layout.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
InspectLayout {}
```

### `LayoutBoundsComponent`
<!-- catalog:component source="LayoutBoundsComponent" mms="direct" names="LayoutBounds" -->
Carries layout bounds state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Layout/scrolling systems; layout registration, `LayoutRootSizeAvailable`, drag, and `Scrolling` signals are relevant.
**Directly constructible** as `LayoutBounds`. Sources: [Rust implementation](../../../src/engine/ecs/component/layout_bounds.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
LayoutBounds {}
```

### `LayoutComponent`
<!-- catalog:component source="LayoutComponent" mms="direct" names="LayoutRoot" -->
Marks a subtree as a layout root and supplies its available dimensions to the layout system. Use it when a tree needs this state or behavior. Layout/scrolling systems; layout registration, `LayoutRootSizeAvailable`, drag, and `Scrolling` signals are relevant.
**Directly constructible** as `LayoutRoot`. Sources: [Rust implementation](../../../src/engine/ecs/component/layout.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
LayoutRoot {}
```

### `ScrollingComponent`
<!-- catalog:component source="ScrollingComponent" mms="direct" names="Scrolling" -->
Carries scrolling state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Layout/scrolling systems; layout registration, `LayoutRootSizeAvailable`, drag, and `Scrolling` signals are relevant.
**Directly constructible** as `Scrolling`. Sources: [Rust implementation](../../../src/engine/ecs/component/scrolling.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Scrolling {}
```

### `StyleComponent`
<!-- catalog:component source="StyleComponent" mms="direct" names="Style" -->
Carries style state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Layout/scrolling systems; layout registration, `LayoutRootSizeAvailable`, drag, and `Scrolling` signals are relevant.
**Directly constructible** as `Style`. Sources: [Rust implementation](../../../src/engine/ecs/component/style.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Style {}
```

### `TextComponent`
<!-- catalog:component source="TextComponent" mms="direct" names="Text" -->
Carries text state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Text engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Text`. Sources: [Rust implementation](../../../src/engine/ecs/component/text.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Text {}
```

### `TextShadowComponent`
<!-- catalog:component source="TextShadowComponent" mms="direct" names="TextShadow" -->
Carries text shadow state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Text Shadow engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `TextShadow`. Sources: [Rust implementation](../../../src/engine/ecs/component/text_shadow.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
TextShadow {}
```

## Audio and music

### `AudioBandPassFilterComponent`
<!-- catalog:component source="AudioBandPassFilterComponent" mms="engine-only" names="" -->
Carries audio band pass filter state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Engine-only.** Direct MMS construction is unavailable; `AudioOutput` is the closest public audio-graph component. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_band_pass_filter.rs).
```mms parse-only
AudioOutput {}
```

### `AudioBufferSizeComponent`
<!-- catalog:component source="AudioBufferSizeComponent" mms="engine-only" names="" -->
Carries audio buffer size state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Engine-only.** Direct MMS construction is unavailable; `AudioOutput` is the closest public audio-graph component. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_buffer_size.rs).
```mms parse-only
AudioOutput {}
```

### `AudioClipComponent`
<!-- catalog:component source="AudioClipComponent" mms="direct" names="AudioClip" -->
Carries audio clip state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Directly constructible** as `AudioClip`. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_clip.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
AudioClip {}
```

### `AudioGainComponent`
<!-- catalog:component source="AudioGainComponent" mms="engine-only" names="" -->
Carries audio gain state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Engine-only.** Direct MMS construction is unavailable; `AudioOutput` is the closest public audio-graph component. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_gain.rs).
```mms parse-only
AudioOutput {}
```

### `AudioHighPassFilterComponent`
<!-- catalog:component source="AudioHighPassFilterComponent" mms="engine-only" names="" -->
Carries audio high pass filter state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Engine-only.** Direct MMS construction is unavailable; `AudioOutput` is the closest public audio-graph component. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_high_pass_filter.rs).
```mms parse-only
AudioOutput {}
```

### `AudioLimiterComponent`
<!-- catalog:component source="AudioLimiterComponent" mms="engine-only" names="" -->
Carries audio limiter state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Engine-only.** Direct MMS construction is unavailable; `AudioOutput` is the closest public audio-graph component. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_limiter.rs).
```mms parse-only
AudioOutput {}
```

### `AudioLowPassFilterComponent`
<!-- catalog:component source="AudioLowPassFilterComponent" mms="engine-only" names="" -->
Carries audio low pass filter state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Engine-only.** Direct MMS construction is unavailable; `AudioOutput` is the closest public audio-graph component. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_low_pass_filter.rs).
```mms parse-only
AudioOutput {}
```

### `AudioMixComponent`
<!-- catalog:component source="AudioMixComponent" mms="engine-only" names="" -->
Carries audio mix state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Engine-only.** Direct MMS construction is unavailable; `AudioOutput` is the closest public audio-graph component. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_mix.rs).
```mms parse-only
AudioOutput {}
```

### `AudioOscillatorComponent`
<!-- catalog:component source="AudioOscillatorComponent" mms="direct" names="AudioOscillator" -->
Carries audio oscillator state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Directly constructible** as `AudioOscillator`. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_oscillator.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
AudioOscillator {}
```

### `AudioOutputComponent`
<!-- catalog:component source="AudioOutputComponent" mms="direct" names="AudioOutput" -->
Carries audio output state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Directly constructible** as `AudioOutput`. Sources: [Rust implementation](../../../src/engine/ecs/component/audio_output.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
AudioOutput {}
```

### `ClockComponent`
<!-- catalog:component source="ClockComponent" mms="direct" names="Clock" -->
Carries clock state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Directly constructible** as `Clock`. Sources: [Rust implementation](../../../src/engine/ecs/component/clock.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Clock {}
```

### `MusicNoteComponent`
<!-- catalog:component source="MusicNoteComponent" mms="direct" names="MusicNote" -->
Carries music note state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Audio and clock systems; audio graph, scheduling, playback, and clock registration intents are relevant.
**Directly constructible** as `MusicNote`. Sources: [Rust implementation](../../../src/engine/ecs/component/music_note.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
MusicNote.a(4, 1)
```

## Signals, data, networking, and lifecycle

### `AssetPayloadComponent`
<!-- catalog:component source="AssetPayloadComponent" mms="direct" names="AssetPayload" -->
Carries asset payload state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Asset Payload engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `AssetPayload`. Sources: [Rust implementation](../../../src/engine/ecs/component/asset_payload.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
AssetPayload {}
```

### `DataComponent`
<!-- catalog:component source="DataComponent" mms="direct" names="Data" -->
Carries data state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Data engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Data`. Sources: [Rust implementation](../../../src/engine/ecs/component/data.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Data {}
```

### `HttpClientComponent`
<!-- catalog:component source="HttpClientComponent" mms="direct" names="HttpClient" -->
Carries http client state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. HTTP client/server systems; request/reply intents and `HttpRequest`, `HttpResponse`, and `HttpError` are relevant.
**Directly constructible** as `HttpClient`. Sources: [Rust implementation](../../../src/engine/ecs/component/http_client.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
HttpClient {}
```

### `HttpServerComponent`
<!-- catalog:component source="HttpServerComponent" mms="direct" names="HttpServer" -->
Carries http server state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. HTTP client/server systems; request/reply intents and `HttpRequest`, `HttpResponse`, and `HttpError` are relevant.
**Directly constructible** as `HttpServer`. Sources: [Rust implementation](../../../src/engine/ecs/component/http_server.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
HttpServer {}
```

### `RouterComponent`
<!-- catalog:component source="RouterComponent" mms="direct" names="Router" -->
Routes selected signals between scopes according to configured routing rules. Use it when a tree needs this state or behavior. RX/pipeline systems; registration intents and routed events are the important signals.
**Directly constructible** as `Router`. Sources: [Rust implementation](../../../src/engine/ecs/component/router.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Router {}
```

### `SerializeComponent`
<!-- catalog:component source="SerializeComponent" mms="direct" names="Serialize" -->
Carries serialize state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Serialize engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Serialize`. Sources: [Rust implementation](../../../src/engine/ecs/component/serialize.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Serialize {}
```

### `SignalObserverRouterComponent`
<!-- catalog:component source="SignalObserverRouterComponent" mms="direct" names="ObserverRouter" -->
Filters named data events before they reach observers in a routed subtree. Use it when a tree needs this state or behavior. RX/pipeline systems; registration intents and routed events are the important signals.
**Directly constructible** as `ObserverRouter`. Sources: [Rust implementation](../../../src/engine/ecs/component/signal_observer_router.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
ObserverRouter {}
```

### `SignalRouteUpwardComponent`
<!-- catalog:component source="SignalRouteUpwardComponent" mms="direct" names="SignalRouteUpward" -->
Projects a named event from one scope upward under another name. Use it when a tree needs this state or behavior. RX/pipeline systems; registration intents and routed events are the important signals.
**Directly constructible** as `SignalRouteUpward`. Sources: [Rust implementation](../../../src/engine/ecs/component/signal_route_upward.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
SignalRouteUpward {}
```

## Animation, rigging, and poses

### `AnimationComponent`
<!-- catalog:component source="AnimationComponent" mms="direct" names="Animation" -->
Carries animation state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Animation engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Animation`. Sources: [Rust implementation](../../../src/engine/ecs/component/animation.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Animation {}
```

### `AvatarBodyYawComponent`
<!-- catalog:component source="AvatarBodyYawComponent" mms="direct" names="AvatarBodyYaw" -->
Carries avatar body yaw state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `AvatarBodyYaw`. Sources: [Rust implementation](../../../src/engine/ecs/component/avatar_body_yaw.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
AvatarBodyYaw {}
```

### `AvatarControlComponent`
<!-- catalog:component source="AvatarControlComponent" mms="direct" names="AvatarControl" -->
Carries avatar control state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `AvatarControl`. Sources: [Rust implementation](../../../src/engine/ecs/component/avatar_control.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
AvatarControl {}
```

### `BoneRestPoseComponent`
<!-- catalog:component source="BoneRestPoseComponent" mms="engine-only" names="" -->
Stores an imported bone rest pose used by skinning and pose systems. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Engine-only.** Direct MMS construction is unavailable; loading `GLTF` is the public path that indirectly creates imported bone rest-pose components. Sources: [Rust implementation](../../../src/engine/ecs/component/bone_rest_pose.rs).
```mms parse-only
GLTF.uri("model.glb")
```

### `IKChainComponent`
<!-- catalog:component source="IKChainComponent" mms="direct" names="IKChain" -->
Carries ik chain state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `IKChain`. Sources: [Rust implementation](../../../src/engine/ecs/component/ik_chain.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
IKChain {}
```

### `KeyframeComponent`
<!-- catalog:component source="KeyframeComponent" mms="direct" names="Keyframe" -->
Carries keyframe state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Keyframe engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Keyframe`. Sources: [Rust implementation](../../../src/engine/ecs/component/keyframe.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Keyframe.at(0)
```

### `PoseCaptureComponent`
<!-- catalog:component source="PoseCaptureComponent" mms="direct" names="PoseCapture" -->
Carries pose capture state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `PoseCapture`. Sources: [Rust implementation](../../../src/engine/ecs/component/pose_capture.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
PoseCapture {}
```

### `PoseCaptureLibraryComponent`
<!-- catalog:component source="PoseCaptureLibraryComponent" mms="direct" names="PoseCaptureLibrary" -->
Carries pose capture library state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `PoseCaptureLibrary`. Sources: [Rust implementation](../../../src/engine/ecs/component/pose_capture.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
PoseCaptureLibrary {}
```

### `PoseCapturePoseComponent`
<!-- catalog:component source="PoseCapturePoseComponent" mms="direct" names="PoseCapturePose" -->
Carries pose capture pose state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `PoseCapturePose`. Sources: [Rust implementation](../../../src/engine/ecs/component/pose_capture.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
PoseCapturePose.new("idle")
```

### `QuatExtractYawComponent`
<!-- catalog:component source="QuatExtractYawComponent" mms="engine-only" names="" -->
Extracts yaw from an upstream quaternion in the transform pipeline. Use it when a tree needs this state or behavior. The Quat Extract Yaw engine subsystem; its component lifecycle is processed at signal drain points.
**Engine-only.** Direct MMS construction is unavailable; `QuatYawFollow` is the closest public quaternion/yaw pipeline component. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_temporal_filter.rs).
```mms parse-only
QuatYawFollow {}
```

### `QuatTemporalFilterComponent`
<!-- catalog:component source="QuatTemporalFilterComponent" mms="direct" names="QuatTemporalFilter" -->
Carries quat temporal filter state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Quat Temporal Filter engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `QuatTemporalFilter`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_temporal_filter.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
QuatTemporalFilter {}
```

### `QuatYawFollowComponent`
<!-- catalog:component source="QuatYawFollowComponent" mms="direct" names="QuatYawFollow" -->
Carries quat yaw follow state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Quat Yaw Follow engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `QuatYawFollow`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_temporal_filter.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
QuatYawFollow {}
```

### `Vector3TemporalFilterComponent`
<!-- catalog:component source="Vector3TemporalFilterComponent" mms="direct" names="Vector3TemporalFilter" -->
Carries vector3 temporal filter state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Vector3 Temporal Filter engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Vector3TemporalFilter`. Sources: [Rust implementation](../../../src/engine/ecs/component/transform_temporal_filter.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Vector3TemporalFilter {}
```

## Editor and gizmos

### `EditorComponent`
<!-- catalog:component source="EditorComponent" mms="direct" names="Editor" -->
Carries editor state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Editor engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Editor`. Sources: [Rust implementation](../../../src/engine/ecs/component/editor.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Editor {}
```

### `EditorUIComponent`
<!-- catalog:component source="EditorUIComponent" mms="direct" names="EditorUI" -->
Owns the shared editor workspace and its canonically ordered typed panel specifications. `EditorUI {}` enables every panel with default configuration.
**Directly constructible** as `EditorUI`. Sources: [Rust implementation](../../../src/engine/ecs/component/editor_ui.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
EditorUI { panels([{ panel = "settings" config = {} }]) }
```

### `GridComponent`
<!-- catalog:component source="GridComponent" mms="direct" names="Grid" -->
Carries grid state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Grid engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Grid`. Sources: [Rust implementation](../../../src/engine/ecs/component/grid.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Grid {}
```

### `GridBindingComponent`
<!-- catalog:component source="GridBindingComponent" mms="direct" names="GridBinding" -->
Persists the grid used to snap a manipulated transform. Place it directly under
that transform and pass a grid owner transform handle or selector. Live handles
serialize as GUID references, preserving the association across save and load.
**Directly constructible** as `GridBinding`. Sources: [Rust implementation](../../../src/engine/ecs/component/grid_binding.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
GridBinding.grid("#grid_transform")
```

## Other engine components

### `AmbientLightComponent`
<!-- catalog:component source="AmbientLightComponent" mms="direct" names="AmbientLight" -->
Carries ambient light state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `AmbientLight`. Sources: [Rust implementation](../../../src/engine/ecs/component/ambient_light.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
AmbientLight {}
```

### `DirectionalLightComponent`
<!-- catalog:component source="DirectionalLightComponent" mms="direct" names="DirectionalLight" -->
Carries directional light state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `DirectionalLight`. Sources: [Rust implementation](../../../src/engine/ecs/component/directional_light.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
DirectionalLight {}
```

### `PointLightComponent`
<!-- catalog:component source="PointLightComponent" mms="direct" names="PointLight" -->
Carries point light state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `PointLight`. Sources: [Rust implementation](../../../src/engine/ecs/component/point_light.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
PointLight {}
```

### `SpotLightComponent`
<!-- catalog:component source="SpotLightComponent" mms="direct" names="SpotLight" -->
Carries spot light state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. Rendering systems; lifecycle registration/removal intents connect it to visual state.
**Directly constructible** as `SpotLight`. Sources: [Rust implementation](../../../src/engine/ecs/component/spot_light.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
SpotLight {}
```

### `TransitionComponent`
<!-- catalog:component source="TransitionComponent" mms="direct" names="Transition" -->
Carries transition state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Transition engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Transition`. Sources: [Rust implementation](../../../src/engine/ecs/component/transition.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Transition {}
```
