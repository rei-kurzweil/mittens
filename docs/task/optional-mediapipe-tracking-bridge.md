# Optional MediaPipe tracking process and transport

Date: 2026-10-09

Status: proposed architecture and protocol outline; no bridge or receiver implemented.

Release target: [mittens-engine 0.10.0](epic/0.10.0/README.md), following the
normalized hand sample/selection foundation. The bridge remains optional.

Depends on the normalized sample/selection contract in
[shared hand tracking](hand-tracking-system-and-finger-retargeting.md).

## Recommendation

Run MediaPipe in a separately installed Python process, with a dedicated virtual environment,
camera capture dependencies, and explicitly installed model assets. Mittens receives compact
tracking snapshots through a provider adapter. Building and starting Mittens, or using OpenXR,
must require none of those dependencies. Keep camera inference outside the render/tick thread.

Start with one-way nonblocking UDP over loopback and full snapshots. Manual launch is sufficient
for the first slice; a later opt-in launcher can manage an explicitly configured interpreter and
report install/model/camera failures. Installation and model download are separate user actions,
not engine startup behavior. Keep the engine/bridge protocol independently versioned.

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
general tracking daemon are deferred. Separate receiver instances must not silently compete for
the same endpoint; multiple avatars should share one received provider snapshot.

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

## Receiver behavior and timing

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
- [ ] Engine builds/runs without Python, MediaPipe, camera libraries, or model files installed.
- [ ] Protocol fixtures verify Python encoding and Rust decoding, including malformed packets,
      sequence reorder, retired epochs, loss, empty detections, and unavailable endpoint.
- [ ] Two hands fit the payload budget and measured receive latency/backlog remains acceptable.
- [ ] Left/right stay correct with mirrored preview, unmirrored camera input, and hands crossing.
- [ ] Calibration and capabilities prevent hand-centered landmarks from moving an absolute wrist.
- [ ] OpenXR retains controller/wrist behavior while MediaPipe articulation is selected or absent.
- [ ] A future shared eye/hand process can reuse camera/lifecycle infrastructure without coupling
      the engine's independent channel adapters or making eye inference mandatory.

Decide the port, exact binary layout, timeout values, supported Python/platform matrix, model
version, and whether managed launch is useful during implementation. Benchmark the simple UDP
slice before adding shared memory or a bidirectional service.
