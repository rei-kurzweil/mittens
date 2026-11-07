# Retained upload buffers: first prototype

Recorded 2026-11-06 against baseline `4a310ae8` plus the uncommitted retained
upload implementation. This is an intermediate checkpoint, before any proposed
post-processing command-buffer reuse; it is not a final performance claim.

## Change

Retain host-visible instance, camera and light upload buffers by view and purpose.
Reuse storage when writable, grow when necessary, and replace busy storage rather
than waiting for it. Reuse the rig descriptor while its deformation output buffer
identity is unchanged. Remove disabled legacy per-slot bone-upload branches.
The main scene and post-processing command buffers are still recorded each frame.

## Capture conditions and results

Stationary `ship_interior.mms` cafeteria; NVIDIA GeForce GTX 1080;
1920 x 1920 per eye; bloom and emission extraction enabled; MSAA 4x;
240 warmup frames and 480 sampled frames. Sequential separate-process controls
use the same implementation, with test-only `SHIP_PROFILE_RETAIN_UPLOADS=0`
disabling retention for the control. The existing parked-truck behavior remains
in this fixture; continuous native truck physics is a separate tracked correction.

| Mean per stereo frame | Disabled | Enabled |
| --- | ---: | ---: |
| CPU update | 4.366 ms | 4.353 ms |
| CPU preparation | 0.540 ms | 0.546 ms |
| CPU recording/submission | 6.843 ms | 6.815 ms |
| GPU graphics | 14.081 ms | 13.765 ms |
| Elapsed frame | 23.629 ms | 23.294 ms |
| Retained-upload allocations | 13 | 0 |
| Retained-upload reuses | 0 | 13 |
| Busy-buffer replacements | 0 | 0 |
| Instance-buffer allocations | 7 | 0 |
| Instance records written | 2319 | 2319 |

The enabled run is approximately 42.9 FPS and remains above the 22.0 ms target.
The allocation reduction is directly measured. The CPU recording difference is
only 0.028 ms; separate-process GPU variation prevents attributing the elapsed
0.335 ms difference entirely to retention. GPU execution overlaps CPU work;
these columns must not be added together. Offscreen results exclude OpenXR,
compositor and ALVR overhead.

## Validation and artifacts

Production `cargo check --lib`, release visual-world tests (22), retained-upload
GPU storage safety, stereo drawing, mirror orientation/freeze and custom anime
material/posed-avatar GPU tests passed for this prototype.

Preserved summaries contain means, medians, p95 values, renderer stage breakdowns,
draw counts and cache counters. Frame samples and logs are kept alongside them:

- Disabled: [summary](data/retained-uploads-off-2026-11-06.json), [frames](data/retained-uploads-off-2026-11-06.frames.json), [log](data/retained-uploads-off-2026-11-06.log).
- Enabled: [summary](data/retained-uploads-on-2026-11-06.json), [frames](data/retained-uploads-on-2026-11-06.frames.json), [log](data/retained-uploads-on-2026-11-06.log).

Append later implementation stages as new captures rather than replacing these
prototype measurements. Next experiments must record their additional changes
and retain these controls as historical evidence.
