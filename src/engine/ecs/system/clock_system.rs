use std::sync::Arc;
use std::time::Instant;

use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::ClockComponent;
use crate::engine::ecs::system::System;
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

pub trait ClockDriver: Send + Sync {
    fn name(&self) -> &'static str;
    fn time_now_sec(&self) -> f64;
}

#[derive(Debug)]
pub struct SystemClockDriver {
    start: Instant,
}

impl SystemClockDriver {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl ClockDriver for SystemClockDriver {
    fn name(&self) -> &'static str {
        "system"
    }

    fn time_now_sec(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

/// Global clock system.
///
/// Exposes a beat-based timeline (beats) driven by a pluggable `ClockDriver`.
pub struct ClockSystem {
    driver: Arc<dyn ClockDriver>,
    bpm: f64,

    tempo_component: Option<ComponentId>,

    // Cached for quick access + for other systems to read.
    time_base_sec: f64,
    beat_base: f64,
    last_time_sec: f64,
    last_beat: f64,
}

impl std::fmt::Debug for ClockSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClockSystem")
            .field("driver", &self.driver.name())
            .field("bpm", &self.bpm)
            .field("last_beat", &self.last_beat)
            .finish()
    }
}

impl Default for ClockSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockSystem {
    pub fn new() -> Self {
        let driver: Arc<dyn ClockDriver> = Arc::new(SystemClockDriver::new());
        let t0 = driver.time_now_sec();
        Self {
            driver,
            bpm: 120.0,
            tempo_component: None,
            time_base_sec: t0,
            beat_base: 0.0,
            last_time_sec: t0,
            last_beat: 0.0,
        }
    }

    pub fn driver_name(&self) -> &'static str {
        self.driver.name()
    }

    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    pub fn beat_now(&self) -> f64 {
        self.last_beat
    }

    pub fn register_clock_component(&mut self, component: ComponentId) {
        self.tempo_component = Some(component);
    }

    pub fn set_bpm(&mut self, bpm: f64) {
        if !bpm.is_finite() || bpm <= 0.0 {
            return;
        }

        if (self.bpm - bpm).abs() < 1e-6 {
            return;
        }

        // Keep beat continuous across tempo changes.
        let now = self.driver.time_now_sec();
        self.beat_base = self.beat_at_time(now);
        self.time_base_sec = now;
        self.bpm = bpm;
    }

    pub fn set_driver(&mut self, driver: Arc<dyn ClockDriver>) {
        if self.driver_name() != driver.name() {
            println!(
                "[ClockSystem] driver: {} -> {}",
                self.driver_name(),
                driver.name()
            );
        }

        // Keep beat continuous across driver changes.
        let last_beat = self.last_beat;
        self.driver = driver;

        let now = self.driver.time_now_sec();
        self.time_base_sec = now;
        self.beat_base = last_beat;
        self.last_time_sec = now;
        self.last_beat = last_beat;
    }

    fn beat_at_time(&self, time_now_sec: f64) -> f64 {
        let dt = (time_now_sec - self.time_base_sec).max(0.0);
        self.beat_base + dt * (self.bpm / 60.0)
    }

    pub fn sample(&mut self) {
        let now = self.driver.time_now_sec();
        self.last_time_sec = now;
        self.last_beat = self.beat_at_time(now);
    }
}

impl System for ClockSystem {
    fn tick(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        if let Some(cid) = self.tempo_component {
            if let Some(clock) = world.get_component_by_id_as::<ClockComponent>(cid) {
                self.set_bpm(clock.bpm);
            }
        }

        // Driver owns its own notion of time; we sample and convert to beats.
        self.sample();
    }
}
