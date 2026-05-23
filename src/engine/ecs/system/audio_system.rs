use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use slotmap::Key;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::engine::ecs::component::AudioBufferSizeComponent;
use crate::engine::ecs::component::AudioOscillator;
use crate::engine::ecs::component::AudioOscillatorComponent;
use crate::engine::ecs::component::AudioOutputComponent;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::clock_system::ClockDriver;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

use crate::engine::ecs::system::audio_system_fundsp::AudioClockState;
use crate::engine::ecs::system::audio_system_fundsp::AudioQueueItem;
use crate::engine::ecs::system::audio_system_fundsp::AudioRtLocalState;
use crate::engine::ecs::system::audio_system_fundsp::MAX_AUDIO_GRAPH_CHILDREN_PER_NODE;
use crate::engine::ecs::system::audio_system_fundsp::MAX_AUDIO_GRAPH_NODES;
use crate::engine::ecs::system::audio_system_fundsp::MAX_OSCS_PER_COMPONENT;
use crate::engine::ecs::system::audio_system_fundsp::RtAudioGraph;
use crate::engine::ecs::system::audio_system_fundsp::RtAudioGraphChild;
use crate::engine::ecs::system::audio_system_fundsp::RtAudioGraphNode;
use crate::engine::ecs::system::audio_system_fundsp::ScheduledGraphOp;
use crate::engine::ecs::system::audio_system_fundsp::SynthRtState;
use crate::engine::ecs::system::audio_system_fundsp::{
    RtAudioGraphNodeKind, RtAudioGraphNodeState,
};

use heapless::Vec as HVec;
use rtrb::Producer;

use crate::engine::ecs::system::audio_graph_compiler::{AudioGraphCompiler, CompiledAudioGraph};

pub use crate::engine::ecs::system::audio_system_fundsp::ScheduledGraphOp as ScheduledGraphOperation;
pub use crate::engine::ecs::system::audio_system_fundsp::{AudioOp, ScheduledAudioOp};

// Keep a simple audio clock driven by the CPAL callback thread.

#[derive(Debug)]
pub struct AudioClockDriver {
    state: Arc<AudioClockState>,
}

impl AudioClockDriver {
    fn new(state: Arc<AudioClockState>) -> Self {
        Self { state }
    }
}

impl ClockDriver for AudioClockDriver {
    fn name(&self) -> &'static str {
        "audio"
    }

    fn time_now_sec(&self) -> f64 {
        let frames = self.state.frames_played.load(Ordering::Relaxed) as f64;
        frames / (self.state.sample_rate_hz as f64).max(1.0)
    }
}

/// Audio system.
///
/// Minimal implementation today:
/// - When an `AudioOutputComponent` is registered, start a CPAL output stream.
/// - Maintain a monotonically increasing audio clock based on rendered frames.
pub struct AudioSystem {
    stream: Option<cpal::Stream>,
    driver: Option<Arc<dyn ClockDriver>>,

    clock_state: Option<Arc<AudioClockState>>,

    output_component: Option<ComponentId>,
    desired_buffer_size_frames: Option<u32>,

    pending_buffer_sizes: Vec<(ComponentId, u32)>,

    /// Registered oscillator components and their oscillator lists.
    pub oscillators: std::collections::HashMap<ComponentId, Vec<AudioOscillator>>,

    /// Audio outputs whose graphs need recompilation.
    dirty_audio_outputs: std::collections::BTreeSet<ComponentId>,

    /// Latest compiled graphs per output (debug/inspection for now).
    compiled_graphs_by_output: std::collections::HashMap<ComponentId, Vec<CompiledAudioGraph>>,

    /// Last transport snapshot received from the main clock (used for scheduling immediate RT swaps).
    last_transport: Option<(f64, f64)>,

    audio_tx: Option<Producer<AudioQueueItem>>,

    /// Decode worker (phase 5 of docs/spec/audio-sources.md).
    decode_thread: Option<super::audio_decode_thread::DecodeThreadHandle>,
    /// Main-thread end of the decode→main completion channel.
    decode_complete_rx:
        Option<std::sync::mpsc::Receiver<super::audio_decode_thread::LoadedClipMessage>>,
    /// Sender held alongside the receiver so the worker can post results.
    decode_complete_tx:
        Option<std::sync::mpsc::Sender<super::audio_decode_thread::LoadedClipMessage>>,
    /// Engine playback format chosen at stream-init time. Cached so the
    /// decode worker resamples / remixes to match the stream.
    playback_format: Option<super::audio_sample_format_convert::PlaybackFormat>,
    /// Registered clip URI per component, used to surface diagnostics
    /// when the decode worker reports back. For cloned instances this
    /// is the URI inherited from the source.
    clip_uris: std::collections::HashMap<ComponentId, String>,
    /// Per-asset decode state, keyed by `asset_key` (URI hash). Allows
    /// multiple `AudioClipComponent`s sharing a URI (including clones
    /// via `source_component`) to dedup a single decode pass.
    /// See docs/draft/audio-clip-instance-cloning.md §2.
    asset_states: std::collections::HashMap<u64, AssetDecodeState>,
    /// Components waiting on a given asset to finish decoding. Drained
    /// when the worker reports back (success or failure).
    asset_subscribers: std::collections::HashMap<u64, Vec<ComponentId>>,
    /// Decoded clip uploads buffered until the RT producer is ready.
    /// Flushed on `register_audio_output`.
    pending_clip_uploads: Vec<AudioQueueItem>,
    /// Voice registrations buffered until the RT producer is ready.
    /// Flushed on `register_audio_output` after asset uploads.
    pending_clip_instance_registrations: Vec<AudioQueueItem>,
}

/// Decode-progress for a shared asset. URI is kept for diagnostics.
#[derive(Debug, Clone)]
enum AssetDecodeState {
    Pending { uri: String },
    Loaded { uri: String },
    Failed { uri: String, reason: String },
}

impl Default for AudioSystem {
    fn default() -> Self {
        Self {
            stream: None,
            driver: None,
            clock_state: None,
            output_component: None,
            desired_buffer_size_frames: None,
            pending_buffer_sizes: Vec::new(),
            oscillators: std::collections::HashMap::new(),
            audio_tx: None,

            dirty_audio_outputs: std::collections::BTreeSet::new(),
            compiled_graphs_by_output: std::collections::HashMap::new(),

            last_transport: None,

            decode_thread: None,
            decode_complete_rx: None,
            decode_complete_tx: None,
            playback_format: None,
            clip_uris: std::collections::HashMap::new(),
            asset_states: std::collections::HashMap::new(),
            asset_subscribers: std::collections::HashMap::new(),
            pending_clip_uploads: Vec::new(),
            pending_clip_instance_registrations: Vec::new(),
        }
    }
}

impl std::fmt::Debug for AudioSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let driver_name = self.driver.as_ref().map(|d| d.name()).unwrap_or("<none>");
        f.debug_struct("AudioSystem")
            .field("active", &self.is_active())
            .field("driver", &driver_name)
            .finish()
    }
}

impl AudioSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.stream.is_some() && self.driver.is_some()
    }

    pub fn driver(&self) -> Option<Arc<dyn ClockDriver>> {
        self.driver.clone()
    }

    pub fn update_transport_from_clock(&mut self, beat_now: f64, bpm: f64) {
        if !beat_now.is_finite() || !bpm.is_finite() || bpm <= 0.0 {
            return;
        }

        self.last_transport = Some((beat_now, bpm));

        // If audio isn't running yet, we still keep `last_transport` updated so that
        // graph swaps can be scheduled as soon as the stream starts.
        let Some(clock) = self.clock_state.as_ref() else {
            return;
        };

        let frames = clock.frames_played.load(Ordering::Relaxed) as f64;
        let sample_rate_hz = (clock.sample_rate_hz as f64).max(1.0);
        let time_sec = frames / sample_rate_hz;
        let beats_per_sec = bpm / 60.0;
        let beat_offset = beat_now - time_sec * beats_per_sec;

        let Some(tx) = self.audio_tx.as_mut() else {
            return;
        };
        let _ = tx.push(AudioQueueItem::SetTransport { bpm, beat_offset });
    }

    pub fn schedule_graph_swap(&mut self, world: &World, source_root: ComponentId, beat: f64) {
        if !beat.is_finite() {
            return;
        }

        let Ok(compiled) = AudioGraphCompiler::compile(world, source_root) else {
            return;
        };

        let Some(graph) = rt_graph_from_compiled(source_root, &compiled) else {
            return;
        };

        let Some(tx) = self.audio_tx.as_mut() else {
            return;
        };

        let target_component_ffi = source_root.data().as_ffi();
        let msg = ScheduledGraphOp {
            beat,
            target_component_ffi,
            graph,
        };

        let _ = tx.push(AudioQueueItem::GraphMessage(msg));
    }

    pub fn schedule_audio_op(&mut self, target_component: ComponentId, beat: f64, op: AudioOp) {
        if !beat.is_finite() {
            return;
        }
        let target_component_ffi = target_component.data().as_ffi();

        let Some(tx) = self.audio_tx.as_mut() else {
            return;
        };

        let event = ScheduledAudioOp {
            beat,
            target_component_ffi,
            op,
        };

        // Drop if full.
        let _ = tx.push(AudioQueueItem::Message(event));
    }

    pub fn register_audio_output(&mut self, world: &mut World, component: ComponentId) {
        if world
            .get_component_by_id_as::<AudioOutputComponent>(component)
            .is_none()
        {
            return;
        }

        if self.stream.is_some() {
            return;
        }

        self.output_component = Some(component);

        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return;
        };

        let Ok(supported_config) = device.default_output_config() else {
            return;
        };

        // Resolve desired buffer size based on the most recently registered
        // AudioBufferSizeComponent that is attached under this output component.
        self.desired_buffer_size_frames =
            self.pending_buffer_sizes
                .iter()
                .rev()
                .find_map(|(cid, frames)| {
                    if *frames == 0 {
                        return None;
                    }

                    // Walk parent chain to check attachment.
                    let mut cur = Some(*cid);
                    while let Some(c) = cur {
                        if c == component {
                            return Some(*frames);
                        }
                        cur = world.parent_of(c);
                    }
                    None
                });

        let sample_rate_hz = supported_config.sample_rate().0;
        let channels = supported_config.channels() as u64;
        let sample_format = supported_config.sample_format();

        let mut stream_config: cpal::StreamConfig = supported_config.into();
        if let Some(frames) = self.desired_buffer_size_frames {
            if frames > 0 {
                stream_config.buffer_size = cpal::BufferSize::Fixed(frames);
            }
        }
        let state = Arc::new(AudioClockState {
            sample_rate_hz,
            frames_played: AtomicU64::new(0),
        });
        let mut synth_state = SynthRtState::default();
        let err_fn = |err| eprintln!("[AudioSystem] stream error: {err}");

        let sample_rate_hz_f32 = (sample_rate_hz as f32).max(1.0);

        fn f32_to_f32(s: f32) -> f32 {
            s
        }

        fn f32_to_i16(s: f32) -> i16 {
            (s * i16::MAX as f32)
                .round()
                .clamp(i16::MIN as f32, i16::MAX as f32) as i16
        }

        fn f32_to_u16(s: f32) -> u16 {
            ((s * 0.5 + 0.5) * u16::MAX as f32)
                .round()
                .clamp(0.0, u16::MAX as f32) as u16
        }

        use crate::engine::ecs::system::audio_system_fundsp::AUDIO_QUEUE_CAP;
        use crate::engine::ecs::system::audio_system_fundsp::render_buffer;

        // Create a queue for GUI-thread -> audio-thread messages.
        let (tx, rx) = rtrb::RingBuffer::<AudioQueueItem>::new(AUDIO_QUEUE_CAP);
        self.audio_tx = Some(tx);

        // Record the engine's chosen playback format so the decode worker
        // resamples / remixes incoming PCM to match this stream.
        self.playback_format = Some(
            super::audio_sample_format_convert::PlaybackFormat {
                sample_rate: sample_rate_hz,
                channels: channels as u16,
            },
        );

        // Seed realtime thread with the most recent oscillator snapshots we know about.
        if let Some(tx) = self.audio_tx.as_mut() {
            for (cid, list) in self.oscillators.iter() {
                let component_ffi = cid.data().as_ffi();
                let mut hv = heapless::Vec::<AudioOscillator, MAX_OSCS_PER_COMPONENT>::new();
                for osc in list.iter().take(MAX_OSCS_PER_COMPONENT) {
                    let _ = hv.push(osc.clone());
                }
                let _ = tx.push(AudioQueueItem::ReplaceOscillators {
                    target_component_ffi: component_ffi,
                    oscillators: hv,
                });
            }
            // Flush any clip uploads that decoded before the stream was up.
            for item in self.pending_clip_uploads.drain(..) {
                let _ = tx.push(item);
            }
            // Then flush voice registrations queued before the stream
            // existed (asset must arrive first so render finds it).
            for item in self.pending_clip_instance_registrations.drain(..) {
                let _ = tx.push(item);
            }
        }

        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let state_for_cb = state.clone();
                let mut rx = rx;
                let mut rt = AudioRtLocalState::default();
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [f32], _info| {
                            render_buffer(
                                data,
                                channels,
                                sample_rate_hz,
                                sample_rate_hz_f32,
                                &state_for_cb,
                                &mut rx,
                                &mut rt,
                                &mut synth_state,
                                f32_to_f32,
                            );
                        },
                        err_fn,
                        None,
                    )
                    .ok()
            }
            cpal::SampleFormat::I16 => {
                let state_for_cb = state.clone();
                let mut rx = rx;
                let mut rt = AudioRtLocalState::default();
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [i16], _info| {
                            render_buffer(
                                data,
                                channels,
                                sample_rate_hz,
                                sample_rate_hz_f32,
                                &state_for_cb,
                                &mut rx,
                                &mut rt,
                                &mut synth_state,
                                f32_to_i16,
                            );
                        },
                        err_fn,
                        None,
                    )
                    .ok()
            }
            cpal::SampleFormat::U16 => {
                let state_for_cb = state.clone();
                let mut rx = rx;
                let mut rt = AudioRtLocalState::default();
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [u16], _info| {
                            render_buffer(
                                data,
                                channels,
                                sample_rate_hz,
                                sample_rate_hz_f32,
                                &state_for_cb,
                                &mut rx,
                                &mut rt,
                                &mut synth_state,
                                f32_to_u16,
                            );
                        },
                        err_fn,
                        None,
                    )
                    .ok()
            }
            _ => None,
        };

        let Some(stream) = stream else {
            return;
        };

        if let Err(e) = stream.play() {
            eprintln!("[AudioSystem] failed to play stream: {e}");
            return;
        }

        let state_for_driver = state.clone();
        self.driver = Some(Arc::new(AudioClockDriver::new(state_for_driver)));
        self.clock_state = Some(state);
        self.stream = Some(stream);

        // Mark graph dirty so we compile once the frame's mutations settle.
        self.dirty_audio_outputs.insert(component);

        // Also schedule an initial graph swap immediately so the RT thread has graphs
        // before any keyframed parameter ops arrive.
        let sources = collect_audio_oscillator_roots(world, component);
        if !sources.is_empty() {
            let (beat_now, bpm) = self.last_transport.unwrap_or((0.0, 120.0));
            let beats_per_sec = bpm / 60.0;
            let beat_epsilon = (beats_per_sec * 0.010).max(0.0); // ~10ms into the future.
            let beat = beat_now + beat_epsilon;
            for src in sources {
                self.schedule_graph_swap(world, src, beat);
            }
        }
    }

    pub fn register_audio_oscillator(&mut self, world: &mut World, component: ComponentId) {
        let Some(c) = world.get_component_by_id_as::<AudioOscillatorComponent>(component) else {
            return;
        };

        let list = c.oscillators.clone();
        self.oscillators.insert(component, list.clone());

        let Some(tx) = self.audio_tx.as_mut() else {
            return;
        };
        let component_ffi = component.data().as_ffi();

        let mut hv = heapless::Vec::<AudioOscillator, MAX_OSCS_PER_COMPONENT>::new();
        for osc in list.iter().take(MAX_OSCS_PER_COMPONENT) {
            let _ = hv.push(osc.clone());
        }

        let _ = tx.push(AudioQueueItem::ReplaceOscillators {
            target_component_ffi: component_ffi,
            oscillators: hv,
        });

        // Any oscillator registration/update may affect compiled audio graphs.
        self.mark_audio_graph_dirty(world, component);
    }

    pub fn register_audio_buffer_size(&mut self, world: &mut World, component: ComponentId) {
        let Some(c) = world.get_component_by_id_as::<AudioBufferSizeComponent>(component) else {
            return;
        };
        if c.frames == 0 {
            return;
        }

        self.pending_buffer_sizes.push((component, c.frames));

        let Some(output) = self.output_component else {
            return;
        };

        // Only apply if attached under the audio output component.
        let mut cur = Some(component);
        let mut attached = false;
        while let Some(cid) = cur {
            if cid == output {
                attached = true;
                break;
            }
            cur = world.parent_of(cid);
        }
        if !attached {
            return;
        }

        self.desired_buffer_size_frames = Some(c.frames);

        // If audio is already active, restart the stream to apply the new size.
        if self.stream.is_some() {
            self.stream = None;
            self.driver = None;
            self.clock_state = None;
            self.audio_tx = None;
            self.register_audio_output(world, output);
        }
    }
}

impl AudioSystem {
    /// Record that something in the audio graph changed.
    ///
    /// `component` can be any node in a subtree under an AudioOutputComponent.
    pub fn mark_audio_graph_dirty(&mut self, world: &World, component: ComponentId) {
        // Walk up parent chain until we find an AudioOutputComponent.
        let mut cur = Some(component);
        while let Some(cid) = cur {
            if world
                .get_component_by_id_as::<AudioOutputComponent>(cid)
                .is_some()
            {
                self.dirty_audio_outputs.insert(cid);
                return;
            }
            cur = world.parent_of(cid);
        }
    }

    /// Recompile all dirty audio output graphs. Intended to be called once per frame
    /// after CommandQueue mutations are applied.
    pub fn rebuild_dirty_audio_graphs(&mut self, world: &World) {
        if self.dirty_audio_outputs.is_empty() {
            return;
        }

        let outputs: Vec<ComponentId> = self.dirty_audio_outputs.iter().copied().collect();
        self.dirty_audio_outputs.clear();

        for output in outputs {
            let sources = collect_audio_oscillator_roots(world, output);

            let mut compiled = Vec::new();
            for &src in sources.iter() {
                if let Ok(g) = AudioGraphCompiler::compile(world, src) {
                    compiled.push(g);
                }
            }

            // Deterministic order (ComponentId sort already used in collector, but keep stable).
            self.compiled_graphs_by_output.insert(output, compiled);

            // Schedule an RT graph swap for each source immediately (beat-timed). This is the
            // "best effort" path for init/live edits; keyframe-driven topology should call
            // `schedule_graph_swap` with the keyframe beat.
            if self.audio_tx.is_some() {
                let Some((beat_now, bpm)) = self.last_transport else {
                    // We don't know current beat yet, so can't schedule.
                    continue;
                };
                let beats_per_sec = bpm / 60.0;
                let beat_epsilon = (beats_per_sec * 0.001).max(0.0); // 1ms into the future.
                let beat = beat_now + beat_epsilon;

                for &src in sources.iter() {
                    self.schedule_graph_swap(world, src, beat);
                }
            }
        }
    }
}

fn rt_graph_from_compiled(
    source_root: ComponentId,
    compiled: &CompiledAudioGraph,
) -> Option<RtAudioGraph> {
    fn kind_and_state(
        k: &crate::engine::ecs::system::audio_graph_compiler::AudioGraphNodeKind,
    ) -> (RtAudioGraphNodeKind, RtAudioGraphNodeState) {
        match *k {
            crate::engine::ecs::system::audio_graph_compiler::AudioGraphNodeKind::OscillatorSource { .. } => {
                (RtAudioGraphNodeKind::OscillatorSource, Default::default())
            }
            crate::engine::ecs::system::audio_graph_compiler::AudioGraphNodeKind::ClipSource => {
                (RtAudioGraphNodeKind::ClipSource, Default::default())
            }
            crate::engine::ecs::system::audio_graph_compiler::AudioGraphNodeKind::Gain { gain } => {
                (RtAudioGraphNodeKind::Gain { gain }, Default::default())
            }
            crate::engine::ecs::system::audio_graph_compiler::AudioGraphNodeKind::LowPass {
                cutoff_hz,
                resonance,
            } => (
                RtAudioGraphNodeKind::LowPass {
                    cutoff_hz,
                    resonance,
                },
                Default::default(),
            ),
            crate::engine::ecs::system::audio_graph_compiler::AudioGraphNodeKind::BandPass {
                center_hz,
                bandwidth_octaves,
                resonance,
            } => (
                RtAudioGraphNodeKind::BandPass {
                    center_hz,
                    bandwidth_octaves,
                    resonance,
                },
                Default::default(),
            ),
            crate::engine::ecs::system::audio_graph_compiler::AudioGraphNodeKind::HighPass {
                cutoff_hz,
                resonance,
            } => (
                RtAudioGraphNodeKind::HighPass {
                    cutoff_hz,
                    resonance,
                },
                Default::default(),
            ),
            crate::engine::ecs::system::audio_graph_compiler::AudioGraphNodeKind::Limiter {
                attack_ms,
                release_ms,
                threshold,
            } => {
                let mut st: RtAudioGraphNodeState = Default::default();
                st.limiter_attack_ms = attack_ms;
                st.limiter_release_ms = release_ms;
                st.limiter_threshold = threshold;
                (RtAudioGraphNodeKind::Limiter, st)
            }
        }
    }

    fn build(
        node: &crate::engine::ecs::system::audio_graph_compiler::AudioGraphNode,
        nodes: &mut HVec<RtAudioGraphNode, MAX_AUDIO_GRAPH_NODES>,
    ) -> Option<u8> {
        let idx = nodes.len();
        if idx >= MAX_AUDIO_GRAPH_NODES {
            return None;
        }

        let (kind, state) = kind_and_state(&node.kind);

        nodes
            .push(RtAudioGraphNode {
                component_ffi: node.component.data().as_ffi(),
                kind,
                state,
                children: HVec::<RtAudioGraphChild, MAX_AUDIO_GRAPH_CHILDREN_PER_NODE>::new(),
            })
            .ok()?;

        let idx_u8 = idx as u8;
        for (i, ch) in node.children.iter().enumerate() {
            let child_idx = build(ch, nodes)?;
            let w = node
                .mix
                .as_ref()
                .and_then(|m| m.weights.get(i))
                .copied()
                .unwrap_or(1.0);

            // Best-effort: if a node has more children than the RT cap, ignore extras.
            let parent = nodes.get_mut(idx).expect("just pushed");
            if parent.children.len() >= MAX_AUDIO_GRAPH_CHILDREN_PER_NODE {
                continue;
            }
            let _ = parent.children.push(RtAudioGraphChild {
                idx: child_idx,
                weight: w,
            });
        }

        Some(idx_u8)
    }

    let mut nodes = HVec::<RtAudioGraphNode, MAX_AUDIO_GRAPH_NODES>::new();
    let _root_idx = build(&compiled.root, &mut nodes)?;

    Some(RtAudioGraph {
        root_component_ffi: source_root.data().as_ffi(),
        nodes,
    })
}

fn collect_audio_oscillator_roots(world: &World, output: ComponentId) -> Vec<ComponentId> {
    use crate::engine::ecs::component::AudioClipComponent;

    // Collect oscillator AND clip components in the output subtree —
    // both are `AudioSource` peers per docs/spec/audio-sources.md §5.
    let mut all = Vec::new();
    let mut stack = vec![output];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            stack.push(ch);
        }

        if node != output
            && (world
                .get_component_by_id_as::<AudioOscillatorComponent>(node)
                .is_some()
                || world
                    .get_component_by_id_as::<AudioClipComponent>(node)
                    .is_some())
        {
            all.push(node);
        }
    }

    // Keep only roots (exclude sources that are under another source).
    all.sort();
    all.dedup();

    all.into_iter()
        .filter(|&cid| {
            let mut cur = world.parent_of(cid);
            while let Some(p) = cur {
                if p == output {
                    return true;
                }
                let is_source = world
                    .get_component_by_id_as::<AudioOscillatorComponent>(p)
                    .is_some()
                    || world
                        .get_component_by_id_as::<AudioClipComponent>(p)
                        .is_some();
                if is_source {
                    return false;
                }
                cur = world.parent_of(p);
            }
            false
        })
        .collect()
}

impl System for AudioSystem {
    fn tick(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        self.drain_decode_completions(world);
    }
}

impl AudioSystem {
    /// Public wrapper for the internal drain — exposed so `SystemWorld`
    /// can flush completions after command processing each frame.
    pub fn drain_decode_completions_public(&mut self, world: &mut World) {
        self.drain_decode_completions(world);
    }

    /// Forward decode-worker results to the audio RT thread and update
    /// `AudioClipComponent.load_state` so the rest of the engine sees the
    /// load outcome. Called from `System::tick`.
    fn drain_decode_completions(&mut self, world: &mut World) {
        use super::audio_decode_thread::LoadedClipMessage;
        use crate::engine::ecs::component::{AudioClipComponent, AudioClipLoadState};

        let Some(rx) = self.decode_complete_rx.as_ref() else {
            return;
        };

        loop {
            let msg = match rx.try_recv() {
                Ok(m) => m,
                Err(_) => break,
            };
            // `clip_id` end-to-end is the URI-derived asset key — we
            // pass it into `LoadClipRequest` and the worker echoes it
            // back. See `register_audio_clip`.
            match msg {
                LoadedClipMessage::Loaded {
                    clip_id: asset_key,
                    samples,
                    channels,
                    sample_rate,
                } => {
                    let upload = AudioQueueItem::UploadClip {
                        asset_key,
                        samples: samples.clone(),
                        channels,
                        sample_rate,
                    };
                    if let Some(tx) = self.audio_tx.as_mut() {
                        let _ = tx.push(upload);
                    } else {
                        self.pending_clip_uploads.push(upload);
                    }
                    let uri = match self.asset_states.get(&asset_key) {
                        Some(AssetDecodeState::Pending { uri })
                        | Some(AssetDecodeState::Loaded { uri })
                        | Some(AssetDecodeState::Failed { uri, .. }) => uri.clone(),
                        None => String::new(),
                    };
                    self.asset_states
                        .insert(asset_key, AssetDecodeState::Loaded { uri });
                    // Mark every subscriber Loaded.
                    if let Some(subs) = self.asset_subscribers.remove(&asset_key) {
                        for cid in subs {
                            if let Some(c) =
                                world.get_component_by_id_as_mut::<AudioClipComponent>(cid)
                            {
                                c.load_state = AudioClipLoadState::Loaded;
                            }
                        }
                    }
                }
                LoadedClipMessage::Failed {
                    clip_id: asset_key,
                    reason,
                } => {
                    let uri = match self.asset_states.get(&asset_key) {
                        Some(AssetDecodeState::Pending { uri })
                        | Some(AssetDecodeState::Loaded { uri })
                        | Some(AssetDecodeState::Failed { uri, .. }) => uri.clone(),
                        None => String::new(),
                    };
                    eprintln!("[AudioClip] {uri}: {reason}");
                    self.asset_states.insert(
                        asset_key,
                        AssetDecodeState::Failed {
                            uri,
                            reason: reason.clone(),
                        },
                    );
                    if let Some(subs) = self.asset_subscribers.remove(&asset_key) {
                        for cid in subs {
                            if let Some(c) =
                                world.get_component_by_id_as_mut::<AudioClipComponent>(cid)
                            {
                                c.load_state = AudioClipLoadState::Failed(reason.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    /// Register an `AudioClipComponent` for decode + RT playback. Spawns
    /// the decode worker lazily on first call.
    ///
    /// Decode is deduplicated by URI hash (`asset_key`): two clips with
    /// the same URI — or a clip and an `.instance_of()` clone — share
    /// one decoded buffer on the RT side. Each component still gets its
    /// own `RtClipInstance` voice via `RegisterClipInstance`.
    pub fn register_audio_clip(&mut self, world: &mut World, component: ComponentId) {
        use super::audio_decode_thread::{spawn_decode_thread, LoadClipRequest};
        use super::audio_sample_format_convert::PlaybackFormat;
        use crate::engine::ecs::component::{AudioClipComponent, AudioClipLoadState};
        use slotmap::Key;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Clones copy the URI in at construction (see
        // `AudioClipComponent::instance_of`), so this is just a
        // straight read.
        let (uri, start_beat, stop_beat) = {
            let Some(clip) = world.get_component_by_id_as::<AudioClipComponent>(component)
            else {
                return;
            };
            (clip.uri.clone(), clip.start_beat, clip.stop_beat)
        };

        if uri.is_empty() {
            if let Some(c) = world.get_component_by_id_as_mut::<AudioClipComponent>(component)
            {
                c.load_state = AudioClipLoadState::Failed("clip has no uri".into());
            }
            return;
        }

        self.clip_uris.insert(component, uri.clone());

        // Compute the asset key (URI hash). DefaultHasher is per-process
        // but the value never escapes this process — only the RT thread
        // sees it via the audio queue.
        let asset_key = {
            let mut h = DefaultHasher::new();
            uri.hash(&mut h);
            h.finish()
        };

        // Ensure the decode thread + completion channel exist.
        if self.decode_thread.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.decode_complete_tx = Some(tx.clone());
            self.decode_complete_rx = Some(rx);
            self.decode_thread = Some(spawn_decode_thread(tx));
        }

        let target = self.playback_format.unwrap_or(PlaybackFormat {
            sample_rate: 48_000,
            channels: 1,
        });

        // Dedup decode: only one LoadClipRequest per asset_key.
        let needs_decode = !self.asset_states.contains_key(&asset_key);
        if needs_decode {
            self.asset_states.insert(
                asset_key,
                AssetDecodeState::Pending {
                    uri: uri.clone(),
                },
            );
            if let Some(handle) = self.decode_thread.as_ref() {
                let _ = handle.tx.send(LoadClipRequest {
                    clip_id: asset_key,
                    uri: uri.clone(),
                    target,
                });
            }
        }

        // Set this component's load_state immediately if the asset is
        // already resolved; otherwise subscribe for the next completion.
        match self.asset_states.get(&asset_key).cloned() {
            Some(AssetDecodeState::Loaded { .. }) => {
                if let Some(c) =
                    world.get_component_by_id_as_mut::<AudioClipComponent>(component)
                {
                    c.load_state = AudioClipLoadState::Loaded;
                }
            }
            Some(AssetDecodeState::Failed { reason, .. }) => {
                if let Some(c) =
                    world.get_component_by_id_as_mut::<AudioClipComponent>(component)
                {
                    c.load_state = AudioClipLoadState::Failed(reason);
                }
            }
            _ => {
                self.asset_subscribers
                    .entry(asset_key)
                    .or_default()
                    .push(component);
            }
        }

        // Register the voice on the RT thread (or buffer it until the
        // stream is up).
        let register = AudioQueueItem::RegisterClipInstance {
            component_ffi: component.data().as_ffi(),
            asset_key,
            start_beat,
            stop_beat,
        };
        if let Some(tx) = self.audio_tx.as_mut() {
            let _ = tx.push(register);
        } else {
            self.pending_clip_instance_registrations.push(register);
        }

        // Touch graph dirtiness so the clip becomes a node in the
        // compiled audio graph.
        self.mark_audio_graph_dirty(world, component);
    }
}
