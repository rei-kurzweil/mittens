use crate::engine::ecs::{ComponentId, EventSignal, SignalEmitter};

/// Local XR calibration, independent of any settings panel or avatar.
#[derive(Debug, Clone, Default)]
pub struct XrSettingsSystem {
    floor_correction_meters: f32,
}

impl XrSettingsSystem {
    pub fn floor_correction_meters(&self) -> f32 {
        self.floor_correction_meters
    }

    /// Positive values lower tracked head/hands relative to the simulated floor.
    pub fn set_floor_correction_meters(
        &mut self,
        meters: f64,
        emit: &mut dyn SignalEmitter,
    ) -> Result<(), &'static str> {
        if !meters.is_finite() || !(-0.5..=0.5).contains(&meters) {
            return Err("floor correction must be finite and between -0.5 and +0.5 metres");
        }
        let meters = meters as f32;
        if self.floor_correction_meters != meters {
            self.floor_correction_meters = meters;
            emit.push_event(
                ComponentId::default(),
                EventSignal::DataEvent {
                    name: "XrSettingsChanged".into(),
                    payload: None,
                },
            );
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Events(Vec<EventSignal>);
    impl SignalEmitter for Events {
        fn push_event(&mut self, _: ComponentId, event: EventSignal) {
            self.0.push(event);
        }
        fn push_intent(&mut self, _: ComponentId, _: crate::engine::ecs::IntentSignal) {}
    }

    #[test]
    fn xr_settings_rejects_nonfinite_and_notifies_only_real_changes() {
        let mut settings = XrSettingsSystem::default();
        let mut events = Events::default();
        settings
            .set_floor_correction_meters(0.15, &mut events)
            .unwrap();
        settings
            .set_floor_correction_meters(0.15, &mut events)
            .unwrap();
        for invalid in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.500001,
            -0.500001,
        ] {
            assert!(
                settings
                    .set_floor_correction_meters(invalid, &mut events)
                    .is_err()
            );
        }
        assert_eq!(settings.floor_correction_meters(), 0.15);
        assert_eq!(events.0.len(), 1);
        assert!(
            matches!(&events.0[0], EventSignal::DataEvent { name, .. } if name == "XrSettingsChanged")
        );
        settings.reset();
        assert_eq!(settings.floor_correction_meters(), 0.0);
    }
}
