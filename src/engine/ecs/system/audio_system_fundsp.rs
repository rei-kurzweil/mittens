use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use heapless::spsc::Consumer;
use heapless::Vec as HVec;

use crate::engine::ecs::component::AudioOscillator;

pub const AUDIO_QUEUE_CAP: usize = 1024;
pub const MAX_OSCS_PER_COMPONENT: usize = 16;

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
        if hz.is_finite() {
            hz.max(0.0)
        } else {
            0.0
        }
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

#[derive(Debug, Default)]
pub(crate) struct SynthRtState {
    pub(crate) osc_snapshot: HashMap<u64, Vec<AudioOscillator>>,
    pub(crate) component_gain: HashMap<u64, f32>,
    pub(crate) component_gate: HashMap<u64, ComponentGate>,
    pub(crate) fundsp: fundsp_backend::FundspState,
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

impl ComponentGate {
    fn ramp_to(&mut self, target: f32, samples: u32, pending_disable: bool) {
        let target = if target.is_finite() { target.clamp(0.0, 1.0) } else { 1.0 };
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
    SetTransport { bpm: f64, beat_offset: f64 },
    ReplaceOscillators {
        target_component_ffi: u64,
        oscillators: HVec<AudioOscillator, MAX_OSCS_PER_COMPONENT>,
    },
    Message(ScheduledAudioOp),
}

#[derive(Debug, Default)]
pub(crate) struct AudioRtLocalState {
    pub(crate) bpm: f64,
    pub(crate) beat_offset: f64,

    pub(crate) events: Vec<ScheduledAudioOp>,

    pub(crate) due_ops: Vec<(usize, u64, AudioOp)>,
}

#[derive(Debug)]
pub(crate) struct AudioClockState {
    pub(crate) sample_rate_hz: u32,
    pub(crate) frames_played: AtomicU64,
}

fn op_priority(op: AudioOp) -> u8 {
    match op {
        AudioOp::SetEnabled(true) => 0,
        AudioOp::SetHz(_) => 1,
        AudioOp::SetGain(_) => 1,
        AudioOp::SetEnabled(false) => 2,
    }
}

fn apply_audio_op(oscs: &mut [AudioOscillator], op: AudioOp) {
    match op {
        AudioOp::SetHz(hz) => {
            for o in oscs.iter_mut() {
                o.frequency = hz;
                o.music_note_applied = true;
            }
        }
        AudioOp::SetGain(_) => {}
        AudioOp::SetEnabled(enabled) => {
            for o in oscs.iter_mut() {
                o.enabled = enabled;
            }
        }
    }
}

fn render_sample_from_map(
    map: &mut HashMap<u64, Vec<AudioOscillator>>,
    gains: &HashMap<u64, f32>,
    gates: &mut HashMap<u64, ComponentGate>,
    fundsp: &mut fundsp_backend::FundspState,
) -> f32 {
    let mut out = 0.0f32;
    for (&cid_ffi, oscs) in map.iter_mut() {
        let base_g = gains.get(&cid_ffi).copied().unwrap_or(1.0);
        let base_g = if base_g.is_finite() { base_g.max(0.0) } else { 1.0 };

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

        for (idx, osc) in oscs.iter().enumerate() {
            if !osc.enabled {
                continue;
            }
            let v = fundsp_backend::sample(fundsp, (cid_ffi, idx), osc);
            out += v * osc.amplitude * g;
        }
    }
    out.clamp(-1.0, 1.0)
}

pub(crate) fn render_buffer<T: Copy>(
    data: &mut [T],
    channels: u64,
    sample_rate_hz: u32,
    _sample_rate_hz_f32: f32,
    state_for_cb: &Arc<AudioClockState>,
    rx: &mut Consumer<'static, AudioQueueItem, AUDIO_QUEUE_CAP>,
    rt: &mut AudioRtLocalState,
    synth_state: &mut SynthRtState,
    to_sample: fn(f32) -> T,
) {
    let ch = (channels.max(1) as usize).max(1);
    let frames_in_buf = (data.len() / ch).max(1) as u64;
    let base_frame = state_for_cb.frames_played.load(Ordering::Relaxed);

    synth_state
        .fundsp
        .set_sample_rate((sample_rate_hz as f64).max(1.0));

    while let Some(item) = rx.dequeue() {
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
                synth_state.fundsp.prune_component(target_component_ffi, new_len);

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
                            synth_state.fundsp.retrigger_voice((target_component_ffi, idx));
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
        }
    }

    let bpm = if rt.bpm.is_finite() && rt.bpm > 0.0 { rt.bpm } else { 120.0 };
    let beats_per_frame = (bpm / 60.0) / (sample_rate_hz as f64).max(1.0);
    let beat_start = rt.beat_offset + (base_frame as f64) * beats_per_frame;
    let beat_end = beat_start + (frames_in_buf as f64) * beats_per_frame;

    rt.due_ops.clear();

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

    let osc_snapshot = &mut synth_state.osc_snapshot;
    let gains = &mut synth_state.component_gain;
    let gates = &mut synth_state.component_gate;
    let fundsp = &mut synth_state.fundsp;

    const ENABLE_RAMP_SEC: f32 = 0.005;
    const DISABLE_RAMP_SEC: f32 = 0.010;
    let enable_ramp_samples = ((sample_rate_hz as f32) * ENABLE_RAMP_SEC).round() as u32;
    let disable_ramp_samples = ((sample_rate_hz as f32) * DISABLE_RAMP_SEC).round() as u32;

    let mut op_cursor = 0usize;
    for (frame_idx, frame) in data.chunks_mut(ch).enumerate() {
        while op_cursor < rt.due_ops.len() && rt.due_ops[op_cursor].0 == frame_idx {
            let (_idx, target, op) = rt.due_ops[op_cursor];
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
                }
            }
            op_cursor += 1;
        }

        let s = render_sample_from_map(osc_snapshot, &*gains, gates, fundsp);
        let t = to_sample(s);
        for v in frame.iter_mut() {
            *v = t;
        }
    }

    state_for_cb
        .frames_played
        .fetch_add((data.len() as u64) / channels.max(1), Ordering::Relaxed);
}
