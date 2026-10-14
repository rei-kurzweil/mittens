use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter};
use crate::engine::graphics::VisualWorld;

/// Authoritative runtime graphics settings, independent of scene UI.
#[derive(Debug, Clone)]
pub struct GraphicsSettingsSystem {
    mirrors_enabled: bool,
    mirror_quality_override: Option<i32>,
    secondary_motion_enabled: bool,
}

impl Default for GraphicsSettingsSystem {
    fn default() -> Self {
        Self {
            mirrors_enabled: true,
            mirror_quality_override: None,
            secondary_motion_enabled: true,
        }
    }
}

impl GraphicsSettingsSystem {
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
        visuals.set_mirror_captures_enabled(self.mirrors_enabled);
        visuals.set_mirror_quality_override(self.mirror_quality_override);
    }

    /// Scene/session replacement enables captures/motion and clears quality override.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
