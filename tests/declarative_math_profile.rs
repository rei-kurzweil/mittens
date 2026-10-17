//! Numeric oracle and performance suite for the proposed declarative math.
//! Native graph/flat backends here are benchmark prototypes, not engine APIs.
//! Run the ignored profile alone in release mode; see the linked task document.

#[path = "support/declarative_math/mod.rs"]
mod math;

use math::mms::{self, MmsKernel, STYLES, Style};
use math::native::{self, Direct, FlatPlan, Graph, Input, KERNELS, Kernel, NativeBackend};
use math::{BatchOutput, native_batch};
use meow_meow_script::Runtime;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    allocations: u64,
    bytes: u64,
}
thread_local! {
    // Thread-local instrumentation keeps unrelated test/runtime allocations out
    // of the measured thread. Gross requested bytes, not peak/live memory.
    static COUNTS: Cell<Option<Counts>> = const { Cell::new(None) };
}
fn record_allocation(size: usize) {
    let _ = COUNTS.try_with(|cell| {
        if let Some(mut counts) = cell.get() {
            counts.allocations += 1;
            counts.bytes += size as u64;
            cell.set(Some(counts));
        }
    });
}
struct CountingAllocator;
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation(size);
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy, Debug)]
struct Measurement {
    ns: u128,
    counts: Counts,
}
fn measure<T>(run: impl FnOnce() -> T) -> (T, Measurement) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COUNTS.with(|counts| counts.set(None));
        }
    }
    COUNTS.with(|counts| {
        assert!(counts.get().is_none(), "nested allocation measurement");
        counts.set(Some(Counts::default()));
    });
    let reset = Reset;
    let start = Instant::now();
    let value = run();
    let ns = start.elapsed().as_nanos();
    let counts = COUNTS.with(|counts| counts.get().unwrap());
    drop(reset);
    (value, Measurement { ns, counts })
}

fn assert_close(actual: BatchOutput, expected: BatchOutput) {
    let close = |a: f64, b: f64| {
        assert!(a.is_finite() && b.is_finite());
        assert!(
            (a - b).abs() <= 1e-10 * (1.0 + b.abs()),
            "actual={a} expected={b}"
        );
    };
    close(actual.checksum, expected.checksum);
    for i in 0..4 {
        close(actual.last[i], expected.last[i]);
    }
}

fn styles(kernel: Kernel) -> &'static [Style] {
    // No vector intermediates exist in this case: don't mislabel three copies
    // of identical scalar code as distinct array implementations.
    if matches!(kernel, Kernel::ScalarChain) {
        &STYLES[..1]
    } else {
        &STYLES
    }
}

#[test]
fn declarative_math_numeric_oracle_covers_all_baseline_backends() {
    let runtime = Runtime::standard();
    for kernel in KERNELS {
        let mut direct = Direct(kernel);
        let mut graph = Graph::new(kernel);
        let mut flat = FlatPlan::new(kernel);
        let mut scripts = styles(kernel)
            .iter()
            .map(|style| MmsKernel::new(&runtime, &mms::source(kernel, *style)))
            .collect::<Vec<_>>();
        // Each individual result is checked, not just an aggregate checksum.
        // Vary inputs across calls to catch frozen/cached numerical outputs.
        for frame in 0..32 {
            let mut input = native::fixture(frame, 0);
            if frame == 0 {
                input.q = [0.0, 0.0, 0.0, 1.0];
            }
            if frame == 1 {
                input.a = [0.0; 3];
                input.t = 2.0;
            }
            if frame == 2 {
                input.q = [1.0, 0.0, 0.0, 0.0];
                input.t = -2.0;
            }
            let expected = native_batch(&mut direct, &[input]);
            assert_close(native_batch(&mut graph, &[input]), expected);
            assert_close(native_batch(&mut flat, &[input]), expected);
            for script in &mut scripts {
                assert_close(script.evaluate(mms::arguments(&[input])), expected);
            }
        }
        let inputs = (0..17)
            .map(|subject| native::fixture(71, subject))
            .collect::<Vec<_>>();
        let expected = native_batch(&mut direct, &inputs);
        assert_close(native_batch(&mut graph, &inputs), expected);
        assert_close(native_batch(&mut flat, &inputs), expected);
        for script in &mut scripts {
            assert_close(script.evaluate(mms::arguments(&inputs)), expected);
        }
    }
}

#[test]
fn declarative_math_rotation_has_an_independent_known_answer() {
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let rotated = native::rotate([0.0, 0.0, s, s], [1.0, 0.0, 0.0]);
    assert!(rotated[0].abs() < 1e-14);
    assert!((rotated[1] - 1.0).abs() < 1e-14);
    assert!(rotated[2].abs() < 1e-14);
    let q = [0.0, s, 0.0, s];
    assert_eq!(native::compose([0.0, 0.0, 0.0, 1.0], q), q);
    let composed = native::compose([s, 0.0, 0.0, s], [0.0, s, 0.0, s]);
    for value in composed {
        assert!((value - 0.5).abs() < 1e-14);
    }
}

#[test]
fn declarative_math_nested_operands_materialize_as_arguments_not_children() {
    use meow_meow_script::Value;
    let tree = Runtime::standard()
        .materialize_component(include_str!(
            "fixtures/declarative_math/operand-builders.mms"
        ))
        .unwrap();
    assert_eq!(tree.component_type, "VectorArithmetic");
    assert_eq!(tree.constructor.name.as_deref(), Some("subtract"));
    assert!(tree.children.is_empty());
    assert_eq!(tree.initializer_calls[0].name, "left");
    assert_eq!(tree.initializer_calls[1].name, "right");
    let Value::ComponentExpr(left) = &tree.initializer_calls[0].arguments[0] else {
        panic!("left operand")
    };
    assert_eq!(left.component_type, "ValueSource");
    let Value::ComponentExpr(right) = &tree.initializer_calls[1].arguments[0] else {
        panic!("right operand")
    };
    assert_eq!(right.component_type, "VectorRotate");
    assert!(right.children.is_empty());
    // Current materialization preserves the first operand but classifies it as
    // a constructor. The proposal requires this to become a builder on an
    // implicit default constructor when the component APIs are implemented.
    assert_eq!(right.constructor.name.as_deref(), Some("rotation"));
    assert_eq!(right.initializer_calls[0].name, "vector");
    for arguments in [
        &right.constructor.arguments,
        &right.initializer_calls[0].arguments,
    ] {
        let Value::ComponentExpr(source) = &arguments[0] else {
            panic!("nested source operand")
        };
        assert_eq!(source.component_type, "ValueSource");
    }
}

#[test]
fn declarative_math_native_prototypes_reuse_buffers_and_shared_values() {
    for kernel in KERNELS {
        let mut graph = Graph::new(kernel);
        let mut flat = FlatPlan::new(kernel);
        let (_, measurement) = measure(|| {
            for frame in 0..512 {
                let input = black_box(native::fixture(frame, 3));
                black_box(graph.evaluate(input));
                black_box(flat.evaluate(input));
            }
        });
        assert_eq!(measurement.counts.allocations, 0, "{kernel:?}");
    }
    let mut graph = Graph::new(Kernel::SharedRotation);
    graph.evaluate(native::fixture(1, 0));
    assert_eq!(
        graph.evaluations, 8,
        "shared rotation is evaluated once, not once per consumer"
    );
}

fn positive_env(name: &str, default: usize) -> usize {
    let Some(value) = std::env::var_os(name) else {
        return default;
    };
    value
        .to_str()
        .expect("UTF-8 benchmark option")
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
        .unwrap_or_else(|| panic!("{name} must be a positive integer"))
}

fn report(
    backend: &str,
    kernel: Kernel,
    phase: &str,
    subjects: usize,
    updates: usize,
    samples: &[Measurement],
) {
    let mut ns = samples
        .iter()
        .map(|sample| sample.ns as f64 / updates as f64)
        .collect::<Vec<_>>();
    ns.sort_by(f64::total_cmp);
    let p95 = ((ns.len() * 95).div_ceil(100) - 1).min(ns.len() - 1);
    let mean_allocations = samples
        .iter()
        .map(|s| s.counts.allocations as f64)
        .sum::<f64>()
        / (samples.len() * updates) as f64;
    let mean_bytes = samples.iter().map(|s| s.counts.bytes as f64).sum::<f64>()
        / (samples.len() * updates) as f64;
    println!(
        "MATH_PROFILE {}",
        serde_json::json!({
            "backend": backend, "kernel": kernel.name(), "phase": phase,
            "precision": "f64", "subjects": subjects, "updates_per_sample": updates,
            "samples": samples.len(), "median_ns_per_update": ns[ns.len()/2],
            "p95_batch_mean_ns_per_update": ns[p95],
            "median_ns_per_subject": ns[ns.len()/2] / subjects as f64,
            "mean_allocation_calls_per_update": mean_allocations,
            "mean_gross_allocated_bytes_per_update": mean_bytes,
            "raw_sample_ns": samples.iter().map(|s| s.ns.to_string()).collect::<Vec<_>>(),
        })
    );
}

fn profile_native<B: NativeBackend>(
    name: &str,
    kernel: Kernel,
    factory: impl Fn() -> B,
    frames: &[Vec<Input>],
    rounds: usize,
) {
    let mut build = Vec::new();
    for _ in 0..rounds {
        let (backend, sample) = measure(|| black_box(factory()));
        black_box(backend);
        build.push(sample);
    }
    report(name, kernel, "build", 1, 1, &build);
    let mut backend = factory();
    for frame in frames {
        black_box(native_batch(&mut backend, frame));
    }
    // Longer native batches keep timer quantization out of tiny kernels.
    // The fixture sequence cycles; every input is black-boxed at its read.
    let updates = frames.len() * 128;
    let mut samples = Vec::new();
    for _ in 0..rounds {
        let (_, sample) = measure(|| {
            for update in 0..updates {
                black_box(native_batch(&mut backend, &frames[update % frames.len()]));
            }
        });
        samples.push(sample);
    }
    report(name, kernel, "steady", frames[0].len(), updates, &samples);
}

fn profile_mms(
    runtime: &Runtime,
    kernel: Kernel,
    style: Style,
    frames: &[Vec<Input>],
    rounds: usize,
) {
    let source = mms::source(kernel, style);
    if let Some(path) = std::env::var_os("MITTENS_MATH_PROFILE_DUMP_MMS") {
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            std::path::Path::new(&path).join(format!("{}-{}.mms", kernel.name(), style.name())),
            &source,
        )
        .unwrap();
    }
    let mut build = Vec::new();
    for _ in 0..rounds {
        let (backend, sample) = measure(|| MmsKernel::new(runtime, &source));
        black_box(backend);
        build.push(sample);
    }
    report(
        style.name(),
        kernel,
        "parse_and_bind_callback",
        1,
        1,
        &build,
    );
    let mut backend = MmsKernel::new(runtime, &source);
    for frame in frames {
        black_box(backend.evaluate(mms::arguments(frame)));
    }
    let mut marshalling = Vec::new();
    let mut samples = Vec::new();
    for _ in 0..rounds {
        // Own the invocation argument arrays before starting the update timer.
        let (arguments, sample) = measure(|| {
            frames
                .iter()
                .map(|frame| mms::arguments(frame))
                .collect::<Vec<_>>()
        });
        marshalling.push(sample);
        let (_, sample) = measure(|| {
            for arguments in arguments {
                black_box(backend.evaluate(arguments));
            }
        });
        samples.push(sample);
    }
    report(
        style.name(),
        kernel,
        "input_marshalling",
        frames[0].len(),
        frames.len(),
        &marshalling,
    );
    report(
        style.name(),
        kernel,
        "steady_retained_callback",
        frames[0].len(),
        frames.len(),
        &samples,
    );
}

#[test]
#[ignore = "opt-in release performance measurements; no wall-clock CI thresholds"]
fn declarative_math_performance_profile() {
    assert!(
        !cfg!(debug_assertions),
        "run this profile with cargo test --release"
    );
    assert!(
        std::env::var_os("MITTENS_MATH_PROFILE_REQUIRE_PRODUCTION").is_none(),
        "production MathGraph backend is not implemented/wired into this suite yet"
    );
    // A standalone --ignored run must not time unchecked implementations.
    // These correctness checks are deliberately outside every timed region.
    declarative_math_rotation_has_an_independent_known_answer();
    declarative_math_numeric_oracle_covers_all_baseline_backends();
    let updates = positive_env("MITTENS_MATH_PROFILE_UPDATES", 16);
    let rounds = positive_env("MITTENS_MATH_PROFILE_SAMPLES", 7);
    let subjects = std::env::var("MITTENS_MATH_PROFILE_SUBJECTS")
        .unwrap_or_else(|_| "1,16,64".into())
        .split(',')
        .map(|value| {
            value
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0)
                .expect("subject counts must be positive integers")
        })
        .collect::<Vec<_>>();
    println!(
        "\nMATH_PROFILE {}",
        serde_json::json!({
            "kind": "metadata", "architecture": std::env::consts::ARCH,
            "os": std::env::consts::OS, "precision": "f64", "chain_length": native::CHAIN_LENGTH,
            "production_backend": "unavailable", "native_graph_and_flat": "benchmark_prototypes_only",
            "mms_runtime": "retained host-neutral Runtime::standard callback; one callback per batch",
            "samples": rounds, "mms_updates_per_sample": updates, "subject_counts": subjects,
            "warmup": "one complete fixture sequence per backend",
            "allocation_instrumentation": "thread_local; alloc/alloc_zeroed/realloc calls and gross requested bytes",
        })
    );
    let runtime = Runtime::standard();
    for kernel in KERNELS {
        for &subjects in &subjects {
            let frames = (0..updates)
                .map(|frame| {
                    (0..subjects)
                        .map(|subject| native::fixture(frame, subject))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            profile_native(
                "rust_direct_reference",
                kernel,
                || Direct(kernel),
                &frames,
                rounds,
            );
            profile_native(
                "rust_graph_prototype",
                kernel,
                || Graph::new(kernel),
                &frames,
                rounds,
            );
            profile_native(
                "rust_flat_plan_prototype",
                kernel,
                || FlatPlan::new(kernel),
                &frames,
                rounds,
            );
            for &style in styles(kernel) {
                profile_mms(&runtime, kernel, style, &frames, rounds);
            }
        }
    }
}
