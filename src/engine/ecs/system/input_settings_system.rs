#[path = "input_settings_file.rs"]
pub mod settings_file;

use crate::engine::ecs::component::XrStickMode;
use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter};

/// Local input preferences. Scenes apply them to their own input components
/// when `InputSettingsChanged` fires, and may load and save them explicitly
/// through `settings_file`.
#[derive(Debug, Clone, Default)]
pub struct InputSettingsSystem {
    xr_stick_mode: XrStickMode,
}

impl InputSettingsSystem {
    pub fn xr_stick_mode(&self) -> XrStickMode {
        self.xr_stick_mode
    }

    /// Persist before applying, so failed writes preserve runtime state and notifications.
    pub fn save_xr_stick_mode(
        &mut self,
        file: &str,
        mode: XrStickMode,
        emit: &mut dyn SignalEmitter,
    ) -> Result<(), String> {
        settings_file::save(file, mode)?;
        self.set_xr_stick_mode(mode, emit);
        Ok(())
    }

    pub fn set_xr_stick_mode(&mut self, mode: XrStickMode, emit: &mut dyn SignalEmitter) {
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
