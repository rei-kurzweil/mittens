use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter};

/// Local preferences for avatars loaded during the current scene session.
#[derive(Debug, Clone, Default)]
pub struct AvatarSettingsSystem {
    desired_height_meters: Option<f32>,
}

impl AvatarSettingsSystem {
    pub fn desired_height_meters(&self) -> Option<f32> {
        self.desired_height_meters
    }

    pub fn set_desired_height_meters(
        &mut self,
        height: f64,
        emit: &mut dyn SignalEmitter,
    ) -> Result<(), &'static str> {
        if !(0.025..=3.0).contains(&height) {
            return Err("desired avatar height must be between 0.025 and 3.0 meters");
        }
        let height = Some(height as f32);
        if self.desired_height_meters != height {
            self.desired_height_meters = height;
            emit.push_event(
                ComponentId::default(),
                EventSignal::DataEvent {
                    name: "AvatarSettingsChanged".into(),
                    payload: None,
                },
            );
        }
        Ok(())
    }
}
