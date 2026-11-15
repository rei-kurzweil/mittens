# Avatar UDP traffic benchmark

This replaces arbitrary eye/session source counts with three avatar profiles.
Run `scripts/benchmark_avatar_udp.sh /tmp/avatar-udp`; optional trailing arguments
such as `--seconds 5 --trials 6` override defaults. Use `BENCH_TEST=1` to also run
profile/codec regression assertions. The runner executes scenarios sequentially,
with excluded warmups and alternating transport order.

Each scenario measures **one VR participant receiving the other participants**.
The observer's own eye sensor feeds its local eye socket at 120 Hz. Other players'
raw eye sensors stay on their machines; their blink weights cross the network.
Ambient eye-look animation remains local, matching the current example's policy.
No MediaPipe/webcam, audio, fingers, weapons, ball, or full-body trackers are included.

| Scenario | Participants | Remote avatars observed | Local eye packets/s | Typed samples/s received | Session packets/s received | Total packets/s received |
| --- | --- | --- | --- | --- | --- | --- |
| `vr2` | 2 VR | 1 VR | 120 | 160 | 20 | 300 |
| `vr4` | 4 VR | 3 VR | 120 | 480 | 60 | 660 |
| `mixed4` | 2 VR + 2 desktop | 1 VR + 2 desktop | 120 | 400 | 60 | 580 |

These are offered rates at scale 1. The three receiving sockets are local HTC
input, session control, and typed channels. Multiple remote avatars share the
same typed socket and use distinct channel IDs. This does not turn the current
two-person MMS example into a four-person demo.

## Avatar data

Deterministic, time-varying trajectories simulate root translation/turning, head
motion, and both hand poses. Each VR avatar transmits eight channels at 20 Hz:
root/head/left-hand/right-hand TRS, two blink weights, and two hand-valid flags.
Each desktop avatar transmits six: root/head TRS, generated blink weights, and
false hand-valid flags. Desktop hand poses and eye-sensor packets are absent.
This desktop profile intentionally models untracked resting hands, rather than
copying the demo's optional scripted hand motion.

TRS uses the demo's sorted typed schema: quaternion `[f32;4]`, scale `[f32;3]`,
translation `[f32;3]`. Blink and validity values are `f32`. Pose samples are 78
bytes; scalar samples are 42 bytes, including the 38-byte channel header.
Thus a VR publisher sends **9,600 bytes/s** of typed samples to each observer;
a desktop publisher sends **6,480 bytes/s**. Received typed traffic is respectively
9,600 / 28,800 / 22,560 bytes/s in the three scenarios. UDP/IP, Tailscale, and link
headers are additional. These are per-recipient figures, not whole-session totals.

The local sensor uses the production 84-byte HTC packet at 120 Hz, with gaze
quaternions, eye openness, and valid position fields. The benchmark's packet ID
occupies a finite position float's mantissa, without adding a wire envelope.
Session packets are representative v2 JSON heartbeat/roster traffic at 20 Hz per
remote participant; roster sizes match the scenario. Avatar poses use binary
channels, never the legacy JSON pose snapshot path.

## What is timed

The harness copies the production typed codec/schema, session protocol, HTC/OSC
decoders, quaternion math, and `UdpSystem` into a small release build. Input packet
encoding happens before measurement; the loopback producer runs on another thread.

- `separate`: one scratch receive buffer per socket.
- `shared`: one scratch buffer reused by the same raw polling loop.
- `runtime`: the actual shared `UdpSystem`, including its normal lease/poll path.

All modes receive/decode/checksum identical packets, using equal socket counts,
rotating polling order, 256-packet/socket budgets, and 90 Hz polling. Native typed
payload decoding uses caller-owned numeric storage. The comparison includes socket
polling, parsing, and benchmark latency/checksum bookkeeping. It excludes channel
registration/ACK/retry state machines, subscription lookups, MMS dispatch,
transform/morph application, IK, asset loading, rendering, and sender encoding.
It is a **steady-state transport benchmark**, not an engine frame-time test.

`cpu_ms` is receiver-thread CPU **summed over the entire one-second traffic run
plus 100 ms drain**, roughly 99 polling updates. It excludes sleeping and producer
CPU. Latency measures actual send-to-decode delay, including polling/scheduling;
it is not motion-to-photon latency. Raw CSV reports typed counts separately in
`avatar_received`; the historical `pairs` column denotes total participants for
avatar scenarios, and `scenario` identifies the profile.

Profile assertions check packet counts, typed byte totals, production-codec
round trips, changing locomotion, false desktop hand validity, and absence of
desktop hand channels. UDP trials count drops, send errors, invalid samples,
duplicates, and semantic checksums. Lossy trials never qualify as CPU speedups.

## Recorded run

[Raw data and summaries](data/avatar-udp-2026-11-14/) use one-second traffic,
four measured trials per mode, and 500 replay rounds. As with the earlier
experiment, this interactive workstation was not reserved for a controlled quiet
window. Treat timing differences as exploratory; do not infer a speedup from
small differences. Hardware networking, encoding cost, binding/IK cost, and
four-avatar rendering need separate measurements.

| Scenario | Actual UdpSystem receiver CPU, median / whole 1.1 s run |
| --- | --- |
| 2 VR | 1.554 ms |
| 4 VR | 2.010 ms |
| 2 VR + 2 desktop | 1.954 ms |

All 36 measured UDP trials were loss-free with no send errors, invalid packets,
or duplicates. Separate/shared/runtime CPU ranges overlap. The medians above
correspond to approximately 16–20 microseconds per polling update, not milliseconds
per engine frame. The separately owned scratch buffers total 132,096 bytes for
these three sockets; the shared/runtime buffer totals 65,536 bytes.
