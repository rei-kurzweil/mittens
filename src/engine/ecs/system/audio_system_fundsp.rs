use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use heapless::Vec as HVec;
use rtrb::Consumer;

use crate::engine::ecs::component::AudioOscillator;

pub const AUDIO_QUEUE_CAP: usize = 1024;
pub const MAX_OSCS_PER_COMPONENT: usize = 16;

pub const MAX_AUDIO_GRAPH_NODES: usize = 64;
pub const MAX_AUDIO_GRAPH_CHILDREN_PER_NODE: usize = 8;

mod fundsp_backend {
    use std::collections::HashMap;

    use fundsp::audiounit::AudioUnit;
    use fundsp::buffer::{BufferMut, BufferRef};
    use fundsp::hacker32::{saw, sine, square, triangle};
    use fundsp::signal::{Signal, SignalFrame};
    use fundsp::simd_items;

    use crate::engine::ecs::component::{AudioOscillator, OscillatorType};

    const ZERO_BLOCK: [f32; fundsp::MAX_BUFFER_SIZE] = [0.0; fundsp::MAX_BUFFER_SIZE];

    pub(crate) struct FundspState {
        voices: HashMap<(u64, usize), Voice>,
        sample_rate_hz: f64,
    }

    struct Voice {
        unit: Box<dyn AudioUnit>,
        last_type: OscillatorType,
    }

    impl Default for FundspState {
        fn default() -> Self {
            Self {
                voices: HashMap::new(),
                sample_rate_hz: fundsp::DEFAULT_SR,
            }
        }
    }

    impl std::fmt::Debug for FundspState {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("FundspState")
                .field("voices_len", &self.voices.len())
                .field("sample_rate_hz", &self.sample_rate_hz)
                .finish()
        }
    }

    impl FundspState {
        pub(crate) fn set_sample_rate(&mut self, sample_rate_hz: f64) {
            let sample_rate_hz = if sample_rate_hz.is_finite() {
                sample_rate_hz.max(1.0)
            } else {
                fundsp::DEFAULT_SR
            };

            if (self.sample_rate_hz - sample_rate_hz).abs() <= 0.5 {
                return;
            }

            self.sample_rate_hz = sample_rate_hz;
            for voice in self.voices.values_mut() {
                voice.unit.set_sample_rate(sample_rate_hz);
            }
        }

        pub(crate) fn retrigger_voice(&mut self, key: (u64, usize)) {
            if let Some(v) = self.voices.get_mut(&key) {
                v.unit.reset();
            }
        }

        pub(crate) fn prune_component(&mut self, component_ffi: u64, new_len: usize) {
            self.voices
                .retain(|(cid, idx), _| *cid != component_ffi || *idx < new_len);
        }
    }

    fn sanitize_hz(hz: f32) -> f32 {
        if hz.is_finite() { hz.max(0.0) } else { 0.0 }
    }

    fn make_unit(ty: OscillatorType) -> Box<dyn AudioUnit> {
        match ty {
            OscillatorType::Sin => Box::new(sine()),
            OscillatorType::Triangle => Box::new(triangle()),
            OscillatorType::Square => Box::new(square()),
            OscillatorType::Saw => Box::new(saw()),
            OscillatorType::Square3 => Box::new(Square3Unit::new()),
            OscillatorType::Drum => Box::new(DrumUnit::new()),
            OscillatorType::Noise => Box::new(NoiseHoldUnit::new()),
        }
    }

    pub(crate) fn sample(state: &mut FundspState, key: (u64, usize), osc: &AudioOscillator) -> f32 {
        if !osc.enabled {
            return 0.0;
        }

        let voice = state.voices.entry(key).or_insert_with(|| {
            let mut unit = make_unit(osc.oscillator_type);
            unit.set_sample_rate(state.sample_rate_hz);
            Voice {
                unit,
                last_type: osc.oscillator_type,
            }
        });

        if voice.last_type != osc.oscillator_type {
            let mut unit = make_unit(osc.oscillator_type);
            unit.set_sample_rate(state.sample_rate_hz);
            voice.unit = unit;
            voice.last_type = osc.oscillator_type;
        }

        let hz = sanitize_hz(osc.frequency);
        let mut out = [0.0f32];
        voice.unit.tick(&[hz], &mut out);
        out[0]
    }

    #[derive(Clone)]
    struct Square3Unit {
        phase: f32,
        sample_rate_hz: f32,
    }

    impl Square3Unit {
        fn new() -> Self {
            Self {
                phase: 0.0,
                sample_rate_hz: fundsp::DEFAULT_SR as f32,
            }
        }
    }

    impl AudioUnit for Square3Unit {
        fn reset(&mut self) {
            self.phase = 0.0;
        }

        fn set_sample_rate(&mut self, sample_rate: f64) {
            self.sample_rate_hz = (sample_rate as f32).max(1.0);
        }

        fn tick(&mut self, input: &[f32], output: &mut [f32]) {
            let hz = input.get(0).copied().unwrap_or(0.0);
            let hz = if hz.is_finite() { hz.max(0.0) } else { 0.0 };

            let t = std::f32::consts::TAU * self.phase;
            let s = t.sin() + (3.0 * t).sin() / 3.0 + (5.0 * t).sin() / 5.0;
            let v = s * (4.0 / std::f32::consts::PI);

            let inc = hz / self.sample_rate_hz;
            self.phase += inc;
            if self.phase >= 1.0 {
                self.phase -= self.phase.floor();
            }

            output[0] = v;
        }

        fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
            let in_hz = if input.channels() > 0 {
                input.channel_f32(0)
            } else {
                &ZERO_BLOCK
            };
            let out = output.channel_f32_mut(0);
            for i in 0..size {
                let mut y = [0.0f32];
                self.tick(&[in_hz[i]], &mut y);
                out[i] = y[0];
            }
            for i in size..fundsp::MAX_BUFFER_SIZE {
                out[i] = 0.0;
            }
            let _ = simd_items(size);
        }

        fn inputs(&self) -> usize {
            1
        }

        fn outputs(&self) -> usize {
            1
        }

        fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
            let mut signal = SignalFrame::new(1);
            signal.fill(Signal::Unknown);
            signal
        }

        fn get_id(&self) -> u64 {
            const ID: u64 = 10_001;
            ID
        }

        fn footprint(&self) -> usize {
            core::mem::size_of::<Self>()
        }
    }

    #[derive(Clone)]
    struct DrumUnit {
        phase: f32,
        t_sec: f32,
        sample_rate_hz: f32,
    }

    impl DrumUnit {
        fn new() -> Self {
            Self {
                phase: 0.0,
                t_sec: 0.0,
                sample_rate_hz: fundsp::DEFAULT_SR as f32,
            }
        }
    }

    impl AudioUnit for DrumUnit {
        fn reset(&mut self) {
            self.phase = 0.0;
            self.t_sec = 0.0;
        }

        fn set_sample_rate(&mut self, sample_rate: f64) {
            self.sample_rate_hz = (sample_rate as f32).max(1.0);
        }

        fn tick(&mut self, input: &[f32], output: &mut [f32]) {
            let pitch_hz = input.get(0).copied().unwrap_or(0.0);

            const C2_HZ: f32 = 65.406_39;
            let pitch_scale = if pitch_hz.is_finite() && pitch_hz > 0.0 {
                (pitch_hz / C2_HZ).max(0.01)
            } else {
                1.0
            };

            let f_start = 2000.0 * pitch_scale;
            let f_end = 20.0 * pitch_scale;
            let tau_sec = 0.05_f32;
            let freq = f_end + (f_start - f_end) * (-self.t_sec / tau_sec).exp();

            let v = (std::f32::consts::TAU * self.phase).sin();

            let inv_sr = 1.0 / self.sample_rate_hz;
            self.t_sec += inv_sr;
            self.phase += (freq * inv_sr).max(0.0);
            if self.phase >= 1.0 {
                self.phase -= self.phase.floor();
            }

            output[0] = v;
        }

        fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
            let in_pitch = if input.channels() > 0 {
                input.channel_f32(0)
            } else {
                &ZERO_BLOCK
            };
            let out = output.channel_f32_mut(0);
            for i in 0..size {
                let mut y = [0.0f32];
                self.tick(&[in_pitch[i]], &mut y);
                out[i] = y[0];
            }
            for i in size..fundsp::MAX_BUFFER_SIZE {
                out[i] = 0.0;
            }
            let _ = simd_items(size);
        }

        fn inputs(&self) -> usize {
            1
        }

        fn outputs(&self) -> usize {
            1
        }

        fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
            let mut signal = SignalFrame::new(1);
            signal.fill(Signal::Unknown);
            signal
        }

        fn get_id(&self) -> u64 {
            const ID: u64 = 10_002;
            ID
        }

        fn footprint(&self) -> usize {
            core::mem::size_of::<Self>()
        }
    }

    fn next_noise_sample(state: &mut u32) -> f32 {
        *state ^= *state << 13;
        *state ^= *state >> 17;
        *state ^= *state << 5;

        let u = (*state as f32) / (u32::MAX as f32);
        u * 2.0 - 1.0
    }

    #[derive(Clone)]
    struct NoiseHoldUnit {
        phase: f32,
        sample_rate_hz: f32,
        rng: u32,
        held: f32,
    }

    impl NoiseHoldUnit {
        fn new() -> Self {
            let mut rng = 0x1234_5678u32;
            let held = next_noise_sample(&mut rng);
            Self {
                phase: 0.0,
                sample_rate_hz: fundsp::DEFAULT_SR as f32,
                rng,
                held,
            }
        }
    }

    impl AudioUnit for NoiseHoldUnit {
        fn reset(&mut self) {
            self.phase = 0.0;
            self.held = next_noise_sample(&mut self.rng);
        }

        fn set_sample_rate(&mut self, sample_rate: f64) {
            self.sample_rate_hz = (sample_rate as f32).max(1.0);
        }

        fn tick(&mut self, input: &[f32], output: &mut [f32]) {
            let hz = input.get(0).copied().unwrap_or(0.0);
            let hz = if hz.is_finite() { hz.max(0.0) } else { 0.0 };

            let inc = hz / self.sample_rate_hz;
            let v = self.held;

            if inc > 0.0 && self.phase + inc >= 1.0 {
                self.held = next_noise_sample(&mut self.rng);
            }

            self.phase += inc;
            if self.phase >= 1.0 {
                self.phase -= self.phase.floor();
            }

            output[0] = v;
        }

        fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
            let in_hz = if input.channels() > 0 {
                input.channel_f32(0)
            } else {
                &ZERO_BLOCK
            };
            let out = output.channel_f32_mut(0);
            for i in 0..size {
                let mut y = [0.0f32];
                self.tick(&[in_hz[i]], &mut y);
                out[i] = y[0];
            }
            for i in size..fundsp::MAX_BUFFER_SIZE {
                out[i] = 0.0;
            }
            let _ = simd_items(size);
        }

        fn inputs(&self) -> usize {
            1
        }

        fn outputs(&self) -> usize {
            1
        }

        fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
            let mut signal = SignalFrame::new(1);
            signal.fill(Signal::Unknown);
            signal
        }

        fn get_id(&self) -> u64 {
            const ID: u64 = 10_003;
            ID
        }

        fn footprint(&self) -> usize {
            core::mem::size_of::<Self>()
        }
    }
}

/// PCM-backed asset usable from the audio thread. Stored interleaved
/// frame-major. Phase 5: filled by `AudioQueueItem::UploadClip` after the
/// decode worker hands back converted PCM.
#[derive(Debug, Clone)]
pub(crate) struct RtClipAsset {
    pub samples: Arc<Vec<f32>>,
    pub channels: u16,
    pub sample_rate: u32,
}

/// Per-clip playback state on the audio thread. One per registered
/// `AudioClipComponent`, keyed by `component_ffi`.
#[derive(Debug, Clone, Default)]
pub(crate) struct RtClipInstance {
    /// Sample-frame cursor. f64 for sub-sample rate playback later;
    /// phase 5 advances at 1.0/frame because clips are resampled to the
    /// engine sample rate at load time.
    pub cursor: f64,
    pub playing: bool,
}

#[derive(Debug, Default)]
pub(crate) struct SynthRtState {
    pub(crate) osc_snapshot: HashMap<u64, Vec<AudioOscillator>>,
    pub(crate) component_gain: HashMap<u64, f32>,
    pub(crate) component_gate: HashMap<u64, ComponentGate>,
    pub(crate) graphs: HashMap<u64, RtAudioGraph>,
    pub(crate) fundsp: fundsp_backend::FundspState,

    /// Asset registry (AudioAssets — docs/draft/audio_decoding_thread.md §1.1).
    pub(crate) clip_assets: HashMap<u64, RtClipAsset>,
    pub(crate) clip_instances: HashMap<u64, RtClipInstance>,

    // Debug-only tracing for audio ops from the RT thread.
    pub(crate) trace_lp_ops_inited: bool,
    pub(crate) trace_lp_ops: bool,
    pub(crate) trace_lp_ops_count: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ComponentGate {
    current: f32,
    target: f32,
    step: f32,
    remaining: u32,
    pending_disable: bool,
}

impl Default for ComponentGate {
    fn default() -> Self {
        Self {
            current: 1.0,
            target: 1.0,
            step: 0.0,
            remaining: 0,
            pending_disable: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RtAudioGraphNodeKind {
    OscillatorSource,
    ClipSource,
    Gain {
        gain: f32,
    },
    LowPass {
        cutoff_hz: f32,
        resonance: f32,
    },
    BandPass {
        center_hz: f32,
        bandwidth_octaves: f32,
        resonance: f32,
    },
    HighPass {
        cutoff_hz: f32,
        resonance: f32,
    },
    Limiter,
}

#[derive(Debug, Clone, Copy)]
pub struct RtAudioGraphNodeState {
    // Low-pass state.
    lp_z1: f32,

    // High-pass state.
    hp_y1: f32,
    hp_x1: f32,

    // Limiter envelope.
    lim_env: f32,

    // Limiter parameters.
    pub(crate) limiter_attack_ms: f32,
    pub(crate) limiter_release_ms: f32,
    pub(crate) limiter_threshold: f32,
}

impl Default for RtAudioGraphNodeState {
    fn default() -> Self {
        Self {
            lp_z1: 0.0,
            hp_y1: 0.0,
            hp_x1: 0.0,
            lim_env: 0.0,
            limiter_attack_ms: 1.0,
            limiter_release_ms: 50.0,
            limiter_threshold: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RtAudioGraphChild {
    pub idx: u8,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct RtAudioGraphNode {
    pub component_ffi: u64,
    pub kind: RtAudioGraphNodeKind,
    pub state: RtAudioGraphNodeState,
    pub children: HVec<RtAudioGraphChild, MAX_AUDIO_GRAPH_CHILDREN_PER_NODE>,
}

#[derive(Debug, Clone)]
pub struct RtAudioGraph {
    pub root_component_ffi: u64,
    pub nodes: HVec<RtAudioGraphNode, MAX_AUDIO_GRAPH_NODES>,
}

#[derive(Debug, Clone)]
pub struct ScheduledGraphOp {
    pub beat: f64,
    pub target_component_ffi: u64,
    pub graph: RtAudioGraph,
}

impl ComponentGate {
    fn ramp_to(&mut self, target: f32, samples: u32, pending_disable: bool) {
        let target = if target.is_finite() {
            target.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if samples == 0 {
            self.current = target;
            self.target = target;
            self.step = 0.0;
            self.remaining = 0;
            self.pending_disable = pending_disable;
            return;
        }

        self.target = target;
        self.step = (target - self.current) / (samples as f32);
        self.remaining = samples;
        self.pending_disable = pending_disable;
    }

    fn tick(&mut self) {
        if self.remaining == 0 {
            return;
        }
        self.current = (self.current + self.step).clamp(0.0, 1.0);
        self.remaining -= 1;
        if self.remaining == 0 {
            self.current = self.target;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AudioOp {
    SetHz(f32),
    /// Set a per-component gain multiplier (velocity).
    ///
    /// This is applied in addition to each oscillator's own `amplitude`.
    SetGain(f32),
    /// Update a compiled RT graph low-pass cutoff in-place.
    ///
    /// Target is the `AudioLowPassFilterComponent` ComponentId (as FFI u64).
    SetLowPassCutoffHz(f32),
    /// Update a compiled RT graph band-pass center frequency in-place.
    ///
    /// Target is the `AudioBandPassFilterComponent` ComponentId (as FFI u64).
    SetBandPassCenterHz(f32),
    SetEnabled(bool),
}

#[derive(Debug, Clone, Copy)]
pub struct ScheduledAudioOp {
    pub beat: f64,
    pub target_component_ffi: u64,
    pub op: AudioOp,
}

#[derive(Debug, Clone)]
pub enum AudioQueueItem {
    SetTransport {
        bpm: f64,
        beat_offset: f64,
    },
    ReplaceOscillators {
        target_component_ffi: u64,
        oscillators: HVec<AudioOscillator, MAX_OSCS_PER_COMPONENT>,
    },
    Message(ScheduledAudioOp),
    GraphMessage(ScheduledGraphOp),
    /// Upload a decoded clip asset. Sent by the main thread after the
    /// decode worker reports completion. `samples` is the engine-format
    /// PCM (interleaved frame-major).
    UploadClip {
        clip_id: u64,
        samples: Arc<Vec<f32>>,
        channels: u16,
        sample_rate: u32,
    },
    /// Drop a previously uploaded clip asset.
    DropClip {
        clip_id: u64,
    },
}

#[derive(Debug, Default)]
pub(crate) struct AudioRtLocalState {
    pub(crate) bpm: f64,
    pub(crate) beat_offset: f64,

    pub(crate) events: Vec<ScheduledAudioOp>,
    pub(crate) graph_events: Vec<ScheduledGraphOp>,

    pub(crate) due_ops: Vec<(usize, u64, AudioOp)>,
    pub(crate) due_graph_ops: Vec<(usize, u64, RtAudioGraph)>,
}

#[derive(Debug)]
pub(crate) struct AudioClockState {
    pub(crate) sample_rate_hz: u32,
    pub(crate) frames_played: AtomicU64,
}

fn op_priority(op: AudioOp) -> u8 {
    // Same-beat ordering: previous-note gate-off first, then parameter
    // updates, then new-note gate-on last. This keeps back-to-back notes
    // (where one note's stop coincides with the next note's start) from
    // having the new gate-on cancelled by the old gate-off. See
    // docs/spec/audio-sources.md §4 (per-variant trigger semantics).
    match op {
        AudioOp::SetEnabled(false) => 0,
        AudioOp::SetHz(_) => 1,
        AudioOp::SetGain(_) => 1,
        AudioOp::SetLowPassCutoffHz(_) => 1,
        AudioOp::SetBandPassCenterHz(_) => 1,
        AudioOp::SetEnabled(true) => 2,
    }
}

fn apply_audio_op(oscs: &mut [AudioOscillator], op: AudioOp) {
    match op {
        AudioOp::SetHz(hz) => {
            for o in oscs.iter_mut() {
                o.frequency = hz;
            }
        }
        AudioOp::SetGain(_) => {}
        AudioOp::SetLowPassCutoffHz(_) => {}
        AudioOp::SetBandPassCenterHz(_) => {}
        AudioOp::SetEnabled(enabled) => {
            for o in oscs.iter_mut() {
                o.enabled = enabled;
            }
        }
    }
}

fn sanitize_param_f32(v: f32, default: f32) -> f32 {
    if v.is_finite() { v } else { default }
}

fn one_pole_lowpass(x: f32, cutoff_hz: f32, sample_rate_hz: f32, z1: &mut f32) -> f32 {
    let cutoff_hz = sanitize_param_f32(cutoff_hz, 0.0).max(0.0);
    if cutoff_hz <= 0.0 {
        *z1 = 0.0;
        return 0.0;
    }

    // a = exp(-2*pi*fc/sr)
    let sr = sample_rate_hz.max(1.0);
    let a = (-std::f32::consts::TAU * cutoff_hz / sr).exp();
    let y = (1.0 - a) * x + a * (*z1);
    *z1 = y;
    y
}

fn one_pole_highpass(
    x: f32,
    cutoff_hz: f32,
    sample_rate_hz: f32,
    y1: &mut f32,
    x1: &mut f32,
) -> f32 {
    let cutoff_hz = sanitize_param_f32(cutoff_hz, 0.0).max(0.0);
    if cutoff_hz <= 0.0 {
        *y1 = 0.0;
        *x1 = 0.0;
        return x;
    }

    let sr = sample_rate_hz.max(1.0);
    let dt = 1.0 / sr;
    let rc = 1.0 / (std::f32::consts::TAU * cutoff_hz);
    let a = rc / (rc + dt);

    let y = a * (*y1 + x - *x1);
    *y1 = y;
    *x1 = x;
    y
}

fn limiter(
    x: f32,
    attack_ms: f32,
    release_ms: f32,
    threshold: f32,
    sample_rate_hz: f32,
    env: &mut f32,
) -> f32 {
    let threshold = sanitize_param_f32(threshold, 1.0).max(0.000_1);
    let attack_ms = sanitize_param_f32(attack_ms, 1.0).max(0.1);
    let release_ms = sanitize_param_f32(release_ms, 50.0).max(0.1);

    let sr = sample_rate_hz.max(1.0);
    let attack_s = attack_ms / 1000.0;
    let release_s = release_ms / 1000.0;
    let a_a = (-1.0 / (attack_s * sr)).exp();
    let a_r = (-1.0 / (release_s * sr)).exp();

    let x_abs = x.abs();
    if x_abs > *env {
        *env = a_a * (*env) + (1.0 - a_a) * x_abs;
    } else {
        *env = a_r * (*env) + (1.0 - a_r) * x_abs;
    }

    let env = env.max(threshold);
    let g = (threshold / env).min(1.0);
    x * g
}

fn process_graph_node(
    graph: &mut RtAudioGraph,
    node_idx: u8,
    input: f32,
    sample_rate_hz: f32,
    depth: u32,
) -> f32 {
    if depth > 64 {
        return input;
    }

    let idx = node_idx as usize;
    if idx >= graph.nodes.len() {
        return input;
    }

    let (kind, children_len) = {
        let node = &graph.nodes[idx];
        (node.kind, node.children.len())
    };

    let mut y = match kind {
        RtAudioGraphNodeKind::OscillatorSource => input,
        // ClipSource is a pass-through: the per-clip mix already computed
        // `input` for this frame (see render_sample_from_clips).
        RtAudioGraphNodeKind::ClipSource => input,
        RtAudioGraphNodeKind::Gain { gain } => input * sanitize_param_f32(gain, 1.0),
        RtAudioGraphNodeKind::LowPass {
            cutoff_hz,
            resonance: _,
        } => {
            let z1 = &mut graph.nodes[idx].state.lp_z1;
            one_pole_lowpass(input, cutoff_hz, sample_rate_hz, z1)
        }
        RtAudioGraphNodeKind::BandPass {
            center_hz,
            bandwidth_octaves,
            resonance: _,
        } => {
            let center_hz = sanitize_param_f32(center_hz, 0.0).max(0.0);
            let bandwidth_octaves = sanitize_param_f32(bandwidth_octaves, 1.0).clamp(0.01, 8.0);

            // Convert an octave bandwidth into low/high cutoff frequencies.
            // width = 2^(bw/2)
            let width = 2.0f32.powf(0.5 * bandwidth_octaves);
            let low_cutoff_hz = (center_hz / width).max(0.0);
            let high_cutoff_hz = (center_hz * width).max(low_cutoff_hz);

            let (hp_y1, hp_x1, lp_z1) = {
                let st = &mut graph.nodes[idx].state;
                (&mut st.hp_y1, &mut st.hp_x1, &mut st.lp_z1)
            };
            let hp = one_pole_highpass(input, low_cutoff_hz, sample_rate_hz, hp_y1, hp_x1);
            one_pole_lowpass(hp, high_cutoff_hz, sample_rate_hz, lp_z1)
        }
        RtAudioGraphNodeKind::HighPass {
            cutoff_hz,
            resonance: _,
        } => {
            let (y1, x1) = {
                let st = &mut graph.nodes[idx].state;
                (&mut st.hp_y1, &mut st.hp_x1)
            };
            one_pole_highpass(input, cutoff_hz, sample_rate_hz, y1, x1)
        }
        RtAudioGraphNodeKind::Limiter => {
            let (attack_ms, release_ms, threshold, env) = {
                let st = &mut graph.nodes[idx].state;
                (
                    st.limiter_attack_ms,
                    st.limiter_release_ms,
                    st.limiter_threshold,
                    &mut st.lim_env,
                )
            };
            limiter(input, attack_ms, release_ms, threshold, sample_rate_hz, env)
        }
    };

    if children_len == 0 {
        return y;
    }

    let mut sum = 0.0f32;
    // Clone child list out to avoid borrow issues while recursing/mutating state.
    let children: HVec<RtAudioGraphChild, MAX_AUDIO_GRAPH_CHILDREN_PER_NODE> =
        graph.nodes[idx].children.clone();
    for ch in children.iter() {
        let w = if ch.weight.is_finite() {
            ch.weight
        } else {
            1.0
        };
        sum += process_graph_node(graph, ch.idx, y, sample_rate_hz, depth + 1) * w;
    }
    y = sum;

    y
}

fn render_sample_from_map(
    map: &mut HashMap<u64, Vec<AudioOscillator>>,
    gains: &HashMap<u64, f32>,
    gates: &mut HashMap<u64, ComponentGate>,
    graphs: &mut HashMap<u64, RtAudioGraph>,
    fundsp: &mut fundsp_backend::FundspState,
    sample_rate_hz: u32,
) -> f32 {
    render_sample_from_map_unclamped(map, gains, gates, graphs, fundsp, sample_rate_hz)
        .clamp(-1.0, 1.0)
}

/// Same as `render_sample_from_map` but returns the raw sum without
/// clamping. Used when mixing additional sources (e.g. clips) before the
/// final clamp.
fn render_sample_from_map_unclamped(
    map: &mut HashMap<u64, Vec<AudioOscillator>>,
    gains: &HashMap<u64, f32>,
    gates: &mut HashMap<u64, ComponentGate>,
    graphs: &mut HashMap<u64, RtAudioGraph>,
    fundsp: &mut fundsp_backend::FundspState,
    sample_rate_hz: u32,
) -> f32 {
    let mut out = 0.0f32;
    for (&cid_ffi, oscs) in map.iter_mut() {
        let base_g = gains.get(&cid_ffi).copied().unwrap_or(1.0);
        let base_g = if base_g.is_finite() {
            base_g.max(0.0)
        } else {
            1.0
        };

        let gate = gates.entry(cid_ffi).or_default();
        gate.tick();

        if gate.pending_disable && gate.remaining == 0 && gate.current <= 0.0 {
            for osc in oscs.iter_mut() {
                osc.enabled = false;
            }
            gate.pending_disable = false;
        }

        let g = base_g * gate.current;
        if g <= 0.0 {
            continue;
        }

        let mut base = 0.0f32;
        for (idx, osc) in oscs.iter().enumerate() {
            if !osc.enabled {
                continue;
            }
            let v = fundsp_backend::sample(fundsp, (cid_ffi, idx), osc);
            base += v * osc.amplitude;
        }
        base *= g;

        if let Some(graph) = graphs.get_mut(&cid_ffi) {
            // Graph nodes are precompiled; node 0 is the oscillator root.
            let sr = (sample_rate_hz as f32).max(1.0);
            out += process_graph_node(graph, 0, base, sr, 0);
        } else {
            out += base;
        }
    }
    out
}

/// Mix one sample frame's contribution from every playing
/// `AudioClipComponent`. Mirrors `render_sample_from_map_unclamped` for
/// oscillators but reads from the asset registry instead of running
/// generators. Effect chains apply the same way (graphs keyed by the
/// clip's component_ffi).
fn render_sample_from_clips(
    clip_instances: &mut HashMap<u64, RtClipInstance>,
    clip_assets: &HashMap<u64, RtClipAsset>,
    gains: &HashMap<u64, f32>,
    gates: &mut HashMap<u64, ComponentGate>,
    graphs: &mut HashMap<u64, RtAudioGraph>,
    sample_rate_hz: u32,
) -> f32 {
    let mut out = 0.0f32;
    for (&cid_ffi, voice) in clip_instances.iter_mut() {
        if !voice.playing {
            continue;
        }
        let Some(asset) = clip_assets.get(&cid_ffi) else {
            continue;
        };
        let ch = asset.channels.max(1) as usize;
        let frame_count = asset.samples.len() / ch;
        if frame_count == 0 {
            voice.playing = false;
            continue;
        }

        let base_g = gains.get(&cid_ffi).copied().unwrap_or(1.0);
        let base_g = if base_g.is_finite() {
            base_g.max(0.0)
        } else {
            1.0
        };
        let gate = gates.entry(cid_ffi).or_default();
        gate.tick();
        if gate.pending_disable && gate.remaining == 0 && gate.current <= 0.0 {
            gate.pending_disable = false;
        }
        let g = base_g * gate.current;

        // Read one frame; advance cursor regardless of gain so paused
        // clips don't suddenly skip ahead when re-enabled mid-stream.
        let frame_idx = voice.cursor as usize;
        if frame_idx >= frame_count {
            voice.playing = false;
            continue;
        }
        let base_offset = frame_idx * ch;
        let mut acc = 0.0f32;
        for c in 0..ch {
            acc += asset.samples[base_offset + c];
        }
        let base = (acc / ch as f32) * g;
        voice.cursor += 1.0;

        if let Some(graph) = graphs.get_mut(&cid_ffi) {
            let sr = (sample_rate_hz as f32).max(1.0);
            out += process_graph_node(graph, 0, base, sr, 0);
        } else {
            out += base;
        }
    }
    out
}

pub(crate) fn render_buffer<T: Copy>(
    data: &mut [T],
    channels: u64,
    sample_rate_hz: u32,
    _sample_rate_hz_f32: f32,
    state_for_cb: &Arc<AudioClockState>,
    rx: &mut Consumer<AudioQueueItem>,
    rt: &mut AudioRtLocalState,
    synth_state: &mut SynthRtState,
    to_sample: fn(f32) -> T,
) {
    if !synth_state.trace_lp_ops_inited {
        synth_state.trace_lp_ops_inited = true;
        synth_state.trace_lp_ops = std::env::var("CAT_AUDIO_TRACE_LP")
            .ok()
            .map(|v| v != "0")
            .unwrap_or(false);
    }

    let ch = (channels.max(1) as usize).max(1);
    let frames_in_buf = (data.len() / ch).max(1) as u64;
    let base_frame = state_for_cb.frames_played.load(Ordering::Relaxed);

    synth_state
        .fundsp
        .set_sample_rate((sample_rate_hz as f64).max(1.0));

    while let Ok(item) = rx.pop() {
        match item {
            AudioQueueItem::SetTransport { bpm, beat_offset } => {
                if bpm.is_finite() && bpm > 0.0 {
                    rt.bpm = bpm;
                }
                if beat_offset.is_finite() {
                    rt.beat_offset = beat_offset;
                }
            }
            AudioQueueItem::ReplaceOscillators {
                target_component_ffi,
                oscillators,
            } => {
                let prev = synth_state.osc_snapshot.get(&target_component_ffi).cloned();
                let mut next: Vec<AudioOscillator> = Vec::with_capacity(oscillators.len());
                for o in oscillators.iter() {
                    next.push(o.clone());
                }
                let new_len = next.len();
                synth_state.osc_snapshot.insert(target_component_ffi, next);
                synth_state
                    .fundsp
                    .prune_component(target_component_ffi, new_len);

                if let Some(next_oscs) = synth_state.osc_snapshot.get(&target_component_ffi) {
                    for (idx, next_osc) in next_oscs.iter().enumerate() {
                        if !next_osc.enabled {
                            continue;
                        }
                        let was_enabled = prev
                            .as_ref()
                            .and_then(|v| v.get(idx))
                            .map(|o| o.enabled)
                            .unwrap_or(false);
                        if !was_enabled {
                            synth_state
                                .fundsp
                                .retrigger_voice((target_component_ffi, idx));
                        }
                    }
                }
            }
            AudioQueueItem::Message(op) => {
                if !op.beat.is_finite() {
                    continue;
                }

                let msg_pri = op_priority(op.op);
                let idx = rt
                    .events
                    .binary_search_by(|e| {
                        let Some(ord) = e.beat.partial_cmp(&op.beat) else {
                            return std::cmp::Ordering::Equal;
                        };
                        if ord != std::cmp::Ordering::Equal {
                            return ord;
                        }
                        op_priority(e.op).cmp(&msg_pri)
                    })
                    .unwrap_or_else(|i| i);
                rt.events.insert(idx, op);
            }
            AudioQueueItem::UploadClip {
                clip_id,
                samples,
                channels,
                sample_rate,
            } => {
                synth_state.clip_assets.insert(
                    clip_id,
                    RtClipAsset {
                        samples,
                        channels,
                        sample_rate,
                    },
                );
                synth_state
                    .clip_instances
                    .entry(clip_id)
                    .or_default();
            }
            AudioQueueItem::DropClip { clip_id } => {
                synth_state.clip_assets.remove(&clip_id);
                synth_state.clip_instances.remove(&clip_id);
            }
            AudioQueueItem::GraphMessage(op) => {
                if !op.beat.is_finite() {
                    continue;
                }

                let idx = rt
                    .graph_events
                    .binary_search_by(|e| {
                        let Some(ord) = e.beat.partial_cmp(&op.beat) else {
                            return std::cmp::Ordering::Equal;
                        };
                        if ord != std::cmp::Ordering::Equal {
                            return ord;
                        }
                        // Stable ordering for identical beat.
                        e.target_component_ffi.cmp(&op.target_component_ffi)
                    })
                    .unwrap_or_else(|i| i);
                rt.graph_events.insert(idx, op);
            }
        }
    }

    let bpm = if rt.bpm.is_finite() && rt.bpm > 0.0 {
        rt.bpm
    } else {
        120.0
    };
    let beats_per_frame = (bpm / 60.0) / (sample_rate_hz as f64).max(1.0);
    let beat_start = rt.beat_offset + (base_frame as f64) * beats_per_frame;
    let beat_end = beat_start + (frames_in_buf as f64) * beats_per_frame;

    rt.due_ops.clear();
    rt.due_graph_ops.clear();

    let mut split_idx = 0usize;
    while split_idx < rt.events.len() {
        if rt.events[split_idx].beat <= beat_end + 1e-12 {
            split_idx += 1;
        } else {
            break;
        }
    }
    if split_idx > 0 {
        for ev in rt.events.drain(0..split_idx) {
            let rel = (ev.beat - beat_start) / beats_per_frame;
            let idx = rel.round() as isize;
            let idx = idx.clamp(0, frames_in_buf as isize - 1) as usize;
            rt.due_ops.push((idx, ev.target_component_ffi, ev.op));
        }
        rt.due_ops.sort_by(|a, b| {
            let idx_ord = a.0.cmp(&b.0);
            if idx_ord != std::cmp::Ordering::Equal {
                return idx_ord;
            }
            op_priority(a.2).cmp(&op_priority(b.2))
        });
    }

    let mut graph_split_idx = 0usize;
    while graph_split_idx < rt.graph_events.len() {
        if rt.graph_events[graph_split_idx].beat <= beat_end + 1e-12 {
            graph_split_idx += 1;
        } else {
            break;
        }
    }
    if graph_split_idx > 0 {
        for ev in rt.graph_events.drain(0..graph_split_idx) {
            let rel = (ev.beat - beat_start) / beats_per_frame;
            let idx = rel.round() as isize;
            let idx = idx.clamp(0, frames_in_buf as isize - 1) as usize;
            rt.due_graph_ops
                .push((idx, ev.target_component_ffi, ev.graph));
        }
        rt.due_graph_ops.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let osc_snapshot = &mut synth_state.osc_snapshot;
    let gains = &mut synth_state.component_gain;
    let gates = &mut synth_state.component_gate;
    let graphs = &mut synth_state.graphs;
    let fundsp = &mut synth_state.fundsp;
    let clip_assets = &synth_state.clip_assets;
    let clip_instances = &mut synth_state.clip_instances;

    const ENABLE_RAMP_SEC: f32 = 0.005;
    const DISABLE_RAMP_SEC: f32 = 0.010;
    let enable_ramp_samples = ((sample_rate_hz as f32) * ENABLE_RAMP_SEC).round() as u32;
    let disable_ramp_samples = ((sample_rate_hz as f32) * DISABLE_RAMP_SEC).round() as u32;

    let mut op_cursor = 0usize;
    let mut graph_cursor = 0usize;
    for (frame_idx, frame) in data.chunks_mut(ch).enumerate() {
        while graph_cursor < rt.due_graph_ops.len() && rt.due_graph_ops[graph_cursor].0 == frame_idx
        {
            let (_idx, target, graph) = rt.due_graph_ops[graph_cursor].clone();
            graphs.insert(target, graph);

            if synth_state.trace_lp_ops {
                if let Some(g) = graphs.get(&target) {
                    let mut lp_ids: [u64; 4] = [0; 4];
                    let mut lp_n = 0usize;
                    for n in g.nodes.iter() {
                        if matches!(n.kind, RtAudioGraphNodeKind::LowPass { .. }) {
                            if lp_n < lp_ids.len() {
                                lp_ids[lp_n] = n.component_ffi;
                            }
                            lp_n += 1;
                        }
                    }
                    eprintln!(
                        "[audio-rt] GraphSwap target_root_ffi={target} nodes={} lowpass_nodes={} lowpass_ids(first4)={:?}",
                        g.nodes.len(),
                        lp_n,
                        lp_ids
                    );
                }
            }

            graph_cursor += 1;
        }

        while op_cursor < rt.due_ops.len() && rt.due_ops[op_cursor].0 == frame_idx {
            let (_idx, target, op) = rt.due_ops[op_cursor];
            match op {
                AudioOp::SetLowPassCutoffHz(cutoff_hz) => {
                    let cutoff_hz = if cutoff_hz.is_finite() {
                        cutoff_hz.max(0.0)
                    } else {
                        0.0
                    };

                    // Apply to any RT graph node that corresponds to this component.
                    let mut matched = 0usize;
                    for graph in graphs.values_mut() {
                        for node in graph.nodes.iter_mut() {
                            if node.component_ffi != target {
                                continue;
                            }

                            if let RtAudioGraphNodeKind::LowPass { resonance, .. } = node.kind {
                                node.kind = RtAudioGraphNodeKind::LowPass {
                                    cutoff_hz,
                                    resonance,
                                };
                                matched += 1;
                            }
                        }
                    }

                    if synth_state.trace_lp_ops {
                        synth_state.trace_lp_ops_count =
                            synth_state.trace_lp_ops_count.wrapping_add(1);
                        let n = synth_state.trace_lp_ops_count;

                        // Rate limit: first 10 ops + then every 256th.
                        if (matched == 0 && n <= 32) || n <= 10 || (n & 0xFF) == 0 {
                            if matched == 0 {
                                let mut any_lp_ids: [u64; 8] = [0; 8];
                                let mut any_lp_n = 0usize;
                                for graph in graphs.values() {
                                    for node in graph.nodes.iter() {
                                        if matches!(node.kind, RtAudioGraphNodeKind::LowPass { .. })
                                        {
                                            if any_lp_n < any_lp_ids.len() {
                                                any_lp_ids[any_lp_n] = node.component_ffi;
                                            }
                                            any_lp_n += 1;
                                        }
                                    }
                                }
                                eprintln!(
                                    "[audio-rt]   available_lowpass_nodes={} lowpass_ids(first8)={:?} graphs={} ",
                                    any_lp_n,
                                    any_lp_ids,
                                    graphs.len(),
                                );
                            }
                            eprintln!(
                                "[audio-rt] SetLowPassCutoffHz target_ffi={target} cutoff_hz={cutoff_hz:.2} matched_nodes={matched} (count={n})"
                            );
                        }
                    }
                }
                AudioOp::SetBandPassCenterHz(center_hz) => {
                    let center_hz = if center_hz.is_finite() {
                        center_hz.max(0.0)
                    } else {
                        0.0
                    };

                    let mut matched = 0usize;
                    for graph in graphs.values_mut() {
                        for node in graph.nodes.iter_mut() {
                            if node.component_ffi != target {
                                continue;
                            }

                            if let RtAudioGraphNodeKind::BandPass {
                                bandwidth_octaves,
                                resonance,
                                ..
                            } = node.kind
                            {
                                node.kind = RtAudioGraphNodeKind::BandPass {
                                    center_hz,
                                    bandwidth_octaves,
                                    resonance,
                                };
                                matched += 1;
                            }
                        }
                    }

                    if synth_state.trace_lp_ops {
                        synth_state.trace_lp_ops_count =
                            synth_state.trace_lp_ops_count.wrapping_add(1);
                        let n = synth_state.trace_lp_ops_count;
                        if (matched == 0 && n <= 32) || n <= 10 || (n & 0xFF) == 0 {
                            eprintln!(
                                "[audio-rt] SetBandPassCenterHz target_ffi={target} center_hz={center_hz:.2} matched_nodes={matched} (count={n})"
                            );
                        }
                    }
                }
                _ => {
                    if let Some(oscs) = osc_snapshot.get_mut(&target) {
                        match op {
                            AudioOp::SetEnabled(true) => {
                                for (idx, osc) in oscs.iter_mut().enumerate() {
                                    osc.enabled = true;
                                    fundsp.retrigger_voice((target, idx));
                                }

                                let gate = gates.entry(target).or_default();
                                gate.current = 0.0;
                                gate.ramp_to(1.0, enable_ramp_samples.max(1), false);
                            }
                            AudioOp::SetEnabled(false) => {
                                let gate = gates.entry(target).or_default();
                                gate.ramp_to(0.0, disable_ramp_samples.max(1), true);
                            }
                            AudioOp::SetHz(_) => apply_audio_op(oscs, op),
                            AudioOp::SetGain(g) => {
                                let g = if g.is_finite() { g.max(0.0) } else { 1.0 };
                                gains.insert(target, g);
                            }
                            AudioOp::SetLowPassCutoffHz(_) => {
                                // Handled above.
                            }
                            AudioOp::SetBandPassCenterHz(_) => {
                                // Handled above.
                            }
                        }
                    } else if let Some(voice) = clip_instances.get_mut(&target) {
                        match op {
                            AudioOp::SetEnabled(true) => {
                                // Spec §4: clip trigger resets cursor to 0.
                                voice.cursor = 0.0;
                                voice.playing = true;
                                let gate = gates.entry(target).or_default();
                                gate.current = 0.0;
                                gate.ramp_to(1.0, enable_ramp_samples.max(1), false);
                            }
                            AudioOp::SetEnabled(false) => {
                                let gate = gates.entry(target).or_default();
                                gate.ramp_to(0.0, disable_ramp_samples.max(1), true);
                                voice.playing = false;
                            }
                            AudioOp::SetGain(g) => {
                                let g = if g.is_finite() { g.max(0.0) } else { 1.0 };
                                gains.insert(target, g);
                            }
                            _ => {}
                        }
                    }
                }
            }
            op_cursor += 1;
        }

        let osc_part = render_sample_from_map_unclamped(
            osc_snapshot,
            &*gains,
            gates,
            graphs,
            fundsp,
            sample_rate_hz,
        );
        let clip_part = render_sample_from_clips(
            clip_instances,
            clip_assets,
            &*gains,
            gates,
            graphs,
            sample_rate_hz,
        );
        let s = (osc_part + clip_part).clamp(-1.0, 1.0);
        let t = to_sample(s);
        for v in frame.iter_mut() {
            *v = t;
        }
    }

    state_for_cb
        .frames_played
        .fetch_add((data.len() as u64) / channels.max(1), Ordering::Relaxed);
}
