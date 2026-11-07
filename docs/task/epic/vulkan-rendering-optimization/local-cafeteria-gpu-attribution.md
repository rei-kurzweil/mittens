# Local cafeteria GPU attribution: camera and vehicle state

Date: 2026-11-07. Follow-up to the [clean walking/driving baseline](local-cafeteria-walking-driving.md).
Engine `2887a7b1` plus test-only profiling changes; no production scene/material
changes. Same ignored local scene: no orchard, one pickup, Mitten player and Miku
vendor. Bridge, barracks and washrooms stay loaded; mirror stays disabled.

## Method

GTX 1080; 1920 x 1920 per eye; MSAA 4x; bloom/emissive extraction enabled. Each
separate process uses 240 warmup / 480 sampled stereo frames. The live demo was
closed before these follow-up runs. No build or other benchmark ran concurrently.

The original captures changed camera position/direction, truck pose, player
mount/pose and control state together. They measure the two experiences, but
cannot alone attribute their GPU difference to vehicle simulation.

Added opt-in test-only `SHIP_PROFILE_GPU_PASSES=1` timestamps around graphics
phases. They run inside retained graphics primaries on every replay. Diagnostic
boundaries can perturb overlap, so these captures supplement rather than replace
the original totals. `background` includes initial graphics setup; opaque is the
whole opaque draw stream, not an isolated truck shader. Postprocess combines
emissive extraction, bloom filtering, deferred overlays and final composite.
Small adjacent stages may overlap in flight; do not treat these as isolated
hardware-unit utilization counters.

`SHIP_PROFILE_CAMERA_REFERENCE=<frames.json>` replays the original recorded
view matrices after scene preparation, updating both eye cameras and their world
transforms while retaining the same projections. It deliberately leaves native
input, player/vehicle transforms, physics, mounting and script controls intact.
Each sampled index uses the corresponding original camera sample; warmup holds
the first reference view. The baseline fixture has identical tracked views for
both eyes, so eye-zero view replay preserves that existing setup.

A camera swap changes where the player/truck appear relative to the renderer.
It is not an identical-image comparison or a realistic alternate seated pose.
It tests whether the extra GPU cost follows the visible scene versus control
execution, and must be interpreted together with the rendering-only removals.

## Camera/state controls

All timings below are means in milliseconds for both eyes.

| Vehicle/player state | Rendered camera path | GPU graphics | GPU opaque | GPU postprocess | CPU update |
| --- | --- | ---: | ---: | ---: | ---: |
| Walking; truck parked | Natural walking | 11.529 | 9.773 | 0.559 | 5.528 |
| Mounted and driving | Natural driving | 15.927 | 14.187 | 0.556 | 13.659 |
| Mounted and driving | Recorded walking | 9.223 | 7.487 | 0.551 | 13.610 |
| Walking; truck parked | Recorded driving | 12.054 | 10.308 | 0.569 | 5.592 |

The natural driving increase is concentrated in opaque drawing (+4.414 ms),
while postprocessing stays near 0.56 ms and deformation near 0.046 ms. Both
natural states retain two graphics-primary cache hits and zero rebuilds/frame.
Draw counts are essentially unchanged; equal draw counts do not imply equal
visible pixel coverage, overdraw, depth rejection or fragment-shader work.

With driving/control execution preserved, swapping to the walking camera drops
GPU graphics by 6.704 ms while CPU update remains about 13.6 ms. GPU cost is
therefore not simply time spent executing the driving script. Conversely, the
driving camera with the truck parked/player elsewhere costs 12.054 ms, showing
that geometry/pose relative to the camera also matters, not just its world path.

## Rendering-only controls from the normal driver view

These keep the natural driving route/camera, native physics and control logic.
The subtree suppression diagnostic skips meshes only during command recording;
it does not remove their ECS objects, collision or avatar deformation. For an
imported GLTF it includes owned spawned-node identities, because AVC may move
those nodes outside the original GLTF component.

| Driver setup | GPU graphics | GPU opaque | GPU postprocess | CPU update |
| --- | ---: | ---: | ---: | ---: |
| Original paint; full scene | 15.927 | 14.187 | 0.556 | 13.659 |
| Hide combined painted body | 10.780 | 9.091 | 0.550 | 13.775 |
| Hide player avatar | 14.957 | 13.231 | 0.560 | 14.532 |
| Cheap paint shader; full geometry | 11.948 | 10.258 | 0.547 | 14.306 |

Hiding the combined body saves **5.147 ms net GPU graphics** and removes exactly
one opaque draw per eye (636 -> 634 stereo opaque calls). This is a whole-scene
difference, not an isolated timer for the body: removing it also changes
occlusion and reveals whatever is behind it. Hiding the player saves about
0.970 ms, so the avatar is not the larger contributor in this driving view.

The stronger material control replaces pickup paint with a plain vertex-color
fragment shader while preserving its `MaterialBase`, geometry and depth behavior.
The temporary shader has one dummy numeric field required by the custom-material
API; all original local scene/prefab/material files remain unchanged. Related
diagnostic MMS copies and shaders stay ignored under `examples/local/`.

**Cheap paint saves 3.979 ms of GPU graphics**, concentrated in opaque rendering
(14.187 -> 10.258 ms). Both original/cheap captures contain 1,781 visual instances
and identical sampled draw counts in every pass: 636 opaque, 216 emission,
12 cutout and 4 single-transparent calls across both eyes. Both have two
graphics-primary cache hits and zero rebuilds/frame. Across all 480 frames, truck
positions, player positions and rendered view matrices match exactly between
the original and all three rendering-only controls.

This demonstrates a substantial **fragment-material cost in the driver view**,
not just increased input/script work or command reconstruction. The paint shader
[`pickup-painted-metal.frag`](../../../../assets/shaders/pickup-painted-metal.frag)
loops over up to 64 scene lights per fragment, performing attenuation, facing
and specular calculations, then matcap/rim/tone mapping. Code inspection identifies
those as candidates; the plain-color control removes them together and does not
yet isolate the light loop versus matcap, specular, rim or other shader terms.
There are no more body draws just because the truck is driving, but viewing its
surfaces close up changes projected pixel coverage and depth/overdraw behavior.
CombineMesh reduces draw overhead; it does not remove that fragment workload.

Postprocessing stays near 0.55 ms, and deformation near 0.046 ms. The scene's two
truck spotlights are authored in both power states; powering controls on does
not add them. GPU and CPU work remain separate bottlenecks: even the cheap-paint
diagnostic averages 27.815 ms total with CPU update 14.306 ms, so it is not a
45 FPS solution by itself. Per-system CPU attribution is still needed; these
results do not establish whether the extra update time is MMS, native collision,
attachment propagation, or another system.

## Next useful slices

1. At the same driver view and visible geometry, isolate paint-light evaluation
   from matcap/specular/rim work. Measure relevant-light selection/culling before
   changing the production finish; preserve the intended lit appearance.
2. If shader cost remains after light evaluation is reduced, measure opaque
   overdraw/depth rejection separately; do not infer that cost from draw count.
3. Independently break down native/MMS update time during actual driving.

## Preserved artifacts and exclusions

All summaries, frame samples and logs remain in ignored
`examples/local/benchmarks/`, with stems
`cafeteria-1920-<state>-<control>-gpu-passes-2026-11-07`:

- walking / natural
- driving / natural
- driving / walking-view
- walking / driving-view
- driving / hidden-paint
- driving / hidden-player-v2
- driving / cheap-paint-v2

Each has `.json`, `.frames.json` and `.log`; source hashes are retained in
`gpu-attribution-manifest-2026-11-07.json`. The temporary cheap-shader scene
starts at `examples/local/ship_interior_gpu_cheap.mms`.

The initial hidden-player attempt selected no meshes due to AVC reparenting and
failed before sampling. The initial cheap-paint attempt omitted the material's
required numeric input fields and produced material-resolution errors/fallback
rendering. Both are **invalid and excluded**. Corrected captures above select
20 player meshes and have no paint material-resolution errors, respectively.
The earlier pre-restart contaminated captures remain excluded as well.

The release library test harness compiled, and all seven valid GPU captures
passed their movement, mounting, power, resident-background-room and disabled-
mirror assertions. Camera-replay controls alter rendering only. Native routes
remain in the cafeteria. Offscreen timing excludes live OpenXR/compositor/ALVR
overhead and is not a sustained headset FPS guarantee.

## Subsequent material correction

After these captures, the user clarified that the truck should not use matcap.
Removed the matcap texture sample, material input and pickup texture declarations
from the shared paint shader/presets and imported/prototype pickup prefabs.
The rim was already a **single view-dependent calculation outside the light
loop**, independent of scene lighting; it remains so. The loop calculates
diffuse/specular scene lighting, not multiple rims.

The seven results above describe the pre-correction shader and remain historical
evidence. They do not measure the saving from removing matcap alone: the
plain-color control removed all paint shading together. A shader compile and
short driving smoke check validate the corrected material separately.

## Subsequent Anime pickup preset

The user then requested the existing Anime shader with a stronger rim. Both
pickup prefabs now use `pickup_anime_material()` from
`assets/components/materials/pickup_anime.mms`: rim strength 1.0 versus ship
character 0.42 (2.38x), rim power 3.5. Character/environment presets are unchanged.
The custom paint shader is no longer assigned to trucks.

The [benchmark ledger](benchmark-history.md) retains the new driving capture:
11.739 ms GPU graphics, 13.586 ms CPU update, 26.714 ms total (28.767 ms p95).
The native route and rendered camera matrices exactly match the previous
natural-driving capture. This remains above the 45 FPS budget.
