# View-local mirror bindings and stable draw topology

Status: implementation and validation capture, after the initial counter slice.
Checkpoint before this work: `80419d7d`; the counter baseline and this change
were developed as separate stages in the working tree.

## Problem and change

The [counter baseline](cache-invalidation-baseline.md) showed two shared draw-cache
rebuilds per stereo frame, caused by texture setter calls. Immediately before
each view, the renderer wrote that view's reflection image into the shared visual
instance. A later eye switched it again. A frozen mirror retained its images but
still participated in these binding changes.

The renderer now resolves reflection textures in a **per-view binding table**.
It clears and reuses the table's capacity between serial view recordings, keyed
by source component identity. It never writes the resolved capture back into the
shared visual instance. Window, XR eye and mirror-capture views select the
appropriate family/index; disabled captures can still supply their last image.
Absent captures use the existing authored texture/default fallback.

`update_texture()` now returns early for an identical handle. Actual handle
changes still invalidate draw topology and propagate to subsequent bindings.
Together these changes remove view-selection writes and redundant texture-system
writes as reasons to rebuild the shared draw program.

## Batch identity and supported paths

A reflective draw batch retains an optional source component identity. Surfaces
that can need different capture images stay in distinct batches even when they
share a material, geometry and authored texture. Other surfaces keep their
existing batching behavior. This applies to ordinary and clipped streams,
background/emission batch helpers and sorted transparency recording.

Source identity comes from the existing resolved `PlanarReflective` capability,
with explicit compatibility for the legacy MIRROR program. That compatibility
is necessary because legacy mirrors can override the program while retaining
inherited Anime shading values whose MaterialBase is an ordinary surface.
This adds no MaterialBase variant or MMS API.

Mask entry/exit draws use their source identity when resolving textures, keeping
the current clip coverage behavior. Outline and stencil debug batch identities
remain independent where they do not sample the reflection. Existing ordering,
clip references, render scopes and depth/stencil policy are preserved.

The table stores lightweight texture handles. Descriptor creation/cache lookup
uses the resolved handle; recorded command buffers retain the existing GPU
resource references. Clearing the table does not retire those resources. The
recorder still uses Vulkano automatic synchronization and builds fresh command
buffers; this change establishes stable draw topology for later command reuse.

The current renderer records views serially. A parallel recorder would need an
immutable view-binding context per worker rather than sharing this mutable
scratch table across workers. This change introduces no concurrent recording.

## Focused source organization

- `visual_world/render_streams.rs`: ordered batch/clip compilation, extracted
  from the oversized visual-world file before extending its batch identity.
- `vulkano_view_bindings.rs`: per-view reflection selection and debug-batch helper,
  extracted from the oversized renderer before extending view binding behavior.
- Existing direct recording helpers resolve the effective texture before cache
  comparisons and descriptor binding. There is no per-draw allocation or new
  runtime-dispatched recorder interface.

## Correctness coverage

CPU tests cover:

- Distinct legacy reflection sources with shared inherited Anime shading and
  identical authored textures, in opaque and sorted transparent batches.
- Shared custom PlanarReflective material sources inside a clip program.
- Identical texture assignments hitting the cache and real replacements
  invalidating it.
- Left/right/mono binding selection, frozen capture selection, unrelated surfaces
  sharing an authored texture, runtime image replacement and mirror removal.
- Resolving a view leaves authored texture identity and the shared draw cache
  unchanged.

Existing GPU coverage verifies upright/horizontally correct stereo reflections,
per-mirror freeze retaining its final pixels while another mirror updates,
global mirror settings and resume, and the custom pool reflection path retaining
its material program and sampling each view's capture. Its old assertion about
mutating the shared water texture now checks the actual view binding instead.

## Measured result

Same cafeteria fixture and render quality as the saved counter baseline:
1920 x 1920 per eye, bloom/emission extraction, MSAA 4x, GTX 1080, 240 warmup
and 480 measured stereo frames. Both stationary and moving-head captures passed.
Head movement changes the rendered view matrix throughout the sampled window.

| Count per stereo frame | Before | After, stationary | After, moving head |
| --- | ---: | ---: | ---: |
| Shared draw-cache rebuilds | 2 | **0** | **0** |
| Shared draw-cache hits | 0 | **2** | **2** |
| Instance-buffer helper allocations | 10 | **7** | **7** |
| Instance records uploaded | 4,334 | **2,319** | **2,319** |
| Mutable material snapshot allocations | 36 | 36 | 36 |

Geometry draw-call/instance histograms are unchanged in both captures. The second
eye now reuses opaque, background, cutout and overlay buffers. Its empty outline
pass can still request an optional refresh without allocating anything.
First-eye model/instance updates remain active; this does not freeze animated
avatars or vehicle physics to gain cache hits.

| Motion | CPU cache/deformation-range/sort stage before | After | Total CPU recording/submission after |
| --- | ---: | ---: | ---: |
| stationary | 0.487 ms | 0.052 ms | 7.126 ms |
| head | 0.484 ms | 0.051 ms | 7.108 ms |

This stage includes deformation-range preparation and transparency work in
addition to the structural draw cache, so it does not become zero. These are
separate-process timing captures with uncontrolled GPU clocks; the counter
changes establish the eliminated work directly. They do not establish a precise
GPU speedup or guarantee live 45 FPS throughout the ship. GPU and whole-frame
sample distributions are retained in the summaries.

## Validation and remaining work

- 21 release visual-world tests passed, including existing stream/clipping tests
  and the new reflection batching/texture-replacement cases.
- The per-view binding test passed, including frozen/mono/stereo/replacement and
  removal behavior without modifying shared topology.
- Both existing stereo mirror orientation/freeze GPU tests passed.
- The custom Anime/reflective pool GPU test passed, including per-eye descriptor
  image-view checks.
- Both 480-frame cafeteria GPU captures passed. Production `cargo check --lib`,
  focused formatting checks and `git diff --check` passed.

Mutable material snapshot churn remains at 36 allocations/uploads per stereo
frame. Attribute material/revision/resource identities before changing its
retention policy. First-eye instance uploads also remain; retaining stable buffer
identities and updating safe frame slots is still necessary before broader
recorded-command reuse. No engine-owned synchronization or parallel recording
was implemented in this slice. Truck behavior remains covered by its separate
pending tracker.

Artifacts use the same projected frame-counter format as the baseline:

- [Stationary summary](data/view-bindings-stationary-2026-11-06.json)
- [Stationary frame samples](data/view-bindings-stationary-2026-11-06.frames.json)
- [Moving-head summary](data/view-bindings-head-2026-11-06.json)
- [Moving-head frame samples](data/view-bindings-head-2026-11-06.frames.json)

Reproduce with the `profile_ship_stereo` variables in the
[counter baseline](cache-invalidation-baseline.md), using the current source.
The saved pre-change artifacts retain the baseline independently of the current
implementation's cache behavior.
