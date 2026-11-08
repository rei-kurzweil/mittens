# Custom versus built-in Anime GPU profile

Status: draw-order/overdraw identified as a major cause; production remedy pending (2026-11-07).

The user reported accidental GPU use during the initial experiments. Those
captures are retained but marked superseded; **all results below use the full
fresh `r2-clean` matrix**, started after checking no benchmark/app remained and
GPU utilization was approximately 1%.

## Follow-up experiment

On 2026-11-08, after checkpoint `ae833df2`, an interim
[custom-first opaque experiment](custom-first-opaque-experiment.md) enables
custom opaque programs before built-ins for VR smoke testing. Statements below
about unchanged production ordering describe the earlier attribution phase.
The experiment does not implement near/middle/far MaterialBase bands.

## Question and controls

The truck's custom binary-gate Anime equivalent retained the GPU slowdown of
custom LambertToon. The [material experiment](lambert-toon-truck-material-experiment.md)
showed no measured cosine-weighting penalty in that control. Narrow down the
custom rendering path using matched shader work, scene geometry, camera and
vehicle motion. Keep global materials unchanged.

Code inspection shows that custom opaque graphics pipelines clone the same
base template as built-in Anime and replace the fragment stage. They retain the
same static vertex stage, depth state, rasterization, blending, MSAA and dynamic
viewport/scissor. However, opaque streams sort by material ID before texture
and mesh: switching programs can also change draw order. The measurements below identify draw ordering as a substantial source of extra
GPU work; they do not establish an intrinsic custom-shader cost.

## Test instrumentation

`SHIP_PROFILE_MATERIAL_DIAGNOSTIC=1` enables pipeline statistics in test builds
only. A focused `vulkano_profile_material_diagnostic.rs` collects input vertices,
vertex shader invocations and fragment shader invocations across both eyes of
the opaque phase. Results are read after GPU completion and written per frame.
Normal builds leave the statistics feature disabled.

`SHIP_PROFILE_TRUCK_ORDER=native|first|last` changes only the combined truck
body's batches. Stable partitioning stays inside contiguous unclipped draw runs,
never crossing clip entry/exit or clipped draws. Batch instance ranges and draw
counts stay intact. The diagnostic logs original and recorded batch positions.
Sorting/temporary allocation happens only when recording a graphics command;
steady-state graphics command reuse remains enabled. The default is native order.

The matrix uses custom binary-gate shading and built-in Anime with identical
ramp/rim parameters, each at native/first/last truck draw order. Local cafeteria,
one truck, Mitten player and Miku vendor, background rooms loaded, mirror off;
GTX 1080, 1920×1920 per eye, MSAA 4x, bloom/emission enabled, 240 warmup and 480
sample frames, per-pass timestamps and pipeline statistics on in every case.
This is offscreen stereo without live OpenXR/ALVR/compositor pacing. Background
desktop/SteamVR processes remain present; initial GPU state is in the manifest.

## Clean rerun results

Times are milliseconds; fragment invocations are millions across both eyes.
Pipeline statistics are diagnostic workload counters, not unique visible pixels.

| Material path | Truck body/order mode | CPU update | GPU graphics | GPU opaque | Opaque fragments M | Elapsed mean | Elapsed p95 |
|---|---|---:|---:|---:|---:|---:|---:|
| custom | native | 7.038 | 15.129 | 13.419 | 25.428 | 23.547 | 27.027 |
| builtin | native | 6.711 | 13.240 | 11.545 | 21.665 | 21.287 | 23.103 |
| custom | first | 7.052 | 12.845 | 11.142 | 21.023 | 21.248 | 24.733 |
| builtin | first | 6.955 | 13.203 | 11.500 | 21.714 | 21.562 | 23.644 |
| custom | last | 6.918 | 15.474 | 13.713 | 25.655 | 23.792 | 25.436 |
| builtin | last | 7.230 | 14.670 | 12.913 | 24.398 | 23.335 | 26.894 |
| custom | geometry | 7.114 | 13.023 | 11.341 | 21.293 | 21.516 | 24.216 |
| builtin | geometry | 7.033 | 12.735 | 11.051 | 20.788 | 21.207 | 23.052 |

Native order puts the truck body at op **67** (built-in material 11) versus
**287** (custom material 1042) in this clean pair. Other custom processes assign
different numeric program IDs; e.g. the clean custom-first run uses material
1046. Numeric program IDs are not a geometry/visibility ordering contract.

Moving the custom body early lowers GPU graphics **15.129 → 12.845 ms**, and
opaque fragment invocations **25.428M → 21.023M** (~17% fewer). Moving the built-in
body late raises GPU graphics **13.240 → 14.670 ms**, and fragments
**21.665M → 24.398M**. Draw ordering/overdraw explains a substantial part of the
apparent material cost. Custom shading is not consistently slower with the body
early: that pair is 12.845 ms custom versus 13.203 ms built-in.

Input vertices remain **693,072** per frame in all eight cases. All 480 per-frame
camera matrices, truck/player poses, power/mount/mirror states and draw counts
match. Both eyes retain graphics commands (two cache hits per frame, zero
rebuilds) with no steady-state material allocations. Existing asset-scan types
warnings remain in both paths; no material compilation errors occurred.

## Whole opaque-order control and limits

`SHIP_PROFILE_TRUCK_ORDER=geometry` uses a material-independent key inside each
unclipped run: mesh, texture, instance count, transforms and colors. Per-batch
geometry fingerprints exclude material/component IDs. This is an experimental
attribution order, not a production front-to-back sort. It preserves clip
boundaries and draw batching.

This mode measures **13.023 ms custom / 12.735 ms built-in**, with **21.293M /
20.788M** opaque fragment invocations. The fingerprints are **not identical**
across the independent scene processes. Therefore this is not a perfect
same-work microbenchmark: equal sort-key rules and equal draw counts do not
prove identical geometry submission order, instance contents or fragment work.
The remaining 0.288 ms is not established as a custom-pipeline penalty.

The evidence supports fixing opaque ordering rather than removing custom
materials, matcap, or the Lambert cosine calculation. It does not prove zero
custom shader overhead in every setting. Hardware counter instrumentation and
background desktop/SteamVR activity also limit comparisons with earlier captures.
These are offscreen timings, not a live headset FPS guarantee.

## Next slices

- [x] Inspect shared pipeline state and compare equivalent diffuse/rim work.
- [x] Count opaque input/vertex/fragment work, retaining cached commands.
- [x] Perform early/late ordering interventions and repeat after GPU interference.
- [ ] Design a general opaque ordering policy that considers occlusion/depth
      before numeric material IDs while retaining useful batching and reusable
      command buffers. Consider coarse view/room depth groups; define when a
      large camera/room/mount change invalidates the order. Avoid per-frame full
      sorting or a hardcoded production priority for trucks.
- [ ] Compare that policy against a measured depth-prepass alternative before
      adding any new render pass. Preserve alpha-cutout/discard behavior, stencil
      clip entry/exit and transparent ordering; validate stereo and visible output.
- [ ] For smaller shader/compiler differences, make a same-frame paired test
      that holds geometry, material IDs, batch order and uniform values fixed
      while selecting the GPU program. Independent scene loads do not yet meet
      that strict equivalence requirement.
- [ ] Benchmark production changes with statistics off after diagnostic counters
      confirm the intended work reduction. No production ordering fix is enabled
      by this investigation; the example retains Lambert shading for visual review.

## Reproduction and provenance

Build: `cargo test --release --lib profile_ship_stereo --no-run`.
Set the usual `SHIP_PROFILE_SOURCE`, motion/resolution/warmup/sample/output
variables, plus `SHIP_PROFILE_GPU_PASSES=1`,
`SHIP_PROFILE_MATERIAL_DIAGNOSTIC=1`, and
`SHIP_PROFILE_TRUCK_ORDER=native|first|last|geometry`.
Use the custom shader's binary gate for these controls and switch the truck
imports to `pickup_anime.mms` for the built-in reference. Always restore the
Lambert source and imports afterwards, as the diagnostic script does in finally.
No test configuration is applied to normal rendering.

Ignored raw captures:
`examples/local/benchmarks/cafeteria-1920-driving-custom-order-{custom,builtin}-{native,first,last,geometry}-r2-clean-2026-11-07.*`.
`custom-anime-clean-rerun-2026-11-07.manifest.json` records ordering/counters,
initial GPU state, test shader source, hashes and the source/command provenance.
The runner script is copied alongside it. Initial `r1` matrices are superseded
because GPU availability was disputed; preserve them as history, not evidence.
Restricted models and example content remain ignored.

## Second complete clean matrix, reversed order

Requested as one additional repetition to check sensitivity to temporary GPU
activity. All eight cases ran again with the **same executable** (SHA-256 checked)
and settings, in the exact reverse order. This gives two clean captures per case,
each with 240 warmup and 480 measured frames. Initial GPU process state and
post-process temperature/clocks/power/utilization snapshots are saved in the
repeat manifest. Those snapshots are not continuous telemetry and do not prove
that no background GPU activity occurred during a capture.

| Path | Order | First mean GPU ms | Repeat mean GPU ms | Repeat median GPU ms | Repeat p95 GPU ms | Repeat fragments M | CPU update ms | Elapsed mean ms | Elapsed p95 ms |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| builtin | geometry | 12.735 | 11.791 | 11.674 | 12.999 | 19.728 | 6.863 | 20.014 | 21.741 |
| custom | geometry | 13.023 | 11.977 | 11.874 | 13.202 | 19.904 | 6.986 | 20.376 | 22.360 |
| builtin | last | 14.670 | 15.243 | 15.284 | 16.295 | 25.604 | 6.754 | 23.380 | 24.871 |
| custom | last | 15.474 | 15.138 | 15.212 | 16.013 | 25.406 | 7.053 | 23.646 | 25.597 |
| builtin | first | 13.203 | 13.690 | 13.698 | 15.404 | 22.580 | 6.915 | 22.006 | 24.031 |
| custom | first | 12.845 | 13.030 | 13.061 | 14.427 | 21.424 | 6.938 | 21.359 | 23.325 |
| builtin | native | 13.240 | 13.413 | 13.446 | 14.992 | 22.131 | 7.038 | 21.888 | 24.537 |
| custom | native | 15.129 | 16.353 | 16.575 | 17.716 | 27.114 | 6.930 | 24.721 | 26.804 |

- First clean matrix: custom native → early GPU 15.129 → 12.845 ms; fragments 25.428M → 21.023M. Built-in native → late GPU 13.240 → 14.670 ms.
- Reverse-order repeat: custom native → early GPU 16.353 → 13.030 ms; fragments 27.114M → 21.424M. Built-in native → late GPU 13.413 → 15.243 ms.

All 480 camera matrices, player/truck poses and draw counts match the first
clean matrix. Input vertices remain 693,072 per frame; graphics reuse remains
two hits per frame with zero rebuilds and no steady-state material allocations.
Lambert source and truck imports are restored after the diagnostic swaps.

The direction repeats in both clean matrices: drawing the custom body early
reduces GPU time by 2.284 ms in the first and 3.324 ms in the repeat. The repeat's
GPU medians also separate (16.575 ms custom native versus 13.061 ms custom early),
so the mean difference is not explained by a few isolated slow tail frames.
The higher repeat-native time accompanies more actual fragment work (27.114M
versus the first native run's 25.428M), reinforcing that the two process loads
are not identical GPU workloads despite matching camera and draw counts.

The two repetitions provide a check against a one-off timing spike; they are
not enough to exclude all machine-load or clock variation. Scene startup also
changes some batch fingerprints/material IDs and fragment counts, so equal
camera/draw counts are not a strict same-work shader microbenchmark. Retain the
per-run numbers instead of averaging away those differences. Production ordering
is still unchanged.

Repeat raw captures use `custom-order` and `r3-clean`; the ignored
`custom-anime-clean-repeat-reversed-2026-11-07.manifest.json` preserves execution
order and measurements. The runner is copied beside it. Earlier `r1` captures
remain superseded; none are counted as a clean repetition.
