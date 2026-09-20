use super::{AmplitudeSample, AmplitudeStatus, Component, ComponentRef};
use crate::engine::ecs::ComponentId;

/// Callback-side decision retained with the latest normalized level for
/// diagnostics.  It is runtime-only, like gain and the sample itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VolumeNormalizationReason {
    #[default]
    Holding,
    Raising,
    PeakReducing,
    SustainedReducing,
    Capped,
    Invalid,
}

/// The immutable callback configuration copied while a capture stream is
/// constructed.  Keeping this small and `Copy` makes the audio callback
/// independent of ECS access and live authoring mutation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VolumeNormalizationPolicy {
    pub min_gain_db: f32,
    pub max_gain_db: f32,
    pub target_rms_low: f32,
    pub target_rms_high: f32,
    pub activity_gate: f32,
    pub quiet_hold_sec: f32,
    pub high_hold_sec: f32,
    pub gain_rise_db_per_sec: f32,
    pub gain_fall_db_per_sec: f32,
    pub peak_headroom: f32,
}

/// Adaptive analysis gain applied to an upstream `AmplitudeComponent`.
///
/// The gain and retained sample are runtime state; only the upstream reference
/// and policy are saved with a scene.  This is deliberately an analysis view,
/// never an audible audio-graph gain node.
#[derive(Debug, Clone)]
pub struct VolumeNormalizationComponent {
    pub source: Option<ComponentRef>,
    pub enabled: bool,
    pub min_gain_db: f32,
    pub max_gain_db: f32,
    pub target_rms_low: f32,
    pub target_rms_high: f32,
    pub activity_gate: f32,
    pub quiet_hold_sec: f32,
    pub high_hold_sec: f32,
    pub gain_rise_db_per_sec: f32,
    pub gain_fall_db_per_sec: f32,
    pub peak_headroom: f32,

    pub generation: u64,
    pub retained: AmplitudeSample,
    pub current_gain_db: f32,
    pub adjustment_reason: VolumeNormalizationReason,
    component: Option<ComponentId>,
}

impl Default for VolumeNormalizationComponent {
    fn default() -> Self {
        Self {
            source: None,
            enabled: true,
            min_gain_db: 0.0,
            max_gain_db: 24.0,
            target_rms_low: 0.03,
            target_rms_high: 0.09,
            // Admit quieter ordinary speech while still freezing adaptation
            // below a conservative room-noise floor. This is 40% below the
            // original 0.005 first-slice calibration.
            activity_gate: 0.003,
            quiet_hold_sec: 0.75,
            high_hold_sec: 0.20,
            gain_rise_db_per_sec: 3.0,
            gain_fall_db_per_sec: 12.0,
            peak_headroom: 0.9,
            generation: 0,
            retained: AmplitudeSample::pending(0),
            current_gain_db: 0.0,
            adjustment_reason: VolumeNormalizationReason::Holding,
            component: None,
        }
    }
}

impl VolumeNormalizationComponent {
    pub fn from(source: ComponentRef) -> Self {
        Self {
            source: Some(source),
            ..Self::default()
        }
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.bump_generation(AmplitudeStatus::Invalid);
        }
        self
    }

    pub fn with_gain_limits(mut self, min_gain_db: f32, max_gain_db: f32) -> Result<Self, String> {
        if !min_gain_db.is_finite() || !max_gain_db.is_finite() || min_gain_db > max_gain_db {
            return Err(
                "VolumeNormalization.gain_limits(min_db, max_db) requires finite min_db <= max_db"
                    .into(),
            );
        }
        self.min_gain_db = min_gain_db;
        self.max_gain_db = max_gain_db;
        self.restart_after_policy_change();
        Ok(self)
    }

    pub fn with_target_rms(mut self, low: f32, high: f32) -> Result<Self, String> {
        if !low.is_finite() || !high.is_finite() || low < 0.0 || low >= high {
            return Err(
                "VolumeNormalization.target_rms(low, high) requires finite 0 <= low < high".into(),
            );
        }
        self.target_rms_low = low;
        self.target_rms_high = high;
        self.restart_after_policy_change();
        Ok(self)
    }

    pub(crate) fn with_nonnegative(
        mut self,
        field: &'static str,
        value: f32,
    ) -> Result<Self, String> {
        if !value.is_finite() || value < 0.0 {
            return Err(format!(
                "VolumeNormalization.{field}(value) requires a finite non-negative value"
            ));
        }
        match field {
            "activity_gate" => self.activity_gate = value,
            "quiet_hold" => self.quiet_hold_sec = value,
            "high_hold" => self.high_hold_sec = value,
            "gain_rise" => self.gain_rise_db_per_sec = value,
            "gain_fall" => self.gain_fall_db_per_sec = value,
            _ => unreachable!("only internal fixed field names are passed"),
        }
        self.restart_after_policy_change();
        Ok(self)
    }

    pub fn with_peak_headroom(mut self, value: f32) -> Result<Self, String> {
        if !value.is_finite() || !(0.0 < value && value <= 1.0) {
            return Err(
                "VolumeNormalization.peak_headroom(value) requires a finite value in (0, 1]".into(),
            );
        }
        self.peak_headroom = value;
        self.restart_after_policy_change();
        Ok(self)
    }

    pub(crate) fn policy(&self) -> VolumeNormalizationPolicy {
        VolumeNormalizationPolicy {
            min_gain_db: self.min_gain_db,
            max_gain_db: self.max_gain_db,
            target_rms_low: self.target_rms_low,
            target_rms_high: self.target_rms_high,
            activity_gate: self.activity_gate,
            quiet_hold_sec: self.quiet_hold_sec,
            high_hold_sec: self.high_hold_sec,
            gain_rise_db_per_sec: self.gain_rise_db_per_sec,
            gain_fall_db_per_sec: self.gain_fall_db_per_sec,
            peak_headroom: self.peak_headroom,
        }
    }

    fn restart_after_policy_change(&mut self) {
        self.bump_generation(if self.enabled {
            AmplitudeStatus::Pending
        } else {
            AmplitudeStatus::Invalid
        });
    }

    pub fn bump_generation(&mut self, status: AmplitudeStatus) {
        self.generation = self.generation.wrapping_add(1);
        self.current_gain_db = self.min_gain_db;
        self.adjustment_reason = if status == AmplitudeStatus::Invalid {
            VolumeNormalizationReason::Invalid
        } else {
            VolumeNormalizationReason::Holding
        };
        self.retained = if status == AmplitudeStatus::Pending {
            AmplitudeSample::pending(self.generation)
        } else {
            AmplitudeSample::neutral(self.generation, status)
        };
    }
}

impl Component for VolumeNormalizationComponent {
    fn name(&self) -> &'static str {
        "volume_normalization"
    }
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let source = self
            .source
            .as_ref()
            .map(|source| match source {
                ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
                ComponentRef::Query(query) => s(query),
            })
            .unwrap_or_else(|| s(""));
        let mut out = ce_call("VolumeNormalization", "from", vec![source]);
        if !self.enabled {
            out = out.with_call("enabled", vec![b(false)]);
        }
        if self.min_gain_db != 0.0 || self.max_gain_db != 24.0 {
            out = out.with_call(
                "gain_limits",
                vec![num(self.min_gain_db as f64), num(self.max_gain_db as f64)],
            );
        }
        if self.target_rms_low != 0.03 || self.target_rms_high != 0.09 {
            out = out.with_call(
                "target_rms",
                vec![
                    num(self.target_rms_low as f64),
                    num(self.target_rms_high as f64),
                ],
            );
        }
        if self.activity_gate != 0.003 {
            out = out.with_call("activity_gate", vec![num(self.activity_gate as f64)]);
        }
        if self.quiet_hold_sec != 0.75 {
            out = out.with_call("quiet_hold", vec![num(self.quiet_hold_sec as f64)]);
        }
        if self.high_hold_sec != 0.20 {
            out = out.with_call("high_hold", vec![num(self.high_hold_sec as f64)]);
        }
        if self.gain_rise_db_per_sec != 3.0 {
            out = out.with_call("gain_rise", vec![num(self.gain_rise_db_per_sec as f64)]);
        }
        if self.gain_fall_db_per_sec != 12.0 {
            out = out.with_call("gain_fall", vec![num(self.gain_fall_db_per_sec as f64)]);
        }
        if self.peak_headroom != 0.9 {
            out = out.with_call("peak_headroom", vec![num(self.peak_headroom as f64)]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_activity_gate_admits_quieter_speech() {
        assert_eq!(VolumeNormalizationComponent::default().activity_gate, 0.003);
    }

    #[test]
    fn rejects_invalid_policy_ranges() {
        assert!(
            VolumeNormalizationComponent::default()
                .with_gain_limits(4.0, 3.0)
                .is_err()
        );
        assert!(
            VolumeNormalizationComponent::default()
                .with_target_rms(0.1, 0.1)
                .is_err()
        );
        assert!(
            VolumeNormalizationComponent::default()
                .with_peak_headroom(0.0)
                .is_err()
        );
    }
}
