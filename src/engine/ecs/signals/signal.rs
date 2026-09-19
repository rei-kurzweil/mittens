use crate::engine::ecs::component::style::SizeDimension;
use crate::engine::ecs::component::{
    AnimationState, ControllerHand, SelectionEntry, SelectionMode, XrAxisControl, XrButtonControl,
};
use crate::engine::ecs::{ComponentId, World};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputCaretDirection {
    Left,
    Right,
}

pub type HttpHeaders = Vec<(String, String)>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PoseApplyMode {
    Replace,
    Overlay,
    RestBlend { amount: f32 },
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

/// Lean keyboard payload shared by the three gameplay keyboard signals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardEvent {
    pub code: Option<String>,
    pub key: String,
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
    /// Emitted once per rendered frame when at least one global subscriber exists.
    FrameTick {
        dt_sec: f32,
    },

    KeyDown(KeyboardEvent),
    KeyUp(KeyboardEvent),
    /// Initial presses and OS repeats; this is not committed text input.
    KeyPress(KeyboardEvent),

    /// A glTF asset finished spawning and its runtime node metadata is queryable.
    /// Scoped to `gltf`.
    GltfInitialized {
        gltf: ComponentId,
        uri: String,
    },

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
        activation_source: PointerActivationSource,
        raycaster: ComponentId,
        renderable: ComponentId,
        hit_point: [f32; 3],

        /// World-space ray direction at the time the drag started.
        ///
        /// This makes DragStart self-contained for consumers that need a stable plane normal
        /// (e.g. gizmo mapping-surface diagnostics / plane-projection drags) without observing
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
        activation_source: PointerActivationSource,
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
        activation_source: PointerActivationSource,
        raycaster: ComponentId,
        renderable: ComponentId,
        hit_point: Option<[f32; 3]>,
    },

    /// A pointer attached a Grabbable target.
    GrabStart {
        pointer: ComponentId,
        raycaster: ComponentId,
        renderable: ComponentId,
        target: ComponentId,
        ray_origin_world: [f32; 3],
        ray_dir_world: [f32; 3],
    },

    /// A pointer released its attached Grabbable target.
    GrabEnd {
        pointer: ComponentId,
        target: ComponentId,
    },

    /// A Rider-to-Mountable attachment transaction committed.
    MountStarted {
        rider: ComponentId,
        mountable: ComponentId,
    },

    /// A previously active Rider-to-Mountable attachment ended.
    MountEnded {
        rider: ComponentId,
        mountable: ComponentId,
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

    /// A generic boolean UI toggle changed value.
    ToggleChanged {
        toggle: ComponentId,
        value: bool,
    },

    SliderChanged {
        slider: ComponentId,
        value: f32,
    },

    SliderCommitted {
        slider: ComponentId,
        value: f32,
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
    SelectionCleared {
        selection_root: ComponentId,
    },

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

    XrButtonDown {
        source_component: ComponentId,
        hand: ControllerHand,
        control: XrButtonControl,
        value: f32,
    },
    XrButtonUp {
        source_component: ComponentId,
        hand: ControllerHand,
        control: XrButtonControl,
        value: f32,
    },
    XrButtonChanged {
        source_component: ComponentId,
        hand: ControllerHand,
        control: XrButtonControl,
        value: f32,
    },
    XrAxisChanged {
        source_component: ComponentId,
        hand: ControllerHand,
        control: XrAxisControl,
        value: [f32; 2],
    },

    HttpRequest {
        request_id: u64,
        method: String,
        path: String,
        query: Option<String>,
        url: String,
        headers: HttpHeaders,
        body_text: String,
        remote_addr: Option<String>,
    },
    HttpResponse {
        request_id: u64,
        status: u16,
        ok: bool,
        headers: HttpHeaders,
        body_text: String,
        url: String,
    },
    HttpError {
        request_id: Option<u64>,
        phase: String,
        message: String,
        url: Option<String>,
        bind_addr: Option<String>,
    },
    XrEyeTrackingUpdated {
        combined_look: Option<[f32; 3]>,
        left_look: Option<[f32; 3]>,
        right_look: Option<[f32; 3]>,
        combined_openness: Option<f32>,
    },
    XrEyeTrackingHtcUpdated {
        left: crate::engine::ecs::system::xr_eye_tracking_system::HtcEye,
        right: crate::engine::ecs::system::xr_eye_tracking_system::HtcEye,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerActivationSource {
    #[default]
    Trigger,
    Grip,
}

impl EventSignal {
    pub fn kind(&self) -> SignalKind {
        match self {
            EventSignal::FrameTick { .. } => SignalKind::FrameTick,
            EventSignal::KeyDown(_) => SignalKind::KeyDown,
            EventSignal::KeyUp(_) => SignalKind::KeyUp,
            EventSignal::KeyPress(_) => SignalKind::KeyPress,
            EventSignal::GltfInitialized { .. } => SignalKind::GltfInitialized,
            EventSignal::ParentChanged { .. } => SignalKind::ParentChanged,
            EventSignal::RayIntersected { .. } => SignalKind::RayIntersected,
            EventSignal::CollisionStarted { .. } => SignalKind::CollisionStarted,
            EventSignal::CollisionEnded { .. } => SignalKind::CollisionEnded,
            EventSignal::DragStart { .. } => SignalKind::DragStart,
            EventSignal::DragMove { .. } => SignalKind::DragMove,
            EventSignal::DragEnd { .. } => SignalKind::DragEnd,
            EventSignal::GrabStart { .. } => SignalKind::GrabStart,
            EventSignal::GrabEnd { .. } => SignalKind::GrabEnd,
            EventSignal::MountStarted { .. } => SignalKind::MountStarted,
            EventSignal::MountEnded { .. } => SignalKind::MountEnded,
            EventSignal::Click { .. } => SignalKind::Click,
            EventSignal::ToggleChanged { .. } => SignalKind::ToggleChanged,
            EventSignal::SliderChanged { .. } => SignalKind::SliderChanged,
            EventSignal::SliderCommitted { .. } => SignalKind::SliderCommitted,
            EventSignal::SelectionChanged { .. } => SignalKind::SelectionChanged,
            EventSignal::SelectionAdded { .. } => SignalKind::SelectionAdded,
            EventSignal::SelectionRemoved { .. } => SignalKind::SelectionRemoved,
            EventSignal::SelectionCleared { .. } => SignalKind::SelectionCleared,
            EventSignal::Scrolling { .. } => SignalKind::Scrolling,
            EventSignal::TextInputFocusChanged { .. } => SignalKind::TextInputFocusChanged,
            EventSignal::TextInputChanged { .. } => SignalKind::TextInputChanged,
            EventSignal::LayoutRootSizeAvailable { .. } => SignalKind::LayoutRootSizeAvailable,
            EventSignal::DataEvent { .. } => SignalKind::DataEvent,
            EventSignal::XrButtonDown { .. } => SignalKind::XrButtonDown,
            EventSignal::XrButtonUp { .. } => SignalKind::XrButtonUp,
            EventSignal::XrButtonChanged { .. } => SignalKind::XrButtonChanged,
            EventSignal::XrAxisChanged { .. } => SignalKind::XrAxisChanged,
            EventSignal::HttpRequest { .. } => SignalKind::HttpRequest,
            EventSignal::HttpResponse { .. } => SignalKind::HttpResponse,
            EventSignal::HttpError { .. } => SignalKind::HttpError,
            EventSignal::XrEyeTrackingUpdated { .. } => SignalKind::XrEyeTrackingUpdated,
            EventSignal::XrEyeTrackingHtcUpdated { .. } => SignalKind::XrEyeTrackingHtcUpdated,
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
    RetryXrRuntime,

    /// Spawn a component tree described by a fully-evaluated `MaterializedCE` and optionally
    /// attach it to a parent. If `parent` is `None` the root becomes a world root.
    SpawnComponentTree {
        root: Box<crate::scripting::object::MaterializedCE>,
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
        component_id: ComponentId,
        rgba: [f32; 4],
    },
    SetText {
        component_id: ComponentId,
        text: String,
    },
    SetEmissiveIntensity {
        component_id: ComponentId,
        intensity: f32,
    },
    SetPosition {
        component_id: ComponentId,
        position: [f32; 3],
    },
    LookAt {
        component_id: ComponentId,
        target_world: [f32; 3],
    },
    GLTFArmatureVisible {
        component_id: ComponentId,
        visible: bool,
    },
    SetLayoutAvailableWidth {
        component_id: ComponentId,
        width: SizeDimension,
    },
    SetLayoutAvailableHeight {
        component_id: ComponentId,
        height: SizeDimension,
    },
    SetLayoutInspect {
        component_id: ComponentId,
        enabled: bool,
    },
    SelectionSet {
        component_id: ComponentId,
        entries: Vec<SelectionEntry>,
        primary: Option<ComponentId>,
    },
    ToggleSet {
        component_id: ComponentId,
        value: bool,
    },
    SliderSet {
        component_id: ComponentId,
        value: f32,
        emit_changed: bool,
    },
    CollisionVisualizationSet {
        component_id: ComponentId,
        scope_roots: Vec<ComponentId>,
        mode: Option<crate::engine::ecs::system::CollisionVisualizationMode>,
    },
    SpringBoneVisualizationSet {
        component_id: ComponentId,
        scope_roots: Vec<ComponentId>,
        visible: bool,
    },
    ZoneVisualizationSet {
        component_id: ComponentId,
        scope_roots: Vec<ComponentId>,
        visible: bool,
    },
    CameraVisualizationSet {
        component_id: ComponentId,
        scope_roots: Vec<ComponentId>,
        visible: bool,
    },

    Attach {
        parent: ComponentId,
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
        parent: ComponentId,
        prefab_root: ComponentId,
    },
    Detach {
        component_id: ComponentId,
    },
    RemoveChild {
        parent: ComponentId,
        index: usize,
    },
    RemoveChildren {
        parent: ComponentId,
    },
    RemoveSubtree {
        component_id: ComponentId,
    },
    /// Internal editor projection update, coalesced by the editor workspace.
    RefreshEditorWorldPanel {
        editor_root: ComponentId,
    },

    AudioGraphRebuild {
        component_id: ComponentId,
    },
    RequestRaycast {
        component_id: ComponentId,
    },

    AudioLowPassSetCutoffHz {
        component_id: ComponentId,
        cutoff_hz: f32,
    },
    AudioBandPassSetCenterHz {
        component_id: ComponentId,
        center_hz: f32,
    },
    OscillatorSetEnabled {
        component_id: ComponentId,
        enabled: bool,
    },
    OscillatorSetPitch {
        component_id: ComponentId,
        frequency_hz: f32,
    },

    /// Schedule a pitch set at beat = beat_context + beat_offset.
    OscillatorScheduleSetPitch {
        component_id: ComponentId,
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
        component_id: ComponentId,
        beat_offset: f64,
        beat_context: Option<f64>,
        note: Option<crate::engine::ecs::component::MusicNote>,
        gain: Option<f32>,
        rate: Option<f32>,
        duration: Option<f64>,
    },

    RegisterRenderable {
        component_id: ComponentId,
    },
    RemoveRenderable {
        component_id: ComponentId,
    },
    RegisterStencilClip {
        component_id: ComponentId,
    },
    UnregisterStencilClip {
        component_id: ComponentId,
    },

    InitializePoseCapture {
        target: ComponentId,
    },
    PoseCapture {
        target: ComponentId,
        pose_name: Option<String>,
    },
    PoseApply {
        target: ComponentId,
        pose: ComponentId,
        mode: PoseApplyMode,
    },
    PoseReset {
        target: ComponentId,
    },

    RegisterRouter {
        component_id: ComponentId,
    },
    RegisterHttpServer {
        component_id: ComponentId,
    },
    RegisterHttpClient {
        component_id: ComponentId,
    },
    HttpClientRequest {
        component_id: ComponentId,
        method: String,
        url: String,
        headers: HttpHeaders,
        body_text: Option<String>,
    },
    HttpServerReply {
        component_id: ComponentId,
        request_id: u64,
        status: u16,
        headers: HttpHeaders,
        body_text: String,
    },
    RegisterScrolling {
        component_id: ComponentId,
    },
    RegisterTransform {
        component_id: ComponentId,
    },
    /// Recompute transform-derived caches (world matrices, skinning, BVH) without modifying the transform value.
    ///
    /// Intended for topology changes (e.g. Attach/Detach) where world matrices need recomputation.
    UpdateTransformWorld {
        component_id: ComponentId,
    },
    UpdateTransform {
        component_id: ComponentId,
        translation: [f32; 3],
        rotation_quat_xyzw: [f32; 4],
        scale: [f32; 3],
    },
    SetTransformTrs {
        component_id: ComponentId,
        space: crate::engine::transform::TransformSpace,
        value: crate::engine::transform::TransformTrs,
    },
    RemoveTransform {
        component_id: ComponentId,
    },

    RegisterCamera3d {
        component_id: ComponentId,
    },
    RegisterCamera2d {
        component_id: ComponentId,
    },
    MakeActiveCamera {
        component_id: ComponentId,
    },

    RegisterInput {
        component_id: ComponentId,
    },
    RegisterUv {
        component_id: ComponentId,
    },

    RegisterLight {
        component_id: ComponentId,
    },
    RegisterColor {
        component_id: ComponentId,
    },
    RegisterOpacity {
        component_id: ComponentId,
    },
    RegisterTransparentCutout {
        component_id: ComponentId,
    },
    RegisterBackgroundColor {
        component_id: ComponentId,
    },
    RegisterRendererSettings {
        component_id: ComponentId,
    },
    RegisterRenderGraph {
        component_id: ComponentId,
    },
    RegisterAmbientLight {
        component_id: ComponentId,
    },
    RegisterEmissive {
        component_id: ComponentId,
    },
    RegisterLightQuantization {
        component_id: ComponentId,
    },
    RegisterAnimeShading {
        component_id: ComponentId,
    },
    RegisterToonOutline {
        component_id: ComponentId,
    },

    RegisterTexture {
        component_id: ComponentId,
    },
    RegisterTextureFiltering {
        component_id: ComponentId,
    },

    RegisterText {
        component_id: ComponentId,
    },
    RegisterGLTF {
        component_id: ComponentId,
    },
    RegisterTextInput {
        component_id: ComponentId,
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
        component_id: ComponentId,
    },
    RemoveCollision {
        component_id: ComponentId,
    },
    RegisterCollisionResponse {
        component_id: ComponentId,
    },
    RemoveCollisionResponse {
        component_id: ComponentId,
    },
    RegisterAvatarControl {
        component_id: ComponentId,
    },
    RegisterHumanoidBoneMap {
        component_id: ComponentId,
    },
    UnregisterHumanoidBoneMap {
        component_id: ComponentId,
    },
    HumanoidBoneMapGltfInitialized {
        component_id: ComponentId,
    },
    HumanoidBoneMapTopologyChanged {
        component_id: ComponentId,
    },
    RegisterAvatarBodyYaw {
        component_id: ComponentId,
    },
    RegisterIkChain {
        component_id: ComponentId,
    },
    RegisterSecondaryMotion {
        component_id: ComponentId,
    },
    RegisterJointRetargetBasis {
        component_id: ComponentId,
    },
    RetargetBasisConfigurationChanged {
        component_id: ComponentId,
    },
    JointRetargetBasisGltfInitialized {
        component_id: ComponentId,
    },
    UnregisterJointRetargetBasis {
        component_id: ComponentId,
    },
    SecondaryMotionConfigurationChanged {
        component_id: ComponentId,
    },
    SecondaryMotionTopologyChanged {
        component_id: ComponentId,
    },
    SecondaryMotionGltfInitialized {
        component_id: ComponentId,
    },
    UnregisterSecondaryMotion {
        component_id: ComponentId,
    },
    ResetSecondaryMotion {
        component_id: ComponentId,
    },

    RegisterXr {
        component_id: ComponentId,
    },
    RegisterInputXr {
        component_id: ComponentId,
    },
    RegisterControllerXr {
        component_id: ComponentId,
    },
    RegisterInputXrGamepad {
        component_id: ComponentId,
    },
    RemoveInputXr {
        component_id: ComponentId,
    },
    RemoveControllerXr {
        component_id: ComponentId,
    },
    RemoveInputXrGamepad {
        component_id: ComponentId,
    },
    RegisterEyeTracking {
        component_id: ComponentId,
    },
    RemoveEyeTracking {
        component_id: ComponentId,
    },

    RegisterRaycast {
        component_id: ComponentId,
    },
    RegisterRaycastable {
        component_id: ComponentId,
    },
    RegisterPointer {
        component_id: ComponentId,
    },
    RegisterGrabbable {
        component_id: ComponentId,
    },
    RegisterMountable {
        component_id: ComponentId,
    },
    RegisterDraggable {
        component_id: ComponentId,
    },
    RegisterSlider {
        component_id: ComponentId,
    },
    RemoveRaycast {
        component_id: ComponentId,
    },
    RemoveRaycastable {
        component_id: ComponentId,
    },

    RegisterAnimation {
        component_id: ComponentId,
    },
    SetAnimationState {
        component_id: ComponentId,
        state: AnimationState,
    },
    StepAnimation {
        component_id: ComponentId,
        direction: crate::engine::ecs::component::AnimationStepDirection,
    },
    RegisterKeyframe {
        component_id: ComponentId,
    },

    RegisterAudioOutput {
        component_id: ComponentId,
    },
    AudioGraphDirtyImmediate {
        component_id: ComponentId,
    },
    RegisterAudioOscillator {
        component_id: ComponentId,
    },
    RegisterAudioClip {
        component_id: ComponentId,
    },
    RegisterAudioBufferSize {
        component_id: ComponentId,
    },
    RegisterClock {
        component_id: ComponentId,
    },
    RegisterTransformGizmo {
        component_id: ComponentId,
    },
    RegisterNormalVis {
        component_id: ComponentId,
    },

    RegisterEditor {
        component_id: ComponentId,
    },
    RegisterEditorUI {
        component_id: ComponentId,
    },

    /// Register/unregister routing operators.
    ///
    /// These are internal mutation-style intents executed by the pipeline system.
    RegisterSignalRouteUpward {
        component_id: ComponentId,
    },
    RemoveSignalRouteUpward {
        component_id: ComponentId,
    },

    ScheduleAudioOp {
        component_id: ComponentId,
        beat: f64,
        op: crate::engine::ecs::system::audio_system::AudioOp,
    },
    ScheduleAudioGraphSwap {
        component_id: ComponentId,
        beat: f64,
    },
    ScheduleAudioPitchSetHz {
        component_id: ComponentId,
        beat: f64,
        frequency_hz: f32,
    },
    ScheduleAudioOscillatorEnabled {
        component_id: ComponentId,
        beat: f64,
        enabled: bool,
    },
    ScheduleAudioGainSet {
        component_id: ComponentId,
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
            IntentValue::RetryXrRuntime => "retry_xr_runtime",
            IntentValue::SpawnComponentTree { .. } => "spawn_component_tree",
            IntentValue::Print { .. } => "print",
            IntentValue::ReplExec { .. } => "repl_exec",

            IntentValue::SetColor { .. } => "set_color",
            IntentValue::SetText { .. } => "set_text",
            IntentValue::SetEmissiveIntensity { .. } => "set_emissive_intensity",
            IntentValue::SetPosition { .. } => "set_position",
            IntentValue::LookAt { .. } => "look_at",
            IntentValue::SetLayoutAvailableWidth { .. } => "set_layout_available_width",
            IntentValue::SetLayoutAvailableHeight { .. } => "set_layout_available_height",
            IntentValue::SetLayoutInspect { .. } => "set_layout_inspect",
            IntentValue::GLTFArmatureVisible { .. } => "gltf_armature_visible",
            IntentValue::SelectionSet { .. } => "selection_set",
            IntentValue::ToggleSet { .. } => "toggle_set",
            IntentValue::SliderSet { .. } => "slider_set",
            IntentValue::CollisionVisualizationSet { .. } => "collision_visualization_set",
            IntentValue::SpringBoneVisualizationSet { .. } => "spring_bone_visualization_set",
            IntentValue::ZoneVisualizationSet { .. } => "zone_visualization_set",
            IntentValue::CameraVisualizationSet { .. } => "camera_visualization_set",

            IntentValue::Attach { .. } => "attach",
            IntentValue::QueryFindComponent { .. } => "query_find_component",
            IntentValue::QueryFindAllComponents { .. } => "query_find_all_components",
            IntentValue::AttachClone { .. } => "attach_clone",
            IntentValue::Detach { .. } => "detach",
            IntentValue::RemoveChild { .. } => "remove_child",
            IntentValue::RemoveChildren { .. } => "remove_children",
            IntentValue::RemoveSubtree { .. } => "remove_subtree",
            IntentValue::RefreshEditorWorldPanel { .. } => "refresh_editor_world_panel",

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
            IntentValue::RegisterHttpServer { .. } => "register_http_server",
            IntentValue::RegisterHttpClient { .. } => "register_http_client",
            IntentValue::HttpClientRequest { .. } => "http_client_request",
            IntentValue::HttpServerReply { .. } => "http_server_reply",
            IntentValue::RegisterScrolling { .. } => "register_scrolling",
            IntentValue::RegisterTransform { .. } => "register_transform",
            IntentValue::UpdateTransformWorld { .. } => "update_transform_world",
            IntentValue::UpdateTransform { .. } => "update_transform",
            IntentValue::SetTransformTrs { .. } => "set_transform_trs",
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
            IntentValue::RegisterAnimeShading { .. } => "register_anime_shading",
            IntentValue::RegisterToonOutline { .. } => "register_toon_outline",

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
            IntentValue::RegisterCollisionResponse { .. } => "register_collision_response",
            IntentValue::RemoveCollisionResponse { .. } => "remove_collision_response",
            IntentValue::RegisterAvatarControl { .. } => "register_avatar_control",
            IntentValue::RegisterHumanoidBoneMap { .. } => "register_humanoid_bone_map",
            IntentValue::UnregisterHumanoidBoneMap { .. } => "unregister_humanoid_bone_map",
            IntentValue::HumanoidBoneMapGltfInitialized { .. } => {
                "humanoid_bone_map_gltf_initialized"
            }
            IntentValue::HumanoidBoneMapTopologyChanged { .. } => {
                "humanoid_bone_map_topology_changed"
            }
            IntentValue::RegisterAvatarBodyYaw { .. } => "register_avatar_body_yaw",
            IntentValue::RegisterIkChain { .. } => "register_ik_chain",
            IntentValue::RegisterSecondaryMotion { .. } => "register_secondary_motion",
            IntentValue::RegisterJointRetargetBasis { .. } => "register_joint_retarget_basis",
            IntentValue::RetargetBasisConfigurationChanged { .. } => {
                "retarget_basis_configuration_changed"
            }
            IntentValue::JointRetargetBasisGltfInitialized { .. } => {
                "joint_retarget_basis_gltf_initialized"
            }
            IntentValue::UnregisterJointRetargetBasis { .. } => "unregister_joint_retarget_basis",
            IntentValue::SecondaryMotionConfigurationChanged { .. } => {
                "secondary_motion_configuration_changed"
            }
            IntentValue::SecondaryMotionTopologyChanged { .. } => {
                "secondary_motion_topology_changed"
            }
            IntentValue::SecondaryMotionGltfInitialized { .. } => {
                "secondary_motion_gltf_initialized"
            }
            IntentValue::UnregisterSecondaryMotion { .. } => "unregister_secondary_motion",
            IntentValue::ResetSecondaryMotion { .. } => "reset_secondary_motion",

            IntentValue::RegisterXr { .. } => "register_xr",
            IntentValue::RegisterInputXr { .. } => "register_input_xr",
            IntentValue::RegisterControllerXr { .. } => "register_controller_xr",
            IntentValue::RegisterInputXrGamepad { .. } => "register_input_xr_gamepad",
            IntentValue::RemoveInputXr { .. } => "remove_input_xr",
            IntentValue::RemoveControllerXr { .. } => "remove_controller_xr",
            IntentValue::RemoveInputXrGamepad { .. } => "remove_input_xr_gamepad",
            IntentValue::RegisterEyeTracking { .. } => "register_eye_tracking",
            IntentValue::RemoveEyeTracking { .. } => "remove_eye_tracking",

            IntentValue::RegisterRaycast { .. } => "register_raycast",
            IntentValue::RegisterRaycastable { .. } => "register_raycastable",
            IntentValue::RegisterPointer { .. } => "register_pointer",
            IntentValue::RegisterGrabbable { .. } => "register_grabbable",
            IntentValue::RegisterMountable { .. } => "register_mountable",
            IntentValue::RegisterDraggable { .. } => "register_draggable",
            IntentValue::RegisterSlider { .. } => "register_slider",
            IntentValue::RemoveRaycast { .. } => "remove_raycast",
            IntentValue::RemoveRaycastable { .. } => "remove_raycastable",

            IntentValue::RegisterAnimation { .. } => "register_animation",
            IntentValue::SetAnimationState { .. } => "set_animation_state",
            IntentValue::StepAnimation { .. } => "step_animation",
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
            IntentValue::RegisterEditorUI { .. } => "register_editor_ui",

            IntentValue::InitializePoseCapture { .. } => "initialize_pose_capture",
            IntentValue::PoseCapture { .. } => "pose_capture",
            IntentValue::PoseApply { .. } => "pose_apply",
            IntentValue::PoseReset { .. } => "pose_reset",

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
    FrameTick,
    KeyDown,
    KeyUp,
    KeyPress,
    GltfInitialized,
    ParentChanged,
    RayIntersected,
    CollisionStarted,
    CollisionEnded,
    DragStart,
    DragMove,
    DragEnd,
    GrabStart,
    GrabEnd,
    MountStarted,
    MountEnded,
    Click,
    ToggleChanged,
    SliderChanged,
    SliderCommitted,
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
    XrButtonDown,
    XrButtonUp,
    XrButtonChanged,
    XrAxisChanged,
    HttpRequest,
    HttpResponse,
    HttpError,
    XrEyeTrackingUpdated,
    XrEyeTrackingHtcUpdated,
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
