use crate::engine::ecs::component::style::SizeDimension;
use crate::engine::ecs::component::{AnimationState, SelectionEntry, SelectionMode};
use crate::engine::ecs::{ComponentId, World};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputCaretDirection {
    Left,
    Right,
}

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

    /// A click: a drag gesture that ended close to where it started.
    ///
    /// Emitted by `GestureSystem` at `DragEnd` time when the net pointer displacement is below
    /// the click threshold (8 px screen-space, or 0.02 world units for non-screen pointers).
    ///
    /// All intermediate `DragMove` events are still emitted; `Click` fires *in addition to*
    /// `DragEnd`, on the same scope (the hit renderable at press time).
    ///
    /// Payload mirrors `DragStart` — the thing clicked is what was under the pointer at press.
    Click {
        raycaster: ComponentId,
        renderable: ComponentId,
        hit_point: [f32; 3],
        screen_pos_px: Option<(f32, f32)>,
    },

    /// A selection scope changed.
    SelectionChanged {
        selection_root: ComponentId,
        mode: SelectionMode,
        selected_entries: Vec<SelectionEntry>,
        selected_component: Option<ComponentId>,
        selected_payload: Option<ComponentId>,
    },

    /// An entry was added to a selection scope.
    SelectionAdded {
        selection_root: ComponentId,
        entry: SelectionEntry,
    },

    /// An entry was removed from a selection scope.
    SelectionRemoved {
        selection_root: ComponentId,
        entry: SelectionEntry,
    },

    /// A selection scope was cleared.
    SelectionCleared { selection_root: ComponentId },

    /// A scrolling component consumed drag motion and updated its offset.
    ///
    /// Scoped to the `ScrollingComponent` so downstream systems can subscribe to scroll state
    /// changes through the scroll owner rather than the ancestor drag-capture surface.
    Scrolling {
        scroll_component: ComponentId,
        drag_scope: ComponentId,
        delta_world: [f32; 3],
        scroll_offset: f32,
        max_scroll: f32,
        viewport_height: f32,
        content_height: f32,
    },

    TextInputFocusChanged {
        old: Option<ComponentId>,
        new: Option<ComponentId>,
    },

    TextInputChanged {
        component_id: ComponentId,
        text: String,
        caret: usize,
    },

    /// A `LayoutComponent` subtree finished layout and its computed size is now
    /// available in world units. Scoped to the `LayoutComponent`'s `ComponentId`.
    LayoutRootSizeAvailable {
        layout_id: ComponentId,
        width_wu: f32,
        height_wu: f32,
    },

    /// A named data event for cross-subtree communication.
    ///
    /// The `name` identifies the event kind (e.g. "asset_selected", "tool_selected").
    /// The `scope` on the `Signal` envelope identifies the shared ancestor on which
    /// handlers are registered. `payload` is an optional `ComponentId` reference.
    DataEvent {
        name: String,
        payload: Option<ComponentId>,
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
            EventSignal::Click { .. } => SignalKind::Click,
            EventSignal::SelectionChanged { .. } => SignalKind::SelectionChanged,
            EventSignal::SelectionAdded { .. } => SignalKind::SelectionAdded,
            EventSignal::SelectionRemoved { .. } => SignalKind::SelectionRemoved,
            EventSignal::SelectionCleared { .. } => SignalKind::SelectionCleared,
            EventSignal::Scrolling { .. } => SignalKind::Scrolling,
            EventSignal::TextInputFocusChanged { .. } => SignalKind::TextInputFocusChanged,
            EventSignal::TextInputChanged { .. } => SignalKind::TextInputChanged,
            EventSignal::LayoutRootSizeAvailable { .. } => SignalKind::LayoutRootSizeAvailable,
            EventSignal::DataEvent { .. } => SignalKind::DataEvent,
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

    /// Spawn a component tree described by a fully-evaluated `MaterializedCE` and optionally
    /// attach it to a parent. If `parent` is `None` the root becomes a world root.
    SpawnComponentTree {
        root: Box<crate::meow_meow::object::MaterializedCE>,
        parent: Option<ComponentId>,
    },
    Print {
        message: String,
    },

    /// Queue a REPL command to be executed on the main thread (if the REPL is enabled).
    ///
    /// This is used for editor integration (e.g. jump to the clicked component).
    ReplExec {
        command: String,
    },

    SetColor {
        component_ids: Vec<ComponentId>,
        rgba: [f32; 4],
    },
    SetText {
        component_ids: Vec<ComponentId>,
        text: String,
    },
    SetPosition {
        component_ids: Vec<ComponentId>,
        position: [f32; 3],
    },
    GLTFArmatureVisible {
        component_ids: Vec<ComponentId>,
        visible: bool,
    },
    SetLayoutAvailableWidth {
        component_ids: Vec<ComponentId>,
        width: SizeDimension,
    },
    SetLayoutAvailableHeight {
        component_ids: Vec<ComponentId>,
        height: SizeDimension,
    },
    SetLayoutInspect {
        component_ids: Vec<ComponentId>,
        enabled: bool,
    },
    SelectionSet {
        component_ids: Vec<ComponentId>,
        entries: Vec<SelectionEntry>,
        primary: Option<ComponentId>,
    },

    Attach {
        parents: Vec<ComponentId>,
        child: ComponentId,
    },
    QueryFindComponent {
        root: ComponentId,
        selector: String,
        reply: Sender<Option<ComponentId>>,
    },
    QueryFindAllComponents {
        root: ComponentId,
        selector: String,
        reply: Sender<Vec<ComponentId>>,
    },
    AttachClone {
        parents: Vec<ComponentId>,
        prefab_root: ComponentId,
    },
    Detach {
        component_ids: Vec<ComponentId>,
    },
    RemoveChild {
        parents: Vec<ComponentId>,
        index: usize,
    },
    RemoveChildren {
        parents: Vec<ComponentId>,
    },
    RemoveSubtree {
        component_ids: Vec<ComponentId>,
    },

    AudioGraphRebuild {
        component_ids: Vec<ComponentId>,
    },
    RequestRaycast {
        component_ids: Vec<ComponentId>,
    },

    AudioLowPassSetCutoffHz {
        component_ids: Vec<ComponentId>,
        cutoff_hz: f32,
    },
    AudioBandPassSetCenterHz {
        component_ids: Vec<ComponentId>,
        center_hz: f32,
    },
    OscillatorSetEnabled {
        component_ids: Vec<ComponentId>,
        enabled: bool,
    },
    OscillatorSetPitch {
        component_ids: Vec<ComponentId>,
        frequency_hz: f32,
    },

    /// Schedule a pitch set at beat = beat_context + beat_offset.
    OscillatorScheduleSetPitch {
        component_ids: Vec<ComponentId>,
        beat_offset: f64,
        beat_context: Option<f64>,
        frequency_hz: f32,
    },

    /// Unified play/trigger intent for any `AudioSource` (oscillator or clip).
    /// Fires at beat = beat_context + beat_offset.
    ///
    /// `note` carries pitch/velocity/duration semantics when meaningful.
    /// `gain` / `rate` / `duration` are generic playback overrides:
    /// - oscillator: `rate` ignored, `gain` overrides note velocity, `duration` overrides note.duration
    /// - clip: `rate` sets playback rate, `gain` overrides note velocity, `duration` overrides note.duration
    /// See docs/spec/audio-sources.md §3 and §4.
    AudioSchedulePlay {
        component_ids: Vec<ComponentId>,
        beat_offset: f64,
        beat_context: Option<f64>,
        note: Option<crate::engine::ecs::component::MusicNote>,
        gain: Option<f32>,
        rate: Option<f32>,
        duration: Option<f64>,
    },

    RegisterRenderable {
        component_ids: Vec<ComponentId>,
    },
    RemoveRenderable {
        component_ids: Vec<ComponentId>,
    },
    RegisterStencilClip {
        component_ids: Vec<ComponentId>,
    },
    UnregisterStencilClip {
        component_ids: Vec<ComponentId>,
    },

    PoseCapture {
        target: ComponentId,
        pose_name: Option<String>,
    },
    PoseApply {
        target: ComponentId,
        pose: ComponentId,
    },

    RegisterRouter {
        component_ids: Vec<ComponentId>,
    },
    RegisterScrolling {
        component_ids: Vec<ComponentId>,
    },
    RegisterTransform {
        component_ids: Vec<ComponentId>,
    },
    /// Recompute transform-derived caches (world matrices, skinning, BVH) without modifying the transform value.
    ///
    /// Intended for topology changes (e.g. Attach/Detach) where world matrices need recomputation.
    UpdateTransformWorld {
        component_ids: Vec<ComponentId>,
    },
    UpdateTransform {
        component_ids: Vec<ComponentId>,
        translation: [f32; 3],
        rotation_quat_xyzw: [f32; 4],
        scale: [f32; 3],
    },
    RemoveTransform {
        component_ids: Vec<ComponentId>,
    },

    RegisterCamera3d {
        component_ids: Vec<ComponentId>,
    },
    RegisterCamera2d {
        component_ids: Vec<ComponentId>,
    },
    MakeActiveCamera {
        component_ids: Vec<ComponentId>,
    },

    RegisterInput {
        component_ids: Vec<ComponentId>,
    },
    RegisterUv {
        component_ids: Vec<ComponentId>,
    },

    RegisterLight {
        component_ids: Vec<ComponentId>,
    },
    RegisterColor {
        component_ids: Vec<ComponentId>,
    },
    RegisterOpacity {
        component_ids: Vec<ComponentId>,
    },
    RegisterTransparentCutout {
        component_ids: Vec<ComponentId>,
    },
    RegisterBackgroundColor {
        component_ids: Vec<ComponentId>,
    },
    RegisterRendererSettings {
        component_ids: Vec<ComponentId>,
    },
    RegisterRenderGraph {
        component_ids: Vec<ComponentId>,
    },
    RegisterAmbientLight {
        component_ids: Vec<ComponentId>,
    },
    RegisterEmissive {
        component_ids: Vec<ComponentId>,
    },
    RegisterLightQuantization {
        component_ids: Vec<ComponentId>,
    },

    RegisterTexture {
        component_ids: Vec<ComponentId>,
    },
    RegisterTextureFiltering {
        component_ids: Vec<ComponentId>,
    },

    RegisterText {
        component_ids: Vec<ComponentId>,
    },
    RegisterGLTF {
        component_ids: Vec<ComponentId>,
    },
    RegisterTextInput {
        component_ids: Vec<ComponentId>,
    },

    TextInputSetFocus {
        component_id: ComponentId,
    },
    TextInputClearFocus,
    TextInputInsertText {
        text: String,
    },
    TextInputBackspace,
    TextInputDeleteForward,
    TextInputMoveCaret {
        direction: TextInputCaretDirection,
        amount: usize,
    },
    TextInputMoveCaretTo {
        index: usize,
    },

    RegisterCollision {
        component_ids: Vec<ComponentId>,
    },
    RemoveCollision {
        component_ids: Vec<ComponentId>,
    },
    RegisterKineticResponse {
        component_ids: Vec<ComponentId>,
    },
    RemoveKineticResponse {
        component_ids: Vec<ComponentId>,
    },

    RegisterOpenxr {
        component_ids: Vec<ComponentId>,
    },
    RegisterInputXr {
        component_ids: Vec<ComponentId>,
    },
    RegisterControllerXr {
        component_ids: Vec<ComponentId>,
    },
    RemoveInputXr {
        component_ids: Vec<ComponentId>,
    },
    RemoveControllerXr {
        component_ids: Vec<ComponentId>,
    },

    RegisterRaycast {
        component_ids: Vec<ComponentId>,
    },
    RegisterPointer {
        component_ids: Vec<ComponentId>,
    },
    RemoveRaycast {
        component_ids: Vec<ComponentId>,
    },

    RegisterAnimation {
        component_ids: Vec<ComponentId>,
    },
    SetAnimationState {
        component_ids: Vec<ComponentId>,
        state: AnimationState,
    },
    RegisterKeyframe {
        component_ids: Vec<ComponentId>,
    },

    RegisterAudioOutput {
        component_ids: Vec<ComponentId>,
    },
    AudioGraphDirtyImmediate {
        component_ids: Vec<ComponentId>,
    },
    RegisterAudioOscillator {
        component_ids: Vec<ComponentId>,
    },
    RegisterAudioClip {
        component_ids: Vec<ComponentId>,
    },
    RegisterAudioBufferSize {
        component_ids: Vec<ComponentId>,
    },
    RegisterClock {
        component_ids: Vec<ComponentId>,
    },
    RegisterTransformGizmo {
        component_ids: Vec<ComponentId>,
    },
    RegisterNormalVis {
        component_ids: Vec<ComponentId>,
    },

    RegisterEditor {
        component_ids: Vec<ComponentId>,
    },

    RegisterAction {
        component_ids: Vec<ComponentId>,
    },

    /// Register/unregister routing operators.
    ///
    /// These are internal mutation-style intents executed by the pipeline system.
    RegisterSignalRouteUpward {
        component_ids: Vec<ComponentId>,
    },
    RemoveSignalRouteUpward {
        component_ids: Vec<ComponentId>,
    },

    ScheduleAudioOp {
        component_ids: Vec<ComponentId>,
        beat: f64,
        op: crate::engine::ecs::system::audio_system::AudioOp,
    },
    ScheduleAudioGraphSwap {
        component_ids: Vec<ComponentId>,
        beat: f64,
    },
    ScheduleAudioPitchSetHz {
        component_ids: Vec<ComponentId>,
        beat: f64,
        frequency_hz: f32,
    },
    ScheduleAudioOscillatorEnabled {
        component_ids: Vec<ComponentId>,
        beat: f64,
        enabled: bool,
    },
    ScheduleAudioGainSet {
        component_ids: Vec<ComponentId>,
        beat: f64,
        gain: f32,
    },
}

impl IntentValue {
    /// Stable, human-readable kind name for routing/filtering.
    ///
    /// Convention: snake_case.
    pub fn kind_name(&self) -> &'static str {
        match self {
            IntentValue::Noop => "noop",
            IntentValue::SpawnComponentTree { .. } => "spawn_component_tree",
            IntentValue::Print { .. } => "print",
            IntentValue::ReplExec { .. } => "repl_exec",

            IntentValue::SetColor { .. } => "set_color",
            IntentValue::SetText { .. } => "set_text",
            IntentValue::SetPosition { .. } => "set_position",
            IntentValue::SetLayoutAvailableWidth { .. } => "set_layout_available_width",
            IntentValue::SetLayoutAvailableHeight { .. } => "set_layout_available_height",
            IntentValue::SetLayoutInspect { .. } => "set_layout_inspect",
            IntentValue::GLTFArmatureVisible { .. } => "gltf_armature_visible",
            IntentValue::SelectionSet { .. } => "selection_set",

            IntentValue::Attach { .. } => "attach",
            IntentValue::QueryFindComponent { .. } => "query_find_component",
            IntentValue::QueryFindAllComponents { .. } => "query_find_all_components",
            IntentValue::AttachClone { .. } => "attach_clone",
            IntentValue::Detach { .. } => "detach",
            IntentValue::RemoveChild { .. } => "remove_child",
            IntentValue::RemoveChildren { .. } => "remove_children",
            IntentValue::RemoveSubtree { .. } => "remove_subtree",

            IntentValue::AudioGraphRebuild { .. } => "audio_graph_rebuild",
            IntentValue::RequestRaycast { .. } => "request_raycast",

            IntentValue::AudioLowPassSetCutoffHz { .. } => "audio_low_pass_set_cutoff_hz",
            IntentValue::AudioBandPassSetCenterHz { .. } => "audio_band_pass_set_center_hz",
            IntentValue::OscillatorSetEnabled { .. } => "oscillator_set_enabled",
            IntentValue::OscillatorSetPitch { .. } => "oscillator_set_pitch",
            IntentValue::OscillatorScheduleSetPitch { .. } => "oscillator_schedule_set_pitch",
            IntentValue::AudioSchedulePlay { .. } => "audio_schedule_play",

            IntentValue::RegisterRenderable { .. } => "register_renderable",
            IntentValue::RemoveRenderable { .. } => "remove_renderable",
            IntentValue::RegisterStencilClip { .. } => "register_stencil_clip",
            IntentValue::UnregisterStencilClip { .. } => "unregister_stencil_clip",
            IntentValue::RegisterRouter { .. } => "register_router",
            IntentValue::RegisterScrolling { .. } => "register_scrolling",
            IntentValue::RegisterTransform { .. } => "register_transform",
            IntentValue::UpdateTransformWorld { .. } => "update_transform_world",
            IntentValue::UpdateTransform { .. } => "update_transform",
            IntentValue::RemoveTransform { .. } => "remove_transform",

            IntentValue::RegisterCamera3d { .. } => "register_camera3d",
            IntentValue::RegisterCamera2d { .. } => "register_camera2d",
            IntentValue::MakeActiveCamera { .. } => "make_active_camera",

            IntentValue::RegisterInput { .. } => "register_input",
            IntentValue::RegisterUv { .. } => "register_uv",

            IntentValue::RegisterLight { .. } => "register_light",
            IntentValue::RegisterColor { .. } => "register_color",
            IntentValue::RegisterOpacity { .. } => "register_opacity",
            IntentValue::RegisterTransparentCutout { .. } => "register_transparent_cutout",
            IntentValue::RegisterBackgroundColor { .. } => "register_background_color",
            IntentValue::RegisterRendererSettings { .. } => "register_renderer_settings",
            IntentValue::RegisterRenderGraph { .. } => "register_render_graph",
            IntentValue::RegisterAmbientLight { .. } => "register_ambient_light",
            IntentValue::RegisterEmissive { .. } => "register_emissive",
            IntentValue::RegisterLightQuantization { .. } => "register_light_quantization",

            IntentValue::RegisterTexture { .. } => "register_texture",
            IntentValue::RegisterTextureFiltering { .. } => "register_texture_filtering",

            IntentValue::RegisterText { .. } => "register_text",
            IntentValue::RegisterGLTF { .. } => "register_gltf",
            IntentValue::RegisterTextInput { .. } => "register_text_input",
            IntentValue::TextInputSetFocus { .. } => "text_input_set_focus",
            IntentValue::TextInputClearFocus => "text_input_clear_focus",
            IntentValue::TextInputInsertText { .. } => "text_input_insert_text",
            IntentValue::TextInputBackspace => "text_input_backspace",
            IntentValue::TextInputDeleteForward => "text_input_delete_forward",
            IntentValue::TextInputMoveCaret { .. } => "text_input_move_caret",
            IntentValue::TextInputMoveCaretTo { .. } => "text_input_move_caret_to",

            IntentValue::RegisterCollision { .. } => "register_collision",
            IntentValue::RemoveCollision { .. } => "remove_collision",
            IntentValue::RegisterKineticResponse { .. } => "register_kinetic_response",
            IntentValue::RemoveKineticResponse { .. } => "remove_kinetic_response",

            IntentValue::RegisterOpenxr { .. } => "register_openxr",
            IntentValue::RegisterInputXr { .. } => "register_input_xr",
            IntentValue::RegisterControllerXr { .. } => "register_controller_xr",
            IntentValue::RemoveInputXr { .. } => "remove_input_xr",
            IntentValue::RemoveControllerXr { .. } => "remove_controller_xr",

            IntentValue::RegisterRaycast { .. } => "register_raycast",
            IntentValue::RegisterPointer { .. } => "register_pointer",
            IntentValue::RemoveRaycast { .. } => "remove_raycast",

            IntentValue::RegisterAnimation { .. } => "register_animation",
            IntentValue::SetAnimationState { .. } => "set_animation_state",
            IntentValue::RegisterKeyframe { .. } => "register_keyframe",

            IntentValue::RegisterAudioOutput { .. } => "register_audio_output",
            IntentValue::AudioGraphDirtyImmediate { .. } => "audio_graph_dirty_immediate",
            IntentValue::RegisterAudioOscillator { .. } => "register_audio_oscillator",
            IntentValue::RegisterAudioClip { .. } => "register_audio_clip",
            IntentValue::RegisterAudioBufferSize { .. } => "register_audio_buffer_size",
            IntentValue::RegisterClock { .. } => "register_clock",
            IntentValue::RegisterTransformGizmo { .. } => "register_transform_gizmo",
            IntentValue::RegisterNormalVis { .. } => "register_normal_vis",
            IntentValue::RegisterEditor { .. } => "register_editor",
            IntentValue::RegisterAction { .. } => "register_action",

            IntentValue::PoseCapture { .. } => "pose_capture",
            IntentValue::PoseApply { .. } => "pose_apply",

            IntentValue::RegisterSignalRouteUpward { .. } => "register_signal_route_upward",
            IntentValue::RemoveSignalRouteUpward { .. } => "remove_signal_route_upward",

            IntentValue::ScheduleAudioOp { .. } => "schedule_audio_op",
            IntentValue::ScheduleAudioGraphSwap { .. } => "schedule_audio_graph_swap",
            IntentValue::ScheduleAudioPitchSetHz { .. } => "schedule_audio_pitch_set_hz",
            IntentValue::ScheduleAudioOscillatorEnabled { .. } => {
                "schedule_audio_oscillator_enabled"
            }
            IntentValue::ScheduleAudioGainSet { .. } => "schedule_audio_gain_set",
        }
    }
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
    Click,
    SelectionChanged,
    SelectionAdded,
    SelectionRemoved,
    SelectionCleared,
    Scrolling,
    TextInputFocusChanged,
    TextInputChanged,
    LayoutRootSizeAvailable,

    /// A named data event for cross-subtree communication.
    ///
    /// Handlers must filter by name inside the closure body since the kind is
    /// a unit variant (no payload).
    DataEvent,
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
