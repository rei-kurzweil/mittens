use crate::engine::ecs::RxWorld;
use crate::engine::ecs::rx::signal_pipeline::SignalPipelineOp;
use crate::engine::ecs::{ComponentId, IntentValue, Signal, World};

/// Pre-execution processor for intent signals.
///
/// This is intended to become the hook point for intent routing/forwarding based on
/// per-target “pipeline” components in the ECS topology.
///
/// Current behavior: no-op passthrough.
#[derive(Debug, Default)]
pub struct SignalPipelineProcessor;

impl SignalPipelineProcessor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process an intent-containing signal envelope before it is executed.
    ///
    /// This is a no-op for now (returns `env` unchanged).
    pub fn process_intent(&mut self, world: &World, rx: &RxWorld, mut env: Signal) -> Signal {
        let Some(intent) = env.intent.as_mut() else {
            return env;
        };

        let kind_name = intent.value.kind_name();

        let Some(component_ids) = Self::recipient_component_ids_mut(&mut intent.value) else {
            return env;
        };

        if component_ids.is_empty() {
            return env;
        }

        let mut out: Vec<ComponentId> = Vec::with_capacity(component_ids.len());
        for &cid in component_ids.iter() {
            let mut cur = cid;

            for pipeline in rx.pipelines_for_component(cid).iter() {
                for op in pipeline.ops.iter() {
                    match op {
                        SignalPipelineOp::RouteUpward(r) => {
                            if r.applies_to_intent_kind(kind_name) {
                                cur = r.route(world, cur);
                            }
                        }
                    }
                }
            }

            out.push(cur);
        }

        out.sort();
        out.dedup();
        *component_ids = out;

        env
    }

    /// Returns the standardized intent recipients, if the intent variant has them.
    pub fn recipient_component_ids(value: &IntentValue) -> Option<&[ComponentId]> {
        match value {
            IntentValue::SetColor { component_ids, .. }
            | IntentValue::SetText { component_ids, .. }
            | IntentValue::SetPosition { component_ids, .. }
            | IntentValue::SetLayoutAvailableWidth { component_ids, .. }
            | IntentValue::SetLayoutAvailableHeight { component_ids, .. }
            | IntentValue::SetLayoutInspect { component_ids, .. }
            | IntentValue::SelectionSet { component_ids, .. }
            | IntentValue::Detach { component_ids }
            | IntentValue::RemoveSubtree { component_ids }
            | IntentValue::AudioGraphRebuild { component_ids }
            | IntentValue::RequestRaycast { component_ids }
            | IntentValue::AudioLowPassSetCutoffHz { component_ids, .. }
            | IntentValue::AudioBandPassSetCenterHz { component_ids, .. }
            | IntentValue::OscillatorSetEnabled { component_ids, .. }
            | IntentValue::OscillatorSetPitch { component_ids, .. }
            | IntentValue::OscillatorScheduleSetPitch { component_ids, .. }
            | IntentValue::AudioSchedulePlay { component_ids, .. }
            | IntentValue::RegisterRenderable { component_ids }
            | IntentValue::RemoveRenderable { component_ids }
            | IntentValue::RegisterStencilClip { component_ids }
            | IntentValue::UnregisterStencilClip { component_ids }
            | IntentValue::RegisterRouter { component_ids }
            | IntentValue::RegisterScrolling { component_ids }
            | IntentValue::RegisterTransform { component_ids }
            | IntentValue::UpdateTransform { component_ids, .. }
            | IntentValue::RemoveTransform { component_ids }
            | IntentValue::RegisterCamera3d { component_ids }
            | IntentValue::RegisterCamera2d { component_ids }
            | IntentValue::MakeActiveCamera { component_ids }
            | IntentValue::RegisterInput { component_ids }
            | IntentValue::RegisterUv { component_ids }
            | IntentValue::RegisterLight { component_ids }
            | IntentValue::RegisterColor { component_ids }
            | IntentValue::RegisterOpacity { component_ids }
            | IntentValue::RegisterTransparentCutout { component_ids }
            | IntentValue::RegisterBackgroundColor { component_ids }
            | IntentValue::RegisterRendererSettings { component_ids }
            | IntentValue::RegisterRenderGraph { component_ids }
            | IntentValue::RegisterAmbientLight { component_ids }
            | IntentValue::RegisterEmissive { component_ids }
            | IntentValue::RegisterLightQuantization { component_ids }
            | IntentValue::RegisterTexture { component_ids }
            | IntentValue::RegisterTextureFiltering { component_ids }
            | IntentValue::RegisterText { component_ids }
            | IntentValue::RegisterTextInput { component_ids }
            | IntentValue::RegisterCollision { component_ids }
            | IntentValue::RemoveCollision { component_ids }
            | IntentValue::RegisterKineticResponse { component_ids }
            | IntentValue::RemoveKineticResponse { component_ids }
            | IntentValue::RegisterOpenxr { component_ids }
            | IntentValue::RegisterInputXr { component_ids }
            | IntentValue::RegisterControllerXr { component_ids }
            | IntentValue::RemoveInputXr { component_ids }
            | IntentValue::RemoveControllerXr { component_ids }
            | IntentValue::RegisterRaycast { component_ids }
            | IntentValue::RegisterPointer { component_ids }
            | IntentValue::RemoveRaycast { component_ids }
            | IntentValue::RegisterAnimation { component_ids }
            | IntentValue::SetAnimationState { component_ids, .. }
            | IntentValue::RegisterKeyframe { component_ids }
            | IntentValue::RegisterAudioOutput { component_ids }
            | IntentValue::AudioGraphDirtyImmediate { component_ids }
            | IntentValue::RegisterAudioOscillator { component_ids }
            | IntentValue::RegisterAudioClip { component_ids }
            | IntentValue::RegisterAudioBufferSize { component_ids }
            | IntentValue::RegisterClock { component_ids }
            | IntentValue::RegisterTransformGizmo { component_ids }
            | IntentValue::RegisterNormalVis { component_ids }
            | IntentValue::RegisterEditor { component_ids }
            | IntentValue::RegisterAction { component_ids }
            | IntentValue::ScheduleAudioOp { component_ids, .. }
            | IntentValue::ScheduleAudioGraphSwap { component_ids, .. }
            | IntentValue::ScheduleAudioPitchSetHz { component_ids, .. }
            | IntentValue::ScheduleAudioOscillatorEnabled { component_ids, .. }
            | IntentValue::ScheduleAudioGainSet { component_ids, .. } => Some(component_ids),

            IntentValue::Noop
            | IntentValue::Print { .. }
            | IntentValue::ReplExec { .. }
            | IntentValue::Attach { .. }
            | IntentValue::AttachClone { .. }
            | IntentValue::QueryFindComponent { .. }
            | IntentValue::QueryFindAllComponents { .. }
            | IntentValue::RemoveChild { .. }
            | IntentValue::RemoveChildren { .. }
            | IntentValue::UpdateTransformWorld { .. }
            | IntentValue::TextInputSetFocus { .. }
            | IntentValue::TextInputClearFocus
            | IntentValue::TextInputInsertText { .. }
            | IntentValue::TextInputBackspace
            | IntentValue::TextInputDeleteForward
            | IntentValue::TextInputMoveCaret { .. }
            | IntentValue::TextInputMoveCaretTo { .. }
            | IntentValue::RegisterSignalRouteUpward { .. }
            | IntentValue::RemoveSignalRouteUpward { .. }
            | IntentValue::SpawnComponentTree { .. }
            | IntentValue::PoseCapture { .. }
            | IntentValue::PoseApply { .. } => None,
        }
    }

    fn recipient_component_ids_mut(value: &mut IntentValue) -> Option<&mut Vec<ComponentId>> {
        match value {
            IntentValue::SetColor { component_ids, .. }
            | IntentValue::SetText { component_ids, .. }
            | IntentValue::SetPosition { component_ids, .. }
            | IntentValue::SetLayoutAvailableWidth { component_ids, .. }
            | IntentValue::SetLayoutAvailableHeight { component_ids, .. }
            | IntentValue::SetLayoutInspect { component_ids, .. }
            | IntentValue::SelectionSet { component_ids, .. }
            | IntentValue::Detach { component_ids }
            | IntentValue::RemoveSubtree { component_ids }
            | IntentValue::AudioGraphRebuild { component_ids }
            | IntentValue::RequestRaycast { component_ids }
            | IntentValue::AudioLowPassSetCutoffHz { component_ids, .. }
            | IntentValue::AudioBandPassSetCenterHz { component_ids, .. }
            | IntentValue::OscillatorSetEnabled { component_ids, .. }
            | IntentValue::OscillatorSetPitch { component_ids, .. }
            | IntentValue::OscillatorScheduleSetPitch { component_ids, .. }
            | IntentValue::AudioSchedulePlay { component_ids, .. }
            | IntentValue::RegisterRenderable { component_ids }
            | IntentValue::RemoveRenderable { component_ids }
            | IntentValue::RegisterStencilClip { component_ids }
            | IntentValue::UnregisterStencilClip { component_ids }
            | IntentValue::RegisterRouter { component_ids }
            | IntentValue::RegisterScrolling { component_ids }
            | IntentValue::RegisterTransform { component_ids }
            | IntentValue::UpdateTransform { component_ids, .. }
            | IntentValue::RemoveTransform { component_ids }
            | IntentValue::RegisterCamera3d { component_ids }
            | IntentValue::RegisterCamera2d { component_ids }
            | IntentValue::MakeActiveCamera { component_ids }
            | IntentValue::RegisterInput { component_ids }
            | IntentValue::RegisterUv { component_ids }
            | IntentValue::RegisterLight { component_ids }
            | IntentValue::RegisterColor { component_ids }
            | IntentValue::RegisterOpacity { component_ids }
            | IntentValue::RegisterTransparentCutout { component_ids }
            | IntentValue::RegisterBackgroundColor { component_ids }
            | IntentValue::RegisterRendererSettings { component_ids }
            | IntentValue::RegisterRenderGraph { component_ids }
            | IntentValue::RegisterAmbientLight { component_ids }
            | IntentValue::RegisterEmissive { component_ids }
            | IntentValue::RegisterLightQuantization { component_ids }
            | IntentValue::RegisterTexture { component_ids }
            | IntentValue::RegisterTextureFiltering { component_ids }
            | IntentValue::RegisterText { component_ids }
            | IntentValue::RegisterTextInput { component_ids }
            | IntentValue::RegisterCollision { component_ids }
            | IntentValue::RemoveCollision { component_ids }
            | IntentValue::RegisterKineticResponse { component_ids }
            | IntentValue::RemoveKineticResponse { component_ids }
            | IntentValue::RegisterOpenxr { component_ids }
            | IntentValue::RegisterInputXr { component_ids }
            | IntentValue::RegisterControllerXr { component_ids }
            | IntentValue::RemoveInputXr { component_ids }
            | IntentValue::RemoveControllerXr { component_ids }
            | IntentValue::RegisterRaycast { component_ids }
            | IntentValue::RegisterPointer { component_ids }
            | IntentValue::RemoveRaycast { component_ids }
            | IntentValue::RegisterAnimation { component_ids }
            | IntentValue::SetAnimationState { component_ids, .. }
            | IntentValue::RegisterKeyframe { component_ids }
            | IntentValue::RegisterAudioOutput { component_ids }
            | IntentValue::AudioGraphDirtyImmediate { component_ids }
            | IntentValue::RegisterAudioOscillator { component_ids }
            | IntentValue::RegisterAudioClip { component_ids }
            | IntentValue::RegisterAudioBufferSize { component_ids }
            | IntentValue::RegisterClock { component_ids }
            | IntentValue::RegisterTransformGizmo { component_ids }
            | IntentValue::RegisterNormalVis { component_ids }
            | IntentValue::RegisterEditor { component_ids }
            | IntentValue::RegisterAction { component_ids }
            | IntentValue::ScheduleAudioOp { component_ids, .. }
            | IntentValue::ScheduleAudioGraphSwap { component_ids, .. }
            | IntentValue::ScheduleAudioPitchSetHz { component_ids, .. }
            | IntentValue::ScheduleAudioOscillatorEnabled { component_ids, .. }
            | IntentValue::ScheduleAudioGainSet { component_ids, .. } => Some(component_ids),

            IntentValue::RegisterSignalRouteUpward { .. }
            | IntentValue::RemoveSignalRouteUpward { .. }
            | IntentValue::Noop
            | IntentValue::Print { .. }
            | IntentValue::ReplExec { .. }
            | IntentValue::Attach { .. }
            | IntentValue::AttachClone { .. }
            | IntentValue::QueryFindComponent { .. }
            | IntentValue::QueryFindAllComponents { .. }
            | IntentValue::RemoveChild { .. }
            | IntentValue::RemoveChildren { .. } => None,

            IntentValue::UpdateTransformWorld { .. } => None,
            IntentValue::TextInputSetFocus { .. } => None,
            IntentValue::TextInputClearFocus => None,
            IntentValue::TextInputInsertText { .. } => None,
            IntentValue::TextInputBackspace => None,
            IntentValue::TextInputDeleteForward => None,
            IntentValue::TextInputMoveCaret { .. } => None,
            IntentValue::TextInputMoveCaretTo { .. } => None,
            IntentValue::SpawnComponentTree { .. }
            | IntentValue::PoseCapture { .. }
            | IntentValue::PoseApply { .. } => None,
        }
    }
}
