use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter};
use crate::engine::graphics::VisualWorld;

/// Authoritative runtime graphics settings, independent of scene UI.
#[derive(Debug, Clone)]
pub struct GraphicsSettingsSystem {
    mirrors_enabled: bool,
    bloom_enabled: bool,
    bloom_layers: u32,
    mirror_quality_override: Option<i32>,
    secondary_motion_enabled: bool,
    secondary_motion_paused: bool,
}

impl Default for GraphicsSettingsSystem {
    fn default() -> Self {
        Self {
            mirrors_enabled: true,
            bloom_enabled: true,
            bloom_layers: 1,
            mirror_quality_override: None,
            secondary_motion_enabled: true,
            secondary_motion_paused: false,
        }
    }
}

impl GraphicsSettingsSystem {
    pub fn bloom_enabled(&self) -> bool {
        self.bloom_enabled
    }
    pub fn bloom_layers(&self) -> u32 {
        self.bloom_layers
    }
    pub fn set_bloom_enabled(&mut self, value: bool, emit: &mut dyn SignalEmitter) {
        if self.bloom_enabled != value {
            self.bloom_enabled = value;
            Self::notify_changed(emit);
        }
    }
    pub fn set_bloom_layers(
        &mut self,
        value: u32,
        emit: &mut dyn SignalEmitter,
    ) -> Result<(), &'static str> {
        if !(1..=6).contains(&value) {
            return Err("bloom layer count must be in [1,6]");
        }
        if self.bloom_layers != value {
            self.bloom_layers = value;
            Self::notify_changed(emit);
        }
        Ok(())
    }

    pub fn mirrors_enabled(&self) -> bool {
        self.mirrors_enabled
    }

    /// Host dispatch runs on the engine thread. Apply synchronously so a
    /// following MMS getter observes this value, then queue UI notification.
    pub fn set_mirrors_enabled(&mut self, enabled: bool, emit: &mut dyn SignalEmitter) {
        if self.mirrors_enabled == enabled {
            return;
        }
        self.mirrors_enabled = enabled;
        Self::notify_changed(emit);
    }

    pub fn mirror_quality_override(&self) -> Option<i32> {
        self.mirror_quality_override
    }

    pub fn set_mirror_quality(&mut self, quality: i32, emit: &mut dyn SignalEmitter) {
        self.update_mirror_quality(Some(quality.clamp(64, 2048)), emit);
    }

    pub fn reset_mirror_quality(&mut self, emit: &mut dyn SignalEmitter) {
        self.update_mirror_quality(None, emit);
    }

    fn update_mirror_quality(&mut self, quality: Option<i32>, emit: &mut dyn SignalEmitter) {
        if self.mirror_quality_override == quality {
            return;
        }
        self.mirror_quality_override = quality;
        Self::notify_changed(emit);
    }

    pub fn secondary_motion_enabled(&self) -> bool {
        self.secondary_motion_enabled
    }

    pub fn set_secondary_motion_enabled(&mut self, enabled: bool, emit: &mut dyn SignalEmitter) {
        if self.secondary_motion_enabled == enabled {
            return;
        }
        self.secondary_motion_enabled = enabled;
        Self::notify_changed(emit);
    }

    pub fn secondary_motion_paused(&self) -> bool {
        self.secondary_motion_paused
    }

    /// Pause integration while retaining the current deformed pose.
    pub fn set_secondary_motion_paused(&mut self, paused: bool, emit: &mut dyn SignalEmitter) {
        if self.secondary_motion_paused == paused {
            return;
        }
        self.secondary_motion_paused = paused;
        Self::notify_changed(emit);
    }

    fn notify_changed(emit: &mut dyn SignalEmitter) {
        emit.push_event(
            ComponentId::default(),
            EventSignal::DataEvent {
                name: "GraphicsSettingsChanged".into(),
                payload: None,
            },
        );
    }

    pub fn publish(&self, visuals: &mut VisualWorld) {
        visuals.set_bloom_settings(self.bloom_enabled, self.bloom_layers);
        visuals.set_mirror_captures_enabled(self.mirrors_enabled);
        visuals.set_mirror_quality_override(self.mirror_quality_override);
    }

    /// Scene/session replacement enables captures/motion and clears quality override.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
