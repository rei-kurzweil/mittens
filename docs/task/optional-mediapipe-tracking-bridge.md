# Optional MediaPipe tracking process and transport

Date: 2026-10-09

Status: proposed architecture and protocol outline; no bridge or receiver implemented.
Ownership decision (2026-10-19): a shared engine-side `MediaPipeSystem` owns the provider
state and publishes independent hand/eye channels to their semantic tracking systems.

Release target: [mittens-engine 0.10.0](epic/0.10.0/README.md), following the
normalized hand sample/selection foundation. The bridge remains optional.

Depends on the normalized sample/selection contract in
[shared hand tracking](hand-tracking-system-and-finger-retargeting.md).

## Recommendation

Run MediaPipe in a separately installed Python process, with a dedicated virtual environment,
camera capture dependencies, and explicitly installed model assets. Mittens receives compact
tracking snapshots through `MediaPipeSystem`. Building and starting Mittens, or using OpenXR,
must require none of those dependencies. Keep camera inference outside the render/tick thread.

Start with one-way nonblocking UDP over loopback and full snapshots. Manual launch is sufficient
for the first slice; a later opt-in launcher can manage an explicitly configured interpreter and
report install/model/camera failures. Installation and model download are separate user actions,
not engine startup behavior. Keep the engine/bridge protocol independently versioned.

## Engine owner: MediaPipeSystem

Introduce `MediaPipeSystem` in `src/engine/ecs/system/mediapipe_system.rs`, retained as
`SystemWorld.mediapipe`. This is the shared engine-side provider owner for hands, eyes, and
eventually mouth/face channels. The external Python process owns camera capture, model loading,
and inference; the Rust system owns ingestion and the provider state consumed by engine systems.
The name and ownership boundary are decided here; this system is not implemented yet.

| Owner | State and logic |
| --- | --- |
| Python MediaPipe bridge | Camera/device, loaded models, inference work, producer identity, capture timestamps, encoded channel snapshots |
| `MediaPipeSystem` | Shared receiver registrations, sockets, bounded receive/decode, protocol validation, stream epochs, retired-stream rejection, per-channel sequences and receipt times, coordinate/calibration metadata, normalization, latest immutable snapshots, provider diagnostics |
| `HandTrackingSystem` | Hand-source selection, articulation freshness policy, rig bindings, finger rotation ownership, and release/fallback |
| `XREyeTrackingSystem` | MediaPipe eye-source subscriptions, eye-source selection, normalized eye state consumed by AVC, and eye-channel staleness policy |
| Future lower-face consumer | Mouth/face routing and composition, independent of eye-source selection |

One `MediaPipeSystem` can retain endpoint-keyed receiver/session records. Each record owns one
socket and the latest validated state for its configured producer, plus subscribers keyed by
component and requested channel. Hand, eye, and future mouth source components share that record;
multiple avatars consume its snapshots without binding additional sockets. Configuration belongs
to the provider/source declarations; components do not each own a copy of the live receiver.
Removing the final subscriber releases its receiver and retained session state. Registration
must remain idle when no MediaPipe consumer is configured.

Keep the common stream envelope separate from channel state. Each channel has its own supported
schema/version, sequence watermark, engine receipt time, validity/capabilities, calibration
generation, and latest sample. Hand traffic cannot refresh eye freshness, or vice versa. A hand
disappearance clears the hand channel while leaving a valid eye channel available. Producer
restart retires the old stream and invalidates every channel from that stream; changed calibration
invalidates the affected normalized samples. Reusing a cached snapshot preserves its original
receipt time and sequence rather than making it appear newly received each tick.

Run a bounded, nonblocking `MediaPipeSystem` receive/update phase before the hand and eye
consumers read samples in `SystemWorld`. If a worker is needed later, it publishes complete
snapshots through a latest-value mailbox; only the engine phase updates retained provider state.
Channel adapters live with the MediaPipe provider and publish the existing semantic contracts:
`HandTrackingFrame` with `HandTrackingProvider::MediaPipe`, and the eye system's normalized
eye-sample contract once its adapter exists. The semantic systems consume those outputs and own
their selection/release decisions. `MediaPipeSystem` does not resolve avatar bones or write poses.

The first implementation supplies hands, using a replay producer before camera inference. Keep
eye/mouth channels explicitly unavailable until their adapters and semantic contracts are
implemented. Sharing this system does not make the existing `MediaPipeEyeTracking` placeholder
functional or require eye inference for hand tracking. Manual bridge launch remains sufficient;
a later explicitly configured process launcher can attach its lifecycle/status to this system.

## Why UDP initially

These are proposed engineering tradeoffs, not an existing protocol:

| Transport | Benefit | Cost / first-slice decision |
| --- | --- | --- |
| Loopback UDP | Portable, easy Python/Rust integration, independent processes, discard old snapshots | Loss/reorder requires sequence and timeout handling; recommended initially |
| Unix datagram socket | Local message boundaries without a port | Platform-specific endpoint/cleanup handling; a possible later local adapter |
| TCP / Unix stream | Reliable framing for configuration and large messages | Must frame records and prevent old poses accumulating; consider for a later control channel |
| Child stdout / pipes | Simple managed-child prototype | Couples launch/logging/lifecycle to ingestion; less convenient for independent restart |
| Shared memory | Avoids serialization overhead | More synchronization and compatibility work than the measured need currently supports |

A socket is the communication endpoint; UDP is one socket transport choice. Keep decoded sample
handling independent of transport so replacing UDP does not rewrite retargeting or source selection.
Existing OSC/HTC eye input uses `std::net::UdpSocket`, providing a local implementation pattern,
but the MediaPipe eye source is currently a placeholder.

Bind an explicitly configured loopback address/port by default. A port conflict reports provider
unavailability without failing engine startup. Remote transport, discovery, encryption, and a
general tracking daemon are deferred. `MediaPipeSystem` shares one receiver per configured
endpoint; multiple avatars consume the same received provider snapshot.

## MediaPipe data and coordinate limits

Hand Landmarker outputs handedness and 21 landmarks in image and world coordinates. World
landmarks are meters with their origin at the hand's geometric center; image landmarks use a
different normalized convention. The Python task needs a model asset, and its live-stream API
accepts timestamps and produces asynchronous results, dropping new input while busy.
[Google's Hand Landmarker guide](https://developers.google.com/edge/mediapipe/solutions/vision/hand_landmarker/python).

Therefore hand world landmarks alone do not establish the wrist's absolute position in the
engine or OpenXR reference space. First support hand-relative finger articulation, optionally
anchored to a controller wrist. Webcam-driven wrist placement requires a separate documented
calibration/placement mode, potentially additional pose/depth input, with its own quality limits.
Do not advertise inferred monocular placement as equivalent to a positioned OpenXR wrist.

The engine adapter must verify camera/world axes experimentally, normalize handedness and mirror
handling, and derive orientation from landmark geometry with explicit uncertainty. A reflected
coordinate transform is not represented by quaternion rotation alone. Calibration specifies
camera-to-engine rotation, any reflection, scale/units, and root placement policy. Camera image
coordinates must never be passed to meter-based retargeting as world positions.

Map landmark semantics explicitly. Wrist is landmark 0; thumb landmarks 1–4 correspond to its
CMC/MCP/IP/tip anatomy; other fingers have MCP/PIP/DIP/tip. Check the thumb mapping against the
target rig. OpenXR has additional palm/metacarpal slots: retain these as absent or explicitly
estimated in the normalized sample. MediaPipe supplies positions, so segment orientations are
derived, not measured joint quaternions. Do not invent per-joint confidence from a handedness score.

## Proposed v1 datagram outline

This is a design outline, not a wire compatibility promise. Freeze field types, byte order,
maximum sizes, and fixtures together before implementation.

Use a small fixed binary header and fixed-shape float32 landmark records to keep both hands in
one datagram. Twenty-one XYZ landmarks per hand take 504 bytes for both hands, leaving room for
metadata within a proposed 1200-byte payload budget. JSON can be a diagnostic export/replay format;
avoid making long JSON snapshots a fragmented live UDP protocol.

Header and per-hand metadata should contain:

- Magic, protocol version, message kind (`hands`), payload length, and landmark schema ID.
- Stream epoch/UUID and increasing sequence; capture timestamp in declared monotonic units.
- Coordinate convention ID, mirror flags, units, and calibration generation.
- A detection bitmask for left/right hands, and optional handedness classification scores with
  their meaning. Records are keyed by anatomical side, not result-list position.
- For each detected hand: fixed 21-position records in declared hand-centered meter space,
  and an explicit validity mask if partial landmarks are supported.

An accepted snapshot with an empty detection mask immediately clears both hands. Omitted hands
in a complete snapshot mean absent, not "reuse previous pose". A provider heartbeat can carry
an empty snapshot; lack of any packet triggers a separate receipt-time timeout.

If eye/face channels are added later, reuse the envelope, stream identity, coordinate metadata,
and bridge lifecycle; give each channel its own payload/version and sequence semantics. Do not
design one giant mandatory packet for all tracking modalities. Eye tracking needs its own real
adapter and normalized gaze contract; the existing placeholder is not made functional by this task.

## MediaPipeSystem receive behavior and timing

1. Decode outside retargeting. Reject wrong magic/version/length/schema, oversized/truncated
   datagrams, nonfinite values, invalid side/masks, and unsupported coordinate conventions.
2. Accept only a configured producer/session. A new stream epoch resets sequence and filters;
   reject delayed packets from retired epochs. First slice uses one producer per endpoint.
3. Drain nonblocking input with a bounded per-tick budget and retain the newest complete snapshot.
   Measure backlog/packet age; use a receiver worker with a latest-value mailbox if tick draining
   cannot stay current. Never replay an accumulated queue of stale poses.
4. Reject duplicate/out-of-order sequences. Use engine monotonic receipt time for timeout;
   producer capture time is useful for ordering but cannot be subtracted from engine time without
   an explicit clock-domain mapping. Clock synchronization and remote latency estimates are deferred.
5. Distinguish bridge available, camera/inference active, hand detected, calibrated root available,
   and fresh articulation. Provider freshness must not imply every capability is valid.
6. On timeout, explicit disappearance, restart, or calibration change, invalidate affected samples
   and filters. Let the hand selector release ownership or select a fresh fallback provider.

Provide counters/status for received, malformed, reordered, dropped/budget-limited, last valid
sequence, receipt age, calibration ID, detected sides, and selected provider. Avoid per-frame
console spam. Payload validation and loss tests must not need a camera or installed MediaPipe.

## Packaging and validation

- [ ] Optional bridge directory contains a pinned/tested Python dependency set, setup/run docs,
      explicit model asset instructions, configurable camera/endpoint, and clear startup errors.
- [ ] A synthetic/replay producer emits the same protocol without importing MediaPipe.
- [ ] Bridge restart and engine restart work independently; receiver never blocks rendering.
- [ ] `SystemWorld.mediapipe` owns shared endpoint/session state; hand/eye consumers subscribe
      without creating competing sockets or duplicating producer lifecycle logic.
- [ ] Removing the final subscriber releases its receiver; unused MediaPipe support remains idle.
- [ ] Engine builds/runs without Python, MediaPipe, camera libraries, or model files installed.
- [ ] Protocol fixtures verify Python encoding and Rust decoding, including malformed packets,
      sequence reorder, retired epochs, loss, empty detections, and unavailable endpoint.
- [ ] Two hands fit the payload budget and measured receive latency/backlog remains acceptable.
- [ ] Left/right stay correct with mirrored preview, unmirrored camera input, and hands crossing.
- [ ] Calibration and capabilities prevent hand-centered landmarks from moving an absolute wrist.
- [ ] OpenXR retains controller/wrist behavior while MediaPipe articulation is selected or absent.
- [ ] A future shared eye/hand process can reuse camera/lifecycle infrastructure without coupling
      the engine's independent channel adapters or making eye inference mandatory.
- [ ] Per-channel receipt times, sequence watermarks, and validity prevent hand traffic from
      refreshing stale eyes, while producer restart invalidates all affected channel snapshots.

Decide the port, exact binary layout, timeout values, supported Python/platform matrix, model
version, and whether managed launch is useful during implementation. Benchmark the simple UDP
slice before adding shared memory or a bidirectional service.
