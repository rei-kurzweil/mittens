# Percent-width inline-block siblings stack vertically instead of laying out side-by-side

## Status

Open.

## Symptom

In `examples/percentage-layout.mms` the LayoutRoot has two children:

```mms
T { name = "sidebar"  Style { display("inline-block")  width(25%) ... } ... }
T { name = "content"  Style { display("inline-block")  width(75%) ... } ... }
```

Expected: sidebar (25%) and content (75%) sit side-by-side on one line,
together filling 100% of the available width.

Actual: sidebar renders **above** the content area as if a line break
was inserted between them — content is on its own line below the
sidebar.

## Expected behavior

Two `display: inline-block` siblings whose summed widths equal the
container's available content width should share a single inline line
(no wrap). With `width(25%) + width(75%) = 100%`, they should fit
exactly, edge-to-edge.

## Likely cause (to investigate)

The inline cursor's wrap decision is presumably comparing measured
box widths against the remaining line budget. Two paths to check:

1. **Percent resolution rounding / off-by-epsilon**: 25% + 75% of
   `avail_content_w_gu` may overshoot the line budget by a tiny float
   epsilon, causing the second item to wrap. `measure_item` resolves
   `Percent(p)` as `avail_content_w_gu * p / 100.0` — two of those
   summed should equal `avail_content_w_gu` exactly but FP may
   produce e.g. `+1 ulp` and break the `<= remaining` check.
2. **Padding/margin double-counting**: the sidebar's `padding(2%)`
   resolves against the *outer* available width, then its
   margin/padding is also subtracted from the line budget at the
   inline cursor. If percent resolution and the inline-cursor budget
   math disagree on whether width-of-box includes padding, the second
   inline-block won't fit.

Most likely #2: the way `Percent(p)` is interpreted for `width` here
("percent of container's avail_content_w_gu") plus a non-zero padding
makes `box_width = content_width + padding_h`, and `box_width_a +
box_width_b` exceeds `avail_w_gu`.

This is actually the standard CSS pitfall — `box-sizing: content-box`
makes `width(50%) + padding` overflow the parent's 100%. CSS users
either drop padding, use `box-sizing: border-box`, or reduce the width
percentage. cat-engine should pick a documented behavior here:

- **Option A** — match CSS default (`content-box`): document that
  `width(25%) + padding(2%)` overflows; users must subtract.
- **Option B** — default to `border-box` semantics: `width(p%)`
  describes the padding box, so padding eats into content.

Decision is design, not just a bug fix — file ticket asks for the
design call to be made.

## Affected examples

- `examples/percentage-layout.mms`

## Related

- `src/engine/ecs/system/layout/measure.rs:measure_item` — Percent
  width resolution
- `src/engine/ecs/system/layout/inline.rs` — inline cursor wrap
  decisions
- `docs/spec/mms-unit-literals.md` — percent semantics docs (needs
  update once `box-sizing` policy is chosen)
