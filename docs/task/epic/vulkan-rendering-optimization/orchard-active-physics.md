# Orchard stereo benchmark: active truck physics

Date: 2026-11-07. Revision **`ae55468f`**, regular
`examples/ship_interior.mms`, before any RoomSystem implementation. The ignored
local Miku experiment is not used in these captures.

## Conditions

- NVIDIA GeForce GTX 1080; synthetic offscreen stereo, **1920 x 1920 per eye**.
- Bloom and emissive extraction enabled; MSAA 4x; far distance 1000 m.
- Existing orchard fixture preset: movement root `[18,6.2,7.5]`, initial yaw
  `-2.1112158`; stationary and moving-head variants. Per-frame camera matrices
  are retained in the samples.
- 240 warmup frames, then 480 sampled stereo frames per process. Sequential run
  order: stationary reuse/fresh, moving-head fresh/reuse. No competing benchmark
  or build was deliberately run during sampling.
- Pickup controls default off, **native gravity/contact/motion remain active**.
  The moving-parent CombineMesh cache fix is present. The prior frozen-truck
  cafeteria workload is a different physics configuration.
- The ship's existing performance-isolation choices remain: orchard trees are
  commented out and the barracks uses the weapons table rather than grocery
  inventory. Room contents are not yet distance-streamed. The cafeteria vendor
  is the ordinary Lumae asset.

## Results

Means are per complete stereo frame. GPU graphics covers both eyes. GPU work
and CPU work overlap; do not add GPU time to CPU stages. Elapsed frame time is
measured independently, including the fixture's completion wait.

| Measurement | Stationary fresh | Stationary reused | Moving-head fresh | Moving-head reused |
| --- | ---: | ---: | ---: | ---: |
| CPU scene update | 9.682 ms | 9.056 ms | 9.694 ms | 9.233 ms |
| CPU preparation | 0.638 ms | 0.635 ms | 0.630 ms | 0.649 ms |
| CPU recording/submission | 7.184 ms | 0.987 ms | 7.198 ms | 0.994 ms |
| GPU graphics, both eyes | 12.595 ms | 12.572 ms | 14.020 ms | 14.286 ms |
| Elapsed frame | 27.937 ms | 23.216 ms | 29.378 ms | 25.150 ms |
| Elapsed p95 | 29.740 ms | 24.776 ms | 32.764 ms | 28.826 ms |
| FPS estimated from mean elapsed | 35.8 | 43.1 | 34.0 | 39.8 |

Both reused runs have exactly two graphics-command cache hits, zero rebuilds
and zero instance-buffer allocations per sampled frame. Fresh/reused draw counts
match. Active deformation remains: the summaries include the dispatch/job and
bone-upload counters. These checks distinguish command reuse from skipping
objects, freezing joint matrices or disabling emission.

Command reuse still removes about **6.2 ms of CPU recording/submission** per
stereo frame. The normal reused orchard runs do **not** reach 45 FPS: stationary
23.216 ms (43.1 FPS), moving-head 25.150 ms (39.8 FPS), against the 22.0 ms goal /
22.22 ms cutoff. Scene update remains about 9 ms. The
[native-physics audit](../../native-velocity-sleeping-and-wake.md) separately
records the substantial active-physics cost and supported orchard pose drift.

Room-distance unloading may remove unrelated room rendering, lights, script
contents and DOM work, but it has not been measured or implemented. It must not
obtain savings by disabling truck physics. Native rest/wake remains separate
work. Sequential-process variation and dynamic viewpoints limit precise GPU
speedup claims; offscreen results exclude live OpenXR, compositor and ALVR.

## Preserved artifacts

- stationary, fresh: [summary](data/orchard-1920-stationary-fresh-2026-11-07.json), [frame samples](data/orchard-1920-stationary-fresh-2026-11-07.frames.json), [log](data/orchard-1920-stationary-fresh-2026-11-07.log).
- stationary, reuse: [summary](data/orchard-1920-stationary-reuse-2026-11-07.json), [frame samples](data/orchard-1920-stationary-reuse-2026-11-07.frames.json), [log](data/orchard-1920-stationary-reuse-2026-11-07.log).
- head, fresh: [summary](data/orchard-1920-head-fresh-2026-11-07.json), [frame samples](data/orchard-1920-head-fresh-2026-11-07.frames.json), [log](data/orchard-1920-head-fresh-2026-11-07.log).
- head, reuse: [summary](data/orchard-1920-head-reuse-2026-11-07.json), [frame samples](data/orchard-1920-head-reuse-2026-11-07.frames.json), [log](data/orchard-1920-head-reuse-2026-11-07.log).

Next design step: [RoomSystem task and proposed MMS syntax](../../room-system-distance-streaming.md).
