//! Application timing, not a claim about compositor-presented or native frames.
use crate::xr_host::Result;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    time::Duration,
};

#[derive(Default, Clone, Copy)]
pub struct Sample {
    pub wait: Duration,
    pub update: Duration,
    pub prepare: Duration,
    pub render_complete: Duration,
    pub submit: Duration,
    pub active: Duration,
    pub interval: Duration,
    pub predicted_period_ms: f64,
    pub predicted_step_ms: f64,
}
pub struct Timings {
    samples: Vec<Sample>,
    budget_ms: f64,
}
fn ms(value: Duration) -> f64 {
    value.as_secs_f64() * 1000.0
}
impl Timings {
    pub fn new(hz: f64) -> Self {
        Self {
            samples: Vec::with_capacity(20_000),
            budget_ms: 1000.0 / hz,
        }
    }
    pub fn record(&mut self, sample: Sample) {
        self.samples.push(sample);
    }
    pub fn write(&self, path: &Path, renderer: &str) -> Result<()> {
        if self.samples.is_empty() {
            return Err("no rendered samples recorded".into());
        }
        let file = File::create(path).map_err(|e| e.to_string())?;
        let mut writer = BufWriter::new(file);
        writeln!(writer, "renderer,frame,wait_frame_ms,update_ms,prepare_ms,render_and_gpu_complete_ms,end_frame_ms,active_frame_ms,app_interval_ms,predicted_period_ms,predicted_step_ms,budget_ms").map_err(|e| e.to_string())?;
        for (frame, s) in self.samples.iter().enumerate() {
            writeln!(
                writer,
                "{renderer},{frame},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
                ms(s.wait),
                ms(s.update),
                ms(s.prepare),
                ms(s.render_complete),
                ms(s.submit),
                ms(s.active),
                ms(s.interval),
                s.predicted_period_ms,
                s.predicted_step_ms,
                self.budget_ms
            )
            .map_err(|e| e.to_string())?;
        }
        writer.flush().map_err(|e| e.to_string())?;
        let mut times: Vec<_> = self.samples.iter().map(|s| ms(s.active)).collect();
        times.sort_by(f64::total_cmp);
        let p = |fraction: f64| times[((times.len() - 1) as f64 * fraction).round() as usize];
        let missed = times.iter().filter(|&&v| v > self.budget_ms).count();
        let interval = self
            .samples
            .iter()
            .map(|s| s.interval.as_secs_f64())
            .sum::<f64>();
        println!(
            "renderer={renderer} samples={} app_submission_hz={:.2} active_ms p50={:.3} p95={:.3} p99={:.3} over_budget={}/{} budget_ms={:.3}",
            times.len(),
            times.len() as f64 / interval.max(1e-9),
            p(0.5),
            p(0.95),
            p(0.99),
            missed,
            times.len(),
            self.budget_ms
        );
        println!(
            "CSV={}; render time includes command recording + submission + fence completion, not a GPU timestamp. xrWaitFrame is excluded from active_ms. App submission rate is not compositor/native display FPS.",
            path.display()
        );
        Ok(())
    }
}
