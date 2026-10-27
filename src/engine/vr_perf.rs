use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::engine::graphics::vulkano_renderer::RendererPerfCounters;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrPerfCase {
    AvatarNoSpringNoMirror,
    AvatarNoSpringMirror,
    AvatarSpringNoVizMirror,
    AvatarSpringVizMirror,
}

impl VrPerfCase {
    pub const ALL: [Self; 4] = [
        Self::AvatarNoSpringNoMirror,
        Self::AvatarNoSpringMirror,
        Self::AvatarSpringNoVizMirror,
        Self::AvatarSpringVizMirror,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|case| case.as_str() == value)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AvatarNoSpringNoMirror => "avatar_no_spring_no_mirror",
            Self::AvatarNoSpringMirror => "avatar_no_spring_mirror",
            Self::AvatarSpringNoVizMirror => "avatar_spring_no_viz_mirror",
            Self::AvatarSpringVizMirror => "avatar_spring_viz_mirror",
        }
    }

    pub fn mirror(self) -> bool {
        !matches!(self, Self::AvatarNoSpringNoMirror)
    }

    pub fn secondary_motion(self) -> bool {
        matches!(
            self,
            Self::AvatarSpringNoVizMirror | Self::AvatarSpringVizMirror
        )
    }

    pub fn visualization(self) -> bool {
        matches!(self, Self::AvatarSpringVizMirror)
    }
}

#[derive(Debug, Clone)]
pub struct VrPerfConfig {
    pub case: VrPerfCase,
    pub warmup: Duration,
    pub sample: Duration,
    pub report_dir: PathBuf,
}

impl VrPerfConfig {
    pub fn new(case: VrPerfCase, warmup: Duration, sample: Duration) -> Self {
        Self {
            case,
            warmup,
            sample,
            report_dir: PathBuf::from("docs/.debug/vr_perf"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VrPerfMetadata {
    pub build_profile: &'static str,
    pub gpu_device: String,
    pub openxr_runtime: String,
    pub render_extent: [u32; 2],
    pub msaa: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VrPerfPreXrCpu {
    pub update_total: Duration,
    pub final_command_processing: Duration,
    pub secondary_motion: Duration,
    pub spring_transform_propagation: Duration,
    pub spring_visualization: Duration,
    pub post_secondary_skinning: Duration,
    pub post_pose_command_flush: Duration,
    pub prepare_render: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VrPerfFrameCpu {
    pub pre_xr: VrPerfPreXrCpu,
    pub total: Duration,
    pub wait_frame: Duration,
    pub eye_render: Duration,
    pub completion_wait: Duration,
    pub submit: Duration,
    pub renderer: RendererPerfCounters,
}

#[derive(Debug)]
enum Phase {
    WaitingForResources,
    Warmup { started: Instant },
    Sampling { started: Instant },
    Complete,
}

#[derive(Debug)]
pub struct VrPerfCollector {
    config: VrPerfConfig,
    phase: Phase,
    last_presented: Option<Instant>,
    frame_times: Vec<Duration>,
    display_intervals: Vec<Duration>,
    cpu_frames: Vec<VrPerfFrameCpu>,
}

impl VrPerfCollector {
    pub fn new(config: VrPerfConfig) -> Self {
        Self {
            config,
            phase: Phase::WaitingForResources,
            last_presented: None,
            frame_times: Vec::new(),
            display_intervals: Vec::new(),
            cpu_frames: Vec::new(),
        }
    }

    pub fn case(&self) -> VrPerfCase {
        self.config.case
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.phase, Phase::Complete)
    }

    /// Records one successfully submitted headset frame.
    ///
    /// `resources_ready` deliberately gates the warm-up clock. This prevents model upload,
    /// deformation-cache creation, and the first visualization spawn from entering warm-up.
    pub fn presented_frame(
        &mut self,
        now: Instant,
        resources_ready: bool,
        display_interval: Duration,
        cpu: VrPerfFrameCpu,
        metadata: &VrPerfMetadata,
    ) -> Option<Result<PathBuf, String>> {
        match self.phase {
            Phase::WaitingForResources if resources_ready => {
                println!(
                    "[vr-perf] preset={} resources ready; warming up for {:.3}s",
                    self.config.case.as_str(),
                    self.config.warmup.as_secs_f64()
                );
                self.phase = Phase::Warmup { started: now };
                self.last_presented = Some(now);
            }
            Phase::WaitingForResources => {}
            Phase::Warmup { started } if now.duration_since(started) >= self.config.warmup => {
                println!(
                    "[vr-perf] preset={} sampling for {:.3}s",
                    self.config.case.as_str(),
                    self.config.sample.as_secs_f64()
                );
                self.phase = Phase::Sampling { started: now };
                self.last_presented = Some(now);
                self.frame_times.clear();
                self.display_intervals.clear();
                self.cpu_frames.clear();
            }
            Phase::Warmup { .. } => {
                self.last_presented = Some(now);
            }
            Phase::Sampling { started } => {
                if let Some(previous) = self.last_presented.replace(now) {
                    self.frame_times.push(now.duration_since(previous));
                    self.display_intervals.push(display_interval);
                    self.cpu_frames.push(cpu);
                }
                if now.duration_since(started) >= self.config.sample {
                    let elapsed = now.duration_since(started);
                    self.phase = Phase::Complete;
                    return Some(self.write_report(elapsed, metadata).map_err(|error| {
                        format!("failed to write VR performance report: {error}")
                    }));
                }
            }
            Phase::Complete => {}
        }
        None
    }

    fn write_report(
        &self,
        elapsed: Duration,
        metadata: &VrPerfMetadata,
    ) -> Result<PathBuf, std::io::Error> {
        fs::create_dir_all(&self.config.report_dir)?;
        let filename = format!(
            "{}__{}.md",
            utc_timestamp(SystemTime::now()),
            self.config.case.as_str()
        );
        let path = non_overwriting_path(&self.config.report_dir, &filename);
        fs::write(&path, self.report_markdown(elapsed, metadata))?;
        Ok(path)
    }

    fn report_markdown(&self, elapsed: Duration, metadata: &VrPerfMetadata) -> String {
        let stats = DurationStats::from_samples(&self.frame_times);
        let target = mean_duration(&self.display_intervals);
        let missed = target.map_or(0, |interval| {
            self.frame_times
                .iter()
                .filter(|frame| **frame > interval)
                .count()
        });
        let frame_count = self.frame_times.len();
        let missed_percent = if frame_count == 0 {
            0.0
        } else {
            missed as f64 * 100.0 / frame_count as f64
        };
        let average_fps = if elapsed.is_zero() {
            0.0
        } else {
            frame_count as f64 / elapsed.as_secs_f64()
        };
        let refresh_hz = target
            .filter(|interval| !interval.is_zero())
            .map(|interval| 1.0 / interval.as_secs_f64());

        let mut out = String::new();
        let _ = writeln!(out, "# XR performance report\n");
        let _ = writeln!(out, "- Preset: `{}`", self.config.case.as_str());
        let _ = writeln!(out, "- Avatar / XR control: on");
        let _ = writeln!(out, "- Mirror: {}", on_off(self.config.case.mirror()));
        let _ = writeln!(
            out,
            "- Secondary motion: {}",
            on_off(self.config.case.secondary_motion())
        );
        let _ = writeln!(
            out,
            "- Spring-bone visualization: {}",
            on_off(self.config.case.visualization())
        );
        let _ = writeln!(
            out,
            "- Warm-up requested: {:.3} s",
            self.config.warmup.as_secs_f64()
        );
        let _ = writeln!(
            out,
            "- Sample requested: {:.3} s\n",
            self.config.sample.as_secs_f64()
        );

        let _ = writeln!(out, "## Headset frame results\n");
        let _ = writeln!(out, "- Sampled headset frames: {frame_count}");
        let _ = writeln!(out, "- Elapsed: {:.3} s", elapsed.as_secs_f64());
        let _ = writeln!(out, "- Arithmetic average FPS: {average_fps:.3}");
        write_duration_stat(&mut out, "Mean", stats.map(|s| s.mean));
        write_duration_stat(&mut out, "Median", stats.map(|s| s.median));
        write_duration_stat(&mut out, "p95", stats.map(|s| s.p95));
        write_duration_stat(&mut out, "p99", stats.map(|s| s.p99));
        write_duration_stat(&mut out, "Minimum", stats.map(|s| s.min));
        write_duration_stat(&mut out, "Maximum", stats.map(|s| s.max));
        match target {
            Some(interval) => {
                let _ = writeln!(
                    out,
                    "- Runtime display interval: {:.3} ms",
                    interval.as_secs_f64() * 1000.0
                );
            }
            None => {
                let _ = writeln!(out, "- Runtime display interval: unavailable");
            }
        }
        let _ = writeln!(
            out,
            "- Frames exceeding display interval: {missed} ({missed_percent:.2}%)"
        );
        let _ = writeln!(out, "- Runtime dropped frames: unavailable");
        let _ = writeln!(out, "- Runtime reprojected frames: unavailable\n");

        let _ = writeln!(out, "## Environment\n");
        let _ = writeln!(out, "- Build profile: {}", metadata.build_profile);
        let _ = writeln!(out, "- GPU / device: {}", metadata.gpu_device);
        let _ = writeln!(out, "- OpenXR runtime: {}", metadata.openxr_runtime);
        match refresh_hz {
            Some(hz) => {
                let _ = writeln!(out, "- Headset target refresh rate: {hz:.3} Hz");
            }
            None => {
                let _ = writeln!(out, "- Headset target refresh rate: unavailable");
            }
        }
        let _ = writeln!(
            out,
            "- Render extent: {} × {}",
            metadata.render_extent[0], metadata.render_extent[1]
        );
        let _ = writeln!(out, "- MSAA: {}\n", metadata.msaa);

        let _ = writeln!(out, "## CPU timing\n");
        write_cpu_mean(&mut out, "Update before XR", &self.cpu_frames, |frame| {
            frame.pre_xr.update_total
        });
        write_cpu_mean(
            &mut out,
            "Final command processing",
            &self.cpu_frames,
            |frame| frame.pre_xr.final_command_processing,
        );
        write_cpu_mean(
            &mut out,
            "Secondary-motion simulation",
            &self.cpu_frames,
            |frame| frame.pre_xr.secondary_motion,
        );
        write_cpu_mean(
            &mut out,
            "Spring transform propagation",
            &self.cpu_frames,
            |frame| frame.pre_xr.spring_transform_propagation,
        );
        write_cpu_mean(
            &mut out,
            "Spring visualization",
            &self.cpu_frames,
            |frame| frame.pre_xr.spring_visualization,
        );
        write_cpu_mean(
            &mut out,
            "Post-secondary skinning",
            &self.cpu_frames,
            |frame| frame.pre_xr.post_secondary_skinning,
        );
        write_cpu_mean(
            &mut out,
            "Post-pose/layout command flush",
            &self.cpu_frames,
            |frame| frame.pre_xr.post_pose_command_flush,
        );
        write_cpu_mean(&mut out, "Render preparation", &self.cpu_frames, |frame| {
            frame.pre_xr.prepare_render
        });
        write_cpu_mean(&mut out, "Total XR frame", &self.cpu_frames, |frame| {
            frame.total
        });
        write_cpu_mean(&mut out, "wait_frame", &self.cpu_frames, |frame| {
            frame.wait_frame
        });
        write_cpu_mean(&mut out, "Eye render", &self.cpu_frames, |frame| {
            frame.eye_render
        });
        write_cpu_mean(&mut out, "GPU completion wait", &self.cpu_frames, |frame| {
            frame.completion_wait
        });
        write_cpu_mean(&mut out, "Frame submit", &self.cpu_frames, |frame| {
            frame.submit
        });

        let _ = writeln!(out, "\n## Detailed renderer / deformation counters\n");
        write_counter(
            &mut out,
            "Vulkan queue submissions",
            &self.cpu_frames,
            |frame| frame.renderer.queue_submissions,
        );
        write_counter(&mut out, "CPU fence waits", &self.cpu_frames, |frame| {
            frame.renderer.cpu_fence_waits
        });
        write_counter(
            &mut out,
            "CPU queue-idle waits",
            &self.cpu_frames,
            |frame| frame.renderer.cpu_queue_waits,
        );
        write_counter(&mut out, "Mirror captures", &self.cpu_frames, |frame| {
            frame.renderer.mirror_captures
        });
        write_counter(&mut out, "XR eyes rendered", &self.cpu_frames, |frame| {
            frame.renderer.xr_eyes
        });
        write_counter(
            &mut out,
            "Deformation dispatches",
            &self.cpu_frames,
            |frame| frame.renderer.deformation_dispatches,
        );
        write_counter(&mut out, "Deformation jobs", &self.cpu_frames, |frame| {
            frame.renderer.deformation_jobs
        });
        write_counter(
            &mut out,
            "Deformation workgroups",
            &self.cpu_frames,
            |frame| frame.renderer.deformation_workgroups,
        );
        write_counter(
            &mut out,
            "Dirty deformation vertices",
            &self.cpu_frames,
            |frame| frame.renderer.deformation_dirty_vertices,
        );
        write_counter(&mut out, "Bone upload bytes", &self.cpu_frames, |frame| {
            frame.renderer.deformation_bone_upload_bytes
        });
        write_counter(&mut out, "Job upload bytes", &self.cpu_frames, |frame| {
            frame.renderer.deformation_job_upload_bytes
        });
        write_counter(
            &mut out,
            "Morph-weight upload bytes",
            &self.cpu_frames,
            |frame| frame.renderer.deformation_weight_upload_bytes,
        );
        write_counter(
            &mut out,
            "Morph-delta upload bytes",
            &self.cpu_frames,
            |frame| frame.renderer.deformation_morph_delta_upload_bytes,
        );
        write_counter(
            &mut out,
            "Deformation buffer allocations",
            &self.cpu_frames,
            |frame| frame.renderer.deformation_buffer_allocations,
        );
        write_counter(
            &mut out,
            "Deformation output resizes",
            &self.cpu_frames,
            |frame| frame.renderer.deformation_output_resizes,
        );
        write_counter(&mut out, "Bone buffer resizes", &self.cpu_frames, |frame| {
            frame.renderer.deformation_bone_resizes
        });
        let _ = writeln!(
            out,
            "\nMirror GPU time, per-eye GPU time, and per-view draw counts: unavailable."
        );
        out
    }
}

#[derive(Debug, Clone, Copy)]
struct DurationStats {
    mean: Duration,
    median: Duration,
    p95: Duration,
    p99: Duration,
    min: Duration,
    max: Duration,
}

impl DurationStats {
    fn from_samples(samples: &[Duration]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        Some(Self {
            mean: mean_duration(&sorted).unwrap(),
            median: percentile(&sorted, 0.50),
            p95: percentile(&sorted, 0.95),
            p99: percentile(&sorted, 0.99),
            min: sorted[0],
            max: sorted[sorted.len() - 1],
        })
    }
}

fn percentile(sorted: &[Duration], quantile: f64) -> Duration {
    let index = ((sorted.len() - 1) as f64 * quantile).ceil() as usize;
    sorted[index]
}

fn mean_duration(samples: &[Duration]) -> Option<Duration> {
    if samples.is_empty() {
        return None;
    }
    let total_nanos: u128 = samples.iter().map(Duration::as_nanos).sum();
    Some(Duration::from_nanos(
        (total_nanos / samples.len() as u128).min(u64::MAX as u128) as u64,
    ))
}

fn write_duration_stat(out: &mut String, label: &str, value: Option<Duration>) {
    match value {
        Some(duration) => {
            let _ = writeln!(
                out,
                "- {label} headset frame time: {:.3} ms",
                duration.as_secs_f64() * 1000.0
            );
        }
        None => {
            let _ = writeln!(out, "- {label} headset frame time: unavailable");
        }
    }
}

fn write_cpu_mean(
    out: &mut String,
    label: &str,
    frames: &[VrPerfFrameCpu],
    get: impl Fn(&VrPerfFrameCpu) -> Duration,
) {
    let samples: Vec<_> = frames.iter().map(get).collect();
    match mean_duration(&samples) {
        Some(duration) => {
            let _ = writeln!(
                out,
                "- Mean {label}: {:.3} ms",
                duration.as_secs_f64() * 1000.0
            );
        }
        None => {
            let _ = writeln!(out, "- Mean {label}: unavailable");
        }
    }
}

fn write_counter(
    out: &mut String,
    label: &str,
    frames: &[VrPerfFrameCpu],
    get: impl Fn(&VrPerfFrameCpu) -> u64,
) {
    let total: u64 = frames.iter().map(get).sum();
    let per_frame = if frames.is_empty() {
        0.0
    } else {
        total as f64 / frames.len() as f64
    };
    let _ = writeln!(
        out,
        "- {label}: {total} total, {per_frame:.3} per headset frame"
    );
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn non_overwriting_path(directory: &Path, filename: &str) -> PathBuf {
    let initial = directory.join(filename);
    if !initial.exists() {
        return initial;
    }
    let (stem, extension) = filename.rsplit_once('.').unwrap_or((filename, ""));
    for suffix in 1_u32.. {
        let candidate = if extension.is_empty() {
            directory.join(format!("{stem}__{suffix}"))
        } else {
            directory.join(format!("{stem}__{suffix}.{extension}"))
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn utc_timestamp(now: SystemTime) -> String {
    let total_seconds = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let days = total_seconds.div_euclid(86_400);
    let seconds = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}Z")
}

// Howard Hinnant's proleptic-Gregorian civil-from-days conversion.
fn civil_from_days(days_since_unix_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_cases_resolve_expected_switches() {
        for case in VrPerfCase::ALL {
            assert_eq!(VrPerfCase::parse(case.as_str()), Some(case));
        }
        assert!(!VrPerfCase::AvatarNoSpringNoMirror.mirror());
        assert!(VrPerfCase::AvatarNoSpringMirror.mirror());
        assert!(!VrPerfCase::AvatarNoSpringMirror.secondary_motion());
        assert!(VrPerfCase::AvatarSpringNoVizMirror.secondary_motion());
        assert!(VrPerfCase::AvatarSpringVizMirror.visualization());
        assert_eq!(VrPerfCase::parse("unknown"), None);
    }

    #[test]
    fn duration_stats_use_nearest_rank_tail_percentiles() {
        let samples: Vec<_> = (1..=100).map(Duration::from_millis).collect();
        let stats = DurationStats::from_samples(&samples).unwrap();
        assert_eq!(
            stats.mean,
            Duration::from_millis(50) + Duration::from_micros(500)
        );
        assert_eq!(stats.median, Duration::from_millis(51));
        assert_eq!(stats.p95, Duration::from_millis(96));
        assert_eq!(stats.p99, Duration::from_millis(100));
        assert_eq!(stats.min, Duration::from_millis(1));
        assert_eq!(stats.max, Duration::from_millis(100));
    }

    #[test]
    fn unix_epoch_formats_as_utc_timestamp() {
        assert_eq!(utc_timestamp(UNIX_EPOCH), "19700101-000000Z");
    }
}
