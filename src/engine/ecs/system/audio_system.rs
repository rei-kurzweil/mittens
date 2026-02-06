use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::engine::ecs::component::AudioOutputComponent;
use crate::engine::ecs::system::clock_system::ClockDriver;
use crate::engine::ecs::system::System;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

#[derive(Debug)]
struct AudioClockState {
    sample_rate_hz: u32,
    frames_played: AtomicU64,
}

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
}

impl Default for AudioSystem {
    fn default() -> Self {
        Self {
            stream: None,
            driver: None,
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
            println!("[AudioSystem] no default output device");
            return;
        };

        let Ok(config) = device.default_output_config() else {
            println!("[AudioSystem] failed to query default output config");
            return;
        };

        let sample_rate_hz = config.sample_rate().0;
        let channels = config.channels() as u64;
        let state = Arc::new(AudioClockState {
            sample_rate_hz,
            frames_played: AtomicU64::new(0),
        });

        let state_for_cb = state.clone();
        let err_fn = |err| eprintln!("[AudioSystem] stream error: {err}");

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device
                .build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _info| {
                        for s in data.iter_mut() {
                            *s = 0.0;
                        }
                        state_for_cb
                            .frames_played
                            .fetch_add((data.len() as u64) / channels.max(1), Ordering::Relaxed);
                    },
                    err_fn,
                    None,
                )
                .ok(),
            cpal::SampleFormat::I16 => device
                .build_output_stream(
                    &config.into(),
                    move |data: &mut [i16], _info| {
                        for s in data.iter_mut() {
                            *s = 0;
                        }
                        state_for_cb
                            .frames_played
                            .fetch_add((data.len() as u64) / channels.max(1), Ordering::Relaxed);
                    },
                    err_fn,
                    None,
                )
                .ok(),
            cpal::SampleFormat::U16 => device
                .build_output_stream(
                    &config.into(),
                    move |data: &mut [u16], _info| {
                        for s in data.iter_mut() {
                            *s = 0;
                        }
                        state_for_cb
                            .frames_played
                            .fetch_add((data.len() as u64) / channels.max(1), Ordering::Relaxed);
                    },
                    err_fn,
                    None,
                )
                .ok(),
            _ => {
                println!("[AudioSystem] unsupported sample format");
                None
            }
        };

        let Some(stream) = stream else {
            println!("[AudioSystem] failed to build output stream");
            return;
        };

        if let Err(e) = stream.play() {
            println!("[AudioSystem] failed to play stream: {e}");
            return;
        }

        println!(
            "[AudioSystem] output active: device='{}' rate={}Hz",
            device.name().unwrap_or_else(|_| "<unknown>".to_string()),
            sample_rate_hz
        );

        self.driver = Some(Arc::new(AudioClockDriver::new(state)));
        self.stream = Some(stream);
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
