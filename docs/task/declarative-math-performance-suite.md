# Task: Declarative math performance and correctness suite

Date: 2026-10-17
Status: initial suite implemented; production math components remain proposed.
The native graph and flat-plan backends below are benchmark-only prototypes.

## Purpose

Establish reproducible MMS numerical baselines before implementing
[declarative vector operator components](../draft/declarative-vector-operator-components.md).
Compare equivalent scalar/Vec3/quaternion calculations, including interpreter
execution and dynamic array costs, against direct Rust and two native execution
prototypes. Add the real engine backend to these same fixtures when it exists.
The prototypes do not implement or register the proposed MMS components.

The suite is `tests/declarative_math_profile.rs`, with native/MMS support under
`tests/support/declarative_math/`. It uses the actual retained
`meow_meow_script::Runtime::standard()` session/callback evaluator, rather than
an emulated interpreter or a fresh parse on every update. No new dependencies
or changes to production evaluation are required.

## Backends and equivalent work

| Backend | What is measured |
| --- | --- |
| `rust_direct_reference` | Handwritten math over native fixed arrays; optimized reference/lower-bound comparison |
| `rust_graph_prototype` | Indexed native nodes, recursive evaluation with a reusable per-update memo; shared nodes evaluate once |
| `rust_flat_plan_prototype` | The same authored prototype graph lowered into fixed native operations with typed slot indices and reusable buffers |
| `mms_scalar_locals` | Ordinary float expressions/statements, with scalar locals for intermediate coordinates |
| `mms_arrays` | Ordinary MMS arrays, indexing, and replacement intermediate arrays |
| `mms_array_helpers` | Ordinary retained MMS helper functions accepting/returning arrays |
| Production MathGraph | Unavailable; explicitly reported as such, never substituted with a prototype |

MMS Number is f64. All current native measurements deliberately use f64 as well.
Native Vec3/quaternion payloads are inline `[f64; 3]` / `[f64; 4]`; MMS arrays
are dynamic `Vec<Value>`. A future f32 engine adapter needs a separately labelled
precision/tolerance comparison and, ideally, an f32 direct reference. Do not
attribute differences in precision alone to the execution representation.

Current kernels:

- A sixteen-stage scalar multiply/add/clamp chain.
- A sixteen-stage vector multiply/add chain.
- Unit-quaternion vector rotation, with xyzw order.
- Ordered quaternion composition.
- Body-position formula: head position minus root-to-neck offset minus rotated
  rest neck segment. This is geometry, not the full AVC constraint/closure solve.
- A shared rotated vector consumed twice, so graph reuse can be checked.

MMS chain stages are unrolled like the native graph. The script loop batches
subjects, rather than adding an extra interpreter loop around every math stage.
The scalar-only kernel runs only the scalar MMS variant; there is no artificial
array variant doing unrelated work.

Current MMS numeric index assignment does not support in-place array writes.
The array baseline therefore constructs/replaces arrays; the suite does not
silently add array mutation to MMS. The scalar-local variant provides a baseline
that avoids intermediate vector arrays, and helper functions measure a common
modular script style. If array mutation is later implemented, add a reuse variant
alongside these baselines instead of changing the historical workload unnoticed.

## Correctness before timing

Ordinary, non-ignored tests compare each backend against the reference across
changing inputs and multi-subject batches. Each individual result is checked,
not only a checksum. Cases include identity/half-turn rotations, zero vectors,
positive/negative clamp saturation, noncommuting quaternion composition, and
general unit quaternions. Known rotation/composition answers independently check
the shared native formulas.

The shared-value fixture verifies one graph evaluation per reachable node.
An allocation-counting test verifies warmed graph/flat evaluation allocates
nothing. This is a property of the prototypes, not evidence about a future
engine integration. Sources, values, and outputs must remain live across updates.

The operand-builder fixture at
`tests/fixtures/declarative_math/operand-builders.mms` verifies current MMS can
parse/materialize nested component expressions as named builder arguments rather
than as children. This does not register their proposed engine types or establish
live shared-node identity; graph ownership/binding still needs implementation.
The fixture omits `.new()` and empty bodies, and puts chained calls on lines
after the component type, as required by the proposal. Today's materializer
still classifies its first operand call as a constructor; default construction
followed by a first operand builder needs implementation and a corresponding
semantic assertion when these APIs land.

## Timing and memory boundaries

The opt-in release profile emits machine-readable JSON records prefixed
`MATH_PROFILE`, with raw batch durations, median time per update/subject,
p95 of batch-mean update times, allocation/reallocation calls, and gross
requested bytes. These bytes are not retained/live/peak memory. Allocation
counting is thread-local, so allocations from another thread are excluded.

Measure phases separately:

1. Native graph construction / flat-plan preparation. Flat construction
   currently includes creating its source graph and lowering it; it is not an
   engine binding/rebuild benchmark.
2. MMS session creation, parse, callback/closure construction and registration.
   Runtime catalog creation and generating the fixture's source text are outside
   this measurement; the same Runtime is reused.
3. MMS input marshalling from native fixtures to owned argument arrays.
4. Warm steady updates using retained sessions/plans. MMS timing includes its
   real callback invocation, captured environment/body/value cloning, lexical
   work, array operations, argument destruction, and result extraction. It does
   not include reparsing or preparing invocation arguments. Native timing
   includes input snapshot copies and reading/checksumming outputs.

Use varied precomputed inputs and black-box reads/results. Both native prototypes
evaluate the same nodes; their construction/constant preparation is outside
steady timing. The graph memoizes shared results once per update. Direct Rust
may benefit from compiler optimizations not applied by either prototype.

Default subject counts are 1, 16, and 64, with changing input sequences, warmup,
and repeated samples. Native batches run more iterations for timer resolution;
records report the actual count and normalize per update. p95 is a percentile
of batch means, not a per-frame tail-latency claim. The current pure-kernel suite
reuses a scratch evaluator across independent subjects; it does not measure
per-avatar temporal buffers, physics, rendering, ECS source discovery, or contact.

There are no wall-clock pass/fail thresholds. Record machine, build flags,
Rust version, CPU/power configuration and competing load when comparing runs.
Counting instrumentation adds overhead, especially to allocation-heavy code;
treat these as instrumented measurements, not uninstrumented frame budgets.

## Commands

Correctness and zero-allocation checks:

```sh
cargo test --test declarative_math_profile
```

Release timing profile, run alone:

```sh
cargo test --release --test declarative_math_profile declarative_math_performance_profile -- --ignored --exact --nocapture --test-threads=1
```

Options:

- `MITTENS_MATH_PROFILE_UPDATES`: MMS updates per sample, default 16.
- `MITTENS_MATH_PROFILE_SAMPLES`: sample count, default 7.
- `MITTENS_MATH_PROFILE_SUBJECTS`: comma-separated subject counts, default 1,16,64.
- `MITTENS_MATH_PROFILE_DUMP_MMS`: optional directory for exact generated MMS
  baselines, suitable for inspection and reuse.
- `MITTENS_MATH_PROFILE_REQUIRE_PRODUCTION`: setting this currently fails with
  an explicit missing-backend error. Use it to prevent prototype-only runs from
  being accepted as production verification.

## Production adapter and remaining slices

The `NativeBackend` evaluation boundary and shared fixtures are the starting
adapter seam. Once real components exist, add a separately named production
backend that constructs the real graph/plan, updates bound input slots, evaluates
through its public native API, and reads accepted outputs. Keep the direct and
prototype baselines. Validate all output components before timing it.

Use the [MMS head/neck specification example](../draft/avc-head-neck-pose-driver-example.md)
and its [source](../draft/examples/avc-head-neck-pose-driver.mms) as a future
integration fixture for nested exports, source authority, bounded geometric
closure and final-anchor acceptance. The documented expected cases are not yet
executed by this suite; the existing body-offset kernel covers only the immediate
placement formula.

Then extend the suite with:

1. Real MMS component construction versus the native builder, lowering,
   source binding, rebind, and invalidation/rebuild measurements.
2. Connected multiple outputs, graph-local sharing versus duplicated expressions,
   multiple graphs/independent per-avatar state, and nontrivial depth/fan-out.
   Include generic `VMNOutput.value` exports (Scalar/Vec3/Quaternion), named
   node-local input bindings and reuse of the same port by numeric and transform
   consumers. Check that pure value exports do not apply scene transforms and
   that missing/mismatched input bindings fail validation.
3. PlanarFollow propose/accept, repeated propagation within an update, rejection
   without state commit, and reset/reacquisition. Pure-kernel speed says nothing
   about correct temporal scheduling.
4. Invalid ports, division/sqrt domains, non-finite/zero quaternions, source loss,
   and output rejection/readiness. Current numerical workloads use valid inputs.
5. Real transform outputs/parent conversion/propagation, existing TOP interaction,
   and source-to-consumer refresh separately from core numerical execution.
   Cover implicit inheritance for one output bundle (including translation plus
   rotation), explicit selection for several bundles, ambiguous-content rejection,
   channel type/role validation, and reuse of exported channel ports as numeric
   inputs. Verify unspecified channels, world/local behavior, downstream local
   TRS exactly once, and atomic output/state acceptance. Multiple consumers must
   reuse one graph evaluation. These are future production integration checks,
   not assertions established by the current prototype suite.
6. f32 precision, constants versus changing inputs, and optimizations such as
   constant folding/fusion only after a measured equivalent baseline exists.

Coordinate source lifetime tests with
[retained transform-operator sources](transform-operator-source-bindings-and-invalidation.md).
Default ancestor sampling stays outside generalized graph scheduling.

## Initial verification, 2026-10-17

Four ordinary correctness/ownership-shape/buffer-reuse tests pass in debug and
release. The opt-in release profile also passes; it runs the numeric oracle
before timing even when selected on its own. Formatting and whitespace checks
pass. No production MathGraph implementation was benchmarked.

One instrumented Linux x86_64 run used rustc 1.98.1, f64, seven samples, sixteen
MMS updates per sample, and subject counts 1/16/64. For body-offset math at
sixteen subjects per batch:

| Backend | Median microseconds per subject | Mean allocation/reallocation calls per subject |
| --- | ---: | ---: |
| Direct Rust reference | 0.023 | 0 |
| Native graph prototype | 0.072 | 0 |
| Native flat-plan prototype | 0.030 | 0 |
| MMS scalar locals | 10.921 | 53.8 |
| MMS arrays | 16.446 | 107.7 |
| MMS array helpers | 44.852 | 627.5 |

These compare complete instrumented retained-callback work against pure native
kernel execution, with the boundaries described above. They are initial
prototype observations, not production speedup/frame-rate promises or stable
performance thresholds. CPU/power configuration was not controlled in this run.
The session's raw log/JSONL and exact generated scripts are under
`/tmp/mittens-declarative-math-profile.log`,
`/tmp/mittens-declarative-math-profile.jsonl`, and
`/tmp/mittens-math-profile-mms/`; these temporary artifacts are not durable
repository baselines. Rerun the suite for a fresh comparison.

## Related work

- [Vector operator components](../draft/declarative-vector-operator-components.md)
- [Declarative transform math](../draft/declarative-transform-math.md)
- [AVC head/neck constraint](xr-head-neck-bend-and-body-offset.md)
