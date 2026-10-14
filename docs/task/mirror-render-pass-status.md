# Mirror rendering: status and improvement task list

Updated: 2026-10-14
Status: existing capture path, graphics panel, capture Toggle, and runtime quality
backend implemented; live resize verification, correctness, and optimization remain.

## Current implementation

- `MirrorComponent` authors `quality` in 64..=2048, default 512.
- `MirrorSystem` derives separate mono/stereo capture views from the viewer
  position reflected across the visible mirror face. The capture basis aligns
  with the mirror plane; an off-axis projection spans its rectangle.
- `VisualMirror` contains capture requests keyed by mirror, viewer family, and
  view index. The renderer creates mirror RenderViews, reuses offscreen capture
  targets while extent/format/view count agree, and publishes runtime textures
  before the consuming window/XR draw.
- Dedicated mirror shaders/pipelines and source-surface exclusion are
  implemented. The old checklist treating these as missing is superseded.
- Capture draws use the normal scene streams with source-surface exclusion;
  there is no mirror-specific CPU bounds/frustum culling.
- Mirror-plane clipping helpers exist, but
  `MIRROR_ENABLE_OBLIQUE_CLIP_PLANE` is false. The disabled flag was introduced
  in debugging commit `4d926c8d` on 2026-06-21. Later projection-debug notes
  explicitly defer clipping until projection alignment is understood. The
  history suggests debugging isolation, not a measured performance choice;
  the precise symptom prompting the switch is not recorded.
- Captures are not skipped merely because their surface is outside the
  player's view. Invalid reflected projections can suppress captures, which
  is not a viewer-visibility optimization.
- The renderer already tracks mirror capture/submission counts internally.
  Per-mirror visibility reasons, candidate/draw counts, allocation counts, and
  CPU/GPU timing need an observability audit rather than assuming no stats exist.
- An importable graphics settings panel UI preview exists at
  `assets/components/panels/graphics_settings_panel.mms`, exercised by the
  XR-only `examples/graphics-settings.mms`. `GraphicsSettingsSystem` now owns
  the capture-enabled flag, exposed by `GraphicsSettings.mirrors.toggle(bool)`
  and `.enabled()`. Off retains the last per-eye/family textures and skips the
  capture pass; on rebuilds reflected views before capture. The Slider now
  applies a scene-wide quality override with readback/reset, preserving authored
  component qualities and per-eye/family keys. The user confirmed the Toggle
  and secondary motion work. The resolution Slider stays on `Authored` in XR:
  [trigger acquisition is blocked by the grabbable panel wrapper](../bugs/graphics-settings-mirror-resolution-slider-unresponsive.md).
  Correct that input path before live resizing and frozen-resize/resume checks.

Relevant runtime code:

- `src/engine/ecs/system/mirror_system.rs`
- `src/engine/graphics/visual_world.rs`
- `src/engine/graphics/vulkano_renderer.rs`
- `assets/shaders/mirror-mesh.vert` and `mirror-mesh.frag`

## Improvement order

1. [ ] Verify the existing importable MMS graphics settings panel's runtime quality
   controls in the headset. The panel is connected to the
   `GraphicsSettings.mirrors` built-in settings table and authoritative
   `GraphicsSettingsSystem`. The capture Toggle is implemented: off skips captures and
   publication while preserving the last reflection and per-eye/family texture
   bindings; on resumes fresh captures. The
   [first focused backend slice](runtime-mirror-resolution-info-panel.md#first-focused-backend-slice-mirror-capture-toggle)
   defines the Toggle's end-to-end scope and completion checks. `quality`,
   `quality_override()`, and `reset_quality()` are implemented. Track the
   asset, backend, and readback contract in
   [graphics settings and runtime mirror controls](runtime-mirror-resolution-info-panel.md).
2. [ ] Restore validated mirror-plane clipping for correctness.
3. [ ] Skip captures for mirrors outside the active viewer's view.
4. [ ] Measure remaining geometry, pixel, allocation, and submission costs.
5. [ ] Investigate optional MMS candidate tracking for mostly static scenes;
   retain the design questions below rather than implement it implicitly.
6. [ ] Revisit adaptive resolution bands and capture-region optimization in
   [adaptive mirror detail](../desktop/adaptive-mirrors.md) when justified by
   measurements. They are separate from manual settings and basic visibility.

Finish live quality verification before further mirror improvements. The user
recalls that the previous clipping
attempt did not work; its cause remains unverified, so merely re-enabling the
existing flag is not treated as a fix. The clipping and visibility slices can
be developed independently. Neither requires solving movable-object tracking,
building a new editor panel type, or
introducing a generalized dependency scheduler.

## Mirror-plane clipping

Reject geometry on the far side of the reflective surface, including geometry
between the virtual reflected camera and that surface. This is a correctness
requirement: it can otherwise occlude valid reflected content, not merely waste
fill rate. See [behind-surface occlusion](../bugs/mirror-behind-surface-occludes-reflection.md).

The CPU builds the capture projection; the GPU clips triangles against its
near plane. This does not remove object submission or vertex processing.
Before enabling the existing helper, validate the Vulkan depth convention,
plane orientation/bias, close-up poses, moving/rotated mirrors, off-axis edges,
and both stereo eyes. Keep intersecting objects eligible and let GPU clipping
trim them. Do not reject a straddling object by testing only its origin.

## Skip mirrors outside the player's view

Use a cheap conservative test of the reflective surface rectangle/bounds
against the actual viewer frustum, plus the supported front/back-face policy.
This gate must run before capture submission and associated image publication.
It does not require per-object rays or scene occlusion queries.

- Make the decision per active viewer family. An XR mirror remains eligible
  when either eye can see it; a mono view does not force hidden XR captures.
- Account for viewer motion/projection and mirror transform/geometry changes.
  Near-plane crossings or uncertain bounds must favor retaining the capture.
- Keep valid targets/textures while temporarily skipping capture work; when
  visible again, refresh before the visible surface consumes the image.
- Define mirror-in-mirror behavior before assuming that invisible to the
  direct viewer means unnecessary everywhere. Source-surface exclusion alone
  does not settle other-mirror policy. Do not introduce recursive capture
  scheduling as an implicit part of this slice.
- Validate mirrors leaving/reentering view, visible to one XR eye only, close
  to screen edges, grabbed/moved mirrors, and simultaneous mono/stereo viewers.

## Optional MMS candidate tracking: design still open

The user's preferred direction is an authored opt-in for scenes with mostly
static scenery and a small number of movable objects. Do not make the engine
watch every renderable or every zone by default.

A possible arrangement is a declared static candidate set, explicitly
nominated movable candidates, and a conservative mirror-associated zone/front
region. Region entry/exit updates eligibility; it does not prove visibility.
Cheap current-view bounds tests still decide which candidates can contribute.
No public MMS spelling, component, filter scheme, or zone shape is committed.

Decisions to investigate:

1. How MMS nominates static branches and movable renderables, including
   objects without physics components. Do not require `Collidable` merely for
   reflection tracking. Static classification is an authored promise that
   needs defined invalidation/fallback if the branch later moves.
2. Whether the front region is an authored finite zone, a mirror half-space,
   or another conservative filter. It must not exclude content visible from
   supported player positions. Use bounds overlap so large or partially
   intersecting objects remain eligible.
3. How movement/shape/topology changes update eligibility without repeating
   selector queries or whole-scene searches for every mirror/view. Include
   animated conservative bounds, mounting, spawn/removal, reparenting,
   teleportation, mirror motion, and edits to the authored candidate set.
4. Whether the existing zone observer can support non-physical renderable
   candidates. Today `Zone.enable_events()` polls enabled slide collidables
   each frame and excludes mounted roots; it does not observe arbitrary
   renderables/grabbables, and enter/exit events are not a movement-driven
   detection implementation. See the [observer filtering tracker](zone-observer-candidate-filtering-and-broad-phase.md).
5. Whether cached bounds and simple tests already cost less than zone-event
   bookkeeping for the small moving set. Profile before adding a spatial
   index. The existing picking BVH excludes renderables without enabled
   raycastable markers, so it is not a complete rendering-candidate index.

Candidate filtering must be absent unless explicitly authored. Decide its
fallback behavior before enabling it: incorrect or incomplete membership must
not silently remove valid reflected geometry.

## Cache invalidation boundaries

A stationary mirror does not have a permanently fixed visible-object list.
Player motion changes the reflected viewing volume; object motion changes its
intersection with that volume.

| Cached input/result | Changes that invalidate it |
| --- | --- |
| Mirror plane and surface corners | Mirror world matrix or geometry/bounds |
| Reflected viewing volume | Viewer position/projection or mirror surface inputs |
| Object world bounds/front-region membership | Object/frame pose, geometry, deformation, region, or membership edits |
| Mirror visible to player | Viewer pose/projection or mirror surface inputs |
| Actual eligible draw set | Viewing volume, candidate bounds/membership, or renderability changes |

Cache stable inputs and reconcile changed candidates. Do not recompute every
scene subtree for every capture, and do not freeze the draw set solely until
the mirror's own world matrix changes.

## Performance acceptance

Compare the unchanged renderer against each slice in release mode, including
mostly static corp scenes with few movable objects and a denser moving fixture.
Measure capture count, submitted instances/draws, CPU visibility time, GPU
capture time, target/publication allocations, and desktop/XR frame time.

Off-screen mirrors should submit no unneeded captures. Candidate filtering
must preserve the supported reflection and show that its CPU/bookkeeping cost
is outweighed by saved rendering work. For small sets, retain simple tests
rather than require a more elaborate acceleration structure.

The current publication path allocates a sampled image per capture, distinct
from offscreen capture-target reuse. Record that existing cost separately;
manual resizing and visibility improvements should not quietly expand into a
texture-lifetime redesign.

## Related work

- [Mirror component spec](../spec/mirror-component.md)
- [Mirror system spec](../spec/mirror_system.md)
- [Projection debug follow-up](mirror-camera-projection-debug-followup.md)
- [Behind-surface reflection occlusion](../bugs/mirror-behind-surface-occludes-reflection.md)
- [Mirror viewer-family captures](mirror-viewer-family-captures.md)
- [Importable MMS graphics settings panel and mirror controls](runtime-mirror-resolution-info-panel.md)
- [Adaptive mirror detail](../desktop/adaptive-mirrors.md)
- [Zone observer candidate filtering](zone-observer-candidate-filtering-and-broad-phase.md)
- [Render-view inventory (historical)](render-view-mirror-inventory.md)
