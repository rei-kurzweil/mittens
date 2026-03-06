use crate::engine::ecs::{IntentValue, Signal, SignalEmitter, World};

/// Built-in executor for **intent** signals.
///
/// This is intentionally minimal scaffolding for the ongoing refactor described in:
/// - docs/signals.md
///
/// The goal is to keep handlers observers-only, and execute side effects via intent signals.
#[derive(Debug, Default)]
pub struct RxIntentExecutor;

impl RxIntentExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether this executor is expected to handle the given signal value.
    ///
    /// Note: during migration, this is intentionally conservative; it is not yet wired into the
    /// drain loop.
    pub fn handles_value(value: &IntentValue) -> bool {
        // These are the “intent interpretation” values that expand into follow-up mutations.
        //
        // Note: `SetText` is currently executed by the default executor for text rebuilds.
        matches!(
            value,
            IntentValue::Noop
                | IntentValue::Print { .. }
                | IntentValue::SetColor { .. }
                | IntentValue::SetPosition { .. }
                | IntentValue::SetTransform { .. }
                | IntentValue::Attach { .. }
                | IntentValue::AttachClone { .. }
                | IntentValue::Detach { .. }
                | IntentValue::RemoveChild { .. }
                | IntentValue::RemoveChildren { .. }
                | IntentValue::RemoveSubtree { .. }
                | IntentValue::AudioGraphRebuild { .. }
                | IntentValue::RequestRaycast { .. }
                | IntentValue::AudioLowPassSetCutoffHz { .. }
                | IntentValue::AudioBandPassSetCenterHz { .. }
                | IntentValue::OscillatorSetEnabled { .. }
                | IntentValue::OscillatorSetPitch { .. }
                | IntentValue::OscillatorScheduleSetPitch { .. }
                | IntentValue::OscillatorScheduleSetNote { .. }
                | IntentValue::OscillatorScheduleMusicNote { .. }
                | IntentValue::MusicSetNote { .. }
        )
    }

    /// Execute an intent signal, emitting follow-up mutation signals via `emit`.
    ///
    pub fn execute(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, env: &Signal) {
        crate::engine::ecs::system::action_system::handle_intent_signal(world, emit, env);
    }
}
