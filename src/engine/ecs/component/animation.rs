use super::Component;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    Playing,
    Looping,
    Paused,
}

/// When the AnimationSystem resolves ActionComponent target sources
/// (selectors / guids) into concrete ComponentIds.
///
/// - `OnAttach` (default): resolve once when the animation is first seen
///   by the system. All targets must exist by then. Cheapest at play
///   time — runtime tick uses the cached ids directly.
/// - `OnPlay`: defer resolution until each Action actually fires. Lets
///   actions reference components that don't exist until the animation
///   is mid-play (procedurally spawned, lazily attached). Pays one
///   resolution per Action on first fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveTargetsMode {
    OnAttach,
    OnPlay,
}

impl Default for ResolveTargetsMode {
    fn default() -> Self {
        Self::OnAttach
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AnimationComponent {
    pub state: AnimationState,
    pub resolve_targets: ResolveTargetsMode,
    /// Explicit loop length in beats. `None` falls back to the derived
    /// default in `AnimationSystem` (`floor(max_keyframe_beat) + 1`).
    /// Authored via `Animation.length(beats)`.
    pub length_beats: Option<f64>,

    component: Option<ComponentId>,
}

impl AnimationComponent {
    pub fn new() -> Self {
        Self {
            state: AnimationState::Looping,
            resolve_targets: ResolveTargetsMode::default(),
            length_beats: None,
            component: None,
        }
    }

    pub fn with_state(mut self, state: AnimationState) -> Self {
        self.state = state;
        self
    }

    pub fn with_resolve_targets(mut self, mode: ResolveTargetsMode) -> Self {
        self.resolve_targets = mode;
        self
    }

    pub fn with_length_beats(mut self, beats: f64) -> Self {
        self.length_beats = Some(beats);
        self
    }

    /// Backward-compatible helper for older callers.
    pub fn with_playing(self, playing: bool) -> Self {
        self.with_state(if playing {
            AnimationState::Looping
        } else {
            AnimationState::Paused
        })
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.component
    }
}

impl Default for AnimationComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AnimationComponent {
    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn name(&self) -> &'static str {
        "animation"
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterAnimation {
                component_ids: vec![component],
            },
        );
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
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let ctor = match self.state {
            AnimationState::Playing => "playing",
            AnimationState::Looping => "looping",
            AnimationState::Paused => "paused",
        };
        let mut ce = ce_call("Animation", ctor, vec![]);
        // Only emit non-default resolve_targets — OnAttach is the default
        // and would just add noise to dumps of typical animations.
        if self.resolve_targets != ResolveTargetsMode::default() {
            let mode = match self.resolve_targets {
                ResolveTargetsMode::OnAttach => "on_attach",
                ResolveTargetsMode::OnPlay => "on_play",
            };
            ce = ce.with_call("resolve_targets", vec![s(mode)]);
        }
        if let Some(n) = self.length_beats {
            ce = ce.with_call("length", vec![num(n)]);
        }
        ce
    }
}
