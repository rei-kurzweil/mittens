# MMS component guide

This is the canonical catalog of concrete engine components and their current Meow Meow Script (MMS) exposure. The Rust implementation is authoritative; this guide deliberately links to it instead of duplicating builder and field APIs that change frequently.

MMS status means:

- **Directly constructible** — the component's canonical MMS name is in `SUPPORTED_COMPONENT_NAMES`.
- **Available through an alias** — MMS constructs this Rust type under a different public name.
- **Engine-only** — runtime systems may create or use it, but MMS cannot construct it directly. Its example shows the closest supported relationship and is not proposed syntax.

Every `mms parse-only` fence is syntax-checked by documentation tests. Fences marked `mms runnable` are also evaluated in an isolated world with render assets.

For common composition patterns, see the [MMS language guide](../../meow_meow/README.md). For signal semantics and the exhaustive signal catalog, see the [MMS signal guide](signals.md).

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

### `SkinnedMeshComponent`
<!-- catalog:component source="SkinnedMeshComponent" mms="direct" names="SkinnedMesh" -->
Carries skinned mesh state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `SkinnedMesh`. Sources: [Rust implementation](../../../src/engine/ecs/component/skinned_mesh.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
SkinnedMesh {}
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
Carries input state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. The Input engine subsystem; its component lifecycle is processed at signal drain points.
**Directly constructible** as `Input`. Sources: [Rust implementation](../../../src/engine/ecs/component/input.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Input {}
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
```

Controller/hand pointers default to 0.05 m grab clearance. Desktop-camera and XR-head
pointers default to 0.75 m. `min_grab_distance` overrides that value per pointer.

### `GrabbableComponent`

`Grabbable`, `Grabbable.on()`, and `Grabbable.parent()` mark transforms for attachment-style
grabbing. XR grip and desktop left mouse temporarily reparent the resolved target beneath the
pointer-driving transform while preserving world pose; release restores the original parent.

```mms parse-only
T { Grabbable {} }
```

### `DraggableComponent`

`Draggable` retains planar translation behavior. XR trigger and desktop left mouse drag it;
`.parent()` targets the next parent transform and `.plane("object" | "camera")` or two authored
world axes constrain movement.

```mms parse-only
T { Draggable.plane("camera") {} }
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
```

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

### `SpringBoneComponent`
<!-- catalog:component source="SpringBoneComponent" mms="direct" names="SpringBone" -->
Carries spring bone state used when that engine feature is present in a component tree. Use it when a tree needs this state or behavior. glTF, animation, avatar, IK, or pose systems; lifecycle intents and `GltfInitialized` are relevant.
**Directly constructible** as `SpringBone`. Sources: [Rust implementation](../../../src/engine/ecs/component/secondary_motion.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
SpringBone {}
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

### `ActionComponent`
<!-- catalog:component source="ActionComponent" mms="direct" names="Action" -->
Stores an authored intent template that is resolved and emitted during component initialization. Use it when a tree needs this state or behavior. RX/pipeline systems; registration intents and routed events are the important signals.
**Directly constructible** as `Action`. Sources: [Rust implementation](../../../src/engine/ecs/component/action.rs) and [MMS registry](../../../src/scripting/component_registry.rs).
```mms parse-only
Action {}
```

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
