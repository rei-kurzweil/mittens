use crate::engine::ecs::{ComponentId, World};

/// Signal envelope.
///
/// Exactly one of `event` or `intent` should be `Some`.
#[derive(Debug, Clone)]
pub struct Signal {
    pub scope: ComponentId,
    pub event: Option<EventSignal>,
    pub intent: Option<IntentSignal>,
}

impl Signal {
    pub fn event(scope: ComponentId, event: EventSignal) -> Self {
        Self {
            scope,
            event: Some(event),
            intent: None,
        }
    }

    pub fn intent(scope: ComponentId, intent: IntentSignal) -> Self {
        Self {
            scope,
            event: None,
            intent: Some(intent),
        }
    }

    pub fn kind(&self) -> Option<SignalKind> {
        self.event.as_ref().map(|e| e.kind())
    }
}

/// Event signal: a fact/observation.
#[derive(Debug, Clone)]
pub enum EventSignal {
    /// Topology changed.
    ParentChanged {
        child: ComponentId,
        old_parent: Option<ComponentId>,
        new_parent: Option<ComponentId>,
    },

    /// A raycast intersected a renderable.
    RayIntersected {
        raycaster: ComponentId,
        renderable: ComponentId,
        t: f32,
        origin: [f32; 3],
        dir: [f32; 3],
    },

    /// Two collision objects began overlapping this tick.
    ///
    /// `delta` is the vector from `a` to `b` in world space: `pos(b) - pos(a)`.
    CollisionStarted {
        a: ComponentId,
        b: ComponentId,
        delta: [f32; 3],
    },

    /// Two collision objects stopped overlapping this tick.
    ///
    /// `delta` is the last known vector from `a` to `b` in world space: `pos(b) - pos(a)`.
    CollisionEnded {
        a: ComponentId,
        b: ComponentId,
        delta: [f32; 3],
    },

    /// A drag gesture started.
    DragStart {
        raycaster: ComponentId,
        renderable: ComponentId,
        hit_point: [f32; 3],

        /// World-space ray direction at the time the drag started.
        ///
        /// This makes DragStart self-contained for consumers that need a stable plane normal
        /// (e.g. gizmo debug drag plane / plane-projection drags) without also observing
        /// RayIntersected events.
        ray_dir_world: [f32; 3],

        /// Optional screen-space cursor/pointer position in pixels.
        ///
        /// Present for screen-space pointers (mouse/touch). Absent for non-screen pointers.
        screen_pos_px: Option<(f32, f32)>,
    },

    /// A drag gesture moved this tick.
    ///
    /// `delta_world` is the world-space movement since the last DragMove for this gesture.
    DragMove {
        raycaster: ComponentId,
        renderable: ComponentId,
        hit_point: [f32; 3],
        delta_world: [f32; 3],

        /// Optional screen-space cursor/pointer position in pixels.
        screen_pos_px: Option<(f32, f32)>,

        /// Optional pixel delta since the previous DragMove for this drag.
        ///
        /// Present for screen-space pointers (mouse/touch) when previous screen position is
        /// known. Absent for non-screen pointers.
        screen_delta_px: Option<(f32, f32)>,
    },

    /// A drag gesture ended.
    DragEnd {
        raycaster: ComponentId,
        renderable: ComponentId,
        hit_point: Option<[f32; 3]>,
    },
}

impl EventSignal {
    pub fn kind(&self) -> SignalKind {
        match self {
            EventSignal::ParentChanged { .. } => SignalKind::ParentChanged,
            EventSignal::RayIntersected { .. } => SignalKind::RayIntersected,
            EventSignal::CollisionStarted { .. } => SignalKind::CollisionStarted,
            EventSignal::CollisionEnded { .. } => SignalKind::CollisionEnded,
            EventSignal::DragStart { .. } => SignalKind::DragStart,
            EventSignal::DragMove { .. } => SignalKind::DragMove,
            EventSignal::DragEnd { .. } => SignalKind::DragEnd,
        }
    }
}

/// Intent signal: requests side effects.
#[derive(Debug, Clone)]
pub struct IntentSignal {
    pub when: SignalWhen,
    pub value: IntentValue,
}

impl IntentSignal {
    pub fn now(value: IntentValue) -> Self {
        Self {
            when: SignalWhen::Now,
            value,
        }
    }

    pub fn at_beat(beat: f64, value: IntentValue) -> Self {
        if !beat.is_finite() {
            return Self::now(value);
        }
        Self {
            when: SignalWhen::AtBeat(beat),
            value,
        }
    }
}

/// Intent payload: both user-facing intent and internal mutation commands live here for now.
#[derive(Debug, Clone)]
pub enum IntentValue {
    Noop,
    Print { message: String },

    SetColor {
        target: Vec<ComponentId>,
        rgba: [f32; 4],
    },
    SetText {
        target: Vec<ComponentId>,
        text: String,
    },
    SetPosition {
        target: Vec<ComponentId>,
        position: [f32; 3],
    },
    SetTransform {
        target: Vec<ComponentId>,
        translation: [f32; 3],
        rotation_quat_xyzw: [f32; 4],
        scale: [f32; 3],
    },

    Attach {
        parents: Vec<ComponentId>,
        child: ComponentId,
    },
    AttachClone {
        parents: Vec<ComponentId>,
        prefab_root: ComponentId,
    },
    Detach { target: Vec<ComponentId> },
    RemoveChild {
        parents: Vec<ComponentId>,
        index: usize,
    },
    RemoveChildren { parents: Vec<ComponentId> },
    RemoveSubtree { target: Vec<ComponentId> },

    AudioGraphRebuild { target: Vec<ComponentId> },
    RequestRaycast { target: Vec<ComponentId> },

    AudioLowPassSetCutoffHz {
        target: Vec<ComponentId>,
        cutoff_hz: f32,
    },
    AudioBandPassSetCenterHz {
        target: Vec<ComponentId>,
        center_hz: f32,
    },
    OscillatorSetEnabled {
        target: Vec<ComponentId>,
        enabled: bool,
    },
    OscillatorSetPitch {
        target: Vec<ComponentId>,
        frequency_hz: f32,
    },

    /// Schedule a pitch set at beat = beat_context + beat_offset.
    OscillatorScheduleSetPitch {
        target: Vec<ComponentId>,
        beat_offset: f64,
        beat_context: Option<f64>,
        frequency_hz: f32,
    },

    /// Schedule a musical note at beat = beat_context + beat_offset.
    OscillatorScheduleSetNote {
        target: Vec<ComponentId>,
        beat_offset: f64,
        beat_context: Option<f64>,
        pitch: crate::engine::ecs::component::NotePitch,
        octave: u16,
        duration_beats: f32,
    },

    /// Schedule a note represented by a MusicNote payload at beat = beat_context + beat_offset.
    OscillatorScheduleMusicNote {
        target: Vec<ComponentId>,
        beat_offset: f64,
        beat_context: Option<f64>,
        note: crate::engine::ecs::component::MusicNote,
    },

    MusicSetNote {
        target: Vec<ComponentId>,
        note: crate::engine::ecs::component::MusicNote,
    },

    RegisterRenderable { component: ComponentId },
    RemoveRenderable { component: ComponentId },
    RegisterTransform { component: ComponentId },
    UpdateTransform {
        component: ComponentId,
        translation: [f32; 3],
        rotation_quat_xyzw: [f32; 4],
        scale: [f32; 3],
    },
    RemoveTransform { component: ComponentId },

    RegisterCamera3d { component: ComponentId },
    RegisterCamera2d { component: ComponentId },
    MakeActiveCamera { component: ComponentId },

    RegisterInput { component: ComponentId },
    RegisterUv { component: ComponentId },

    RegisterLight { component: ComponentId },
    RegisterColor { component: ComponentId },
    RegisterOpacity { component: ComponentId },
    RegisterTransparentCutout { component: ComponentId },
    RegisterBackgroundColor { component: ComponentId },
    RegisterAmbientLight { component: ComponentId },
    RegisterEmissive { component: ComponentId },
    RegisterLightQuantization { component: ComponentId },

    RegisterTexture { component: ComponentId },
    RegisterTextureFiltering { component: ComponentId },

    RegisterText { component: ComponentId },

    RegisterCollision { component: ComponentId },
    RemoveCollision { component: ComponentId },
    RegisterKineticResponse { component: ComponentId },
    RemoveKineticResponse { component: ComponentId },

    RegisterOpenxr { component: ComponentId },
    RegisterControllerXr { component: ComponentId },
    RemoveControllerXr { component: ComponentId },

    RegisterRaycast { component: ComponentId },
    RemoveRaycast { component: ComponentId },

    RegisterAnimation { component: ComponentId },
    RegisterKeyframe { component: ComponentId },

    RegisterAudioOutput { component: ComponentId },
    AudioGraphDirtyImmediate { component: ComponentId },
    RegisterAudioOscillator { component: ComponentId },
    RegisterAudioBufferSize { component: ComponentId },
    RegisterClock { component: ComponentId },
    RegisterTransformGizmo { component: ComponentId },

    RegisterEditor { component: ComponentId },

    RegisterAction { component: ComponentId },

    ScheduleAudioOp {
        component: ComponentId,
        beat: f64,
        op: crate::engine::ecs::system::audio_system::AudioOp,
    },
    ScheduleAudioGraphSwap { component: ComponentId, beat: f64 },
    ScheduleAudioPitchSetHz {
        component: ComponentId,
        beat: f64,
        frequency_hz: f32,
    },
    ScheduleAudioOscillatorEnabled {
        component: ComponentId,
        beat: f64,
        enabled: bool,
    },
    ScheduleAudioGainSet {
        component: ComponentId,
        beat: f64,
        gain: f32,
    },
}

/// Event kinds used for handler routing.
///
/// Note: `SignalKind::Action` intentionally does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    Any,
    ParentChanged,
    RayIntersected,
    CollisionStarted,
    CollisionEnded,
    DragStart,
    DragMove,
    DragEnd,
}

/// Optional timing metadata on the signal envelope.
///
/// Semantics:
/// - `Now`: signal is eligible for execution/dispatch immediately at the next drain point.
/// - `AtBeat(b)`: signal is held in a pending queue until the transport beat is >= `b`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SignalWhen {
    Now,
    AtBeat(f64),
}

impl Default for SignalWhen {
    fn default() -> Self {
        Self::Now
    }
}

impl SignalWhen {
    pub fn at_beat(beat: f64) -> Self {
        Self::AtBeat(beat)
    }

    pub fn beat(&self) -> Option<f64> {
        match *self {
            Self::Now => None,
            Self::AtBeat(b) => Some(b),
        }
    }
}

pub trait SignalEmitter {
    fn push_event(&mut self, scope: ComponentId, event: EventSignal);
    fn push_intent(&mut self, scope: ComponentId, intent: IntentSignal);

    fn push_intent_now(&mut self, scope: ComponentId, value: IntentValue) {
        self.push_intent(scope, IntentSignal::now(value));
    }

    fn push_intent_at_beat(&mut self, scope: ComponentId, beat: f64, value: IntentValue) {
        self.push_intent(scope, IntentSignal::at_beat(beat, value));
    }
}

pub type SignalHandler = fn(&mut World, &mut dyn SignalEmitter, &Signal);
