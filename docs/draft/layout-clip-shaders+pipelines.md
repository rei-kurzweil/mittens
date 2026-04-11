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

## Approach

### Stencil Buffer

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

---

## Implementation Status ( ᐛ )و

### ✅ Done

#### Depth/stencil format (`vulkano_swapchain.rs`, `post_processing.rs`)

- `VulkanoSwapchainState::DEPTH_FORMAT` changed from `D32_SFLOAT` →
  `D32_SFLOAT_S8_UINT`.
- All depth image allocations (window, XR offscreen, post-process) now use this format.
- Image views are created with explicit `ImageAspects::DEPTH | ImageAspects::STENCIL`
  via `ImageViewCreateInfo` rather than `new_default`.

**Vulkan constraint discovered:** dynamic rendering requires the **same** image view
object for both `depth_attachment` and `stencil_attachment` when using a combined
format. Separate depth-only / stencil-only views are rejected by
`VUID-VkRenderingInfo-pDepthAttachment-06085`. The combined view satisfies both
`VUID-VkRenderingInfo-pStencilAttachment-06548` (must have stencil aspect) and the
same-view constraint.

#### Stencil attachment (deferred — `vulkano_renderer.rs`)

- `stencil_attachment` is currently **`None`** in both render scopes.
- The combined depth+stencil view is ready to wire in once the new pipelines exist.
- **Reason for deferral:** all existing pipelines were created without
  `stencil_attachment_format` in `PipelineRenderingCreateInfo`. Enabling stencil
  attachment while old pipelines are active triggers
  `VUID-vkCmdDrawIndexed-pStencilAttachment-06182`. The attachment goes live
  alongside the new pipelines.

#### VisualWorld data model (`visual_world.rs`)

- `VisualInstance` — new fields: `stencil_ref: u8`, `is_stencil_clip: bool`
- `DrawBatch` — new field: `stencil_ref: u8`; batch boundary breaks on `stencil_ref`
  change so clipped/unclipped instances never merge.
- `stencil_clip_order: Vec<u32>` — indices into `instances` where
  `is_stencil_clip=true`, sorted ascending by `stencil_ref`. Rebuilt with draw cache.
- Overlay sort key updated: `stencil_ref` prepended as primary key so all instances
  for the same clip region are consecutive.
- New public API: `register_stencil_clip(handle, stencil_ref)`,
  `unregister_stencil_clip(handle)`, `update_stencil_ref(handle, stencil_ref)`,
  `stencil_clip_order() -> &[u32]`.
- `register()` initialises both new fields to `0` / `false`.

### 🔲 Remaining

#### New Vulkan pipelines (`VulkanoState`)

| Pipeline | Color write | Depth | Stencil op | Stencil test |
|---|---|---|---|---|
| `pipeline_stencil_write` | off | off/pass-always | REPLACE ref=N | always |
| `pipeline_stencil_clear` | off | off | REPLACE ref=0 | always |
| `pipeline_overlay_clipped` | on | same as overlay | KEEP | EQUAL ref=N |
| `pipeline_emissive_overlay_clipped` | on | same | KEEP | EQUAL ref=N |

All existing overlay pipelines need `stencil_attachment_format` added to
`PipelineRenderingCreateInfo` and explicit `stencil test = always, op = KEEP`.
This is also when `stencil_attachment: Some(...)` gets wired back in.

The `stencil_ref` value changes per clip-region group — delivered as a push constant.

#### Draw loop (`record_overlay_draws()` in `vulkano_cbb.rs`)

```
for each (stencil_ref, batches) group in overlay_batches:
  if stencil_ref == 0:
    draw batches with pipeline_overlay (unchanged)
  else:
    look up clip quad from stencil_clip_order
    draw clip quad  → pipeline_stencil_write, ref = stencil_ref
    draw batches    → pipeline_overlay_clipped, ref = stencil_ref
    restore stencil → pipeline_stencil_write, ref = parent stencil_ref (or 0)
```

#### ECS (`StencilClipComponent`)

See [ECS Changes Needed](#ecs-changes-needed) section below — design unchanged.

#### Layout wiring (`sync_bg_quad`)

When `overflow: Hidden | Scroll` is set, `sync_bg_quad` attaches
`StencilClipComponent` to `__bg_tc`. When it returns to `Visible`, it detaches.
When `background_color: None` but `overflow: Hidden | Scroll`, `__bg_tc` is still
spawned with a transparent `ColorComponent` so the clip geometry exists.

---

## ECS Changes Needed

### `StencilClip` Component (new, first-class)

A standalone ECS component. **Not layout-specific** — anything can attach one.

```rust
/// Declares a renderable as a stencil clip boundary.
///
/// Attach alongside a `RenderableComponent` on any TC. On `init()`, emits
/// `RegisterStencilClip` (analogous to `RegisterRenderable`) so VisualWorld
/// records it immediately — no scanning required.
///
/// The renderer draws the referenced renderable into the stencil buffer
/// (color write off, stencil REPLACE, ref = `stencil_ref`) before drawing
/// the TC's descendants, then restores stencil afterward. The same renderable
/// is also drawn normally in the color pass — it does double duty.
///
/// ## Node hierarchy (layout case)
///
/// ```text
/// item_tc   (TransformComponent — layout positions this)
///   StyleComponent { overflow: Hidden, background_color: Some(...) }
///   __bg_tc  (TransformComponent — sized to content box by block::layout)
///     StencilClipComponent     ← wraps the renderable below
///     ColorComponent
///     RenderableComponent      ← clip shape; also the normal background quad
/// ```
///
/// ## Manual use
///
/// Attach to any TC + `RenderableComponent`. The mesh shape determines the clip
/// region — a square for rectangular clips, a circle, or an arbitrary hull.
pub struct StencilClipComponent {
    /// Stencil reference value for this clip boundary.
    /// `0` = VisualWorld assigns based on ancestor nesting depth at registration time.
    /// Set explicitly only when managing depth manually.
    pub stencil_ref: u8,
}

impl Component for StencilClipComponent {
    fn init(&mut self, id: ComponentId, emit: &mut dyn SignalEmitter) {
        // Mirrors RenderableComponent::init / RegisterRenderable.
        emit.push_intent_now(id, IntentValue::RegisterStencilClip {
            component_ids: vec![id],
        });
    }
    fn cleanup(&mut self, id: ComponentId, emit: &mut dyn SignalEmitter) {
        emit.push_intent_now(id, IntentValue::UnregisterStencilClip {
            component_ids: vec![id],
        });
    }
    // ...
}
```

`RegisterStencilClip` is handled by `RxIntentExecutor` → VisualWorld, which:
1. Walks the ECS ancestor chain from `id` to find the nearest enclosing
   `StencilClipComponent` ancestor (for nesting depth).
2. Assigns `stencil_ref = ancestor_depth + 1` (1-indexed, 0 = unclipped).
3. Records the clip entry — the `VisualInstance` for the sibling `RenderableComponent`
   on the same TC is already registered (or will be); VisualWorld marks that instance
   as `is_stencil_clip = true`.

**Lifecycle (layout):**
- `sync_bg_quad` spawns `__bg_tc` with `ColorComponent` + `RenderableComponent` when
  `background_color` is set. `init_component_tree` on `__bg_tc` fires `RegisterRenderable`.
- When `overflow: Hidden | Scroll` is also present, `sync_bg_quad` additionally attaches
  `StencilClipComponent` to `__bg_tc`. `init` fires `RegisterStencilClip` immediately.
- When `overflow` returns to `Visible`, `StencilClipComponent` is detached; `cleanup`
  fires `UnregisterStencilClip` to remove it from VisualWorld.
- When `background_color: None` but `overflow: Hidden | Scroll`: `sync_bg_quad` still
  spawns `__bg_tc` with a transparent `ColorComponent` so the geometry exists.
  Both `RegisterRenderable` and `RegisterStencilClip` fire on init.

**Lifecycle (manual):** attach `StencilClipComponent` to any TC with a `RenderableComponent`.
`RegisterStencilClip` fires immediately via `init()`.

**Why reuse `__bg` rather than a dedicated stencil-only node?**

The background quad already covers exactly the right region (the padding box). A second
renderable at the same position would be wasted geometry. `__bg` does double duty:
color draw in the normal pass, stencil write before the clipped subtree draws.

### New intent variants

```rust
IntentValue::RegisterStencilClip { component_ids: Vec<ComponentId> }
IntentValue::UnregisterStencilClip { component_ids: Vec<ComponentId> }
```

Handled by `RxIntentExecutor` → `VisualWorld::register_stencil_clip(id)` /
`VisualWorld::unregister_stencil_clip(id)`. Mirrors the existing `RegisterRenderable` /
`RemoveSubtree` path.

### `StyleComponent` / `LayoutSystem` wiring

- `StyleComponent::overflow` already exists (`Overflow` enum: `Visible | Hidden | Scroll | Auto`).
- `block::layout` calls `sync_bg_quad` for each item. `sync_bg_quad` is extended to also
  read `overflow` from the item's `StyleComponent` children and manage `StencilClipComponent`
  on `__bg_tc` accordingly.
- No other layout code changes.

---

## Open Questions / To Confirm

1. **MSAA + stencil** — with MSAA, the stencil buffer is also multisampled.
   Stencil write quad must be drawn with MSAA off or the same sample count, TBD.
2. **Ordering within stencil groups** — overlay instances are currently instanced and
  batched by material/texture/mesh. Clipped instances break batching unless all
  instances in a batch share the same clip region. May need per-draw-call bucketing
  by clip region.
3. **Stencil quad mesh** — use the existing `square` primitive (`RenderableComponent::square()`).
   The stencil write pipeline needs a pipeline that accepts the same vertex format.
4. **`overflow: scroll` vs `overflow: hidden`** — same clip behavior; `scroll` additionally
   enables `ScrollingComponent` interaction (already implemented). Clip region logic is identical.
5. **Non-overlay renderables** — `overflow: hidden` on non-overlay content (e.g. 3D mesh
   children inside a clipping volume) needs the same treatment. Punting this for now;
   initial implementation targets the overlay/panel use case.
