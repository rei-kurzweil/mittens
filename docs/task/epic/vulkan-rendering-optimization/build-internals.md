# Inside Vulkano command-buffer build

Measured 2026-11-06 using an isolated instrumented copy of **Vulkano 0.35.2**.
The purpose is to split the earlier 3.462 ms `AutoCommandBufferBuilder::build()`
measurement, not to change rendering or introduce a dependency fork.

The cafeteria fixture remains 1920 x 1920 **per eye**, bloom/emissive extraction,
MSAA 4x, GTX 1080, 240 warmup frames and 480 sampled stereo frames. The probe
recorded exactly 960 primary builds after `SHIP_PROFILE_BEGIN`; the summary
script checks that each measured frame has exactly two matching builds.

## Results

Mean CPU elapsed milliseconds summed across both eyes. Percentages below use the
**3.311 ms inner timed total**, excluding wrapper/probe-output overhead.

| Phase inside build | Mean ms | p95 ms | Share of inner total |
| --- | ---: | ---: | ---: |
| Synchronization analysis (`add_command` loop) | 1.990 | 2.124 | 60.1% |
| Vulkan command and intermediate barrier emission | 0.748 | 0.831 | 22.6% |
| Finalize barriers and resource-usage tables | 0.385 | 0.438 | 11.6% |
| Retain command closures and release command metadata | 0.176 | 0.199 | 5.3% |
| End native command buffer (`vkEndCommandBuffer` wrapper) | 0.007 | 0.008 | 0.2% |
| Emit final barriers | 0.003 | 0.003 | 0.1% |
| Drop remaining builder state | 0.002 | 0.002 | 0.1% |
| Initialize synchronization state | 0.001 | 0.001 | 0.0% |

The outer application timer measured **3.417 ms** in the instrumented run. The
remaining **0.106 ms** includes wrapper work, diagnostic resource-use counting,
phase bookkeeping and output of two probe records per frame. Do not treat that
remainder as a separately identified Vulkano bottleneck. Emission includes both
Vulkano closure dispatch and native Vulkan/driver calls, not solely driver time.

A matching run of the same temporary build with probe output/timers disabled
measured **3.429 ms** in outer `build()`, versus the original **3.462 ms** capture.
This supports the conclusion that the roughly 3.4 ms cost is real; differences
between these separate runs do not isolate timer overhead. Do not subtract
means to claim a precise logging cost or a performance improvement.

Across the two eyes of every sampled frame:

- **6,256 queued builder commands**, including state/bind/rendering commands.
- **9,214 buffer/image resource-use entries** submitted to synchronization analysis.
- These are not 6,256 draw calls or 9,214 unique resources. The earlier geometry
  instrumentation counts 1,484 draws across both eyes.

## What consumes the time

In `src/command_buffer/auto/builder.rs`, `end_unchecked()` walks every queued
command through `AutoSyncState::add_command()`. That checks buffer/image range
conflicts, records accesses and determines barriers. This loop costs
**1.990 ms, or 60.1% of the inner total** in this capture.

`AutoSyncState::build()` then finishes image layouts/barriers, constructs final
resource-usage ranges and indices, and disposes of intermediate state. That
costs another **0.385 ms**. Combined synchronization analysis/finalization is
**2.376 ms**, about 71.8% of the inner total.

After analysis, Vulkano walks the queued closures to emit native commands and
barriers: **0.748 ms**. Calling the native command-buffer ending wrapper itself
costs only **0.007 ms**. Moving the final end call to another thread would not
address the dominant work.

These are CPU phase measurements, not GPU execution timings. This capture does
not subdivide the 1.990 ms loop into individual conflict searches, hash/range-map
operations and access bookkeeping. Those remain an optional next profiling slice.

## Consequence for command reuse

Reusing a complete compatible command buffer can avoid its repeated analysis
and emission. Reusing a secondary can avoid rerecording its native draws, but
**a cached secondary is not automatically a shortcut around primary analysis**.

Source inspection of Vulkano 0.35.2 shows that
`commands/secondary.rs::execute_commands_locked()` collects the secondary's
`SecondaryCommandBufferResourcesUsage` entries into the primary command's
resource list. `AutoSyncState::add_buffer()` and `add_image()` append those
secondary usage entries as accesses are processed. Repeated uses can therefore
be fed back through primary synchronization analysis even with retained
secondaries. This is an architectural observation, not a measured cached-
secondary performance result.

The first reuse experiment should measure both primary and secondary builds,
command/resource-use counts and total recording time. Compare a small stable
opaque group with an appropriate complete per-slot recording candidate. Retain
explicit dependency generations and in-flight resource ownership. Keep current
synchronization guarantees while deciding the cache boundary.

## Method and controls

1. Tried `perf record -e cycles:u -F 999 --call-graph dwarf,16384 --delay 6000`
   against the cafeteria fixture. It retained 15,507 samples and lost 69,844
   (81.83%). Its top symbol suggested `AutoSyncState::add_command`, but the loss
   and incomplete call chains make it unsuitable for quantitative phase shares.
   Its frame/GPU timings are also excluded from the baseline comparison.
2. Copied registry Vulkano 0.35.2 under `/tmp` without editing the registry copy.
3. Extracted only `end_unchecked()` into a diagnostic include and added coarse
   `Instant` boundaries. No timer is placed around individual commands/accesses.
   The existing tuple-result construction is made explicit to time native end,
   closure collection/metadata release and the remaining builder-state drop.
4. Built with an invocation-local Cargo patch. The build wrapper saved and
   restored `Cargo.lock` in a `finally` block. `Cargo.toml` and `Cargo.lock` are
   unchanged after profiling; the engine source needs no dependency probe hooks.
5. Ran the enabled probe and an otherwise matching disabled control separately.
   Both GPU tests passed. GPU clocks were not locked, so GPU values in their raw
   summaries are context rather than a new GPU optimization comparison.

## Artifacts and reproduction

- [Phase summary, metadata and renderer/control summaries](data/cafeteria-build-internals-2026-11-06.json)
- [480 paired frame phase records](data/cafeteria-build-internals-2026-11-06.frames.json)
- [Prepare isolated dependency probe](../../../../scripts/profiling/prepare_vulkano_build_probe.py)
- [Summarize/reconcile probe records](../../../../scripts/profiling/summarize_vulkano_build_probe.py)

Prepare a fresh temporary copy with the script, supplying the local registry
Vulkano directory and a new target directory:

```sh
python3 scripts/profiling/prepare_vulkano_build_probe.py \
  /path/to/registry/vulkano-0.35.2 /tmp/vulkano-build-probe
```

Build the release test executable using the invocation-local Cargo configuration
`--config 'patch.crates-io.vulkano.path="/tmp/vulkano-build-probe"'`. Save the
existing lockfile first and restore its exact bytes in a `finally` block after
this temporary build; the probe is not intended as a committed dependency patch.
`--message-format=json` gives the resulting `compiler-artifact.executable` path.

Run that test executable with the same ship variables as the
[baseline capture](measurements.md), plus `MITTENS_VULKANO_BUILD_PROBE=1`, and
redirect stdout/stderr to one log. Leave the variable absent for the control.
Then reconcile measured records:

```sh
python3 scripts/profiling/summarize_vulkano_build_probe.py \
  /tmp/capture.log /tmp/capture.frames.json /tmp/build-phases.json
```

The scripts compiled with `py_compile`; the analyzer successfully reconciled all
480 real captured frames. The native dependency probe also compiled and rendered
both test runs successfully. No command caching or synchronization changes are
included in the project by this profiling slice.
