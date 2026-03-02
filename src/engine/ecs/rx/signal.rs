use crate::engine::ecs::CommandQueue;
use crate::engine::ecs::{ComponentId, World};

/// A single unified signal stream.
///
/// - "Actions" are just signal variants that request side effects.
/// - "Facts" (what were previously called events) are just signal variants that describe
///   something that happened.
#[derive(Debug, Clone)]
pub enum SignalValue {
    // --- User-facing / side-effectful signals (handled by ActionSystem) ---
    Noop,
    Print {
        message: String,
    },

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
    Detach {
        target: Vec<ComponentId>,
    },
    RemoveChild {
        parents: Vec<ComponentId>,
        index: usize,
    },
    RemoveChildren {
        parents: Vec<ComponentId>,
    },
    RemoveSubtree {
        target: Vec<ComponentId>,
    },

    AudioGraphRebuild {
        target: Vec<ComponentId>,
    },

    /// Request a raycast on the target RayCastComponent(s) this frame.
    RequestRaycast {
        target: Vec<ComponentId>,
    },

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

    /// Escape hatch to invoke legacy command queue commands.
    CommandQueue {
        target: Vec<ComponentId>,
        command_name: String,
        params: Vec<serde_json::Value>,
    },

    // --- Facts (what used to be called events) ---
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    Any,
    Action,
    ParentChanged,
    RayIntersected,
    CollisionStarted,
    CollisionEnded,
    DragStart,
    DragMove,
    DragEnd,
}

impl SignalValue {
    pub fn kind(&self) -> SignalKind {
        match self {
            // Side-effectful signals.
            SignalValue::Noop
            | SignalValue::Print { .. }
            | SignalValue::SetColor { .. }
            | SignalValue::SetText { .. }
            | SignalValue::SetPosition { .. }
            | SignalValue::SetTransform { .. }
            | SignalValue::Attach { .. }
            | SignalValue::AttachClone { .. }
            | SignalValue::Detach { .. }
            | SignalValue::RemoveChild { .. }
            | SignalValue::RemoveChildren { .. }
            | SignalValue::RemoveSubtree { .. }
            | SignalValue::AudioGraphRebuild { .. }
            | SignalValue::RequestRaycast { .. }
            | SignalValue::AudioLowPassSetCutoffHz { .. }
            | SignalValue::AudioBandPassSetCenterHz { .. }
            | SignalValue::OscillatorSetEnabled { .. }
            | SignalValue::OscillatorSetPitch { .. }
            | SignalValue::OscillatorScheduleSetPitch { .. }
            | SignalValue::OscillatorScheduleSetNote { .. }
            | SignalValue::OscillatorScheduleMusicNote { .. }
            | SignalValue::MusicSetNote { .. }
            | SignalValue::CommandQueue { .. } => SignalKind::Action,

            // Facts.
            SignalValue::ParentChanged { .. } => SignalKind::ParentChanged,
            SignalValue::RayIntersected { .. } => SignalKind::RayIntersected,
            SignalValue::CollisionStarted { .. } => SignalKind::CollisionStarted,
            SignalValue::CollisionEnded { .. } => SignalKind::CollisionEnded,
            SignalValue::DragStart { .. } => SignalKind::DragStart,
            SignalValue::DragMove { .. } => SignalKind::DragMove,
            SignalValue::DragEnd { .. } => SignalKind::DragEnd,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub scope: ComponentId,
    pub value: SignalValue,
}

impl Signal {
    pub fn kind(&self) -> SignalKind {
        self.value.kind()
    }
}

pub trait SignalEmitter {
    fn push(&mut self, scope: ComponentId, value: SignalValue);
}

pub type SignalHandler = fn(&mut World, &mut CommandQueue, &mut dyn SignalEmitter, &Signal);
