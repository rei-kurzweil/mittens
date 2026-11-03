use super::*;
use std::time::Duration;

#[test]
fn labeled_intervals_measure_duration_and_real_samples() {
    let mut benchmarks = Benchmarks::default();
    let now = Instant::now();
    benchmarks.start_at("off", now).unwrap();
    let ticket = benchmarks.frame_ticket();
    benchmarks.record_cpu_update(ticket, 1.0);
    benchmarks.record_cpu_update(ticket, 3.0);
    benchmarks.record_frame(ticket, Some(4.0));
    benchmarks.record_frame(ticket, Some(2.0));
    benchmarks.record_frame(ticket, None);
    let result = benchmarks
        .stop_at("off", now + Duration::from_millis(250))
        .unwrap();
    assert_eq!(result.duration_ms, 250.0);
    assert_eq!(result.frame_count, 3);
    assert_eq!(
        result.cpu_update_ms.unwrap(),
        MetricSummary {
            samples: 2,
            mean_ms: 2.0,
            min_ms: 1.0,
            max_ms: 3.0
        }
    );
    assert_eq!(
        result.gpu_frame_ms.unwrap(),
        MetricSummary {
            samples: 2,
            mean_ms: 3.0,
            min_ms: 2.0,
            max_ms: 4.0
        }
    );
    assert!(!benchmarks.has_active());
}
#[test]
fn overlapping_labels_have_independent_boundaries_and_restarts_reject_late_samples() {
    let mut benchmarks = Benchmarks::default();
    benchmarks.start("outer").unwrap();
    let old = benchmarks.frame_ticket();
    benchmarks.start("inner").unwrap();
    benchmarks.record_frame(old, Some(99.0));
    benchmarks.record_frame(benchmarks.frame_ticket(), Some(2.0));
    let inner = benchmarks.stop("inner").unwrap();
    assert_eq!(inner.frame_count, 1);
    assert_eq!(inner.gpu_frame_ms.unwrap().mean_ms, 2.0);
    let late = benchmarks.frame_ticket();
    benchmarks.start("inner").unwrap();
    benchmarks.record_frame(late, Some(88.0));
    assert_eq!(benchmarks.stop("inner").unwrap().frame_count, 0);
    assert_eq!(benchmarks.stop("outer").unwrap().frame_count, 3);
}
#[test]
fn absent_and_invalid_samples_are_unavailable_instead_of_zero() {
    let mut benchmarks = Benchmarks::default();
    benchmarks.start("empty").unwrap();
    let result = benchmarks.stop("empty").unwrap();
    assert_eq!(result.cpu_update_ms, None);
    assert_eq!(result.gpu_frame_ms, None);
    benchmarks.start("invalid").unwrap();
    let ticket = benchmarks.frame_ticket();
    for value in [f64::NAN, f64::INFINITY, -1.0] {
        benchmarks.record_cpu_update(ticket, value);
        benchmarks.record_frame(ticket, Some(value));
    }
    let result = benchmarks.stop("invalid").unwrap();
    assert_eq!(result.frame_count, 3);
    assert_eq!(result.cpu_update_ms, None);
    assert_eq!(result.gpu_frame_ms, None);
}
#[test]
fn invalid_lifecycle_is_explicit_and_reset_invalidates_old_tickets() {
    let mut benchmarks = Benchmarks::default();
    assert!(benchmarks.start("  ").is_err());
    assert!(benchmarks.start(&"x".repeat(257)).is_err());
    assert!(benchmarks.stop("missing").is_err());
    benchmarks.start("test").unwrap();
    let ticket = benchmarks.frame_ticket();
    assert!(benchmarks.start("test").is_err());
    benchmarks.reset();
    assert!(benchmarks.stop("test").is_err());
    benchmarks.start("test").unwrap();
    benchmarks.record_frame(ticket, Some(99.0));
    assert_eq!(benchmarks.stop("test").unwrap().frame_count, 0);
}
