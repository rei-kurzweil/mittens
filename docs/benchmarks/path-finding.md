# Path finding: MMS A* vs PathFindingSystem

Measures the interceptor NPC's route planning before and after A* moves from
`assets/components/targets/interceptor_paths.mms` into a Rust
`PathFindingSystem`. The scene is `examples/path-finding-benchmark.mms`, a copy
of `examples/interceptor.mms` with a benchmark driver appended.

## Method

The driver calls the planner only through the entry points the game uses:
`paths.plan`, `paths.retreat` and `armory.nearest`. The same scene
file runs at both commits.

The planning calls use 15 fixed points on the roof: NPC spawns, stand
approaches, both sides of the hull, outside each wall and door, and two
corners.

| Phase | Work | Reported |
|---|---|---|
| settle | 3 s, nothing measured | lets pistols settle on their stands |
| idle-frames | 4 s of normal frames | `cpu_update_ms`, `gpu_frame_ms` |
| verify | untimed, one pass | route fingerprints |
| plan | per frame: one point to the other 14; 3 passes (630 calls) | ms/call, worst frame |
| retreat | per frame: one point, both player positions; 3 passes (90 calls) | ms/call, worst frame |
| nearest | per frame: `armory.nearest` from one point; 3 passes (45 calls) | ms/call, worst frame |
| nearest-frames | 4 s, `armory.nearest` every frame | `cpu_update_ms`, `gpu_frame_ms` |

Timed phases wrap each frame's chunk in `Benchmark.start`/`stop` inside one
callback, so `duration_ms` is wall-clock time for that chunk only. The
`*-frames` windows span real frames. Compare `nearest-frames` with
`idle-frames` to see the cost inside a frame.

Fingerprints sum route lengths (rounded to 0.01 m) and waypoint counts per
entry point, plus the armory scores. Waypoints are only counted, because
equal-length routes may break ties differently. The before and after
fingerprints should match.

`nearest` currently runs one A* search per loaded weapon. If the
PathFindingSystem version searches for all of them at once, its `nearest` row
combines the batching gain with the move to Rust. The `plan` row is the
like-for-like comparison.

## Running

Run solo and stand still. Don't touch the pistols until the results board
appears above the panels. The results also go to the log.

```sh
cargo run --release -- load examples/path-finding-benchmark.mms
```

## Results

### Before: MMS A* (commit `TODO`)

| Device | plan ms/call | plan worst frame | retreat ms/call | nearest ms/call | nearest worst frame | idle cpu mean / max | nearest-frames cpu mean / max |
|---|---|---|---|---|---|---|---|
| GTX 1080 desktop | | | | | | | |
| Focus 3 (XR2) | | | | | | | |

Fingerprints:

```text
fingerprint plan: ...
fingerprint retreat: ...
fingerprint nearest: ...
```

### After: PathFindingSystem (commit `TODO`)

| Device | plan ms/call | plan worst frame | retreat ms/call | nearest ms/call | nearest worst frame | idle cpu mean / max | nearest-frames cpu mean / max |
|---|---|---|---|---|---|---|---|
| GTX 1080 desktop | | | | | | | |
| Focus 3 (XR2) | | | | | | | |

Fingerprints match before: TODO
