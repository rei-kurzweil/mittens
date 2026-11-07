# Local cafeteria: walking and driving

Date: 2026-11-07. Engine revision `2887a7b1` plus test-only source selection
and native locomotion/mounting benchmark support. RoomSystem remains a proposal.
No scene, vehicle, mirror or production rendering behavior was changed.

## Conditions and validity

- NVIDIA GeForce GTX 1080, driver 580.159.04; offscreen stereo **1920 x 1920 per eye**,
  MSAA 4x, bloom and emissive extraction enabled, far distance 1000 m.
- Ignored `examples/local/ship_interior.mms`: Mitten player, Miku hot-dog vendor,
  one cafeteria pickup; orchard/biodome absent. Bridge, bridge hallway, barracks,
  washroom passage and women's washroom remain instantiated. Existing authored
  performance isolation remains (for example, barracks weapon table without the
  grocery inventory); this is not a restored maximum-detail scene.
- Existing washroom mirror defaults off. Every frame asserts it stays off; zero
  mirror captures. The fixture does not exercise mirror rendering or entry zones.
- Native truck gravity/contact stays active in both runs. Truck controls are off
  when walking; driving uses the existing mount transaction, power-button Click
  handler and XR thumbstick input. No manual camera-follow or truck teleport loop.
- Walking: forward/back at 60% stick, changing direction every 2 synthetic seconds.
  Driving: forward/reverse at 32% throttle, changing direction every 1.8 seconds,
  20% steering. Both include a small head turn and use native input/locomotion.
  This is a repeatable short cafeteria route, not an exhaustive tour of the room.
- Each scenario is a fresh process: 240 warmup then 480 sampled stereo frames,
  synthetic simulation at 75 Hz. Run order walking then driving, `clean-r1`.
- Initial `smoke` and `full` captures were **discarded**: the user reported the
  live app was still open. Both scenarios were restarted after it closed. Before
  the clean restart, `nvidia-smi` showed 1% GPU utilization and no live demo.
  Desktop/browser and SteamVR/ALVR processes remained present. No benchmark or
  Rust build was run concurrently with the clean captures.
- Microphone uses the existing benchmark's silent oscillator substitution. No
  live headset, compositor submission or ALVR encoding is included in the timed
  workload. FPS below is an estimate from elapsed frame time, not headset FPS.

## Clean results

Times are milliseconds per complete stereo frame. GPU graphics includes both
eyes. Elapsed time is measured independently; CPU and GPU stages overlap and
should not be added together.

| Measurement | Walking | Driving |
| --- | ---: | ---: |
| CPU scene update | 5.486 | 13.718 |
| CPU preparation | 0.351 | 0.500 |
| CPU recording/submission | 0.883 | 0.967 |
| GPU deformation | 0.045 | 0.046 |
| GPU graphics | 11.547 | 15.362 |
| Elapsed frame | 18.238 | 30.537 |
| Elapsed p95 | 20.395 | 31.891 |
| FPS estimated from mean elapsed | 54.8 | 32.7 |

Walking meets the 22.0 ms goal in this capture, including its 20.395 ms p95.
Driving does not meet the 22.22 ms / 45 FPS cutoff: 30.537 ms mean, 31.891 ms p95.
It adds about 8.23 ms of CPU scene update and 3.81 ms of GPU graphics relative to
the walking route. The viewpoints differ, so this is not a controlled isolation
of vehicle cost or proof that a particular vehicle subsystem caused the CPU
increase. A per-system CPU breakdown during driving is the next useful profile.

Both scenarios reuse two graphics primaries per frame, with zero rebuilds, zero
instance-buffer allocations and zero material binding misses/evictions during
sampling. The driving slowdown is therefore not graphics-primary reconstruction.
Dynamic deformation remains active in both captures.

Recorded walking positions span X -0.071..0.051, Z 9.999..11.497 m. The driving
truck spans X -9.506..-9.309, Z 12.173..15.000 m; the rider remains mounted and
controls remain powered throughout. Assertions also verify background room
identities remain present. These checks prevent a stationary-camera fixture or
accidental unloading from masquerading as a moving-scene improvement.

The older regular-orchard capture is a different workload with extra geometry,
vehicles and a different vendor; do not attribute the difference solely to room
streaming or the removed orchard. Room streaming is not implemented.

## Reproduction and preserved artifacts

The local scene and restricted model remain ignored and are not redistributed.
Raw summaries, frame positions/timings and logs are preserved under the ignored
`examples/local/benchmarks/` folder, using these stems:

- `cafeteria-1920-walking-clean-r1-2026-11-07`
- `cafeteria-1920-driving-clean-r1-2026-11-07`

Each stem has `.json`, `.frames.json` and `.log` files.
`clean-run-manifest-2026-11-07.json` records revision/source hashes and discarded
run identifiers. The folder README marks the contaminated runs invalid.

With the same local scene/model available, run separately for `walking` and
`driving` (use a new output name for every capture):

```sh
SHIP_PROFILE_SOURCE=examples/local/ship_interior.mms \
SHIP_PROFILE_ROOM=cafeteria SHIP_PROFILE_CASE=full \
SHIP_PROFILE_MOTION=walking \
SHIP_PROFILE_WIDTH=1920 SHIP_PROFILE_HEIGHT=1920 \
SHIP_PROFILE_WARMUP=240 SHIP_PROFILE_FRAMES=480 \
SHIP_PROFILE_OUTPUT=examples/local/benchmarks/new-walking.json \
cargo test --release --lib profile_ship_stereo -- --ignored --nocapture
```

Historical timings are retained in the [benchmark ledger](benchmark-history.md).
