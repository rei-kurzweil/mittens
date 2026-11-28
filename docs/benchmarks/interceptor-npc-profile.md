# Interceptor NPC performance comparison

Compare zero and two NPC Mitten with the same build, camera, resolution,
graphics settings and warm-up. Both NPCs run the actual interceptor AI; their
random initial positions and subsequent patrol or combat may vary. Capture
several runs and record whether they are visible, walking, firing or idle.
The two-NPC case starts them automatically after one second for profiling;
the ordinary scene still starts with none.

## Headless CPU and MMS

```sh
cargo test --release --lib mms_scene_bench --no-run
MMS_BENCH_SCENES=examples/interceptor.mms MMS_BENCH_PREPARE_RENDER=1 \
  MMS_BENCH_WARMUP=240 MMS_BENCH_FRAMES=600 CAT_PROFILE_SYSTEMS=1 \
  MMS_BENCH_OUTPUT=/tmp/interceptor-cpu-0.json \
  cargo test --release --lib mms_scene_bench -- --ignored --nocapture
MMS_BENCH_SCENES=examples/interceptor.mms MMS_BENCH_PREPARE_RENDER=1 \
  MMS_BENCH_INTERCEPTOR_NPCS=2 MMS_BENCH_WARMUP=240 MMS_BENCH_FRAMES=600 \
  CAT_PROFILE_SYSTEMS=1 MMS_BENCH_OUTPUT=/tmp/interceptor-cpu-2.json \
  cargo test --release --lib mms_scene_bench -- --ignored --nocapture
```

The two-NPC run asserts that at least three Mitten models are present. Update
time and CPU render preparation are separate; the uploader does not use a GPU.
`MMS_BENCH_INTERCEPTOR_NPCS` takes any count up to 12. Set
`MMS_BENCH_INTERCEPTOR_SECONDARY_MOTION=off` on an NPC run to pause
both NPCs' secondary motion (as the demo's "Secondary" pedestal does) and
compare with the default, where it runs. The report records the average
number of paused secondary-motion roots, which should be non-zero only in the
`off` run.
Callback timings exclude deferred visual-phase callbacks, so they cannot be
subtracted from all script work. The headless scene has synthetic XR input,
no headset pacing and no GPU time; it cannot predict headset FPS.

## Stereo GPU timestamps

On a machine with Vulkan hardware, use the same GPU, extent and scene state:

`tests/profile-interceptor-gpu.sh` runs all three cases: zero NPCs, two with
secondary motion running, and two with it paused. Its per-frame JSON includes
`paused_motion_roots`; check it before attributing a timing difference to
pausing.

```sh
SHIP_PROFILE_SOURCE=examples/interceptor.mms SHIP_PROFILE_ROOM=interceptor \
  SHIP_PROFILE_CASE=full SHIP_PROFILE_GPU_PASSES=1 SHIP_PROFILE_UPDATE_TIMINGS=1 \
  SHIP_PROFILE_WARMUP=240 SHIP_PROFILE_FRAMES=480 \
  SHIP_PROFILE_OUTPUT=/tmp/interceptor-gpu-0.json \
  cargo test --release --lib profile_ship_stereo -- --ignored --nocapture
SHIP_PROFILE_SOURCE=examples/interceptor.mms SHIP_PROFILE_ROOM=interceptor \
  SHIP_PROFILE_CASE=full SHIP_PROFILE_GPU_PASSES=1 SHIP_PROFILE_UPDATE_TIMINGS=1 \
  SHIP_PROFILE_INTERCEPTOR_NPCS=2 SHIP_PROFILE_WARMUP=240 SHIP_PROFILE_FRAMES=480 \
  SHIP_PROFILE_OUTPUT=/tmp/interceptor-gpu-2.json \
  cargo test --release --lib profile_ship_stereo -- --ignored --nocapture
```

Add `SHIP_PROFILE_INTERCEPTOR_SECONDARY_MOTION=off` to the two-NPC command for
the paused comparison. Spawn positions and behavior can vary between runs.

Set `SHIP_PROFILE_WIDTH` and `SHIP_PROFILE_HEIGHT` to the headset's per-eye
extent. This is an offscreen stereo capture, not an OpenXR compositor run;
compare its GPU pass timestamps and deformation counters rather than FPS.

## MSAA, bloom and NPC matrix

`tests/profile-interceptor-matrix.sh` runs 18 one-minute offscreen stereo
captures at 1920x1920 per eye: 0, 1 and 2 NPCs x MSAA off, 2x and 4x x bloom
off and on. Each run grabs the spawn-side launcher with the right laser,
then walks, looks around, sways the hand and fires every 2 s
(`SHIP_PROFILE_MOTION=interceptor_play`); the launcher restocks itself.
`SHIP_PROFILE_MSAA`, `SHIP_PROFILE_BLOOM` and `SHIP_PROFILE_INTERCEPTOR_NPCS`
(any count up to 12) select a single run. The table lands in
`/tmp/interceptor-matrix/results.md` and `results.csv`; a "Launcher held"
value below 100% means the grab fixture missed.

### First matrix run (2026-11-28, GTX 1080, 1920x1920 per eye)

[Raw results](data/interceptor-matrix-2026-11-28-gtx1080.csv). The
launcher grab missed (held 0–5%), so no rockets were fired
([fix](../task/sta-launcher-handle-first-grab.md)). The 1- and 2-NPC rows
match closely; later runs report `npcs_present` to confirm both spawned.

- 0 NPCs: GPU 3.7–5.2 ms, CPU frame 12.6–14.5 ms, update ~4.9 ms.
- 1–2 NPCs: GPU 5.7–9.1 ms, CPU frame 22.4–27.5 ms, update ~11.4 ms. NPC
  logic adds ~6.5 ms of update per frame; that, not rendering, is the limit.
- MSAA off/2x/4x differences are within run-to-run noise.
- Bloom adds ~1.5–2.6 ms GPU and ~2.5 ms CPU with NPCs.

## Vantage sweep

`tests/profile-interceptor-vantages.sh` holds the XR camera at each of 9
deck sections (a 3 x 3 grid; north is -Z) x 26 directions (the neighbours of
a 3 x 3 x 3 cube) for 40 frames while the scene runs, for 0 and 2 NPCs at
MSAA 4x with bloom off. `vantages.md` maps estimated fps per section, per
direction and per vantage, colour-coded at 60 and 45 fps, with the slowest
vantages listed. Estimated fps is 1000 / max(CPU, GPU) headless, so the
headset will be lower.

## Headset capture

```sh
cargo build --release --example profile-sakura-xr
CAT_PROFILE_SYSTEMS=1 target/release/examples/profile-sakura-xr interceptor-0 /tmp/interceptor-xr-0 --warmup 30 --sample 60
CAT_PROFILE_SYSTEMS=1 target/release/examples/profile-sakura-xr interceptor-2 /tmp/interceptor-xr-2 --warmup 30 --sample 60
```

Use the same viewpoint and player movement in both captures. Compare projection
rate, frame time tails, update phases, skinning, render preparation and
deformation counts. GPU completion wait is a CPU wait, not measured GPU time;
the headset report does not yet include per-eye GPU timestamps. Use the
offscreen timestamp capture above or the headset vendor's GPU profiler for
actual GPU timings.

## Initial visibility comparison

On 2026-11-27, a headless release capture with 240 warm-up and 600 sampled
frames averaged 12.416 ms/update with full motion and 12.461 ms with visibility
control. The visibility run averaged 1.60 paused motion roots; secondary
motion averaged 5.600 ms versus 5.541 ms, while callback time rose from 1.751
to 1.888 ms. CPU render preparation was 0.262 versus 0.264 ms. This single
capture does not demonstrate a speedup: NPC behavior and the amount of
occlusion can change during sampling. Inspect collision-frame construction,
constraint work and which chains were paused, and repeat in a controlled
occlusion scene before deciding whether the policy is worthwhile.

The interceptor demo no longer runs this check: it showed no speedup and added
callback time. Its NPCs' secondary motion is now switched from a pedestal
instead. `secondary_motion_visibility` remains available to other scenes
through the `secondary_motion_observer` option.
