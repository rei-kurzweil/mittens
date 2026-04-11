# Layout Clip — Shaders & Pipelines

Draft spec for implementing `overflow: hidden | scroll` clipping in cat-engine.

---

## Problem

CSS `overflow: hidden` and `overflow: scroll` require that content outside a container's
content box is not visible. In a 2D browser this is a rectangle clip in screen space.
In cat-engine, layout nodes are 3D objects (via `TransformComponent::matrix_world`) and can be
placed at arbitrary positions and orientations (e.g. a VR panel tilted 30°). This means:

- **`DiscardRectangleMode`** (Vulkano / `VK_EXT_discard_rectangles`) is out — it only
  accepts screen-axis-aligned pixel rects and cannot track a 3D panel.
- **`set_scissor`** same problem — screen-aligned only.
- Children of an `overflow: hidden` container can themselves contain
  `overflow: hidden` children, so the solution needs to nest correctly.

---

## Candidate Methods

### A — Stencil Buffer (recommended, confirmed viable)

**Concept:**

Use the GPU stencil buffer to mark the *visible region* of an `overflow: hidden`
container before drawing its children.

**Render order per frame:**

1. **Stencil write sub-pass** (new, before overlay draw):  
   For each `overflow: hidden` container, render its content-box quad with:
   - Color write mask = `0` (no color output)
   - Depth write = off, depth test = off (or pass-always)
   - Stencil op = `REPLACE` with reference value = nesting depth (1-indexed)
   
   This "stamps" the container's visible area into the stencil buffer.

2. **Clipped content draw sub-pass** (modified overlay pipeline):  
   Children inside `overflow: hidden` containers draw with stencil test:
   - `EQUAL` to the nesting depth of their innermost `overflow: hidden` ancestor.

3. **Stencil restoration** after all nested content:  
   Decrement stencil back (or clear to 0) once a container's subtree is done,
   to restore the parent's clip region for siblings.

**Nesting:** increment reference value per level of `overflow: hidden` nesting.
Max nesting depth = GPU stencil bit depth (8 bits on all Vulkan hardware → 255 levels,
more than enough).

**What this needs confirmed:**
- Does `VulkanoState`'s dynamic rendering setup for overlay already include a
  stencil attachment, or does it need to be added to the renderpass?  
  → Check `begin_rendering` call in `vulkano_renderer.rs` for overlay phase.
- Stencil format: `D16_UNORM` has no stencil; need `D24_UNORM_S8_UINT` or
  `D32_SFLOAT_S8_UINT`. Check current depth format used.

**Pipeline changes needed (VulkanoState):**
- `pipeline_stencil_write` — new pipeline:  
  minimal vert + null frag, color writes off, stencil `REPLACE`.
- `pipeline_overlay_clipped` (and emissive variant) — modified overlay pipeline:  
  stencil test `EQUAL` to push-constant ref value.
- All existing non-clipped pipelines remain unchanged (stencil test = always pass,
  stencil op = keep).

---

### B — Fragment Shader Local-Space Bounds Test (simpler, flat panels only)

**Concept:**

Pass the `overflow: hidden` container's inverse world matrix + content-box AABB to
the fragment shader. Each fragment transforms its world position into the container's
local space and discards if outside the box.

```glsl
// push constant or per-instance data
uniform mat4 clip_inv_model;   // inverse of container's matrix_world
uniform vec4 clip_min;         // content box min in local space (xyz, w unused)
uniform vec4 clip_max;         // content box max in local space

// in fragment shader
vec3 local = (clip_inv_model * vec4(v_world_pos, 1.0)).xyz;
if (any(lessThan(local, clip_min.xyz)) || any(greaterThan(local, clip_max.xyz)))
    discard;
```

**Pros:** No extra render passes. Works for flat/non-rotating panels.

**Cons:**
- Nesting requires passing all ancestor clip rects — shader needs array of clip volumes.
- Per-draw uniform changes break current instanced batching (each clipped batch
  would need its own draw call).
- World position reconstruction in fragment shader is available (`v_world_pos` is
  already output by `toon-mesh.vert`), so fragment side is cheap; the per-draw
  overhead is the real cost.
- For non-planar panels (rotated in 3D) the local-space bounds test is still correct
  as long as the container's world transform is used — but the stencil approach
  handles this automatically without any shader logic.

**Verdict:** Useful as a quick fallback or for a "single flat panel" use case with
no nesting, but doesn't scale cleanly to the general case.

---

### C — Clip Distances (`gl_ClipDistance`)

Vulkan vertex shaders can write up to 8 clip distances (`gl_ClipDistance[i]`).
A negative value discards the vertex (hardware clips the primitive).

**How it would work:** Each `overflow: hidden` container defines 4 clip planes
(top, bottom, left, right of the content box in world space). Pass these as UBO.
Vertex shader evaluates `dot(world_pos, plane) + d` for each plane and writes to
`gl_ClipDistance`.

**Problems:**
- Requires `VkPhysicalDeviceFeatures::shaderClipDistance = true` — widely supported
  but must be confirmed enabled in `VulkanoConfig`.
- 8-plane limit caps nesting at 2 levels of `overflow: hidden` (4 planes each).
- Clipping is per-primitive (at rasterization), not per-fragment, so the clip boundary
  is a hard geometric edge — fine for axis-aligned panels, slightly wrong for panels
  with anti-aliased borders.
- Needs pipeline rebuild with `gl_ClipDistance` feature enabled per pipeline.

**Verdict:** Worth considering for the non-nested case (single clip level), but the
nesting limit rules it out as the primary mechanism.

---

## Recommended Approach

**Primary: Stencil buffer (Method A).**

Stencil is the standard GPU mechanism for this exact problem (browsers use it internally
for `overflow: hidden`). It handles arbitrary nesting, works for any 3D orientation, and
the geometry used to write the stencil mask is the same content-box quad already in the
scene graph.

**Possible hybrid:** Use fragment-shader bounds test (Method B) for the most common case
(a single non-nested panel clip) and fall back to stencil for nested cases. The shader
variant can be selected per-draw based on whether the instance is inside a nested clip.
This avoids the extra stencil pass for the common case. Whether this complexity is worth it
is TBD.

---

## ECS Changes Needed

### `StencilClip` Component (new, first-class)

A standalone ECS component that marks a `TransformComponent` node as a stencil clip region.
**Not layout-specific** — anything can attach one.

```rust
/// Marks this TC as a stencil clip boundary.
///
/// The renderer renders a quad at this TC's world transform (scaled to `size`)
/// into the stencil buffer before drawing the subtree, then restores stencil
/// afterward. All descendant renderables are drawn with `stencil_ref` as their
/// stencil test reference.
///
/// Attach alongside a `TransformComponent`. Size is in the TC's local units.
///
/// `LayoutSystem` auto-attaches this when it encounters `overflow: Hidden | Scroll`
/// on a `StyleComponent`, using the item's computed content-box dimensions.
/// Manual attachment is also valid for non-layout use cases (e.g. a custom
/// clip mask on a 3D panel or HUD element).
pub struct StencilClipComponent {
    /// Width and height of the clip quad in the TC's local units.
    pub size: [f32; 2],
    /// Stencil reference value. Assigned automatically by the renderer based on
    /// ancestor nesting depth; can also be set manually.
    /// `0` = unclipped. `1` = outermost clip. Higher = deeper nesting.
    pub stencil_ref: u8,
}
```

**Lifecycle:**
- `LayoutSystem` calls `world.add_component` to attach `StencilClipComponent` to the
  layout item's TC when `overflow: Hidden | Scroll` is first encountered, sized to the
  content box.
- On subsequent layout passes (resize, reflow), `LayoutSystem` updates `size` in-place
  if the content box changes.
- `LayoutSystem` removes it (via `RemoveSubtree` intent or direct detach) when
  `overflow` changes back to `Visible`.
- Manual use: attach directly to any TC; `stencil_ref = 0` lets the renderer assign
  the nesting depth automatically at sync time.

**Why a component, not a flag on `StyleComponent`?**

Keeps the clip region as a first-class ECS node — it has a `ComponentId` that
`VisualWorld` and `VulkanoRenderer` can reference directly, without coupling them to
`StyleComponent` internals. Non-layout subsystems (e.g. a custom HUD panel, a
map/minimap frame, a portal effect) can clip their subtrees without any layout involvement.

### `StyleComponent` / `LayoutSystem` wiring

- `StyleComponent::overflow` already exists (`Overflow` enum: `Visible | Hidden | Scroll | Auto`).
- During `block::layout`, when an item's `StyleComponent.overflow` is `Hidden | Scroll`:
  1. Read the computed content-box dimensions (`box_width_gu`, `box_height_gu` from `MeasuredItem`).
  2. Convert to TC local units: `size = [box_width_gu * unit_scale, box_height_gu * unit_scale]`.
  3. If no `StencilClipComponent` child exists on the item TC → attach one.
  4. If one exists → update `size` if changed.
- When `overflow` is `Visible` and a `StencilClipComponent` child exists → detach/remove it.

---

## VisualWorld Changes

### `VisualInstance` additions

```rust
/// Stencil reference value for this instance.
/// 0 = unclipped (stencil test disabled / always-pass).
/// ≥1 = draw only where stencil == this value (set by an ancestor StencilClipComponent).
pub stencil_ref: u8,
```

`stencil_ref` is resolved at sync time: when `RegisterRenderable` fires, VisualWorld
walks the ECS ancestor chain looking for `StencilClipComponent` nodes. The deepest
ancestor's `stencil_ref` is used. If no ancestor has one, `stencil_ref = 0`.

Ancestor depth assignment: VisualWorld (or a pre-pass in the system tick) walks all
`StencilClipComponent` nodes depth-first and assigns `stencil_ref = 1, 2, 3...`
by nesting level. This assignment only needs to run when the ECS tree topology
changes (handled via dirty flags or the existing `Attach`/`Detach` intent path).

### New per-frame data

```rust
/// Ordered list of stencil clip quads to render this frame, outer → inner.
/// Built during draw-cache rebuild from all live StencilClipComponent nodes.
/// Each entry: (model_matrix, [width, height] in world units, stencil_ref).
stencil_clip_quads: Vec<([[f32; 4]; 4], [f32; 2], u8)>,
dirty_stencil_clips: bool,
```

Built by scanning `all_components()` for `StencilClipComponent` + sibling
`TransformComponent::matrix_world`. Rebuilt when `dirty_draw_cache` is set.

### Draw grouping

Overlay instances are currently sorted by material+mesh for instanced batching.
With stencil clipping, instances must additionally be grouped by `stencil_ref` so
the stencil quad is written before the group and restored after.

Proposed sort key: `(stencil_ref, material, mesh)` — instances with `stencil_ref=0`
draw first (unclipped), then grouped by ascending stencil depth.

Within each stencil group, existing batch-merging still applies.

---

## VulkanoRenderer Changes

### Depth/stencil attachment

Current depth format needs to be checked:
- If `D16_UNORM` → must change to `D24_UNORM_S8_UINT` or `D32_SFLOAT_S8_UINT`.
- The `begin_rendering` call for the overlay phase must include a stencil attachment.

### New pipelines

| Pipeline | Color write | Depth | Stencil op | Stencil test |
|---|---|---|---|---|
| `pipeline_stencil_write` | off | off/pass-always | REPLACE ref=N | always |
| `pipeline_stencil_clear` | off | off | REPLACE ref=0 | always |
| `pipeline_overlay_clipped` | on | same as overlay | KEEP | EQUAL ref=N |
| `pipeline_emissive_overlay_clipped` | on | same | KEEP | EQUAL ref=N |

Existing overlay pipelines: add `stencil test = always, stencil op = KEEP` (no-op
but makes state explicit and avoids undefined stencil behavior).

The `stencil_ref` value is a push constant (already used for other per-draw data,
or a new one). It changes per clip-region group.

### Draw loop change (overlay phase)

```
stencil_ref = 0
for each (region, instances) in overlay draw groups:
  if region == Unclipped:
    draw with pipeline_overlay
  else:
    write stencil for region quad  → pipeline_stencil_write, ref = region.depth
    draw instances                  → pipeline_overlay_clipped, ref = region.depth
    clear stencil for region quad  → pipeline_stencil_clear, ref = 0
```

For nested regions the "clear stencil" step is replaced by "restore parent stencil"
(re-draw the parent region quad with REPLACE ref = parent.depth).

---

## Open Questions / To Confirm

1. **Current depth format** — check `vulkano_renderer.rs` near depth image creation.
   Stencil support requires `D24_UNORM_S8_UINT` or `D32_SFLOAT_S8_UINT`.
2. **XR path** — `xr_renderer.rs` has its own render loop. Stencil changes must be
   mirrored there.
3. **MSAA + stencil** — with MSAA, the stencil buffer is also multisampled.
   Stencil write quad must be drawn with MSAA off or the same sample count, TBD.
4. **Ordering within stencil groups** — overlay instances are currently instanced and
   batched by material+mesh. Clipped instances break batching unless all instances in
   a batch share the same clip region. May need per-draw-call bucketing by clip region.
5. **Stencil quad mesh** — use the existing `square` primitive (`RenderableComponent::square()`).
   The stencil write pipeline needs a pipeline that accepts the same vertex format.
6. **`overflow: scroll` vs `overflow: hidden`** — same clip behavior; `scroll` additionally
   enables `ScrollingComponent` interaction (already implemented). Clip region logic is identical.
7. **Non-overlay renderables** — `overflow: hidden` on non-overlay content (e.g. 3D mesh
   children inside a clipping volume) needs the same treatment. Punting this for now;
   initial implementation targets the overlay/panel use case.
