# Shared native UDP transport

Status: implemented and verified by focused regressions and native measurements. Keep protocol bytes and public MMS API stable.

UdpSystem owns common bind/send/bounded receive/error handling, one lazily
allocated receive scratch buffer, leased endpoint registration and diagnostics.
SystemWorld retains one instance shared by typed channels, multiplayer and eye
receivers. Installed UDP terminal HID uses that same instance. Standalone tools
can own an independent instance of the same implementation.

Protocol consumers retain codecs, peer/sequence/ACK policy, source selection,
semantic coalescing and scene writes. Endpoint leases release sockets on final
drop; the diagnostic registry holds weak references, not live socket owners.
No datagram queues/copies or allocations are introduced in the raw receive path.
Poll callbacks borrow scratch data only for the duration of the call. Replies
use a socket handle without acquiring the scratch lock. Scratch contention or
nested polling defers work nonblockingly; no worker thread per socket.

Bound work per endpoint; preserve terminal limits (64 receive / 16 ACK packets)
and channel/session limits (256). Eye drains become bounded too. Detect oversized
datagrams before protocol dispatch, preserve IPv4/IPv6 and external OSC/HTC/terminal
formats, and leave protocol-specific WouldBlock/send policy in adapters.

Large endpoint.rs, xr_eye_tracking_system.rs, SystemWorld update/registration
files remain scoped adapters. Existing endpoint/registry boundary proposals apply;
future splits should group protocol state machines or eye provider adapters, not
individual operations. Retain direct numeric paths and generic inlined callbacks,
no per-draw allocations, map scans per packet, or dynamic dispatch for separation.

Validate shared-instance wiring, lease cleanup/port reuse, busy/recursive polls,
budgets/fairness, oversized packets, counters, callback ACKs, channel epochs and
schemas, multiplayer lifecycle, OSC/HTC source selection and terminal retry/ACK.
Run focused scene and transport tests plus optimized transport measurements;
record that the initial buffer-only experiment is not the runtime implementation.

## Public boundary

The native service is engine::udp::UdpSystem (also re-exported with ECS systems).
No new MMS component or signal is introduced. Scene adapters receive the shared
owner explicitly at registration/update points. Existing standalone constructors
remain, alongside bind_on/new_on context-taking variants. Diagnostic snapshots
allocate only on inspection; the poll hot path uses direct endpoint leases and
one scratch try-lock per poll, without endpoint registry lookup.

## Verification

25 focused UDP/channel/terminal checks, 13 multiplayer checks, 23 eye-tracking
checks, and 3 shared-grab checks pass; the isolated optimized codec check and
shell-client compile check pass. New coverage includes real OSC/HTC packets,
scene-wide owner sharing, callback ACKs, nonblocking concurrent/nested polls,
endpoint fairness/budgets, IPv6 where available, oversize rejection and lease
cleanup/port reuse. Physical XR/Tailscale validation remains manual.

The real-owner benchmark has 42 loss-free loopback trials, with similar latency
and overlapping CPU timing ranges. No universal speedup is claimed. See
[measurement report](../benchmarks/udp-transport-locality.md) for raw data, workload
counts and whole-run CPU units.

A later user report of workspace interaction during benchmarking further limits
timing comparisons. Functional regressions remain valid; repeat performance
measurements with a quiet host before interpreting small differences.
