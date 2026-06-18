# `SystemWorld::tick` Timing Breakdown and Terminal Graph

Date: 2026-06-18

## Context

Startup regressions are hard to localize from the current log stream alone.

We do not need a broad cross-system profiling design for the first pass. We already have a single
high-value choke point:

- `SystemWorld::tick`

That is where the first frame's startup work actually runs through:

- major system ticks
- queue drains into `RxWorld`
- ready event drains
- ready intent drains
- follow-on drain loops when new work is enqueued during processing

That is enough to explain most "it hangs / gets killed during startup" failures.

## Goal

Instrument `SystemWorld::tick` directly, measure the time spent:

- before and after each major tick step
- around each signal-drain phase
- across the whole tick

Then print a compact ANSI-colored horizontal bar graph in the terminal, with a legend, for:

- the first tick by default
- optionally the accumulated total of the first `N` frames when a flag is enabled

## Scope

This task is intentionally narrow.

It is about:

- instrumentation inside `src/engine/ecs/system/system_world.rs`
- timing real execution phases in `SystemWorld::tick`
- reporting a terminal summary after the measured tick window ends

It is not about:

- parsing existing stdout logs
- adding profiling hooks across many independent files first
- a continuous profiler UI
- a general tracing framework

## Default behavior

By default, we only care about the first tick.

That means:

- start timing at the beginning of `SystemWorld::tick`
- accumulate phase timings for tick `0`
- print the summary once that tick finishes

This is the default startup-report mode.

## Optional mode

Add a flag for accumulating the first `N` frames instead of just one.

For example:

- default: `1` frame
- optional debug mode: `N` frames

That lets us answer:

- "is the first tick the problem?"
- "or is startup spread across the first few frames?"

The exact flag shape can be simple:

```rust
StartupTimingConfig {
    enabled: bool,
    frame_count: usize, // default 1
}
```

or equivalent.

## What to measure

The instrumentation should live around the actual steps in `SystemWorld::tick`.

### 1. Whole tick

- total tick wall time

### 2. Per-system sections

Measure each major call as its own phase, for example:

- `clock.tick`
- `transform_stream.tick`
- `transform.tick`
- `skinned_mesh.tick`
- `bvh.tick`
- `camera.tick`
- avatar/IK-related ticks
- `layout.tick`
- `fit_bounds.tick`
- `renderable.tick`
- `text.tick`
- `light.tick`
- `mirror.tick`
- any other major first-tick step that is already explicit in `SystemWorld::tick`

The first pass does not need every tiny helper call. It should measure the explicit top-level
sections already visible in `tick`.

### 3. Queue and RX drain phases

Measure these explicitly, because startup cost may hide there:

- `queue.drain_into_rx`
- `rx.drain_ready_events`
- event execution
- `rx.drain_ready_intents`
- intent execution
- repeated follow-on queue drain / ready-work drain loops

This matters because startup can be dominated by cascading work rather than just the top-level
system calls.

### 4. Post-tick cleanup

If the tick path includes clear end-of-tick work such as decode-completion drains or other
finalization steps, measure those too.

## Reporting format

After the measured window completes, print:

### 1. Horizontal ANSI-colored block bar

- one line
- fixed width, e.g. `40` or `60` cells
- each segment proportional to that phase's share of total time
- use block characters such as `█`

### 2. Legend

One line per measured phase, including:

- color
- stable label
- elapsed milliseconds
- percent of total

### 3. Total

- total measured time
- number of frames included

Example shape:

```text
startup-tick: ████████████████▓▓▓▓▓▓▓▓▒▒▒▒▒▒░░░░██
legend:
  red     queue drain            41.2ms  12.4%
  yellow  event execute          58.7ms  17.7%
  green   gltf/skinned/layout    96.1ms  29.0%
  cyan    editor/panels          74.8ms  22.5%
  blue    render/text/light      47.0ms  14.2%
  white   other                  13.4ms   4.0%
total: 331.2ms over 1 frame
```

The exact labels can be more granular if the output remains readable.

## Phase naming strategy

Prefer labels that correspond directly to the code in `SystemWorld::tick`.

Good:

- `transform.tick`
- `layout.tick`
- `queue.drain_into_rx`
- `rx.drain_ready_events`
- `event execution`

Avoid vague labels in the first implementation if a direct code-site label is available.

## Implementation model

Use `std::time::Instant` around each step inside `SystemWorld::tick`.

A simple accumulator is enough:

```rust
struct TickTimingAccumulator {
    frames_remaining: usize,
    phases: BTreeMap<&'static str, Duration>,
    total: Duration,
}
```

The implementation can:

1. start a timer before each measured section
2. stop it immediately after
3. add the elapsed duration to that section label
4. once the configured frame window is complete, print the graph and disable itself

## Where to hook

Primary file:

- `src/engine/ecs/system/system_world.rs`

Primary function:

- `SystemWorld::tick`

This task should start there, not in scattered subsystems.

## Acceptance criteria

- [ ] `SystemWorld::tick` records timings around its major explicit sections
- [ ] queue/RX drain phases are timed explicitly
- [ ] the default mode reports only the first tick
- [ ] an optional flag allows accumulating the first `N` frames
- [ ] a single ANSI-colored horizontal bar graph is printed after the measured window completes
- [ ] a legend prints per-phase totals and total measured time

## Non-goals

- a full-time always-on profiler
- profiling every helper function in every subsystem
- reconstructing phase timing from existing log text
- a GUI visualization

## Related

- `src/engine/ecs/system/system_world.rs`
