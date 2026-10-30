//! Low-overhead rolling frame measurements for the headset bring-up.
use crate::{renderer::Stats, xr_host::Report};

const WARMUP: u32 = 90;
const WINDOW: u32 = 120;

#[derive(Default)]
pub(crate) struct FrameProfile {
    warmup: u32,
    count: u32,
    sums: [f64; 6],
    maxima: [f64; 6],
    gpu_samples: u32,
    gpu_sum: f64,
    gpu_max: f64,
}
impl FrameProfile {
    pub fn record(
        &mut self,
        predicted_interval_ms: Option<f64>,
        update_ms: f64,
        assets_ms: f64,
        stats: Stats,
        report: &mut Report,
    ) {
        if self.warmup < WARMUP {
            self.warmup += 1;
            return;
        }
        let values = [
            predicted_interval_ms.unwrap_or(0.0),
            update_ms,
            assets_ms,
            stats.cpu_prepare_ms,
            stats.cpu_record_ms,
            stats.gpu_wait_ms,
        ];
        for (index, value) in values.into_iter().enumerate() {
            self.sums[index] += value;
            self.maxima[index] = self.maxima[index].max(value);
        }
        if let Some(gpu_ms) = stats.gpu_ms {
            self.gpu_samples += 1;
            self.gpu_sum += gpu_ms;
            self.gpu_max = self.gpu_max.max(gpu_ms);
        }
        self.count += 1;
        if self.count == WINDOW {
            let avg = self.sums.map(|sum| sum / WINDOW as f64);
            let gpu = (self.gpu_samples > 0).then(|| self.gpu_sum / self.gpu_samples as f64);
            report.line(format!("FRAME_PROFILE samples={} predicted_interval_ms={:.2} update_cpu_ms={:.2} assets_cpu_ms={:.2} renderer_prepare_cpu_ms={:.2} record_cpu_ms={:.2} submit_and_fence_wait_ms={:.2} gpu_eye_pass_ms={gpu:?} gpu_samples={} max_update_cpu_ms={:.2} max_gpu_eye_pass_ms={:.2} draws={} instances={}", self.count, avg[0], avg[1], avg[2], avg[3], avg[4], avg[5], self.gpu_samples, self.maxima[1], self.gpu_max, stats.draws, stats.instances));
            self.count = 0;
            self.sums = [0.0; 6];
            self.maxima = [0.0; 6];
            self.gpu_samples = 0;
            self.gpu_sum = 0.0;
            self.gpu_max = 0.0;
        }
    }
}
