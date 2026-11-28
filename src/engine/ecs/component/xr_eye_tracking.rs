use super::Component;
use crate::engine::ecs::ComponentId;

/// A transport implementation that can provide normalized eye-tracking data.
///
/// The generic `XREyeTracking` selector will use this enum for authored source
/// priority. `MediaPipe` is reserved now so scenes can describe their intended
/// ordering before the webcam transport is implemented.
///
/// `NativeOpenXr` is the host's own OpenXR session (for example the Android
/// build's XR_HTC_facial_tracking on a Vive Focus 3). `Htc` is the bespoke
/// binary UDP receiver, not native HTC tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EyeTrackingSource {
    NativeOpenXr,
    Htc,
    VrChatOsc,
    MediaPipe,
}

impl EyeTrackingSource {
    /// Native data wins when the host provides it; otherwise the first
    /// transport that is actually delivering samples is used.
    pub const DEFAULT_PRIORITY: [Self; 4] = [
        Self::NativeOpenXr,
        Self::Htc,
        Self::VrChatOsc,
        Self::MediaPipe,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "openxr" => Some(Self::NativeOpenXr),
            "htc" => Some(Self::Htc),
            "vrchat_osc" => Some(Self::VrChatOsc),
            "mediapipe" => Some(Self::MediaPipe),
            _ => None,
        }
    }
}

/// Declares the coordinate convention used by an eye-tracking transport.
///
/// `CancelHeadRotation` treats reported directions as world-relative and
/// converts them into the target eye bone's parent-local basis in AVC.  The
/// transport system intentionally retains the raw direction: only AVC knows
/// the avatar hierarchy that supplies that basis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HeadRotationCompensation {
    #[default]
    Off,
    CancelHeadRotation,
}

impl HeadRotationCompensation {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "cancel" => Some(Self::CancelHeadRotation),
            _ => None,
        }
    }
}

/// Latest usable gaze directions received from an eye-tracking source.
///
/// This is runtime state, intentionally not exposed through MMS.  `sequence`
/// is assigned by `XREyeTrackingSystem` from one counter shared by both wire
/// protocols, so consumers can deterministically choose the newest source.
#[derive(Debug, Clone, Copy, Default)]
pub struct EyeGazeSample {
    pub left: Option<[f32; 3]>,
    pub right: Option<[f32; 3]>,
    pub sequence: u64,
}

/// Latest normalized per-eye closure values (`0 = open`, `1 = closed`).
///
/// It has its own sequence because closure packets are independent from gaze
/// packets. Transports with one combined value duplicate it into both eyes;
/// transports with independent openness preserve each eye after conversion.
#[derive(Debug, Clone, Copy, Default)]
pub struct EyeClosureSample {
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub sequence: u64,
}

/// Independent angular caps around the head-local forward axis (`-Z`).
/// Values are radians and are validated by the MMS builder before storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EyeRotationLimits {
    pub left: f32,
    pub right: f32,
    pub up: f32,
    pub down: f32,
}

impl EyeRotationLimits {
    pub const fn from_array(values: [f32; 4]) -> Self {
        Self {
            left: values[0],
            right: values[1],
            up: values[2],
            down: values[3],
        }
    }

    /// Apply both configured policies, taking the stricter cap per direction.
    pub fn tighter(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            right: self.right.min(other.right),
            up: self.up.min(other.up),
            down: self.down.min(other.down),
        }
    }
}

pub fn combined_eye_rotation_limits(
    shared: Option<EyeRotationLimits>,
    per_eye: Option<EyeRotationLimits>,
) -> Option<EyeRotationLimits> {
    match (shared, per_eye) {
        (Some(shared), Some(per_eye)) => Some(shared.tighter(per_eye)),
        (Some(limits), None) | (None, Some(limits)) => Some(limits),
        (None, None) => None,
    }
}

/// Transport-neutral eye-tracking selector.
///
/// Source-specific components are direct children of this component. The
/// eye-tracking system creates default children for priority entries that do
/// not have an authored configuration and copies the selected normalized
/// samples here for AVC to consume.
#[derive(Debug, Clone)]
pub struct XREyeTrackingComponent {
    pub priority: Vec<EyeTrackingSource>,
    /// Whether retained gaze samples may rotate mapped avatar eye bones.
    ///
    /// This deliberately does not affect transport polling, samples, events,
    /// or closure-driven blink morphs.  "Pupil direction" is the historical
    /// user-facing name for the mapped-eye-bone rotation path; it is not the
    /// HTC 2D pupil-position signal.
    pub enable_pupil_direction_tracking: bool,
    pub head_rotation_compensation: HeadRotationCompensation,
    pub rotation_limits: Option<EyeRotationLimits>,
    pub rotation_limits_per_eye: [Option<EyeRotationLimits>; 2],
    pub(crate) legacy_osc_endpoint: Option<(String, u16)>,
    pub(crate) gaze_sample: EyeGazeSample,
    pub(crate) closure_sample: EyeClosureSample,
    pub(crate) gaze_source: Option<EyeTrackingSource>,
    pub(crate) closure_source: Option<EyeTrackingSource>,
}

impl XREyeTrackingComponent {
    pub fn on() -> Self {
        Self {
            priority: EyeTrackingSource::DEFAULT_PRIORITY.to_vec(),
            enable_pupil_direction_tracking: true,
            head_rotation_compensation: HeadRotationCompensation::Off,
            rotation_limits: None,
            rotation_limits_per_eye: [None; 2],
            legacy_osc_endpoint: None,
            gaze_sample: EyeGazeSample::default(),
            closure_sample: EyeClosureSample::default(),
            gaze_source: None,
            closure_source: None,
        }
    }

    /// Compatibility constructor for the old OSC-specific API.
    pub fn listen(host: impl Into<String>, port: u16) -> Self {
        let mut component = Self::on();
        component.priority = vec![EyeTrackingSource::VrChatOsc];
        component.legacy_osc_endpoint = Some((host.into(), port));
        component
    }

    pub fn with_priority(mut self, priority: Vec<EyeTrackingSource>) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_enable_pupil_direction_tracking(mut self, enabled: bool) -> Self {
        self.enable_pupil_direction_tracking = enabled;
        self
    }

    pub fn with_head_rotation_compensation(mut self, value: HeadRotationCompensation) -> Self {
        self.head_rotation_compensation = value;
        self
    }

    pub fn with_rotation_limits(mut self, values: [f32; 4]) -> Self {
        self.rotation_limits = Some(EyeRotationLimits::from_array(values));
        self
    }

    pub fn with_rotation_limits_per_eye(mut self, left: [f32; 4], right: [f32; 4]) -> Self {
        self.rotation_limits_per_eye = [
            Some(EyeRotationLimits::from_array(left)),
            Some(EyeRotationLimits::from_array(right)),
        ];
        self
    }
}

impl Default for XREyeTrackingComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl Component for XREyeTrackingComponent {
    fn name(&self) -> &'static str {
        "xr_eye_tracking"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterEyeTracking {
                component_id: component,
            },
        );
    }
    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveEyeTracking {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = ce_call("XREyeTracking", "on", vec![]);
        if self.priority != EyeTrackingSource::DEFAULT_PRIORITY {
            let priority = self
                .priority
                .iter()
                .map(|source| match source {
                    EyeTrackingSource::NativeOpenXr => s("openxr"),
                    EyeTrackingSource::Htc => s("htc"),
                    EyeTrackingSource::VrChatOsc => s("vrchat_osc"),
                    EyeTrackingSource::MediaPipe => s("mediapipe"),
                })
                .collect();
            ce = ce.with_call("priority", vec![array(priority)]);
        }
        if !self.enable_pupil_direction_tracking {
            ce = ce.with_call("enable_pupil_direction_tracking", vec![b(false)]);
        }
        ce
    }
}

#[derive(Debug, Clone)]
pub struct VRChatOSCEyeTrackingComponent {
    pub host: String,
    pub port: u16,
    /// See [`XREyeTrackingComponent::enable_pupil_direction_tracking`].
    pub enable_pupil_direction_tracking: bool,
    pub head_rotation_compensation: HeadRotationCompensation,
    pub rotation_limits: Option<EyeRotationLimits>,
    pub rotation_limits_per_eye: [Option<EyeRotationLimits>; 2],
    pub(crate) gaze_sample: EyeGazeSample,
    pub(crate) closure_sample: EyeClosureSample,
}
impl VRChatOSCEyeTrackingComponent {
    pub fn on() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 9000,
            enable_pupil_direction_tracking: true,
            head_rotation_compensation: HeadRotationCompensation::Off,
            rotation_limits: None,
            rotation_limits_per_eye: [None; 2],
            gaze_sample: EyeGazeSample::default(),
            closure_sample: EyeClosureSample::default(),
        }
    }
    pub fn listen(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            enable_pupil_direction_tracking: true,
            head_rotation_compensation: HeadRotationCompensation::Off,
            rotation_limits: None,
            rotation_limits_per_eye: [None; 2],
            gaze_sample: EyeGazeSample::default(),
            closure_sample: EyeClosureSample::default(),
        }
    }
    pub fn with_head_rotation_compensation(mut self, value: HeadRotationCompensation) -> Self {
        self.head_rotation_compensation = value;
        self
    }
    pub fn with_enable_pupil_direction_tracking(mut self, enabled: bool) -> Self {
        self.enable_pupil_direction_tracking = enabled;
        self
    }
    pub fn with_rotation_limits(mut self, values: [f32; 4]) -> Self {
        self.rotation_limits = Some(EyeRotationLimits::from_array(values));
        self
    }
    pub fn with_rotation_limits_per_eye(mut self, left: [f32; 4], right: [f32; 4]) -> Self {
        self.rotation_limits_per_eye = [
            Some(EyeRotationLimits::from_array(left)),
            Some(EyeRotationLimits::from_array(right)),
        ];
        self
    }
}
impl Default for VRChatOSCEyeTrackingComponent {
    fn default() -> Self {
        Self::on()
    }
}
impl Component for VRChatOSCEyeTrackingComponent {
    fn name(&self) -> &'static str {
        "vrchat_osc_eye_tracking"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterEyeTracking {
                component_id: component,
            },
        );
    }
    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveEyeTracking {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = if self.host == "127.0.0.1" && self.port == 9000 {
            ce_call("VRChatOSCEyeTracking", "on", vec![])
        } else {
            ce_call(
                "VRChatOSCEyeTracking",
                "listen",
                vec![s(&self.host), num(self.port as f64)],
            )
        };
        if !self.enable_pupil_direction_tracking {
            ce = ce.with_call("enable_pupil_direction_tracking", vec![b(false)]);
        }
        ce
    }
}

#[derive(Debug, Clone)]
pub struct HTCEyeTrackingComponent {
    pub host: String,
    pub port: u16,
    /// See [`XREyeTrackingComponent::enable_pupil_direction_tracking`].
    pub enable_pupil_direction_tracking: bool,
    pub head_rotation_compensation: HeadRotationCompensation,
    pub rotation_limits: Option<EyeRotationLimits>,
    pub rotation_limits_per_eye: [Option<EyeRotationLimits>; 2],
    pub(crate) gaze_sample: EyeGazeSample,
    pub(crate) closure_sample: EyeClosureSample,
}
impl HTCEyeTrackingComponent {
    pub fn on() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 9002,
            enable_pupil_direction_tracking: true,
            head_rotation_compensation: HeadRotationCompensation::Off,
            rotation_limits: None,
            rotation_limits_per_eye: [None; 2],
            gaze_sample: EyeGazeSample::default(),
            closure_sample: EyeClosureSample::default(),
        }
    }
    pub fn listen(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            enable_pupil_direction_tracking: true,
            head_rotation_compensation: HeadRotationCompensation::Off,
            rotation_limits: None,
            rotation_limits_per_eye: [None; 2],
            gaze_sample: EyeGazeSample::default(),
            closure_sample: EyeClosureSample::default(),
        }
    }
    pub fn with_head_rotation_compensation(mut self, value: HeadRotationCompensation) -> Self {
        self.head_rotation_compensation = value;
        self
    }
    pub fn with_enable_pupil_direction_tracking(mut self, enabled: bool) -> Self {
        self.enable_pupil_direction_tracking = enabled;
        self
    }
    pub fn with_rotation_limits(mut self, values: [f32; 4]) -> Self {
        self.rotation_limits = Some(EyeRotationLimits::from_array(values));
        self
    }
    pub fn with_rotation_limits_per_eye(mut self, left: [f32; 4], right: [f32; 4]) -> Self {
        self.rotation_limits_per_eye = [
            Some(EyeRotationLimits::from_array(left)),
            Some(EyeRotationLimits::from_array(right)),
        ];
        self
    }
}
impl Default for HTCEyeTrackingComponent {
    fn default() -> Self {
        Self::on()
    }
}
impl Component for HTCEyeTrackingComponent {
    fn name(&self) -> &'static str {
        "htc_eye_tracking"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterEyeTracking {
                component_id: component,
            },
        );
    }
    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveEyeTracking {
                component_id: component,
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = if self.host == "127.0.0.1" && self.port == 9002 {
            ce_call("HTCEyeTracking", "on", vec![])
        } else {
            ce_call(
                "HTCEyeTracking",
                "listen",
                vec![s(&self.host), num(self.port as f64)],
            )
        };
        if !self.enable_pupil_direction_tracking {
            ce = ce.with_call("enable_pupil_direction_tracking", vec![b(false)]);
        }
        ce
    }
}

/// Eye data from the host's own OpenXR session, delivered on the host's
/// `ExternalXrFrame` after its backend adapter has normalized it. Today that
/// is per-eye closure only; gaze stays absent until a native gaze adapter
/// exists. Created automatically for `XREyeTracking` priority `"openxr"`.
#[derive(Debug, Clone, Default)]
pub struct OpenXREyeTrackingComponent {
    pub(crate) gaze_sample: EyeGazeSample,
    pub(crate) closure_sample: EyeClosureSample,
}

impl OpenXREyeTrackingComponent {
    pub fn on() -> Self {
        Self::default()
    }
}

impl Component for OpenXREyeTrackingComponent {
    fn name(&self) -> &'static str {
        "openxr_eye_tracking"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterEyeTracking {
                component_id: component,
            },
        );
    }
    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveEyeTracking {
                component_id: component,
            },
        );
    }
}

/// Configuration anchor for the future webcam/MediaPipe eye-tracking source.
///
/// It is intentionally constructible before the transport exists so generic
/// source priority and scene topology do not need another API migration later.
/// The eye-tracking system does not currently produce samples for this type.
#[derive(Debug, Clone)]
pub struct MediaPipeEyeTrackingComponent {
    pub head_rotation_compensation: HeadRotationCompensation,
    pub rotation_limits: Option<EyeRotationLimits>,
    pub rotation_limits_per_eye: [Option<EyeRotationLimits>; 2],
}

impl MediaPipeEyeTrackingComponent {
    pub fn on() -> Self {
        Self {
            head_rotation_compensation: HeadRotationCompensation::Off,
            rotation_limits: None,
            rotation_limits_per_eye: [None; 2],
        }
    }
}

impl Default for MediaPipeEyeTrackingComponent {
    fn default() -> Self {
        Self::on()
    }
}

impl Component for MediaPipeEyeTrackingComponent {
    fn name(&self) -> &'static str {
        "mediapipe_eye_tracking"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn set_id(&mut self, _: ComponentId) {}
    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterEyeTracking {
                component_id: component,
            },
        );
    }
    fn cleanup(
        &mut self,
        emit: &mut dyn crate::engine::ecs::SignalEmitter,
        component: ComponentId,
    ) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RemoveEyeTracking {
                component_id: component,
            },
        );
    }
}

/// Compatibility name for the original HTC-specific Rust type.
pub type XREyeTrackingHtcComponent = HTCEyeTrackingComponent;

#[cfg(test)]
mod tests {
    use super::{
        EyeTrackingSource, HTCEyeTrackingComponent, VRChatOSCEyeTrackingComponent,
        XREyeTrackingComponent,
    };

    #[test]
    fn eye_tracking_source_names_and_default_priority_are_stable() {
        assert_eq!(
            EyeTrackingSource::parse("htc"),
            Some(EyeTrackingSource::Htc)
        );
        assert_eq!(
            EyeTrackingSource::parse("vrchat_osc"),
            Some(EyeTrackingSource::VrChatOsc)
        );
        assert_eq!(
            EyeTrackingSource::parse("mediapipe"),
            Some(EyeTrackingSource::MediaPipe)
        );
        assert_eq!(
            EyeTrackingSource::parse("openxr"),
            Some(EyeTrackingSource::NativeOpenXr)
        );
        assert_eq!(EyeTrackingSource::parse("osc"), None);
        assert_eq!(
            EyeTrackingSource::DEFAULT_PRIORITY,
            [
                EyeTrackingSource::NativeOpenXr,
                EyeTrackingSource::Htc,
                EyeTrackingSource::VrChatOsc,
                EyeTrackingSource::MediaPipe,
            ]
        );
    }

    #[test]
    fn pupil_direction_tracking_defaults_on_and_can_be_disabled_per_tracker() {
        assert!(XREyeTrackingComponent::on().enable_pupil_direction_tracking);
        assert!(VRChatOSCEyeTrackingComponent::on().enable_pupil_direction_tracking);
        assert!(HTCEyeTrackingComponent::on().enable_pupil_direction_tracking);
        assert!(
            !XREyeTrackingComponent::on()
                .with_enable_pupil_direction_tracking(false)
                .enable_pupil_direction_tracking
        );
    }

    #[test]
    fn disabled_pupil_direction_tracking_serializes_explicitly() {
        use crate::engine::ecs::component::Component;

        let world = crate::engine::ecs::World::default();
        for text in [
            crate::scripting::unparser::unparse_component(
                &XREyeTrackingComponent::on()
                    .with_enable_pupil_direction_tracking(false)
                    .to_mms_ast(&world),
            ),
            crate::scripting::unparser::unparse_component(
                &HTCEyeTrackingComponent::on()
                    .with_enable_pupil_direction_tracking(false)
                    .to_mms_ast(&world),
            ),
        ] {
            assert!(
                text.contains("enable_pupil_direction_tracking(false)"),
                "{text}"
            );
        }
    }
}
