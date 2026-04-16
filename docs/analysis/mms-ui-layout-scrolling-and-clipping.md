# MMS UI layout scrolling and clipping

Date: 2026-04-15

Tracks the current findings for the `examples/ui-layout.mms` scrolling + clipping reference shape before making further `src/` changes.

---

## Goal

We want the authored MMS shape in [examples/ui-layout.mms](../../examples/ui-layout.mms) to work as a first-class reference for:

- clipping content to a viewport defined by `R.plane()`
- scrolling content inside that viewport
- using the same architectural ideas that `InspectorSystem` uses for editor panels

The immediate question is:

- what topology does `InspectorSystem` actually spawn today?
- how does that differ from the authored `ui-layout.mms` topology?
- which differences explain the current failures: no clipping, no scrolling, and overly large vertical spacing?

---

## 1. What `InspectorSystem` actually creates

The panel topology is created in [src/engine/ecs/system/inspector_system.rs](../../src/engine/ecs/system/inspector_system.rs).

### World / inspector panel topology

The actual spawned shape is roughly:

```text
panel_anchor                      SelectableComponent::off()
└── panel_transform               TransformComponent(position = panel pose)
    ├── panel_layout              LayoutComponent(width, height, unit_scale = TEXT_SCALE)
    │   ├── header_slot           TransformComponent
    │   │   ├── header_el         HtmlElementComponent::header()
    │   │   ├── header_style      StyleComponent(height = title bar, margin.bottom = gap)
    │   │   ├── panel_titlebar_t  TransformComponent
    │   │   │   └── panel_titlebar_col
    │   │   │       └── panel_titlebar_r   RenderableComponent::square()
    │   │   └── panel_titlebar_label_t
    │   │       └── panel_titlebar_label_col
    │   │           └── panel_titlebar_label   TextComponent
    │   └── content_slot          TransformComponent
    │       ├── content_style     StyleComponent { overflow = Scroll }
    │       ├── drag_plane_t      TransformComponent
    │       │   └── drag_plane_col
    │       │       └── drag_plane_r         RenderableComponent::square()
    │       │           ├── drag_plane_opacity
    │       │           ├── drag_plane_rc    RaycastableComponent::drag_only()
    │       │           └── drag_plane_shape RaycastableShapeComponent::Quad2D
    │       └── panel_component   WorldPanelComponent or InspectorPanelComponent
    │           └── scrolling     ScrollingComponent
    │               └── rows_track          TransformComponent
    │                   └── rows_layout     LayoutComponent(unit_scale = TEXT_SCALE)
    │                       └── row TransformComponents...
    └── panel_gizmo               TransformGizmoComponent
```

### Important observations

1. The viewport boundary is the `content_slot` layout item, not the `ScrollingComponent` itself.
2. Clipping is requested by `content_style.overflow = Overflow::Scroll`.
3. That layout overflow causes `LayoutSystem` to auto-manage a `__bg` helper quad plus `StencilClipComponent` under the `content_slot` layout item.
4. Scrolling is separate from clipping:
   - `ScrollingComponent` owns offset / moved track
   - `content_slot` owns the clip viewport
5. Drag input does not come from the content rows.
   - It comes from a dedicated `drag_plane` subtree with `RaycastableComponent::drag_only()`.
6. There is currently no `OverlayComponent` in this spawned panel path, despite some older comments elsewhere implying one.

---

## 2. How clipping works for inspector panels

The clip path is layout-driven, not authored manually in panel code.

### Layout-managed clip shape

In [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs):

- every layout item can get an auto-managed `__bg` helper transform
- when its `StyleComponent` has `overflow = Hidden | Scroll`, layout attaches a `StencilClipComponent`
- the helper `__bg` quad becomes the stencil clip geometry

So the effective clip structure for the panel content is conceptually:

```text
content_slot
├── content_style { overflow = Scroll }
├── __bg                  auto-managed by LayoutSystem
│   ├── ColorComponent
│   ├── RenderableComponent::square()
│   └── StencilClipComponent
├── drag_plane_t
└── panel_component
    └── ScrollingComponent
        └── rows_track
            └── rows...
```

That means the scrolled rows are siblings of the auto-managed clip quad, inside the same clipped layout item.

---

## 3. What `ui-layout.mms` is authoring

The authored reference in [examples/ui-layout.mms](../../examples/ui-layout.mms) is:

```text
viewport_t                  Transform(position, scale = 1.8)
└── viewport_plane          RenderableComponent::plane()
    ├── ColorComponent
    └── StencilClipComponent
        └── TransformPipeline
            ├── TransformForkTRS
            │   ├── TransformMapTranslation
            │   ├── TransformMapRotation
            │   ├── TransformMapScale
            │   │   └── TransformDrop
            │   └── TransformMergeTRS
            └── TransformPipelineOutput
                └── content_t       TransformComponent
                    └── ScrollingComponent(viewport=1.0, content=100.0)
                        └── item transforms...
```

This is an intentionally different expression style from the inspector panels:

- manual clip shape instead of layout-generated `__bg`
- transform-pipeline-controlled viewport/content relationship
- no layout system involved for the rows
- no explicit drag plane

That difference is acceptable in principle. The runtime should support both:

1. layout-driven clipping for editor panels
2. manual `StencilClip { ... }` around authored content for MMS scenes

---

## 4. Topology comparison: inspector vs `ui-layout.mms`

### Inspector panel shape

```text
clip viewport root
├── overflow style
├── auto `__bg` clip quad
├── dedicated drag plane (raycastable)
└── scrolling subtree
    └── moved track
        └── layout rows
```

### `ui-layout.mms` shape

```text
manual clip plane renderable
├── color
└── stencil clip
    └── transform pipeline
        └── output transform
            └── scrolling subtree
                └── authored items
```

### Main mismatches

1. **Input surface mismatch**
   - Inspector panels have a dedicated drag target.
   - `ui-layout.mms` does not.

2. **Viewport ownership mismatch**
   - Inspector panels use a layout item with `overflow = Scroll` as the clip viewport.
   - `ui-layout.mms` uses a manual renderable clip plane.

3. **Content layout mismatch**
   - Inspector rows are laid out in glyph-space by `LayoutComponent` with `unit_scale = TEXT_SCALE`.
   - `ui-layout.mms` uses raw authored transforms like `T.position(0, y, 0.01)`.

4. **Scroll gesture hookup mismatch**
   - Inspector panels feed `DragMove` into scrolling via the drag plane.
   - `ui-layout.mms` has no obvious raycastable or drag-only surface in the clipped area.

---

## 5. Likely cause of “no scrolling” in `ui-layout.mms`

### Current scrolling runtime assumption

`ScrollingSystem` in [src/engine/ecs/system/scroll_system.rs](../../src/engine/ecs/system/scroll_system.rs):

- registers a `ScrollingComponent`
- finds a moved track transform from the nearest ancestor `TransformComponent`
- finds a drag scope from the nearest ancestor `StencilClipComponent` or `RenderableComponent`
- installs a `DragMove` handler on that ancestor scope

For the authored MMS subtree, that means:

- the moved track is likely the inner `T` under `TransformPipelineOutput`
- the drag scope is likely the `StencilClipComponent` ancestor

### Why that still does not produce user-visible scrolling

The runtime still needs `DragMove` events to exist.

The authored `ui-layout.mms` viewport currently has:

- no `RaycastableComponent::drag_only()` subtree
- no obvious explicit raycastable quad covering the viewport
- no dedicated input surface comparable to the inspector panel `drag_plane`

So the likely failure mode is:

1. `ScrollingComponent` registers successfully
2. `ScrollingSystem` installs a handler on the `StencilClip` scope
3. but nothing in this subtree actually produces drag events scoped into that branch
4. so `scroll_offset` never changes

### Conclusion

The current MMS subtree is missing the equivalent of the inspector panel drag plane.

That is the most likely reason the sample is not scrollable, independent of clipping.

---

## 6. Likely cause of the large vertical spacing

The spacing in the sample is very likely authored-space mismatch, not a separate clipping bug.

The authored rows are:

```c
T.position(0, y, 0.01).scale(0.12, 0.12, 0.12) {
    Text { "item "+y }
}
```

### Why this creates big gaps

After the transform pipeline drops scale inheritance, the `content_t` subtree is effectively operating in world units.

That means:

- `position.y = 1` means **1.0 world unit** of separation
- but the text itself is only scaled to `0.12`

So the content is spaced roughly:

- line height visual size: about `0.12`
- line-to-line step: `1.0`

That is much larger than the panel row spacing used by the inspector, where layout rows are placed in glyph units and converted by `TEXT_SCALE` (about `0.08` world units per row).

### Conclusion

The large gaps between `item 0`, `item 1`, `item 2`, ... are almost certainly not the core scrolling bug.

They are a direct result of authoring the row transforms at `y = 0, 1, 2, ...` in raw world space after dropping inherited scale.

---

## 7. Likely cause of “no clipping” in `ui-layout.mms`

There are two distinct clipping questions.

### 7.1 Was the clip source registered?

Yes, structurally the authored subtree is valid for manual clipping:

- `R.plane()` provides renderable geometry
- `StencilClip` is attached in that branch
- content is authored beneath that clip branch

So the intended clip ownership is reasonable.

### 7.2 Did descendants inherit the clip region?

Historically, this was a real runtime gap.

The clip source itself could be registered as a stencil clip, while descendant renderables still kept `stencil_ref = 0`, which meant:

- the stencil mask existed
- but the content draw batches were not actually tested against it

That is exactly the failure shape that would make the viewport appear unclipped.

### Current status

The current runtime now includes stencil-ref propagation in [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs):

- new renderables get an inherited stencil ref
- `register_stencil_clip(...)` re-syncs stencil refs across the affected subtree

So the remaining likely issue for `ui-layout.mms` is no longer “descendants never inherit clip refs”, but rather one of:

1. the authored clip scope still does not match the actual intended subtree shape
2. the runtime needs a visual verification pass to confirm the propagation fix works for this exact MMS topology
3. scrolling still appears broken because there is no drag surface, making the viewport look static even if clipping is working partially

### Practical takeaway

Clipping and scrolling should now be debugged separately:

- clipping = does the plane stencil-mask descendant text geometry?
- scrolling = is there a drag-producing surface in the viewport branch?

---

## 8. What the generated `ui-layout.rs` does and does not show

[examples/ui-layout.rs](../../examples/ui-layout.rs) is only a runtime wrapper:

- evaluate MMS
- push resulting intents into the world
- process commands
- run the app

It does **not** contain a lowered static Rust tree for this topology.

So for this investigation it is not a topology source; the topology has to be inferred from:

- the authored MMS
- component registration rules
- runtime system behavior

---

## 9. Working hypothesis

The current `ui-layout.mms` sample is failing for **multiple independent reasons**:

### A. No scrolling interaction

Most likely cause:

- no dedicated drag/raycast input surface in the viewport subtree

### B. Excessive line spacing

Most likely cause:

- rows are authored at `y = 0, 1, 2, ...` in world units after dropping scale

### C. Clipping may still need scene verification

The structural/runtime clip bug was previously that descendants were not inheriting stencil refs.

That propagation path is now present in `SystemWorld`, so the next question is not purely architectural anymore; it is whether this exact MMS topology visually clips as expected at runtime.

---

## 10. Likely next steps before more refactoring

### 1. Verify the exact runtime behavior of the current sample

Using the current `src/` state, check:

- does the text outside the plane bounds still render?
- if yes, is the content renderable subtree getting nonzero `stencil_ref`?
- is the plane itself the intended clip scope root for the authored branch?

### 2. Add an MMS-side drag surface equivalent

To make the reference sample actually scrollable, it likely needs an authored subtree equivalent to the inspector drag plane:

- a quad covering the viewport
- raycastable / drag-only
- routed into the same scope chain as the scrolling component

### 3. Normalize the row spacing

The sample should author rows in viewport-local visual units, not unit steps of `1.0` after scale-drop.

For example, rows should likely step by something closer to the text scale, not by whole world units.

### 4. Decide what the reference topology should be

We should decide whether the canonical MMS reference for clipped scrolling is:

- **manual clip topology**
  - `R.plane() -> StencilClip -> TransformPipeline -> Scrolling`
- or **layout-owned viewport topology**
  - `T -> Style { overflow = Scroll } -> Scrolling`

Both may be supported, but the reference example should intentionally represent one model.

---

## 11. Summary

`InspectorSystem` currently succeeds because it creates **three cooperating pieces**:

1. a viewport container that requests clipping via layout overflow
2. a separate scrolling subtree with a moved track transform
3. a dedicated drag plane that actually produces drag input

The current `ui-layout.mms` reference only clearly expresses **two** of those ideas:

1. a clip boundary (`R.plane()` + `StencilClip`)
2. a scrolling subtree (`ScrollingComponent` under a moved transform)

It does **not** yet clearly express the third:

3. an input surface that produces drag events in the same scope chain

And its row spacing is authored in much larger units than the inspector panel rows.

So the current best explanation is:

- **not scrollable** because there is no drag-capture surface
- **looks badly spaced** because the items are positioned in raw world-unit steps
- **clipping needs runtime verification against the new stencil-ref propagation path**
