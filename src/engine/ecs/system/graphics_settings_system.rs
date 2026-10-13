use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter};
use crate::engine::graphics::VisualWorld;

/// Authoritative runtime graphics settings, independent of scene UI.
#[derive(Debug, Clone)]
pub struct GraphicsSettingsSystem {
    mirrors_enabled: bool,
}

impl Default for GraphicsSettingsSystem {
    fn default() -> Self {
        Self {
            mirrors_enabled: true,
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
    }

    /// Scene/session replacement starts with the usual enabled default.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
