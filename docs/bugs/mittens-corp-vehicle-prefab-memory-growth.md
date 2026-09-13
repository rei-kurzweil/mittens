# `mittens-corp` retained callback animation materialization exhausts memory

Status: mitigated in the Mittens Corp fixtures; root engine/runtime fix open.

## Report

System memory grows rapidly while either of these scenes is running:

- `examples/mittens-corp-desktop.mms`
- `examples/mittens-corp.mms`

The user reports that the rough-transmission desktop example and the
`vtuber-mirror-example` XR example do not exhibit comparable growth. The
regression was noticed after the shared vehicle prefab was introduced, but
that was correlation only: the vehicle prefab is not the cause.

## Confirmed execution boundary

The first `SystemWorld::tick` completes with approximately 518 MiB RSS,
including GLTF loading, zones, secondary motion, layout, mirrors, and the
vehicle prefab. The retained session then starts exactly one callback:
Bisket's `GLTFInitialized` handler.

That handler completes head/camera setup and both eye queries. It then
registers the two `Transition` components created by `ambient_eye_saccades`
and starts registering its `Animation`. Memory grows without bound immediately
after `register_component component_type=Animation`, before the first keyframe
host operation. Runs consumed roughly 20 GiB of system memory.

This is a callback-return conversion defect, not animation playback. A
`Keyframe` stores a deferred body with a snapshot of its lexical environment.
The 32-keyframe animation is returned through the retained MMS session, whose
configured host falls back to `external_tree_to_legacy` for `Animation`.
That conversion recursively deep-copies each deferred closure environment and
every nested function environment it contains; it does not preserve shared
`Arc` environments or detect cycles. The inner `pose` closure and the callback
scope make those captured graphs large, and copying them once per keyframe
causes the runaway allocation before any keyframe can execute.

## Reproduction matrix

| Scene | Runtime | Reported growth |
| --- | --- | --- |
| `mittens-corp-desktop.mms` | desktop | yes |
| `mittens-corp.mms` | XR | yes |
| rough-transmission example | desktop | no observed growth |
| `vtuber-mirror-example` | XR | no observed growth |

Do not mark the non-reproducing examples as proof that their corresponding
backends are safe; they are controls for the scene/prefab comparison only.

## Instrumentation

Set `MITTENS_DEBUG_GROWTH_AUDIT=1` before launching an affected scene. Every
120 frames (and once at frame 1), `SystemWorld` logs:

- Linux process RSS when `/proc/self/status` is available;
- live ECS component, Transform, Renderable, GLTF, and Zone counts;
- retained zone-marker count;
- `VisualWorld` instance/mirror counts;
- CPU/imported mesh counts; and
- loaded AssetSystem module/item counts.

For a first-frame stall it also logs update phases, retained callback entry,
and every host operation performed while executing that callback.

Example:

```sh
MITTENS_DEBUG_GROWTH_AUDIT=1 cargo run --release -- load examples/mittens-corp-desktop.mms
```

Capture equivalent 2--5 minute runs for both failing scenes and both controls.
Record the full `[GrowthAudit]` sequence, approximate wall-clock time, whether
Show Zones is enabled, and whether the car was mounted or interacted with.

## Mitigation

The two Mittens Corp fixtures no longer import or construct
`ambient_eye_saccades` from `GLTFInitialized`. Desktop retains its head-camera
attachment; both avatars retain their authored/rest eye pose. The vehicle
prefab remains enabled.

## Required engine fix

Make `Animation` construction inside a retained `RuntimeSpecSession` callback
bounded and equivalent to initial scene evaluation. Either handle `Animation`
and `Keyframe` in the configured registry without legacy conversion, or make
the external-to-legacy conversion graph-aware so shared/cyclic captured
environments remain shared. Add a regression test that invokes a
GLTF-initialized callback which creates an animation with multiple keyframe
closures, and assert it completes within an allocation budget.
