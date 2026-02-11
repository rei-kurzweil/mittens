use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use slotmap::Key;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::engine::ecs::component::AudioBufferSizeComponent;
use crate::engine::ecs::component::AudioOutputComponent;
use crate::engine::ecs::component::AudioOscillatorComponent;
use crate::engine::ecs::component::AudioOscillator;
use crate::engine::ecs::system::clock_system::ClockDriver;
use crate::engine::ecs::system::System;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

use crate::engine::ecs::system::audio_system_fundsp::AudioClockState;
use crate::engine::ecs::system::audio_system_fundsp::AudioQueueItem;
use crate::engine::ecs::system::audio_system_fundsp::AudioRtLocalState;
use crate::engine::ecs::system::audio_system_fundsp::SynthRtState;
use crate::engine::ecs::system::audio_system_fundsp::MAX_OSCS_PER_COMPONENT;

use heapless::spsc::Producer;

pub use crate::engine::ecs::system::audio_system_fundsp::{ScheduledAudioOp, AudioOp};

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

    audio_tx: Option<Producer<'static, AudioQueueItem, { crate::engine::ecs::system::audio_system_fundsp::AUDIO_QUEUE_CAP }>>,
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
        }
    }
}

impl std::fmt::Debug for AudioSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let driver_name = self
            .driver
            .as_ref()
            .map(|d| d.name())
            .unwrap_or("<none>");
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
        let Some(clock) = self.clock_state.as_ref() else {
            return;
        };
        if !beat_now.is_finite() || !bpm.is_finite() || bpm <= 0.0 {
            return;
        }

        let frames = clock.frames_played.load(Ordering::Relaxed) as f64;
        let sample_rate_hz = (clock.sample_rate_hz as f64).max(1.0);
        let time_sec = frames / sample_rate_hz;
        let beats_per_sec = bpm / 60.0;
        let beat_offset = beat_now - time_sec * beats_per_sec;

        let Some(tx) = self.audio_tx.as_mut() else {
            return;
        };
        let _ = tx.enqueue(AudioQueueItem::SetTransport { bpm, beat_offset });
    }

    pub fn schedule_audio_op(
        &mut self,
        target_component: ComponentId,
        beat: f64,
        op: AudioOp,
    ) {
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
        let _ = tx.enqueue(AudioQueueItem::Message(event));
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

        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return;
        };

        let Ok(supported_config) = device.default_output_config() else {
            return;
        };

        // Resolve desired buffer size based on the most recently registered
        // AudioBufferSizeComponent that is attached under this output component.
        self.desired_buffer_size_frames = self
            .pending_buffer_sizes
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

        use crate::engine::ecs::system::audio_system_fundsp::render_buffer;
        use crate::engine::ecs::system::audio_system_fundsp::AUDIO_QUEUE_CAP;

        // Create a queue for GUI-thread -> audio-thread messages.
        let q: &'static mut heapless::spsc::Queue<AudioQueueItem, AUDIO_QUEUE_CAP> =
            Box::leak(Box::new(heapless::spsc::Queue::new()));
        let (tx, rx) = q.split();
        self.audio_tx = Some(tx);

        // Seed realtime thread with the most recent oscillator snapshots we know about.
        if let Some(tx) = self.audio_tx.as_mut() {
            for (cid, list) in self.oscillators.iter() {
                let component_ffi = cid.data().as_ffi();
                let mut hv = heapless::Vec::<AudioOscillator, MAX_OSCS_PER_COMPONENT>::new();
                for osc in list.iter().take(MAX_OSCS_PER_COMPONENT) {
                    let _ = hv.push(osc.clone());
                }
                let _ = tx.enqueue(AudioQueueItem::ReplaceOscillators {
                    target_component_ffi: component_ffi,
                    oscillators: hv,
                });
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
            _ => {
                None
            }
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

        let _ = tx.enqueue(AudioQueueItem::ReplaceOscillators {
            target_component_ffi: component_ffi,
            oscillators: hv,
        });
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

impl System for AudioSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // Audio runs on the CPAL callback thread. Nothing to do per-frame yet.
    }
}
