# MMS interpreter: closure and scene cost

Baseline for the evaluator prerequisites in
[`mms-bytecode-vm.md`](../task/mms-bytecode-vm.md): closures capturing by
reference, heap arrays and slot resolution. The same benchmarks run before and
after each change.

## Micro-benchmark: closures vs scope size

`crates/meow-meow-script/tests/closure_call_bench.rs`, hostless. Each cell is
microseconds per loop iteration over 20,000 iterations. Rows set what is
visible when the closure is defined: N number globals, N 64-element arrays or N
top-level functions. Columns:

| Case | Body |
|---|---|
| empty loop | `acc.total = acc.total + i` (loop and table overhead) |
| named fn call | `acc.total = add(acc.total, i)` |
| table fn call | `api.add(i)` |
| closure create | `let f = fn(x) { return x + i }` |

The evaluator currently snapshots every visible binding into each new closure
(`Evaluator::snapshot`) and clones that snapshot on every call. Call and create
columns should grow with the row's scope size, and arrays should cost more than
numbers because they deep-clone. With capture by reference both should be flat.

```sh
cargo test --release -p meow-meow-script --test closure_call_bench -- --ignored --nocapture
```

## Scene benchmark

`src/scripting/mms_scene_bench.rs`, headless (no GPU, external XR frame,
microphone disabled). Default scenes are the three with the most transitively
imported closures and handlers:

| Scene | `fn(` | `on(` | FrameTick | Modules |
|---|---|---|---|---|
| `examples/sakura-twilight-simpler.mms` | 272 | 165 | 15 | 126 |
| `examples/truck-bed-demo.mms` | 255 | 149 | 14 | 102 |
| `examples/interceptor.mms` | 200 | 128 | 11 | 81 |

Per scene it reports wall-clock load time, then after 120 warm-up frames, per
frame over 600 frames:

- `update_ms`: the whole `Universe::update`;
- `callback_invoke`: queued MMS signal callbacks, split into host request time
  and the rest (interpreter time);
- the hottest callbacks by time, labelled with their signal and scope.

Deferred phase callbacks (keyframes, audio and visual phases) go through
`invoke_deferred_callback` and are not in `callback_invoke`; they are in
`update_ms`. No one is playing, so this measures idle scene logic: NPCs,
patrols, smoke, FrameTick handlers. Weapon firing and vehicle driving need
scripted input and are not covered yet.

```sh
cargo test --release --lib mms_scene_bench -- --ignored --nocapture
# options
MMS_BENCH_SCENES=examples/interceptor.mms MMS_BENCH_FRAMES=1200 \
MMS_BENCH_OUTPUT=docs/benchmarks/data/mms-scene-before.json \
  cargo test --release --lib mms_scene_bench -- --ignored --nocapture
```

## Results

Not yet recorded.
