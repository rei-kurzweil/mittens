//! Labeled measurement intervals. Hosts feed completed measurements explicitly;
//! this service has no renderer dependency and never invents missing GPU samples.
use std::{collections::HashMap, time::Instant};

#[derive(Debug, Clone, Copy)]
pub struct FrameTicket(u64);

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MetricSummary {
    pub samples: u64,
    pub mean_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}
impl MetricSummary {
    fn record(&mut self, value: f64) {
        if !value.is_finite() || value < 0.0 {
            return;
        }
        self.samples += 1;
        self.mean_ms += (value - self.mean_ms) / self.samples as f64;
        if self.samples == 1 {
            self.min_ms = value;
            self.max_ms = value;
        } else {
            self.min_ms = self.min_ms.min(value);
            self.max_ms = self.max_ms.max(value);
        }
    }
    fn available(self) -> Option<Self> {
        (self.samples > 0).then_some(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkResult {
    pub label: String,
    pub duration_ms: f64,
    pub frame_count: u64,
    pub cpu_update_ms: Option<MetricSummary>,
    pub gpu_frame_ms: Option<MetricSummary>,
}
#[derive(Debug, Clone)]
struct Interval {
    started: Instant,
    generation: u64,
    frames: u64,
    cpu: MetricSummary,
    gpu: MetricSummary,
}
#[derive(Debug, Default, Clone)]
pub struct Benchmarks {
    active: HashMap<String, Interval>,
    generation: u64,
}
impl Benchmarks {
    pub fn start(&mut self, label: &str) -> Result<(), String> {
        self.start_at(label, Instant::now())
    }
    fn start_at(&mut self, label: &str, now: Instant) -> Result<(), String> {
        if label.trim().is_empty() || label.len() > 256 {
            return Err("label must be nonblank and at most 256 bytes".into());
        }
        if self.active.contains_key(label) {
            return Err(format!("benchmark {label:?} is already running"));
        }
        if self.active.len() >= 64 {
            return Err("at most 64 benchmark intervals may overlap".into());
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("benchmark generation exhausted")?;
        self.active.insert(
            label.into(),
            Interval {
                started: now,
                generation: self.generation,
                frames: 0,
                cpu: MetricSummary::default(),
                gpu: MetricSummary::default(),
            },
        );
        Ok(())
    }
    pub fn stop(&mut self, label: &str) -> Result<BenchmarkResult, String> {
        self.stop_at(label, Instant::now())
    }
    fn stop_at(&mut self, label: &str, now: Instant) -> Result<BenchmarkResult, String> {
        let run = self
            .active
            .remove(label)
            .ok_or_else(|| format!("benchmark {label:?} is not running"))?;
        Ok(BenchmarkResult {
            label: label.into(),
            duration_ms: now.saturating_duration_since(run.started).as_secs_f64() * 1000.0,
            frame_count: run.frames,
            cpu_update_ms: run.cpu.available(),
            gpu_frame_ms: run.gpu.available(),
        })
    }
    /// Capture before the work being measured, then deliver its result using
    /// this ticket. A delayed result cannot leak into a newly started interval,
    /// including when the same label has been stopped and restarted.
    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }
    pub fn frame_ticket(&self) -> FrameTicket {
        FrameTicket(self.generation)
    }
    pub fn record_cpu_update(&mut self, ticket: FrameTicket, elapsed_ms: f64) {
        for run in self
            .active
            .values_mut()
            .filter(|run| run.generation <= ticket.0)
        {
            run.cpu.record(elapsed_ms);
        }
    }
    /// GPU duration covers the host's complete rendered frame (both eyes on
    /// Android). Calling this with None counts a frame with no GPU timing.
    pub fn record_frame(&mut self, ticket: FrameTicket, gpu_ms: Option<f64>) {
        for run in self
            .active
            .values_mut()
            .filter(|run| run.generation <= ticket.0)
        {
            run.frames += 1;
            if let Some(value) = gpu_ms {
                run.gpu.record(value);
            }
        }
    }
    /// Drop active intervals on scene/session replacement. Keep generations
    /// monotonic so outstanding host tickets remain invalid after reset.
    pub fn reset(&mut self) {
        self.active.clear();
    }
}

#[cfg(test)]
#[path = "benchmark_tests.rs"]
mod tests;
