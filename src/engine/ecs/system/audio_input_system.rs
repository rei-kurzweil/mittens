use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, Producer};

use crate::engine::ecs::component::{
    AmplitudeSample, AmplitudeStatus, AudioInputComponent, AudioInputDeviceSelector,
    VolumeNormalizationReason,
};
use crate::engine::ecs::system::amplitude_system::{
    AmplitudeSnapshot, InputAmplitudeConsumer, InputNormalizationConsumer,
    VolumeNormalizationSnapshot,
};
use crate::engine::ecs::{ComponentId, World};

use super::AmplitudeSystem;

const SNAPSHOT_QUEUE_CAPACITY: usize = 512;
const NO_DATA_TIMEOUT: Duration = Duration::from_secs(3);
const DIAGNOSTIC_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq)]
struct CaptureSignature {
    device: AudioInputDeviceSelector,
    selection_generation: u64,
    consumers: Vec<InputAmplitudeConsumer>,
    normalizers: Vec<InputNormalizationConsumer>,
}

#[derive(Default)]
struct DesiredConsumers {
    consumers: Vec<InputAmplitudeConsumer>,
    normalizers: Vec<InputNormalizationConsumer>,
}

#[derive(Clone, Copy)]
enum CaptureSnapshot {
    Amplitude(AmplitudeSnapshot),
    Normalization(VolumeNormalizationSnapshot),
}

/// A capture backend can report a stream as successfully created but never
/// supply audio. Do not repeatedly reopen such a backend: CPAL/ALSA probing is
/// expensive and some backends are not robust to rapid stream churn. A new
/// device selection (including selecting the same row again) is the explicit
/// request to try it again.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UnavailableCapture {
    device: AudioInputDeviceSelector,
    selection_generation: u64,
}

impl From<&CaptureSignature> for UnavailableCapture {
    fn from(signature: &CaptureSignature) -> Self {
        Self {
            device: signature.device.clone(),
            selection_generation: signature.selection_generation,
        }
    }
}

struct CaptureRuntime {
    signature: CaptureSignature,
    _stream: cpal::Stream,
    snapshots: Consumer<CaptureSnapshot>,
    failed: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
    reported_dropped: u64,
    last_snapshot: Instant,
}

pub struct AudioInputSystem {
    runtimes: HashMap<ComponentId, CaptureRuntime>,
    unavailable: HashMap<ComponentId, UnavailableCapture>,
    last_diagnostic:
        HashMap<ComponentId, (Instant, AmplitudeStatus, Option<VolumeNormalizationReason>)>,
}

impl std::fmt::Debug for AudioInputSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioInputSystem")
            .field("active_sources", &self.runtimes.len())
            .field("unavailable_sources", &self.unavailable.len())
            .finish()
    }
}

impl Default for AudioInputSystem {
    fn default() -> Self {
        Self {
            runtimes: HashMap::new(),
            unavailable: HashMap::new(),
            last_diagnostic: HashMap::new(),
        }
    }
}

impl AudioInputSystem {
    fn is_unavailable(&self, source: ComponentId, signature: &CaptureSignature) -> bool {
        self.unavailable
            .get(&source)
            .is_some_and(|blocked| *blocked == UnavailableCapture::from(signature))
    }

    pub fn tick(&mut self, world: &mut World, amplitude: &mut AmplitudeSystem) {
        amplitude.refresh_consumers(world);
        let mut desired: HashMap<ComponentId, DesiredConsumers> = HashMap::new();
        for consumer in amplitude.input_consumers(world) {
            desired
                .entry(consumer.source)
                .or_default()
                .consumers
                .push(consumer);
        }
        for consumer in amplitude.input_normalization_consumers(world) {
            desired
                .entry(consumer.source)
                .or_default()
                .normalizers
                .push(consumer);
        }

        self.runtimes
            .retain(|source, _| desired.contains_key(source));
        self.unavailable
            .retain(|source, _| desired.contains_key(source));
        self.last_diagnostic.retain(|observer, _| {
            desired.values().any(|group| {
                group
                    .consumers
                    .iter()
                    .any(|consumer| consumer.observer == *observer)
                    || group
                        .normalizers
                        .iter()
                        .any(|consumer| consumer.normalizer == *observer)
            })
        });

        for (&source, consumers) in &desired {
            let Some(input) = world.get_component_by_id_as::<AudioInputComponent>(source) else {
                continue;
            };
            let mut signature = CaptureSignature {
                device: input.device.clone(),
                selection_generation: input.selection_generation,
                consumers: consumers.consumers.clone(),
                normalizers: consumers.normalizers.clone(),
            };
            if self.is_unavailable(source, &signature) {
                continue;
            }
            // The input selection changed, so this is an explicit new attempt.
            self.unavailable.remove(&source);
            let needs_rebuild = self
                .runtimes
                .get(&source)
                .is_none_or(|runtime| runtime.signature != signature);
            if !needs_rebuild {
                continue;
            }
            if self.runtimes.remove(&source).is_some() {
                // A selector/consumer configuration change replaces the
                // capture stream. Never retain a measurement from the old
                // device while the new stream negotiates and starts. This
                // also changes the amplitude generation, so rebuild the
                // signature *after* rebinding consumers; otherwise the new
                // callback immediately looks stale and is reopened forever.
                amplitude.invalidate_source(world, source);
                amplitude.refresh_consumers(world);
                signature.consumers = amplitude
                    .input_consumers(world)
                    .into_iter()
                    .filter(|consumer| consumer.source == source)
                    .collect();
                signature.normalizers = amplitude
                    .input_normalization_consumers(world)
                    .into_iter()
                    .filter(|consumer| consumer.source == source)
                    .collect();
            }
            let unavailable = UnavailableCapture::from(&signature);
            match start_capture(source, signature) {
                Ok(runtime) => {
                    self.runtimes.insert(source, runtime);
                }
                Err(error) => {
                    eprintln!(
                        "[AudioInput] source={source:?} capture start failed: {error}; waiting for a new device selection"
                    );
                    self.unavailable.insert(source, unavailable);
                    amplitude.invalidate_source(world, source);
                }
            }
        }

        let now = Instant::now();
        let failed: Vec<_> = self
            .runtimes
            .iter()
            .filter_map(|(&source, runtime)| {
                (runtime.failed.load(Ordering::Acquire)
                    || (now.duration_since(runtime.last_snapshot) >= NO_DATA_TIMEOUT
                        && runtime.snapshots.is_empty()))
                .then(|| (source, runtime.signature.clone()))
            })
            .collect();
        for (source, signature) in failed {
            eprintln!(
                "[AudioInput] source={source:?} capture stream failed or supplied no samples; waiting for a new device selection"
            );
            self.runtimes.remove(&source);
            self.unavailable
                .insert(source, UnavailableCapture::from(&signature));
            amplitude.invalidate_source(world, source);
        }

        for runtime in self.runtimes.values_mut() {
            while let Ok(snapshot) = runtime.snapshots.pop() {
                runtime.last_snapshot = Instant::now();
                let (observer, source, sample, normalization) = match snapshot {
                    CaptureSnapshot::Amplitude(snapshot) => {
                        (snapshot.observer, snapshot.source, snapshot.sample, None)
                    }
                    CaptureSnapshot::Normalization(snapshot) => (
                        snapshot.normalizer,
                        snapshot.source,
                        snapshot.sample,
                        Some((snapshot.gain_db, snapshot.reason)),
                    ),
                };
                let reason = normalization.map(|(_, reason)| reason);
                let diagnostic = self.last_diagnostic.entry(observer).or_insert((
                    Instant::now() - DIAGNOSTIC_INTERVAL,
                    AmplitudeStatus::Pending,
                    None,
                ));
                if diagnostic.1 != sample.status
                    || diagnostic.2 != reason
                    || diagnostic.0.elapsed() >= DIAGNOSTIC_INTERVAL
                {
                    if let Some((gain_db, reason)) = normalization {
                        eprintln!(
                            "[VolumeNormalization] observer={observer:?} source={source:?} status={:?} rms={:.6} peak={:.6} gain_db={gain_db:.2} reason={reason:?} frames={} dropped={}",
                            sample.status,
                            sample.rms,
                            sample.peak,
                            sample.valid_frames,
                            runtime.dropped.load(Ordering::Relaxed),
                        );
                    } else {
                        eprintln!(
                            "[Amplitude] observer={observer:?} source={source:?} status={:?} rms={:.6} peak={:.6} frames={} dropped={}",
                            sample.status,
                            sample.rms,
                            sample.peak,
                            sample.valid_frames,
                            runtime.dropped.load(Ordering::Relaxed),
                        );
                    }
                    *diagnostic = (Instant::now(), sample.status, reason);
                }
                match snapshot {
                    CaptureSnapshot::Amplitude(snapshot) => amplitude.submit_snapshot(snapshot),
                    CaptureSnapshot::Normalization(snapshot) => {
                        amplitude.submit_normalized_snapshot(snapshot)
                    }
                }
            }
            let dropped = runtime.dropped.load(Ordering::Relaxed);
            let new_drops = dropped.wrapping_sub(runtime.reported_dropped);
            if new_drops != 0 {
                amplitude.record_dropped_snapshots(new_drops);
                runtime.reported_dropped = dropped;
            }
        }
        amplitude.drain_pending(world);
    }
}

fn selected_device(selector: &AudioInputDeviceSelector) -> Result<(cpal::Device, String), String> {
    let host = cpal::default_host();
    let device = match selector {
        AudioInputDeviceSelector::Default => host
            .default_input_device()
            .ok_or_else(|| "no default input device is available".to_string())?,
        AudioInputDeviceSelector::DeviceNumber(index) => host
            .input_devices()
            .map_err(|error| format!("cannot enumerate input devices: {error}"))?
            .nth(*index)
            .ok_or_else(|| format!("input device number {index} is not available"))?,
    };
    let name = device.name().unwrap_or_else(|_| "<unnamed input>".into());
    Ok((device, name))
}

fn start_capture(
    source: ComponentId,
    signature: CaptureSignature,
) -> Result<CaptureRuntime, String> {
    let (device, device_name) = selected_device(&signature.device)?;
    let supported = device
        .default_input_config()
        .map_err(|error| format!("cannot query default input format: {error}"))?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let accumulators = signature
        .consumers
        .iter()
        .map(|consumer| RollingRms::new(*consumer, sample_rate))
        .collect::<Vec<_>>();
    let normalizers = signature
        .normalizers
        .iter()
        .map(|consumer| NormalizingRms::new(consumer.clone(), sample_rate))
        .collect::<Vec<_>>();
    let windows = signature
        .consumers
        .iter()
        .map(|consumer| format!("{:.3}s", consumer.window_sec))
        .collect::<Vec<_>>()
        .join(",");
    let (producer, snapshots) = rtrb::RingBuffer::new(SNAPSHOT_QUEUE_CAPACITY);
    let failed = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicU64::new(0));
    let error_flag = failed.clone();
    let error_callback = move |_error| {
        error_flag.store(true, Ordering::Release);
    };

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let mut callback = CaptureCallback::new(
                source,
                channels,
                sample_rate,
                accumulators,
                normalizers,
                producer,
                dropped.clone(),
            );
            device.build_input_stream(
                &config,
                move |data: &[f32], _| callback.process(data, |v| v),
                error_callback,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let mut callback = CaptureCallback::new(
                source,
                channels,
                sample_rate,
                accumulators,
                normalizers,
                producer,
                dropped.clone(),
            );
            device.build_input_stream(
                &config,
                move |data: &[i16], _| callback.process(data, |v| v as f32 / i16::MAX as f32),
                error_callback,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let mut callback = CaptureCallback::new(
                source,
                channels,
                sample_rate,
                accumulators,
                normalizers,
                producer,
                dropped.clone(),
            );
            device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    callback.process(data, |v| v as f32 / u16::MAX as f32 * 2.0 - 1.0)
                },
                error_callback,
                None,
            )
        }
        other => return Err(format!("unsupported input sample format {other:?}")),
    }
    .map_err(|error| format!("cannot build input stream: {error}"))?;
    stream
        .play()
        .map_err(|error| format!("cannot start input stream: {error}"))?;
    eprintln!(
        "[AudioInput] source={source:?} device={device_name:?} format={sample_format:?} sample_rate={sample_rate} channels={channels} windows=[{windows}]"
    );
    Ok(CaptureRuntime {
        signature,
        _stream: stream,
        snapshots,
        failed,
        dropped,
        reported_dropped: 0,
        last_snapshot: Instant::now(),
    })
}

struct CaptureCallback {
    source: ComponentId,
    channels: usize,
    sample_rate: u32,
    frame_count: u64,
    accumulators: Vec<RollingRms>,
    normalizers: Vec<NormalizingRms>,
    snapshots: Producer<CaptureSnapshot>,
    dropped: Arc<AtomicU64>,
}

impl CaptureCallback {
    fn new(
        source: ComponentId,
        channels: usize,
        sample_rate: u32,
        accumulators: Vec<RollingRms>,
        normalizers: Vec<NormalizingRms>,
        snapshots: Producer<CaptureSnapshot>,
        dropped: Arc<AtomicU64>,
    ) -> Self {
        Self {
            source,
            channels: channels.max(1),
            sample_rate,
            frame_count: 0,
            accumulators,
            normalizers,
            snapshots,
            dropped,
        }
    }

    fn process<T: Copy>(&mut self, data: &[T], convert: impl Fn(T) -> f32) {
        let before = self.frame_count;
        for frame in data.chunks_exact(self.channels) {
            let mut sum_squares = 0.0;
            let mut peak = 0.0_f32;
            for &sample in frame {
                let value = convert(sample);
                let value = if value.is_finite() {
                    value.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                sum_squares += value * value;
                peak = peak.max(value.abs());
            }
            let mean_square = sum_squares / self.channels as f32;
            for accumulator in &mut self.accumulators {
                accumulator.push(mean_square, peak);
            }
            for normalizer in &mut self.normalizers {
                normalizer.push(mean_square, peak);
            }
            self.frame_count = self.frame_count.wrapping_add(1);
        }
        let valid_frames = self.frame_count.wrapping_sub(before) as u32;
        if valid_frames == 0 {
            return;
        }
        let timestamp_sec = self.frame_count as f64 / self.sample_rate.max(1) as f64;
        for accumulator in &mut self.accumulators {
            let snapshot = accumulator.snapshot(self.source, timestamp_sec, valid_frames);
            if self
                .snapshots
                .push(CaptureSnapshot::Amplitude(snapshot))
                .is_err()
            {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
        for normalizer in &mut self.normalizers {
            let snapshot =
                normalizer.snapshot(self.source, timestamp_sec, valid_frames, self.sample_rate);
            if self
                .snapshots
                .push(CaptureSnapshot::Normalization(snapshot))
                .is_err()
            {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

struct RollingRms {
    consumer: InputAmplitudeConsumer,
    squares: Vec<f32>,
    peaks: Vec<f32>,
    cursor: usize,
    filled: usize,
    sum_squares: f64,
    sequence: u64,
}

impl RollingRms {
    fn new(consumer: InputAmplitudeConsumer, sample_rate: u32) -> Self {
        let frames = (consumer.window_sec as f64 * sample_rate as f64)
            .round()
            .clamp(1.0, usize::MAX as f64) as usize;
        Self {
            consumer,
            squares: vec![0.0; frames],
            peaks: vec![0.0; frames],
            cursor: 0,
            filled: 0,
            sum_squares: 0.0,
            sequence: 0,
        }
    }

    fn push(&mut self, square: f32, peak: f32) {
        if self.filled == self.squares.len() {
            self.sum_squares -= self.squares[self.cursor] as f64;
        } else {
            self.filled += 1;
        }
        self.squares[self.cursor] = square;
        self.peaks[self.cursor] = peak;
        self.sum_squares += square as f64;
        self.cursor = (self.cursor + 1) % self.squares.len();
    }

    fn snapshot(
        &mut self,
        source: ComponentId,
        timestamp_sec: f64,
        valid_frames: u32,
    ) -> AmplitudeSnapshot {
        self.sequence = self.sequence.wrapping_add(1);
        let rms = (self.sum_squares.max(0.0) / self.filled.max(1) as f64).sqrt() as f32;
        let peak = self.peaks[..self.filled]
            .iter()
            .copied()
            .fold(0.0_f32, f32::max);
        let status = if rms <= f32::EPSILON {
            AmplitudeStatus::Neutral
        } else {
            AmplitudeStatus::Live
        };
        AmplitudeSnapshot {
            observer: self.consumer.observer,
            source,
            sample: AmplitudeSample {
                generation: self.consumer.generation,
                sequence: self.sequence,
                timestamp_sec,
                valid_frames,
                rms,
                peak,
                status,
            },
        }
    }
}

/// A fused rolling measurement and bounded AGC controller.  It is built on the
/// control thread and then used only by the capture callback.
struct NormalizingRms {
    consumer: InputNormalizationConsumer,
    squares: Vec<f32>,
    peaks: Vec<f32>,
    cursor: usize,
    filled: usize,
    sum_squares: f64,
    sequence: u64,
    gain_db: f32,
    quiet_sec: f32,
    high_sec: f32,
    high_latched: bool,
}

impl NormalizingRms {
    fn new(consumer: InputNormalizationConsumer, sample_rate: u32) -> Self {
        let frames = (consumer.window_sec as f64 * sample_rate as f64)
            .round()
            .clamp(1.0, usize::MAX as f64) as usize;
        let policy = consumer.policy.load();
        Self {
            // Start at unity even when policy permits attenuation. The
            // controller then learns either direction from actual input.
            gain_db: 0.0_f32.clamp(policy.min_gain_db, policy.max_gain_db),
            consumer,
            squares: vec![0.0; frames],
            peaks: vec![0.0; frames],
            cursor: 0,
            filled: 0,
            sum_squares: 0.0,
            sequence: 0,
            quiet_sec: 0.0,
            high_sec: 0.0,
            high_latched: false,
        }
    }

    fn push(&mut self, square: f32, peak: f32) {
        if self.filled == self.squares.len() {
            self.sum_squares -= self.squares[self.cursor] as f64;
        } else {
            self.filled += 1;
        }
        self.squares[self.cursor] = square;
        self.peaks[self.cursor] = peak;
        self.sum_squares += square as f64;
        self.cursor = (self.cursor + 1) % self.squares.len();
    }

    fn snapshot(
        &mut self,
        source: ComponentId,
        timestamp_sec: f64,
        valid_frames: u32,
        sample_rate: u32,
    ) -> VolumeNormalizationSnapshot {
        self.sequence = self.sequence.wrapping_add(1);
        let raw_rms = (self.sum_squares.max(0.0) / self.filled.max(1) as f64).sqrt() as f32;
        let raw_peak = self.peaks[..self.filled]
            .iter()
            .copied()
            .fold(0.0_f32, f32::max);
        let dt = valid_frames as f32 / sample_rate.max(1) as f32;
        let (rms, peak, status, reason) = if raw_rms <= f32::EPSILON {
            // Digital silence is neutral and deliberately never teaches gain.
            self.quiet_sec = 0.0;
            self.high_sec = 0.0;
            self.high_latched = false;
            (
                0.0,
                0.0,
                AmplitudeStatus::Neutral,
                VolumeNormalizationReason::Holding,
            )
        } else {
            let reason = self.advance_controller(raw_rms, raw_peak, dt);
            let gain = db_to_linear(self.gain_db);
            (
                raw_rms * gain,
                raw_peak * gain,
                AmplitudeStatus::Live,
                reason,
            )
        };
        VolumeNormalizationSnapshot {
            normalizer: self.consumer.normalizer,
            source,
            sample: AmplitudeSample {
                generation: self.consumer.generation,
                sequence: self.sequence,
                timestamp_sec,
                valid_frames,
                rms,
                peak,
                status,
            },
            gain_db: self.gain_db,
            reason,
        }
    }

    fn advance_controller(
        &mut self,
        raw_rms: f32,
        raw_peak: f32,
        dt: f32,
    ) -> VolumeNormalizationReason {
        let policy = self.consumer.policy.load();
        let gain = db_to_linear(self.gain_db);
        let normalized_rms = raw_rms * gain;
        let normalized_peak = raw_peak * gain;

        // A peak breach is corrected in this callback, without waiting for a
        // long RMS hold.  The gain caps still bound the correction.
        if raw_peak > f32::EPSILON && normalized_peak > policy.peak_headroom {
            let safe_gain_db = linear_to_db(policy.peak_headroom / raw_peak);
            let next = safe_gain_db.clamp(policy.min_gain_db, policy.max_gain_db);
            self.gain_db = self.gain_db.min(next);
            self.quiet_sec = 0.0;
            self.high_sec = 0.0;
            self.high_latched = false;
            return if (raw_peak * db_to_linear(self.gain_db))
                > policy.peak_headroom * (1.0 + 1.0e-5)
            {
                VolumeNormalizationReason::Capped
            } else {
                VolumeNormalizationReason::PeakReducing
            };
        }

        if normalized_rms >= policy.target_rms_high {
            self.high_latched = true;
        } else if normalized_rms <= policy.target_rms_high * 0.90 {
            self.high_latched = false;
        }
        if self.high_latched {
            self.high_sec += dt;
            self.quiet_sec = 0.0;
            if self.high_sec >= policy.high_hold_sec {
                let target = linear_to_db(policy.target_rms_high / raw_rms);
                let next = (self.gain_db - policy.gain_fall_db_per_sec * dt)
                    .max(target)
                    .max(policy.min_gain_db);
                if next < self.gain_db {
                    self.gain_db = next;
                    return VolumeNormalizationReason::SustainedReducing;
                }
                return VolumeNormalizationReason::Capped;
            }
            return VolumeNormalizationReason::Holding;
        }
        self.high_sec = 0.0;

        if raw_rms >= policy.activity_gate
            && normalized_rms < policy.target_rms_low
            && normalized_peak < policy.peak_headroom
        {
            self.quiet_sec += dt;
            if self.quiet_sec >= policy.quiet_hold_sec {
                let target = linear_to_db(policy.target_rms_low / raw_rms);
                let next = (self.gain_db + policy.gain_rise_db_per_sec * dt)
                    .min(target)
                    .min(policy.max_gain_db);
                if next > self.gain_db {
                    self.gain_db = next;
                    return VolumeNormalizationReason::Raising;
                }
                return VolumeNormalizationReason::Capped;
            }
        } else {
            // Below-gate noise and ordinary in-band activity must not accrue
            // a future gain increase.
            self.quiet_sec = 0.0;
        }
        VolumeNormalizationReason::Holding
    }
}

fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

fn linear_to_db(linear: f32) -> f32 {
    20.0 * linear.max(f32::MIN_POSITIVE).log10()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::volume_normalization::{
        LiveVolumeNormalizationPolicy, VolumeNormalizationPolicy,
    };
    use crate::engine::ecs::component::{AmplitudeComponent, AudioInputComponent, ComponentRef};
    use std::sync::Arc;

    fn consumer(window_sec: f32) -> InputAmplitudeConsumer {
        InputAmplitudeConsumer {
            observer: ComponentId::default(),
            source: ComponentId::default(),
            generation: 3,
            window_sec,
        }
    }

    fn normalizer_consumer(policy: VolumeNormalizationPolicy) -> InputNormalizationConsumer {
        InputNormalizationConsumer {
            normalizer: ComponentId::default(),
            source: ComponentId::default(),
            generation: 4,
            window_sec: 0.01,
            policy: Arc::new(LiveVolumeNormalizationPolicy::new(policy)),
        }
    }

    fn policy() -> VolumeNormalizationPolicy {
        VolumeNormalizationPolicy {
            min_gain_db: -24.0,
            max_gain_db: 24.0,
            target_rms_low: 0.027,
            target_rms_high: 0.03,
            activity_gate: 0.003,
            quiet_hold_sec: 0.75,
            high_hold_sec: 0.75,
            gain_rise_db_per_sec: 3.0,
            gain_fall_db_per_sec: 12.0,
            peak_headroom: 0.9,
        }
    }

    #[test]
    fn rolling_rms_expires_old_frames_and_reports_peak() {
        let mut rms = RollingRms::new(consumer(0.5), 4);
        rms.push(0.25, 0.5);
        rms.push(1.0, 1.0);
        let first = rms.snapshot(ComponentId::default(), 0.5, 2).sample;
        assert!((first.rms - (0.625_f32).sqrt()).abs() < 1e-6);
        assert_eq!(first.peak, 1.0);
        rms.push(0.0, 0.0);
        let rolled = rms.snapshot(ComponentId::default(), 0.75, 1).sample;
        assert!((rolled.rms - (0.5_f32).sqrt()).abs() < 1e-6);
        assert_eq!(rolled.peak, 1.0);
    }

    #[test]
    fn exact_silence_is_neutral() {
        let mut rms = RollingRms::new(consumer(0.25), 4);
        rms.push(0.0, 0.0);
        assert_eq!(
            rms.snapshot(ComponentId::default(), 0.25, 1).sample.status,
            AmplitudeStatus::Neutral
        );
    }

    #[test]
    fn normalizer_raises_only_after_hold_and_never_from_silence_or_noise() {
        let mut normalizer = NormalizingRms::new(normalizer_consumer(policy()), 100);
        for i in 0..7 {
            normalizer.push(0.0001, 0.01);
            let snapshot = normalizer.snapshot(ComponentId::default(), i as f64 * 0.1, 10, 100);
            assert_eq!(snapshot.reason, VolumeNormalizationReason::Holding);
            assert_eq!(snapshot.gain_db, 0.0);
        }
        normalizer.push(0.0001, 0.01);
        let raised = normalizer.snapshot(ComponentId::default(), 0.8, 10, 100);
        assert_eq!(raised.reason, VolumeNormalizationReason::Raising);
        assert!(raised.gain_db > 0.0);

        let learned_gain = raised.gain_db;
        normalizer.push(0.0, 0.0);
        let silence = normalizer.snapshot(ComponentId::default(), 0.9, 10, 100);
        assert_eq!(silence.sample.status, AmplitudeStatus::Neutral);
        assert_eq!(silence.gain_db, learned_gain);

        let mut noise = NormalizingRms::new(normalizer_consumer(policy()), 100);
        for i in 0..12 {
            noise.push(0.000004, 0.004);
            let snapshot = noise.snapshot(ComponentId::default(), i as f64 * 0.1, 10, 100);
            assert_eq!(snapshot.gain_db, 0.0);
        }
    }

    #[test]
    fn normalizer_reduces_peak_immediately_and_sustained_high_after_hold() {
        let mut peak_policy = policy();
        peak_policy.min_gain_db = -24.0;
        let mut peak = NormalizingRms::new(normalizer_consumer(peak_policy), 100);
        peak.gain_db = 12.0;
        peak.push(0.04, 0.8);
        let safe = peak.snapshot(ComponentId::default(), 0.1, 10, 100);
        assert_eq!(safe.reason, VolumeNormalizationReason::PeakReducing);
        assert!(safe.sample.peak <= 0.9001);
        assert!(safe.gain_db < 12.0);

        let mut high_policy = policy();
        high_policy.high_hold_sec = 0.20;
        let mut high = NormalizingRms::new(normalizer_consumer(high_policy), 100);
        high.gain_db = 12.0;
        high.push(0.0064, 0.08); // RMS 0.08 -> comfortably above the high band with gain.
        let held = high.snapshot(ComponentId::default(), 0.1, 10, 100);
        assert_eq!(held.reason, VolumeNormalizationReason::Holding);
        high.push(0.0064, 0.08);
        let reduced = high.snapshot(ComponentId::default(), 0.2, 10, 100);
        assert_eq!(reduced.reason, VolumeNormalizationReason::SustainedReducing);
        assert!(reduced.gain_db < 12.0);
    }

    #[test]
    fn default_normalizer_attenuates_after_point_zero_three_for_three_quarters_second() {
        let mut normalizer = NormalizingRms::new(normalizer_consumer(policy()), 100);
        for step in 0..7 {
            normalizer.push(0.04_f32.powi(2), 0.06);
            let snapshot = normalizer.snapshot(ComponentId::default(), step as f64 * 0.1, 10, 100);
            assert_eq!(snapshot.reason, VolumeNormalizationReason::Holding);
            assert_eq!(snapshot.gain_db, 0.0);
        }
        normalizer.push(0.04_f32.powi(2), 0.06);
        let reduced = normalizer.snapshot(ComponentId::default(), 0.8, 10, 100);
        assert_eq!(reduced.reason, VolumeNormalizationReason::SustainedReducing);
        assert!(reduced.gain_db < 0.0);
    }

    #[test]
    fn normalizer_observes_live_policy_changes_without_reconstruction() {
        let consumer = normalizer_consumer(policy());
        let live_policy = consumer.policy.clone();
        let mut normalizer = NormalizingRms::new(consumer, 100);

        // Change the policy after the rolling buffers and controller exist.
        // A 20 ms RMS is initially in the old target band, but must begin
        // attenuation immediately under this new 10 ms upper target.
        let mut updated = live_policy.load();
        updated.target_rms_low = 0.008;
        updated.target_rms_high = 0.010;
        updated.high_hold_sec = 0.0;
        updated.gain_fall_db_per_sec = 12.0;
        live_policy.store(updated);

        normalizer.push(0.02_f32.powi(2), 0.04);
        let snapshot = normalizer.snapshot(ComponentId::default(), 0.1, 10, 100);
        assert_eq!(
            snapshot.reason,
            VolumeNormalizationReason::SustainedReducing
        );
        assert!(snapshot.gain_db < 0.0);
    }

    #[test]
    fn callback_converts_stereo_frames_and_queue_overflow_never_blocks() {
        let source = ComponentId::default();
        let accumulator = RollingRms::new(consumer(1.0), 2);
        let (producer, mut snapshots) = rtrb::RingBuffer::new(1);
        let dropped = Arc::new(AtomicU64::new(0));
        let mut callback = CaptureCallback::new(
            source,
            2,
            2,
            vec![accumulator],
            vec![],
            producer,
            dropped.clone(),
        );
        callback.process(&[1.0_f32, -1.0, 0.5, 0.5], |value| value);
        let CaptureSnapshot::Amplitude(snapshot) = snapshots.pop().unwrap() else {
            panic!("raw accumulator must publish an amplitude snapshot");
        };
        let sample = snapshot.sample;
        assert!((sample.rms - 0.625_f32.sqrt()).abs() < 1e-6);
        assert_eq!(sample.peak, 1.0);

        callback.process(&[0.25_f32, 0.25], |value| value);
        callback.process(&[0.25_f32, 0.25], |value| value);
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unavailable_capture_waits_for_an_explicit_new_selection() {
        let source = ComponentId::default();
        let initial = CaptureSignature {
            device: AudioInputDeviceSelector::DeviceNumber(2),
            selection_generation: 4,
            consumers: vec![consumer(0.08)],
            normalizers: vec![],
        };
        let mut system = AudioInputSystem::default();
        system
            .unavailable
            .insert(source, UnavailableCapture::from(&initial));
        assert!(system.is_unavailable(source, &initial));

        let retry = CaptureSignature {
            selection_generation: 5,
            ..initial
        };
        assert!(!system.is_unavailable(source, &retry));
    }

    #[test]
    fn invalidated_source_rebinds_consumers_before_replacement_capture() {
        let mut world = World::default();
        let source = world.add_component(AudioInputComponent::new());
        let source_guid = world.get_component_record(source).unwrap().guid;
        world.add_component(
            AmplitudeComponent::rolling_window(0.08)
                .unwrap()
                .with_source(ComponentRef::Guid(source_guid)),
        );
        let mut amplitude = AmplitudeSystem::new();
        amplitude.refresh_consumers(&mut world);
        let initial = amplitude.input_consumers(&world);

        amplitude.invalidate_source(&mut world, source);
        amplitude.refresh_consumers(&mut world);
        let rebound = amplitude.input_consumers(&world);

        assert_eq!(initial.len(), 1);
        assert_eq!(rebound.len(), 1);
        assert_ne!(initial[0].generation, rebound[0].generation);
    }
}
