# Generic XR eye-tracking source selection

Date: 2026-09-06

Status: source extraction and first generic selection slice implemented; liveness, shared receivers,
and explicit-override diagnostics remain

Policy revision: [One-shot XR eye-tracking source election](one-shot-xr-eye-tracking-source-election.md)
replaces the continuous/per-channel selection described in this task with a bounded startup window
and one locked whole-eye source.

Naming direction (2026-10-20): [Avatar tracking components and MediaPipe](avatar-tracking-components-and-mediapipe.md)
proposes `WebcamEyeTracking` (lowercase `c`) as the webcam counterpart to the existing
`XREyeTracking`, with MediaPipe as one webcam backend. The MediaPipe enum/component placeholders
documented here remain existing compatibility surface; that proposed webcam component is not yet
implemented. There is currently no registered unprefixed `EyeTracking` component.

Parent: [Eye and face tracking epic](epic/eye-and-face-tracking.md)

Related:

- [Unified two-eye tracking normalization and AVC routing](unified-two-eye-tracking-normalization-and-avc-routing.md)
- [AVC runtime eye-tracking selection panel](avc-eye-tracking-runtime-selection.md)

## Outcome

Make `XREyeTracking.on()` the automatic, transport-neutral eye-tracking component its name implies.
When it is a direct child of `AVC`, it discovers the supported sources, chooses the highest-priority
source that is currently producing valid samples, and fails over when that source stops.

The source set is:

```rust
enum EyeTrackingSource {
    Htc,
    VrChatOsc,
    MediaPipe,
}
```

`MediaPipe` belongs in the public/configuration enum now so scenes can express their intended final
priority order, but it reports `unsupported`/`unavailable` until webcam tracking is implemented.

Keep every source as a first-class component with its own settings:

- `VRChatOSCEyeTracking`
- `HTCEyeTracking`
- `MediaPipeEyeTracking` (configurable placeholder until implemented)

`XREyeTracking` is the selector, not a transport implementation. It creates default source
components when none are authored, or consumes source-specific children whose settings override
those defaults. A source-specific tracker placed directly beneath an `AVC` bypasses automatic
selection and explicitly chooses that source.

After this task, the recently updated XR examples can keep one simple direct child:

```mms
AVC {
    XREyeTracking.on()
    // avatar, controllers, microphone amplitude, etc.
}
```

and receive HTC binary UDP or VRChat Eye OSC without editing the example.

## Starting point

Before the first implementation slice, the names did not match their actual scopes:

| MMS component | Current transport | Default endpoint | AVC use |
|---|---|---|---|
| `XREyeTracking` | ALVR/VRChat Eye OSC | `127.0.0.1:9000` | direct-child gaze and closure |
| `XREyeTrackingHTC` | custom Mittens HTC/ALVR binary UDP | `127.0.0.1:9002` | direct-child gaze and closure |

`XREyeTrackingSystem::tick` always runs two independent scans. `tick_standard` only finds
`XREyeTrackingComponent`; `tick_htc` only finds `XREyeTrackingHtcComponent`. Each component owns a
source-specific socket registration. Consequently, `XREyeTracking.on()` never checks the HTC
transport.

AVC is already mostly transport-neutral. `newest_direct_eye_gaze` and
`newest_direct_eye_closure` inspect both direct-child component types and compare a receive sequence
shared by both transports. This is sample arbitration across separately authored trackers, not
automatic source discovery. It also means that, if both components are attached, whichever one sent
the newest packet can take control independently for each eye/channel; there is no authored source
priority.

The XR examples commonly attached `XREyeTracking.on()` directly to `AVC`, which previously opted in
to OSC only. The focused microphone/eye-tracking example worked around this with a UI that physically
swapped the old `XREyeTracking` and `XREyeTrackingHTC` transport components. It now uses the
canonical `VRChatOSCEyeTracking` and `HTCEyeTracking` names and remains useful as an explicit
transport demo, while ordinary examples use the generic selector.

The target names deliberately separate the selector from the implementations:

| Target component | Responsibility |
|---|---|
| `XREyeTracking` | automatic discovery, priority, failover, and normalized selected output |
| `VRChatOSCEyeTracking` | VRChat Eye OSC endpoint, decoding, liveness, and OSC-specific settings |
| `HTCEyeTracking` | HTC/Mittens binary endpoint, decoding, liveness, and HTC-specific settings |
| `MediaPipeEyeTracking` | webcam/device/model settings and MediaPipe samples once implemented |

Keep `XREyeTrackingHTC` as a compatibility alias during migration if removing it would break scenes.
The canonical new name should be `HTCEyeTracking`, symmetric with the other source components.

## Design decisions

### Availability means recent valid data

Both implemented sources are connectionless UDP listeners. A successful socket bind only means the
local endpoint is free; it does not mean a tracker exists. Treat a source as available only after a
valid semantic sample arrives, and keep it available only while valid samples arrive within a
documented liveness window.

Track liveness per source and preferably per semantic channel. At minimum, gaze liveness must not be
refreshed by closure-only traffic, and closure liveness must not be refreshed by gaze-only traffic.
The existing indefinitely retained gaze and one-tick closure behavior can remain the downstream
channel policies after source selection, but cannot substitute for source availability.

Malformed packets, packets with no valid eye fields, bind failure, and the unimplemented MediaPipe
variant do not make a source available.

### Priority is an ordered source list

Expose a builder whose value is an ordered list of `EyeTrackingSource`, highest priority first. An
ordered list is clearer than assigning numeric levels independently and makes ties impossible.

Proposed MMS surface:

```mms
// Default: prefer richer XR-native data, then interoperable OSC, then webcam.
XREyeTracking.on()
    .priority(["htc", "vrchat_osc", "mediapipe"])

// Force OSC and do not probe any other source.
XREyeTracking.on()
    .priority(["vrchat_osc"])
```

The concrete Rust field should use enum values, not strings. MMS parsing accepts stable lowercase
names and rejects unknown or duplicate entries. An omitted source is disabled, which lets the same
builder express both ordering and an allow-list. Reject an empty list.

Recommended default order is `Htc`, `VrChatOsc`, `MediaPipe`: the HTC packet is the richer per-eye
XR source, OSC is the interoperable XR baseline, and a future webcam estimator is the final
fallback. This default is policy, not a transport-quality claim, and scenes can reverse it.

Selection is deterministic:

1. Among sources whose channel is live, use the first source in the authored order.
2. Promote immediately when a higher-priority source supplies a valid sample.
3. Fall back only after the selected source's liveness window expires or it becomes invalid.
4. Do not alternate based merely on newest packet when two sources remain live.
5. Resolve gaze and closure independently so a source lacking one channel does not suppress a valid
   lower-priority provider for that channel. Retain active-source provenance per channel.

The timeout and clock must be injectable or controllable in tests. If live testing shows boundary
flapping, add a small hysteresis/grace policy explicitly rather than relying on frame order.

### Source components own source settings

Do not move endpoint, camera, model, calibration, or transport-specific options onto the generic
selector. Each source component owns those settings even when it is usually created implicitly.

The fully authored topology is:

```mms
AVC {
    XREyeTracking.on()
        .priority(["htc", "vrchat_osc", "mediapipe"]) {
        VRChatOSCEyeTracking.listen("127.0.0.1", 9000)
        HTCEyeTracking.listen("127.0.0.1", 9002)
        MediaPipeEyeTracking.on()
            .camera(0)
    }
}
```

The exact future MediaPipe builders are illustrative, not committed API. Its component can be
constructed and retained now, but must expose an honest unsupported state rather than pretending a
camera or model is active.

When no source children are authored, `XREyeTracking.on()` materializes or internally owns one
default configuration for every source named in its priority list. An authored child replaces the
default configuration for that source kind. Missing children do not disable a source; omission from
the priority list does. Reject duplicate children of the same source kind unless multi-device
selection is designed explicitly later.

This gives ordinary examples zero-config discovery while retaining a place for settings such as:

- OSC/HTC host and port;
- transport-specific packet or protocol versions;
- MediaPipe camera/device, model, confidence, and performance options;
- future source-specific coordinate conversion or calibration.

Shared post-selection pose policy—rotation limits and head-rotation compensation—remains on
`XREyeTracking`. When a source component is attached directly to AVC, it also needs those common
builders so explicit single-source use remains self-contained.

### Explicit topology overrides automatic discovery

Topology communicates composition versus explicit selection. For one `AVC`:

- a direct `XREyeTracking` child, with default or authored source children: automatic selection is
  active;
- a direct source-specific child such as `HTCEyeTracking`: that source configuration is
  authoritative and bypasses automatic selection for that AVC;
- multiple explicit source-specific children: retain the existing normalized newest-sample behavior
  initially, but diagnose the ambiguous configuration and recommend one explicit component or one
  automatic component;
- source components nested beneath `XREyeTracking` are candidates for that selector, not independent
  AVC drivers;
- trackers elsewhere in the world do not drive that AVC.

Disabling the automatic component for an explicit sibling avoids two listeners competing for the
same UDP endpoint and preserves the established rule that direct AVC children are intentional pose
drivers. Do not infer an override from unrelated trackers elsewhere in the world.

Use `VRChatOSCEyeTracking` rather than `XREyeTrackingOSC`: the former identifies the actual protocol
contract and leaves room for other OSC schemas. Do not encode transport choice in AVC.

### Receive once, consume many

Do not implement automatic probing by binding ports once per avatar or once per candidate. Multiple
avatars using `XREyeTracking.on()` should be able to consume the same local tracking feed.

Refactor the transport layer toward shared receivers keyed by source and endpoint:

```text
UDP/MediaPipe receiver -> source-native decode -> normalized channel samples
                                          -> generic tracker subscriptions
                                          -> explicit tracker subscriptions/events
```

One receiver can fan the latest normalized sample out to multiple generic components. Reference
counts or subscriber sets determine receiver lifetime. This also gives component removal one place
to release sockets immediately, instead of waiting for the next full-world retain scan.

Custom endpoints remain settings on the source-specific components. Migrate the current
`XREyeTracking.listen(host, port)` to `VRChatOSCEyeTracking.listen(host, port)`, preserving the old
constructor as a compatibility alias/deprecation path as needed. Do not silently apply one
host/port pair to multiple UDP protocols.

## Component and event compatibility

Changing `XREyeTracking.on()` from OSC-only to an automatic selector is an intentional behavioral expansion:
existing OSC-only scenes continue to work, while a live higher-priority HTC feed may now win. A
scene requiring the old strict behavior uses `.priority(["vrchat_osc"])`.

Keep the normalized state consumed by AVC source-neutral. The generic component should retain the
selected left/right gaze and closure plus source provenance; AVC should not add HTC-, OSC-, or
MediaPipe-specific pose branches.

Preserve `XrEyeTrackingUpdated` and `XrEyeTrackingHtcUpdated` for source-native diagnostics and old
scripts. Define a new source-neutral event only if scripts need to observe the generic selection;
do not change the meanings of the existing event payloads. A useful later event would include
`source`, `channel`, and `available/selected` state without exposing socket details.

All existing shared calibration builders (`head_rotation_compensation`, `rotation_limits`, and
`rotation_limits_per_eye`) apply after selection and therefore behave identically for every source.
Transport/camera-specific configuration belongs to the corresponding source component.

## Implementation plan

### Implemented first slice

- Added `EyeTrackingSource` with `Htc`, `VrChatOsc`, and the reserved `MediaPipe` variant.
- Split the transport structs into canonical `VRChatOSCEyeTrackingComponent` and
  `HTCEyeTrackingComponent` types and added a constructible `MediaPipeEyeTrackingComponent`
  placeholder.
- Registered `VRChatOSCEyeTracking`, `HTCEyeTracking`, and `MediaPipeEyeTracking` in MMS while
  preserving the old HTC name as a compatibility entry point.
- Made `XREyeTrackingComponent` transport-neutral, with ordered priority, selected normalized
  samples, and per-channel source provenance.
- Added `.priority(["htc", "vrchat_osc", "mediapipe"])` parsing with empty, unknown, and duplicate
  rejection.
- Made the system create missing default source children and select gaze and closure independently
  by configured priority.
- Preserved legacy `XREyeTracking.listen(host, port)` as an OSC-only generic configuration and
  forwards source-native update events from implicit children to the generic component.
- Extended AVC to accept the generic selector and direct canonical source components.

This slice makes ordinary `XREyeTracking.on()` examples probe both implemented default transports.
It does not yet expire retained gaze based on source liveness, share a single receiver among
multiple avatars, or suppress a generic selector when an explicit source sibling is also attached.

1. Add `EyeTrackingSource`, its strict MMS parser, the default ordered list, and the `priority([...])`
   builder to `XREyeTrackingComponent`, registry dispatch, runtime signatures, and serialization or
   round-trip support where applicable. Include `MediaPipe` but mark it unimplemented.
2. Split the current transport implementations into canonical `VRChatOSCEyeTrackingComponent` and
   `HTCEyeTrackingComponent` types. Add `MediaPipeEyeTrackingComponent` as an unsupported
   configuration placeholder. Preserve current names/constructors through deliberate aliases or a
   documented migration.
3. Make generic `XREyeTracking` discover its source-specific children, synthesize defaults for
   priority entries without an authored child, and reject duplicate source kinds.
4. Separate the current OSC-specific receiver/state from the generic component identity. Add
   source/endpoint-keyed shared receiver state so OSC and HTC defaults can be probed concurrently
   without per-avatar bind conflicts.
5. Record valid-sample arrival times and normalized samples per source/channel. Implement stable
   ordered selection and retain selected-source provenance on the generic component.
6. During AVC/direct-child resolution, distinguish a direct explicit source component from source
   components nested under the generic selector. Disable the
   generic candidate for that AVC and emit a once-per-configuration diagnostic. Keep source
   selection out of avatar bone and morph application code.
7. Preserve source-native events, explicit `XREyeTrackingHTC` behavior, custom endpoints, removal
   cleanup, and the existing normalization rules.
8. Change the general XR examples only if syntax or comments need migration. They should end with a
   plain `XREyeTracking.on()` and automatically accept either implemented transport.
9. Keep `vtuber-microphone-speaking-xr-eye-tracking.mms` as the explicit/manual selection test, or
   add an automatic mode to it; do not remove the ability to validate one transport in isolation.
10. Update the eye/face tracking epic and component documentation once the API lands.

## Tests

- Default and custom priority parsing, unknown names, duplicates, omitted sources, and empty lists.
- `MediaPipe` is accepted in configuration but remains unavailable and never blocks fallback.
- OSC only, HTC only, neither source, and both sources live under each priority order.
- Higher-priority promotion, timeout fallback, recovery, malformed packets, and bind failure.
- Independent gaze/closure selection when one source supplies only one channel.
- Default source materialization, authored per-source settings, omitted priority entries, and
  duplicate source-child rejection.
- Two generic trackers/avatars consume the same default receivers without address-in-use failures.
- A nested `HTCEyeTracking` is a candidate of its parent selector; a direct `HTCEyeTracking` is an
  explicit AVC driver. A tracker beneath another AVC has no effect.
- Component/subtree removal drops subscriptions and closes an unused receiver immediately.
- Existing source-native events retain their payloads and component routing.
- Existing OSC scenes remain valid; `.priority(["vrchat_osc"])` reproduces strict old behavior.
- Calibration limits and head-rotation policy apply after selection for OSC and HTC alike.

## Acceptance criteria

- `XREyeTracking.on()` can drive an AVC from either VRChat Eye OSC or HTC binary UDP without a scene
  edit or runtime selection UI.
- Selection follows the authored order of live sources and does not reduce to newest-packet wins.
- Loss of the selected source causes bounded, tested fallback; a bound-but-silent UDP socket is not
  considered available.
- `EyeTrackingSource::MediaPipe` and its MMS spelling exist, while its unavailable status is honest
  until implementation.
- `VRChatOSCEyeTracking`, `HTCEyeTracking`, and `MediaPipeEyeTracking` are independently
  constructible and retain their own source-specific settings.
- A direct source-specific tracker is an explicit topology override and cannot contend with the
  generic tracker for a port or AVC ownership.
- AVC continues to consume one normalized two-eye contract and contains no transport decoder or
  transport-specific rig logic.
- Multiple avatars can share one tracking feed.
- The updated XR examples use the generic component and work with whichever implemented source is
  actually present.

## Files expected to change

- `src/engine/ecs/component/xr_eye_tracking.rs`
- `src/engine/ecs/system/xr_eye_tracking_system.rs`
- `src/engine/ecs/system/avatar_control_system.rs`
- `src/scripting/component_registry.rs`
- `src/scripting/runtime_config.rs`
- `src/scripting/tests.rs`
- XR examples containing `XREyeTracking.on()`
- eye-tracking task/epic documentation
