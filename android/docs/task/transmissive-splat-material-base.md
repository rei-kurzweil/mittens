# Transmissive splats

Status: ideas only, not scheduled. Builds on the
[glow splat material base](glow-splat-material-base.md), which must exist
first. Captures add an intermediate render target, which is an explicit scope
change for the Android renderer (AGENTS.md).

## Idea

Inside a dark building, looking out through windows to a brighter area, the
light coming through an opening should read as a soft glow in the colour of
what lies beyond it. A transmissive splat is a glow splat whose colour comes
from a low-resolution capture of the far side of an opening, instead of a
constant.

- One capture per opening, not per splat: a render-to-texture camera placed
  where the opening is expected to be seen from, or at the opening looking
  out with a wide field of view.
- Splats placed in front of the opening, between the observer and the
  window, sample the capture (a blurred mip for wide halos) and blend
  additively, as glow splats do.
- The window surface itself can sample the same capture, giving a cheap
  low-resolution view out of it.

Parallax is wrong away from the capture point. At glow resolution and blur
this should not be visible; check it on the headset.

## Capture update modes

| Mode | Cost | Use |
|---|---|---|
| Baked at load | One capture per opening, once | Static exteriors |
| Slow, amortised | One opening per N frames, round robin | Day/night, moving traffic outside |
| Real-time | Every frame | A few openings with fast changes beyond them |

On the Focus 3:

- Captures are separate render passes before the eye passes, in the same
  command buffer. Keep them tiny (32²–64² per opening) in one atlas or array
  texture, with transient/lazy depth. Never render a capture inside an eye
  pass.
- The capture is monoscopic; both eyes sample the same image.
- Without HDR targets a bright exterior clips in the capture. Store an
  intensity scale per opening if the glow needs to exceed white.

## Visibility from each side

Two dark rooms joined by an opening, each with something bright in it: a
viewer in room A should see room B's glow, and a viewer in room B should see
room A's. Options, cheapest first:

1. **Facing.** Each opening gets one single-sided splat per side, facing that
   side's observers, and each samples the capture of the opposite side. Do
   the test in the vertex stage: if the viewer is behind the splat's plane,
   collapse the quad to a degenerate one, so the binner drops it before any
   tile work. This is better than hardware back-face culling, which still
   bins the triangles, and it works for kinds that are not in-plane. Fade by
   the view angle near grazing instead of popping. No CPU work.
2. **Region masks (CPU).** Tag splats with the regions (rooms, zones) they
   are visible from. Each frame, the head's region selects which groups
   draw. Group splats by region in the buffer, so a change only alters draw
   ranges, not data. Needed where facing is not enough, for example an
   L-shaped room that sees an opening from both sides.

Start with facing; add region masks when a scene needs them.

## Quads or triangles

Fill dominates splat cost, not vertex count. A rectangular opening fits a
quad exactly; the smallest triangle around a rectangle has twice its area, so
it shades about twice the pixels to save one vertex. Use quads for
rectangles, single triangles only for triangular shapes, and a small convex
fan for other openings.

## Open questions

- **Base or option.** `MaterialBase.transmissive_splat()` as its own base, or
  a capture-driven colour source on `MaterialBase.splat(kind)`. The vertex
  stage, blending and pass placement are shared; the differences are the
  capture texture binding, the per-splat capture index and the facing test.
  An option keeps one base.
- **Capture placement and authoring.** Whether an opening node declares its
  capture point and update mode, or the engine derives them from the
  opening's plane.
- **Desktop.** The desktop renderer has per-view planar reflection captures
  (`PlanarReflective`); check what of that capture scheduling can be shared.
- **Content in front of the capture.** Exclude the splats themselves and the
  near-side interior from each capture.

## Validation (when scheduled)

- Two-room scene: each side sees only the far side's glow; no glow from
  behind the viewer's own opening.
- Capture modes: baked does no work after load; amortised does at most one
  capture per frame; capture passes appear before the eye passes in a GPU
  capture, at the expected resolution.
- `FRAME_PROFILE` with 0, 10 and 50 openings, for each update mode.
