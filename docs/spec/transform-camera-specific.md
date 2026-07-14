# TransformCameraSpecific

`TransformCameraSpecificComponent` selects one ordinary TRS settings transform according to the
camera family currently driving the frame. It is a transform-stream primitive: it changes the
world basis inherited by downstream content without introducing a special renderer material or
shader path.

The component currently supports two constructors:

```text
TransformCameraSpecific.active_monoscopic
TransformCameraSpecific.active_stereoscopic
```

The important limitation is that selection produces one effective ECS world matrix. It does not
allow the same content to have a monoscopic transform in a desktop view and a stereoscopic
transform in an XR view at the same time.

## Goals

- Keep camera-specific settings as ordinary authored `TransformComponent` TRS values.
- Make the selected matrix consistent across rendering, `TransformSystem::world_model`, BVH
  bounds, and narrow-phase raycasting.
- Use the existing transform-stream propagation and boundary rules.
- Support camera-dependent policies such as constant-angular-size gizmos without requiring a
  custom shader.

## Authored topology

The generic anchor is a `TransformComponent`. Camera-specific markers and their settings
transforms are configuration children; all other direct children are downstream content.

```text
anchor Transform
  TransformCameraSpecific.active_monoscopic
    mono_settings Transform
  TransformCameraSpecific.active_stereoscopic
    stereo_settings Transform
  downstream_content
    renderables / transforms / other components
```

For each marker, the first direct `TransformComponent` child is its settings transform. The marker
and settings transform are not traversed as rendered downstream content.

Both settings transforms remain normal local transforms. They can carry translation, rotation,
and scale, and serialize using the regular `Transform` representation. Camera policy belongs to
the system that writes those values, not to `TransformCameraSpecificComponent` itself.

## Evaluation contract

Evaluation happens in this order:

1. `TransformSystem` evaluates the generic anchor and its upstream transform-stream basis.
2. `TransformStreamSystem` selects the settings transform for the active camera family.
3. It computes:

   ```text
   effective_world = generic_anchor_world * selected_settings_local
   ```

4. The anchor's cached `matrix_world` becomes `effective_world`.
5. Ordinary downstream children inherit `effective_world`.

Composition order matters. A settings translation is expressed in the generic anchor's local
basis; it is not a world-space post-process.

If the selected family has no marker or no settings transform, the generic anchor world matrix is
used unchanged. Stereo is selected when an active XR camera has published one or more eyes.
Otherwise mono is selected.

Camera-specific anchors are transform-stream boundaries. If an independently updated descendant
is propagated later, `TransformSystem` preserves the selected basis instead of reconstructing its
world matrix solely from authored ancestor TRS values.

## Preventing repeated-refresh compounding

The cached anchor `matrix_world` contains the effective camera-specific result. A later direct
refresh of that anchor may therefore arrive with the previous effective matrix as its input. If
the settings scale were multiplied onto that matrix again, the content would grow or shrink every
frame.

`TransformStreamSystem` retains both the pre-camera basis and the previous effective matrix for
each camera-specific anchor. When a refresh feeds back the previous effective matrix, evaluation
reuses the retained pre-camera basis. A genuinely new upstream matrix replaces that basis.

This invariant is essential:

```text
refresh(refresh(anchor)) == refresh(anchor)
```

for unchanged upstream and settings transforms.

## Gizmo use

`TransformGizmoSystem` uses this primitive to keep editor gizmos approximately constant in angular
size. The gizmo first uses the existing TRS stream to keep inherited translation and rotation but
drop inherited scale. Its visual anchor then has mono and stereo settings transforms:

```text
target Transform
  gizmo TransformGizmo
    gizmo_pipeline TransformForkTRS (drops inherited scale)
      gizmo_root Transform
        TransformCameraSpecific.active_monoscopic
          mono_settings Transform
        TransformCameraSpecific.active_stereoscopic
          stereo_settings Transform
        Overlay
          gizmo handles
```

After window and XR cameras publish their current matrices, but before pointer activation and
raycasting, the gizmo system calculates the selected settings scale:

```text
effective_scale = gizmo.scale * positive_camera_depth / 4.0
```

The final scale is clamped to `[0.02, 20.0]`. Mono depth comes from the active window camera.
Stereo depth is averaged across the active XR eyes, producing one cyclopean scale. Missing camera
data or an anchor behind the active camera leaves the last valid settings scale unchanged.

After changing settings, the engine refreshes the camera-specific anchor, propagates its effective
matrix to `VisualWorld`, and refits affected BVH bounds before raycasting. Rendering and picking
therefore observe the same scale in the same frame.

## What it does not do

`TransformCameraSpecific` does not currently produce a matrix per render view or per eye.

There is one selected effective `matrix_world` for the anchor at any moment. Consequently:

- both XR eyes share one cyclopean transform;
- a desktop mirror rendered while XR is active sees the XR-selected transform too;
- window and XR render targets cannot simultaneously use different settings transforms for the
  same renderable;
- the BVH contains one world-space bound, not a bound per camera family;
- `TransformSystem::world_model` has only one answer for the anchor and its descendants.

This is intentional for the current phase. A single matrix keeps rendering, scene queries, and
picking coherent and is sufficient for constant-angular gizmos in either desktop-only or active-XR
operation.

It is also not a constant-pixel-width mechanism. Lines and strokes that must retain an exact pixel
width still require renderer or shader support.

## Speculative paths to simultaneous camera-family transforms

Supporting different transforms for window and XR views at the same time changes the model from
"one component has one world transform" to "one component may have a transform per view family."
That decision reaches beyond this primitive. Several designs are plausible.

### 1. Renderer-side per-view model overrides

The ECS could retain one canonical `matrix_world`, while `VisualWorld` stores optional model
overrides keyed by renderable and camera family:

```text
(renderable, Window) -> mono model
(renderable, Xr)     -> stereo/cyclopean model
```

The renderer would choose the override when building each view. This is relatively contained for
visuals, but BVH and raycasting would still need an explicit policy: use canonical bounds, maintain
family-specific bounds, or transform rays into a canonical picking space.

### 2. Multi-valued transform streams

`TransformStreamSystem` could produce a bundle rather than one matrix:

```text
TransformVariants {
    canonical,
    window,
    xr,
}
```

Propagation would preserve variants through descendant transforms, and consumers would request a
family explicitly. This is the most general interpretation, but it substantially changes
`TransformSystem::world_model`, renderable updates, lights, collisions, skinning, and every other
consumer currently expecting one matrix.

### 3. Duplicate view-specific presentation subtrees

The engine or author could instantiate separate presentation subtrees for window and XR while
sharing a logical target:

```text
logical gizmo state
  window presentation subtree
  XR presentation subtree
```

Each subtree would keep an ordinary single matrix and could be filtered by render target. This
preserves current transform semantics but duplicates renderables and interaction bookkeeping.
Picking would need to map either presentation back to the same logical gizmo operation.

### 4. Shader-derived camera-relative scale

For constant-angular objects specifically, the renderer could derive scale from camera-space depth
in the vertex shader. Each render view and XR eye would then naturally receive its own apparent
scale without multiple ECS matrices.

This is attractive for rendering and exact per-eye behavior, but CPU picking would no longer match
automatically. The BVH could use conservative bounds and narrow-phase tests could reproduce the
same camera-relative scale, or picking could move to a view-aware GPU/CPU query path.

### Likely incremental direction

A practical next phase would probably start with renderer-side per-family overrides because it
limits the initial blast radius. Gizmos could retain their canonical cyclopean ECS/BVH transform
for interaction while the desktop mirror receives a mono visual override. If exact mirror picking
is later required, family-specific BVH bounds or a view-aware narrow phase could be added.

Before adopting that design, the engine should explicitly answer:

- Which transform is canonical for non-render systems?
- Can callers request `world_model(component, camera_family)`?
- Does picking select bounds based on the ray's source camera family?
- Are XR transforms cyclopean or genuinely per-eye?
- How are per-view overrides inherited by descendant transforms?
- How are logical interaction targets shared across duplicated or overridden presentations?

Until those contracts are defined, `TransformCameraSpecific` deliberately remains a
single-effective-matrix transform-stream primitive.
