use crate::engine::ecs::component::XrStickMode;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter};

/// Local input preferences for the current scene session. Components can
/// inherit these preferences. Changes notify scripts and settings panels.
#[derive(Debug, Clone, Default)]
pub struct InputSettingsSystem {
    xr_stick_mode: XrStickMode,
}

impl InputSettingsSystem {
    pub fn xr_stick_mode(&self) -> XrStickMode {
        self.xr_stick_mode
    }

    pub fn set_xr_stick_mode(&mut self, mode: XrStickMode, emit: &mut dyn SignalEmitter) {
        let mode = if mode == XrStickMode::Inherit {
            XrStickMode::default()
        } else {
            mode
        };
        if self.xr_stick_mode != mode {
            self.xr_stick_mode = mode;
            emit.push_event(
                ComponentId::default(),
                EventSignal::DataEvent {
                    name: "InputSettingsChanged".into(),
                    payload: None,
                },
            );
        }
    }
}
