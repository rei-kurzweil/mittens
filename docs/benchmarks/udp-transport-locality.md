# UDP shared receive-buffer experiment

The original buffer-only experiment is retained below. The harness now also
supports `--mode runtime` for the actual shared native `UdpSystem`, or `--mode all`
for all three variants. Runtime mode measures loopback only (no fabricated
in-memory equivalent), including scratch synchronization, limits and counters.
The production migration is described in [the transport task](../task/shared-udp-system.md).

Original experiment:
It tests whether consolidating receive scratch buffers helps when OSC eye traffic
and multiplayer session traffic arrive together. A shared transport's other
possible lifecycle/diagnostic benefits are separate from a measured speedup.

## What is compared

- **Separate:** one 1,024-byte scratch buffer per eye socket, and one 65,536-byte
  buffer per session socket, modeling current receiver buffer sizes/ownership.
- **Shared:** the same sockets and endpoint ordering, but one 65,536-byte scratch
  buffer reused sequentially by the common poller.
- Both use the same bounded polling implementation, socket syscalls, real OSC
  decoder, real multiplayer JSON decoder, and result handling. The single changed
  design variable is scratch-buffer ownership. This isolates a data-locality
  hypothesis; it deliberately does not compare differently optimized decoders.

The separate mode models current buffer ownership; it does **not** instantiate
XREyeTrackingSystem or MultiplayerSystem. Both modes use a common polling loop,
so this does not measure instruction-cache improvements from deduplicating the
existing polling implementations. Session handshake/roster state, typed-channel
registration, ECS/render work, OSC source selection and audio are outside scope.
It is not an end-to-end engine benchmark or a full shared-transport prototype.

## Workload and measurements

Each simulated source pair has two receiving loopback sockets: 120 OSC eye packets
and 20 session packets per second, polled at 90 Hz. Test 1, 8 and 32 pairs. Each
poll has the same 256-datagram budget per endpoint, with rotating first endpoint.
Default session traffic mixes heartbeats and roster snapshots; `--poses` adds
player/entity transforms to model the older automatic replication workload.
`--rate-scale 16` increases both offered rates without changing their ratio.

The build script extracts the current self-contained production OSC decoder and
copies the current multiplayer protocol module into a small optimized temporary
crate. It uses workspace-locked cached dependencies; no Vulkan build or changes
to the runtime source are needed. If the decoder source boundary moves, update
the extraction rather than maintaining an unverified second decoder.

Packets are encoded once before measurement and are identical between modes.
A valid OSC float's mantissa carries a synthetic sample identity; session packets
use their existing sequence field. Shared monotonic send timestamps live in a
benchmark-side atomic array, not a new wire header. This is synthetic compatible
wire traffic, not playback of a physical eye tracker.

1. **In-memory replay:** copy and decode the same interleaved packet trace into
   separate/shared buffers. Measures wall nanoseconds per packet without sockets
   or sender scheduling. Includes decoding, not just a memcpy microbenchmark.
2. **Paced loopback:** a producer thread sends that trace while the receiver polls
   at the configured frequency. Measures receiver-thread CPU time (excluding sleep
   and sender-thread CPU), received counts, send errors, drops and decode errors,
   plus send-to-decoded-handler p50/p95/p99 and per-protocol p95 latency.
3. **Optional perf counters:** cycles, instructions, cache references and misses.
   Replay-only runs avoid network scheduling, but counters still include trace
   preparation, warmups and decoder work. Generic cache events do not establish
   which cache level caused an effect; use platform-specific events as needed.

There is one excluded warmup per mode and each measured replay also warms its
trace/buffers. Trials alternate AB/BA order. Scratch allocation, socket creation,
thread startup and trace construction are outside timing. Per-kind latency vectors
are preallocated; percentile sorting is outside the CPU measurement. Kernel
receive buffers/socket count are unchanged by scratch sharing.

The producer excludes serialization cost by sending pre-encoded packets. This
measures receiver handling; it is not total application CPU. The sender runs on
the same machine, so its scheduling/cache activity can still affect the receiver.

Semantic checksums must match when all packets arrive. Reduced received work
must never count as a performance win: lossy/error trials remain in CSV but are
excluded from CPU speedup summaries. The UDP rate is offered load, not a peak
throughput measurement. Report counts and loss alongside timing.

## Run

From the repository root, with dependencies already cached:

```sh
scripts/benchmark_udp_transport.sh --pairs 1,8,32 --seconds 1 --trials 7 --rounds 1000 > /tmp/udp-normal.csv
scripts/summarize_udp_transport.py /tmp/udp-normal.csv

scripts/benchmark_udp_transport.sh --pairs 8 --seconds 1 --trials 5 --rounds 100 --rate-scale 16 > /tmp/udp-stress.csv
scripts/summarize_udp_transport.py /tmp/udp-stress.csv
```

Linux is required for receiver-thread CPU timing. UDP socket access is required;
a restricted sandbox can prevent loopback binding. No external network is used.

For cache counters, compare modes separately with equal workload. After building,
run the binary directly so compiler/build work is not counted:

```sh
perf stat -e cycles,instructions,cache-references,cache-misses -- \
  target/udp-transport-bench/release/mittens-udp-transport-bench \
  --replay-only --mode separate --pairs 1,8,32 --trials 7 --rounds 5000
perf stat -e cycles,instructions,cache-references,cache-misses -- \
  target/udp-transport-bench/release/mittens-udp-transport-bench \
  --replay-only --mode shared --pairs 1,8,32 --trials 7 --rounds 5000
```

`PERF=1 scripts/benchmark_udp_transport.sh ...` also builds first and then wraps
only the resulting binary in perf. Counter access depends on host permissions.
CPU affinity, frequency governor, other engine processes and background load
matter; record them and repeat on XR hardware before a runtime decision.

## Interpretation

A shared 64 KiB scratch allocation is smaller than many independent allocations,
but each receive only touches the packet-sized prefix. Separate buffers may
already fit comfortably in cache. JSON decoding, socket syscalls and frame polling
can dominate the difference. Allocated bytes are not measured cache working set.

Look for repeatable paired timing differences larger than trial spread, with
equal packet counts, checksums and latency. If results overlap, do not claim a
cache-locality win. Even a positive result here needs a runtime prototype and
representative endpoint/session state before replacing existing implementations.
External OSC/HTC/terminal wire compatibility must remain intact.

Code: [harness](../../benchmarks/udp_transport/main.rs),
[experimental poller](../../benchmarks/udp_transport/transport.rs),
[packet workload](../../benchmarks/udp_transport/workload.rs),
[build/run script](../../scripts/benchmark_udp_transport.sh),
[CSV summary](../../scripts/summarize_udp_transport.py).

## Recorded result: 2026-11-14

Intel i5-10400, Linux x86_64, Rust 1.98.1 optimized builds, powersave governor,
no CPU pinning. Other process activity was not isolated. Seven alternating trials
per mode at normal rates; five per mode at 16× rates.

| Pairs | Mode | Replay ns/packet, median [min–max] | Receiver CPU ms, median [min–max] | UDP p95 µs, median | Drops / send errors | Scratch bytes |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | separate | 117.743 [115.928–137.317] | 1.275 [1.131–1.349] | 11123.1 | 0 / 0 | 66560 |
| 1 | shared | 117.312 [113.333–125.354] | 1.378 [1.300–1.489] | 11122.2 | 0 / 0 | 65536 |
| 8 | separate | 113.671 [111.850–124.205] | 3.005 [2.779–3.173] | 11115.5 | 0 / 0 | 532480 |
| 8 | shared | 112.894 [111.851–123.224] | 3.052 [2.968–3.261] | 11115.6 | 0 / 0 | 65536 |
| 32 | separate | 114.380 [112.003–116.448] | 8.114 [7.919–8.252] | 11106.6 | 0 / 0 | 2129920 |
| 32 | shared | 111.314 [110.692–111.771] | 7.839 [7.313–8.479] | 11092.1 | 0 / 0 | 65536 |

Normal offered rates were 140, 1,120 and 4,480 packets/s respectively. All 42
normal UDP trials were loss-free with matching semantic checksums. The 16×
eight-pair run offered 17,920 packets/s; all ten trials were also loss-free.
Its median receiver CPU was 13.474 ms separate versus 13.348 ms shared, with
overlapping ranges; replay was 114.022 versus 114.776 ns/packet.

Hardware-counter comparison used the same longer replay workload, twice per
mode, reversing whole-run order on the second comparison:

| Comparison | Separate cache misses | Shared cache misses | Miss reduction | Separate elapsed s | Shared elapsed s | Time reduction |
| --- | --- | --- | --- | --- | --- | --- |
| Separate then shared | 31,992,592 | 26,241,595 | 17.98% | 22.455 | 22.315 | 0.62% |
| Shared then separate | 34,325,572 | 28,264,278 | 17.66% | 22.632 | 22.431 | 0.89% |

For this modeled buffer layout, the counter reduction is repeatable, but elapsed
time improves by less than 1% in those longer runs. At the actual one-pair load,
we do not establish a receiver-CPU or latency benefit. The first normal 32-pair
replay showed about 2.7% improvement; the longer counter runs showed a smaller
difference. Do not generalize one favorable high-source trial to the real demo.

Conclusion: shared scratch storage reduces allocation footprint and can reduce
cache misses here, but this experiment does not demonstrate a material speedup
for the current two-source case. A broader transport consolidation should first
justify its lifecycle/maintenance benefits and then measure real engine endpoint
state and frame time on desktop/XR. Decoder/syscall/poll scheduling costs may be
more consequential. No runtime transport was migrated by this benchmark.

Raw data: [normal CSV](data/udp-transport-normal.csv),
[stress CSV](data/udp-transport-stress.csv),
[environment](data/udp-transport-environment.json),
[first separate counters](data/udp-transport-perf-separate.txt),
[first shared counters](data/udp-transport-perf-shared.txt),
[reverse separate counters](data/udp-transport-perf-separate-reverse.txt),
[reverse shared counters](data/udp-transport-perf-shared-reverse.txt).
The perf replay-only summaries show no UDP timings because that phase was skipped;
their `NaN` CPU medians/zero loss-free UDP-trial counts are not packet-loss results.

## Actual runtime comparison

Use `scripts/benchmark_udp_transport.sh --mode all --pairs 1,8 --seconds 1
--trials 7 --rounds 1000` (on one shell line). The runtime row uses the actual
`src/engine/udp/mod.rs`, copied into the small optimized harness. It keeps the same
wire decoders, source rates, sockets, polling order/budgets, and checksum/latency
accounting. It measures receiver transport/decoding, not full ECS frame time.
Runtime rows have no replay measurement; compare their loss-free loopback CPU and
latency to the separate/shared-buffer baselines. The old raw data remain unchanged.

### Runtime result after migration

Same desktop/governor and settings as above, with `--mode all --pairs 1,8
--seconds 1 --trials 7 --rounds 1000`. Seven trials per mode, 42 loopback trials
total. No drops, invalid packets, duplicate packets or send errors.

| Pairs | Mode | Replay ns/packet, median [min–max] | Receiver CPU ms, median [min–max] | UDP p95 µs, median | Drops / send errors | Scratch bytes |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | runtime | — | 1.362 [1.294–1.573] | 11119.6 | 0 / 0 | 65536 |
| 1 | separate | 113.769 [112.111–131.644] | 1.447 [1.292–1.575] | 11136.8 | 0 / 0 | 66560 |
| 1 | shared | 114.003 [111.681–114.862] | 1.395 [1.269–1.593] | 11124.7 | 0 / 0 | 65536 |
| 8 | runtime | — | 3.188 [3.046–3.266] | 11118.7 | 0 / 0 | 65536 |
| 8 | separate | 113.704 [110.352–115.907] | 3.034 [2.707–3.161] | 11117.6 | 0 / 0 | 532480 |
| 8 | shared | 114.188 [110.766–120.925] | 3.016 [2.806–3.090] | 11113.5 | 0 / 0 | 65536 |

`runtime` includes the actual UdpSystem endpoint leases, shared-scratch try-lock,
payload limits and counters. CPU figures are milliseconds on the receiver thread
for a 1 s offered workload plus a 100 ms drain interval, not whole engine/frame
time. Lower is faster. At eight pairs the runtime median is about 0.15 ms higher
than separate; at one pair it is about 0.09 ms lower. Trial ranges overlap and
latency is essentially unchanged, so this is not evidence of a general speedup.
The consolidation's primary justification is maintainability and consistent
nonblocking I/O/lifecycle behavior.

[Runtime raw CSV](data/udp-system-runtime.csv),
[run summaries](data/udp-system-runtime.txt). Hardware/XR and full frame-time
validation remain separate acceptance work.

### Units and source counts

A source pair means one eye stream plus one session stream, on two receiving
sockets. It is unrelated to ID/payload fields. One pair offers 140 packets/s;
eight offer 1,120 packets/s. Always compare separate/shared/runtime at the same
source count; eight sources naturally require more work than one.

The runtime's approximately 3.19 ms figure at eight pairs is receiver-thread CPU
**summed over the whole 1.1 s run**, including the drain interval. At 90 Hz that
is about 99 polling updates, or 0.032 ms (32 microseconds) CPU per update on
average. It is not 3.19 ms per frame or per packet. At one pair, 1.36 ms total
is approximately 14 microseconds per update. These are not full engine frames,
and the benchmark's own timestamp/latency accounting is included.

### Interactive-use caveat

Rei reported briefly using the computer/switching workspaces during testing.
The exact affected trials are unknown. Background interaction can change cache
activity, scheduling, and frequency even though the CPU timer excludes sleeping
time. Treat every timing/counter result here as exploratory, not a controlled
faster/slower conclusion. The observed small differences need a quiet-window
rerun before guiding performance decisions. Functional checks and recorded
packet/checksum accounting are not invalidated by that interaction.
