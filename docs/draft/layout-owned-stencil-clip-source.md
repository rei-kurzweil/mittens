# Layout-owned stencil clip source

Status: draft

Historical note: examples below that mention `TransformPipeline` / `TransformPipelineOutput` describe the removed authored wrapper/output topology. Current authored scrolling/clipping shapes use `TransformForkTRS` with downstream content attached directly under the fork root.

Companion to:
- `docs/draft/stencil-clip-algorithm.md`
- `docs/refactor/scrolling-component-layout-system.md`

## Goal

For layout-generated `overflow: Hidden` / `overflow: Scroll` containers, the visible background quad should define the stencil clip shape.

That means the clip shape should come from the computed layout-owned `__bg` renderable, not from:
- raw style width / height values
- guessed content bounds
- virtual-window visibility logic
- arbitrary descendant or subtree renderables

This draft is specifically about **which renderable provides the clip shape**.
It is not the CPU-culling design. Conservative bounding-box calculation and CPU-side reject for content completely outside the clip volume are future work.

---

## Problem

The desired layout topology has two competing needs:

1. The clip shape should match the actual visible background quad.
2. The content must remain on a separate branch so the background quad's scale does not contaminate authored content transforms.

If a layout container like this is authored:

```text
T {
  Style { height: 640, width: 240, background_color: [1,0,0,1], overflow: scroll }
  T {}
  T {}
  ...
}
```

then the layout system wants a runtime topology conceptually like:

```text
T {
  Style { height: 640, width: 240, background_color: [1,0,0,1], overflow: scroll }

  StencilClip {
    Scrolling {
      T {}   // item 1
      T {}   // item 2
      ...
    }
  }

  T.scale(640, 240, 1) {
    name = "__bg"
    R.plane() {}
  }
}
```

In this shape, `StencilClip` and `__bg` are siblings.
So a pure "nearest ancestor renderable only" rule is too strict for layout-generated clips.

At the same time, a general sibling search would be too loose and error-prone.

---

## Proposal

### 1. Keep authored content on a separate branch

Layout should continue to keep the background quad on its own `__bg` branch.
This avoids pushing viewport width / height scale into the authored content subtree.

This separation is intentional and should not be "fixed" by moving content under the quad renderable.

### 2. Layout-generated clips may resolve to the adjacent `__bg` renderable

For layout-owned clip helpers only, the clip source resolution rule becomes:

- first, check for an immediate ancestor renderable
- otherwise, allow one specific fallback: the adjacent layout-owned `__bg` renderable generated for the same styled container

This is **not** a general sibling renderable lookup.
It is a reserved relationship between two layout-owned helpers created from the same container.

### 3. Use the computed `__bg` transform, not style fields

The clip shape must come from the actual computed `__bg` transform/renderable that layout produced.
Do not derive clip geometry directly from:
- `StyleComponent.width`
- `StyleComponent.height`
- inherited style values
- guessed intrinsic size

Reason:
- the final box may depend on auto sizing
- the final box may depend on inherited or resolved layout state
- the final box may differ from authored values after layout calculation

The runtime clip shape should therefore read the actual `__bg` renderable/transform that layout emitted.

### 4. No virtual-window coupling

This model assumes scroll containers keep their content subtree live and rely on stencil for correctness.
It should not depend on row-window rebuilds, hide/show virtualization, or scroll-window ownership in panel systems.

### 5. CPU-side culling is later work

Future optimization work may compute conservative bounds for:
- the clip shape
- candidate renderables inside the scroll/content subtree

and skip items whose bounds are completely outside the clip volume.
But that is explicitly separate from the clip-source decision in this draft.

---

## Resolution rule

For a `StencilClipComponent`, resolve the clip source in this order:

1. **Immediate ancestor renderable path**
   - If the clip node is under a renderable ancestor that is designated as its owner, use that renderable.
   - This remains the default rule for manually authored clips.

2. **Layout-owned sibling `__bg` path**
   - If the clip node is a layout-generated helper for a styled container,
   - and there is no qualifying immediate ancestor renderable,
   - resolve to the adjacent layout-owned `__bg` renderable created for that same styled container.

3. **Otherwise: no clip source**
   - Do not scan arbitrary siblings.
   - Do not scan descendants.
   - Do not scan the whole subtree for "some renderable that looks close enough".

---

## Required invariants

If the sibling-`__bg` path is used, the implementation should keep the relationship narrow and explicit.

Suggested invariants:
- the `StencilClipComponent` is layout-generated, not user-authored
- the `__bg` node is layout-generated, not user-authored
- both helpers belong to the same styled container
- the `__bg` node is identified by reserved helper ownership, not only by the literal name `__bg`
- the resolved renderable is the actual visible background quad for that container

If those invariants are not met, clip resolution should fail rather than silently bind to the wrong renderable.

---

## Why not derive the clip directly from the style node?

Because style data is not the final clip geometry.

The clip source must reflect the actual layout result, including:
- resolved width / height
- auto sizing
- inherited values
- padding / border decisions if they later affect the background quad
- final transform and world-space placement

The `__bg` helper is already the concrete renderable embodiment of that result.
Using it as the source of truth is simpler and more correct than recreating the same box again from style data.

---

## Expected effect on MMS-authored usage

### Manual / authored clip topology

This existing authored pattern remains valid:

```text
T.scale(viewport) {
  R.plane() {
    StencilClip {
      TransformPipeline {
        ...content...
      }
    }
  }
}
```

That topology already satisfies the ancestor-renderable rule.
It should continue to work unchanged.

### Layout-owned `overflow: Scroll`

For HTML/layout-authored overflow containers, layout already synthesizes helper topology.
Those cases are the intended target of this proposal.

So the rule is:
- **manual clips:** keep ancestor-renderable ownership
- **layout-generated clips:** allow explicit sibling `__bg` ownership

This avoids breaking current MMS-authored manual clip examples while letting layout own the viewport box shape.

---

## Audit against current examples

### `examples/ui-layout.mms`

Current pattern:
- `R.plane()` owns `StencilClip`
- content is routed through a transform pipeline under that clip
- the plane itself is the visible viewport surface and clip shape

Result under this proposal:
- no breakage expected
- it continues to use the existing ancestor-renderable path
- no sibling fallback is needed

### `examples/html-layout.mms`

Current pattern:
- authored HTML uses `Style { overflow("scroll") }`
- layout is expected to synthesize viewport helpers

Result under this proposal:
- this is the primary beneficiary of the sibling-`__bg` rule
- clip shape comes from the computed layout-generated background quad
- no authored MMS syntax change is required

---

## Non-goals

This draft does not yet specify:
- bounding-volume math for clip-aware CPU culling
- how scroll offset is stored long-term
- the full helper topology for layout-owned scrolling
- shader or pipeline changes
- generalized sibling-based clip ownership outside layout-owned helpers

---

## Implementation note

If we adopt this model, the engine should track the layout helper relationship explicitly.
A `StencilClipComponent` should not discover its `__bg` partner by loose tree scanning every frame.

The intended shape is a recorded ownership relationship such as:
- styled container -> layout-owned clip helper
- styled container -> layout-owned `__bg` helper
- layout-owned clip helper -> resolved clip-source renderable

That keeps the sibling fallback precise and avoids reintroducing the old "search around and hope" behavior.
