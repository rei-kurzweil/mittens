use super::{Component, ComponentRef};
use crate::engine::ecs::ComponentId;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[derive(Debug)]
pub(crate) struct LiveHighPassParameters {
    cutoff_bits: AtomicU32,
    resonance_bits: AtomicU32,
    sequence: AtomicU64,
}

impl LiveHighPassParameters {
    fn new() -> Self {
        Self {
            cutoff_bits: AtomicU32::new(0),
            resonance_bits: AtomicU32::new(0.707_f32.to_bits()),
            sequence: AtomicU64::new(0),
        }
    }

    fn store(&self, cutoff_hz: Option<f32>, q: f32, generation: u64) {
        self.sequence
            .store(generation.wrapping_mul(2).wrapping_add(1), Ordering::SeqCst);
        self.cutoff_bits
            .store(cutoff_hz.unwrap_or(0.0).to_bits(), Ordering::SeqCst);
        self.resonance_bits.store(q.to_bits(), Ordering::SeqCst);
        self.sequence
            .store(generation.wrapping_mul(2), Ordering::SeqCst);
    }

    pub(crate) fn load(&self) -> Option<(f32, f32, u64)> {
        let first = self.sequence.load(Ordering::SeqCst);
        if first & 1 != 0 {
            return None;
        }
        let cutoff = f32::from_bits(self.cutoff_bits.load(Ordering::SeqCst));
        let q = f32::from_bits(self.resonance_bits.load(Ordering::SeqCst));
        let second = self.sequence.load(Ordering::SeqCst);
        (first == second).then_some((cutoff, q, second / 2))
    }
}

/// The main-thread validity state of an amplitude observation.
///
/// This is intentionally independent from a capture stream's lifetime: the
/// component retains the last accepted snapshot while the audio runtime owns
/// its worker and real-time buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmplitudeStatus {
    /// No source sample has been accepted for this component generation.
    Pending,
    /// The retained sample is current and safe for a main-thread consumer.
    Live,
    /// The source is silent or an observation has been deliberately neutralized.
    Neutral,
    /// A discontinuity, source failure, disable, or removal invalidated the sample.
    Invalid,
}

/// A bounded PCM measurement transferred from the source runtime to the main
/// thread. It is retained on `AmplitudeComponent`, never serialized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmplitudeSample {
    pub generation: u64,
    pub sequence: u64,
    pub timestamp_sec: f64,
    pub valid_frames: u32,
    pub rms: f32,
    pub peak: f32,
    pub status: AmplitudeStatus,
}

impl AmplitudeSample {
    pub fn pending(generation: u64) -> Self {
        Self {
            generation,
            sequence: 0,
            timestamp_sec: 0.0,
            valid_frames: 0,
            rms: 0.0,
            peak: 0.0,
            status: AmplitudeStatus::Pending,
        }
    }

    pub fn neutral(generation: u64, status: AmplitudeStatus) -> Self {
        debug_assert!(matches!(
            status,
            AmplitudeStatus::Neutral | AmplitudeStatus::Invalid
        ));
        Self {
            status,
            ..Self::pending(generation)
        }
    }

    pub fn is_live(&self) -> bool {
        self.status == AmplitudeStatus::Live
            && self.timestamp_sec.is_finite()
            && self.rms.is_finite()
            && self.peak.is_finite()
            && self.rms >= 0.0
            && self.peak >= 0.0
    }
}

/// Source-bound rolling RMS observer. The authored reference and requested
/// window survive serialization; all measurement state is main-thread runtime
/// state and is deliberately omitted from scene data.
#[derive(Debug, Clone)]
pub struct AmplitudeComponent {
    pub source: Option<ComponentRef>,
    pub window_sec: f32,
    pub highpass_hz: Option<f32>,
    pub highpass_resonance: f32,
    pub(crate) live_highpass: Arc<LiveHighPassParameters>,
    pub enabled: bool,
    pub generation: u64,
    pub retained: AmplitudeSample,
    /// Runtime cache populated by `AmplitudeSystem`. The durable `source`
    /// reference is resolved only on initial binding or after this target dies.
    pub(crate) resolved_source: Option<ComponentId>,
    component: Option<ComponentId>,
}

impl Default for AmplitudeComponent {
    fn default() -> Self {
        // A quarter-second window is the first-slice authoring default and
        // matches the documented AVC example. Authors may always choose an
        // explicit window with `Amplitude.rolling_window(seconds)`.
        Self::rolling_window(0.25).expect("constant default window is valid")
    }
}

impl AmplitudeComponent {
    pub fn rolling_window(window_sec: f32) -> Result<Self, String> {
        if !window_sec.is_finite() || window_sec <= 0.0 {
            return Err(
                "Amplitude.rolling_window(seconds) requires a finite positive window".into(),
            );
        }
        Ok(Self {
            source: None,
            window_sec,
            highpass_hz: None,
            highpass_resonance: 0.707,
            live_highpass: Arc::new(LiveHighPassParameters::new()),
            enabled: true,
            generation: 0,
            retained: AmplitudeSample::pending(0),
            resolved_source: None,
            component: None,
        })
    }

    pub fn with_source(mut self, source: ComponentRef) -> Self {
        self.source = Some(source);
        self.resolved_source = None;
        self
    }

    pub fn with_highpass(mut self, cutoff_hz: f32) -> Result<Self, String> {
        if !cutoff_hz.is_finite() || cutoff_hz <= 0.0 {
            return Err("Amplitude.highpass requires a finite positive cutoff in Hz".into());
        }
        if self.highpass_hz != Some(cutoff_hz) {
            self.highpass_hz = Some(cutoff_hz);
            self.bump_generation(AmplitudeStatus::Pending);
        }
        Ok(self)
    }

    pub fn with_highpass_resonance(mut self, q: f32) -> Result<Self, String> {
        if self.highpass_hz.is_none() {
            return Err("Amplitude.highpass_resonance requires highpass first".into());
        }
        if !q.is_finite() || !(0.1..=10.0).contains(&q) {
            return Err("Amplitude.highpass_resonance requires Q in 0.1..=10".into());
        }
        if self.highpass_resonance != q {
            self.highpass_resonance = q;
            self.bump_generation(AmplitudeStatus::Pending);
        }
        Ok(self)
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.bump_generation(AmplitudeStatus::Invalid);
        }
        self
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }

    /// Begins a fresh observation epoch. Called by the future control plane on
    /// reconfiguration and lifecycle changes so queued old samples are rejected.
    pub fn bump_generation(&mut self, status: AmplitudeStatus) {
        self.generation = self.generation.wrapping_add(1);
        self.live_highpass
            .store(self.highpass_hz, self.highpass_resonance, self.generation);
        self.retained = if status == AmplitudeStatus::Pending {
            AmplitudeSample::pending(self.generation)
        } else {
            AmplitudeSample::neutral(self.generation, status)
        };
    }
}

impl Component for AmplitudeComponent {
    fn name(&self) -> &'static str {
        "amplitude"
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
        let mut out = ce_call(
            "Amplitude",
            "rolling_window",
            vec![num(self.window_sec as f64)],
        );
        if let Some(cutoff_hz) = self.highpass_hz {
            out = out.with_call("highpass", vec![num(cutoff_hz as f64)]);
            if self.highpass_resonance != 0.707 {
                out = out.with_call(
                    "highpass_resonance",
                    vec![num(self.highpass_resonance as f64)],
                );
            }
        }
        if let Some(source) = &self.source {
            let source = match source {
                ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
                ComponentRef::Query(query) => s(query),
            };
            out = out.with_call("from", vec![source]);
        }
        if !self.enabled {
            out = out.with_call("enabled", vec![b(false)]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_windows() {
        assert!(AmplitudeComponent::rolling_window(0.0).is_err());
        assert!(AmplitudeComponent::rolling_window(f32::NAN).is_err());
        assert!(AmplitudeComponent::default().with_highpass(0.0).is_err());
        assert!(
            AmplitudeComponent::default()
                .with_highpass(f32::NAN)
                .is_err()
        );
        assert!(
            AmplitudeComponent::default()
                .with_highpass_resonance(0.7)
                .is_err()
        );
        assert!(
            AmplitudeComponent::default()
                .with_highpass(120.0)
                .unwrap()
                .with_highpass_resonance(11.0)
                .is_err()
        );
    }

    #[test]
    fn enable_change_invalidates_the_previous_generation() {
        let c = AmplitudeComponent::rolling_window(0.25)
            .unwrap()
            .with_enabled(false);
        assert_eq!(c.generation, 1);
        assert_eq!(c.retained.status, AmplitudeStatus::Invalid);
        assert!(!c.retained.is_live());
    }
}
