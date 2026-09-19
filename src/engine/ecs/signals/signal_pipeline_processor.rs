use crate::engine::ecs::RxWorld;
use crate::engine::ecs::signals::signal_pipeline::SignalPipelineOp;
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

        let Some(component_id) = Self::recipient_component_id_mut(&mut intent.value) else {
            return env;
        };

        let mut routed = *component_id;
        for pipeline in rx.pipelines_for_component(*component_id).iter() {
            for op in pipeline.ops.iter() {
                match op {
                    SignalPipelineOp::RouteUpward(r) => {
                        if r.applies_to_intent_kind(kind_name) {
                            routed = r.route(world, routed);
                        }
                    }
                }
            }
        }
        *component_id = routed;

        env
    }

    /// Returns the standardized intent recipients, if the intent variant has them.
    pub fn recipient_component_id(value: &IntentValue) -> Option<ComponentId> {
        match value {
            IntentValue::SetColor { component_id, .. }
            | IntentValue::SetText { component_id, .. }
            | IntentValue::SetEmissiveIntensity { component_id, .. }
            | IntentValue::SetPosition { component_id, .. }
            | IntentValue::LookAt { component_id, .. }
            | IntentValue::GLTFArmatureVisible { component_id, .. }
            | IntentValue::SetLayoutAvailableWidth { component_id, .. }
            | IntentValue::SetLayoutAvailableHeight { component_id, .. }
            | IntentValue::SetLayoutInspect { component_id, .. }
            | IntentValue::SelectionSet { component_id, .. }
            | IntentValue::ToggleSet { component_id, .. }
            | IntentValue::SliderSet { component_id, .. }
            | IntentValue::CollisionVisualizationSet { component_id, .. }
            | IntentValue::SpringBoneVisualizationSet { component_id, .. }
            | IntentValue::ZoneVisualizationSet { component_id, .. }
            | IntentValue::CameraVisualizationSet { component_id, .. }
            | IntentValue::Detach { component_id }
            | IntentValue::RemoveSubtree { component_id }
            | IntentValue::AudioGraphRebuild { component_id }
            | IntentValue::RequestRaycast { component_id }
            | IntentValue::AudioLowPassSetCutoffHz { component_id, .. }
            | IntentValue::AudioBandPassSetCenterHz { component_id, .. }
            | IntentValue::OscillatorSetEnabled { component_id, .. }
            | IntentValue::OscillatorSetPitch { component_id, .. }
            | IntentValue::OscillatorScheduleSetPitch { component_id, .. }
            | IntentValue::AudioSchedulePlay { component_id, .. }
            | IntentValue::RegisterRenderable { component_id }
            | IntentValue::RemoveRenderable { component_id }
            | IntentValue::RegisterStencilClip { component_id }
            | IntentValue::UnregisterStencilClip { component_id }
            | IntentValue::RegisterRouter { component_id }
            | IntentValue::RegisterHttpServer { component_id }
            | IntentValue::RegisterHttpClient { component_id }
            | IntentValue::RegisterScrolling { component_id }
            | IntentValue::RegisterTransform { component_id }
            | IntentValue::UpdateTransform { component_id, .. }
            | IntentValue::SetTransformTrs { component_id, .. }
            | IntentValue::RemoveTransform { component_id }
            | IntentValue::RegisterCamera3d { component_id }
            | IntentValue::RegisterCamera2d { component_id }
            | IntentValue::MakeActiveCamera { component_id }
            | IntentValue::RegisterInput { component_id }
            | IntentValue::RegisterUv { component_id }
            | IntentValue::RegisterLight { component_id }
            | IntentValue::RegisterColor { component_id }
            | IntentValue::RegisterOpacity { component_id }
            | IntentValue::RegisterTransparentCutout { component_id }
            | IntentValue::RegisterBackgroundColor { component_id }
            | IntentValue::RegisterRendererSettings { component_id }
            | IntentValue::RegisterRenderGraph { component_id }
            | IntentValue::RegisterAmbientLight { component_id }
            | IntentValue::RegisterEmissive { component_id }
            | IntentValue::RegisterLightQuantization { component_id }
            | IntentValue::RegisterAnimeShading { component_id }
            | IntentValue::RegisterToonOutline { component_id }
            | IntentValue::RegisterTexture { component_id }
            | IntentValue::RegisterTextureFiltering { component_id }
            | IntentValue::RegisterText { component_id }
            | IntentValue::RegisterGLTF { component_id }
            | IntentValue::RegisterTextInput { component_id }
            | IntentValue::RegisterCollision { component_id }
            | IntentValue::RemoveCollision { component_id }
            | IntentValue::RegisterCollisionResponse { component_id }
            | IntentValue::RemoveCollisionResponse { component_id }
            | IntentValue::RegisterAvatarControl { component_id }
            | IntentValue::RegisterHumanoidBoneMap { component_id }
            | IntentValue::UnregisterHumanoidBoneMap { component_id }
            | IntentValue::HumanoidBoneMapGltfInitialized { component_id }
            | IntentValue::HumanoidBoneMapTopologyChanged { component_id }
            | IntentValue::RegisterAvatarBodyYaw { component_id }
            | IntentValue::RegisterIkChain { component_id }
            | IntentValue::RegisterSecondaryMotion { component_id }
            | IntentValue::RegisterJointRetargetBasis { component_id }
            | IntentValue::RetargetBasisConfigurationChanged { component_id }
            | IntentValue::JointRetargetBasisGltfInitialized { component_id }
            | IntentValue::UnregisterJointRetargetBasis { component_id }
            | IntentValue::SecondaryMotionConfigurationChanged { component_id }
            | IntentValue::SecondaryMotionTopologyChanged { component_id }
            | IntentValue::SecondaryMotionGltfInitialized { component_id }
            | IntentValue::UnregisterSecondaryMotion { component_id }
            | IntentValue::ResetSecondaryMotion { component_id }
            | IntentValue::RegisterXr { component_id }
            | IntentValue::RegisterInputXr { component_id }
            | IntentValue::RegisterControllerXr { component_id }
            | IntentValue::RegisterInputXrGamepad { component_id }
            | IntentValue::RemoveInputXr { component_id }
            | IntentValue::RemoveControllerXr { component_id }
            | IntentValue::RemoveInputXrGamepad { component_id }
            | IntentValue::RegisterEyeTracking { component_id }
            | IntentValue::RemoveEyeTracking { component_id }
            | IntentValue::RegisterRaycast { component_id }
            | IntentValue::RegisterRaycastable { component_id }
            | IntentValue::RegisterPointer { component_id }
            | IntentValue::RegisterGrabbable { component_id }
            | IntentValue::RegisterMountable { component_id }
            | IntentValue::RegisterDraggable { component_id }
            | IntentValue::RegisterSlider { component_id }
            | IntentValue::RemoveRaycast { component_id }
            | IntentValue::RemoveRaycastable { component_id }
            | IntentValue::RegisterAnimation { component_id }
            | IntentValue::SetAnimationState { component_id, .. }
            | IntentValue::StepAnimation { component_id, .. }
            | IntentValue::RegisterKeyframe { component_id }
            | IntentValue::RegisterAudioOutput { component_id }
            | IntentValue::AudioGraphDirtyImmediate { component_id }
            | IntentValue::RegisterAudioOscillator { component_id }
            | IntentValue::RegisterAudioClip { component_id }
            | IntentValue::RegisterAudioBufferSize { component_id }
            | IntentValue::RegisterClock { component_id }
            | IntentValue::RegisterTransformGizmo { component_id }
            | IntentValue::RegisterNormalVis { component_id }
            | IntentValue::RegisterEditor { component_id }
            | IntentValue::RegisterEditorUI { component_id }
            | IntentValue::ScheduleAudioOp { component_id, .. }
            | IntentValue::ScheduleAudioGraphSwap { component_id, .. }
            | IntentValue::ScheduleAudioPitchSetHz { component_id, .. }
            | IntentValue::ScheduleAudioOscillatorEnabled { component_id, .. }
            | IntentValue::ScheduleAudioGainSet { component_id, .. } => Some(*component_id),

            IntentValue::Noop
            | IntentValue::RetryXrRuntime
            | IntentValue::RefreshEditorWorldPanel { .. }
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
            | IntentValue::InitializePoseCapture { .. }
            | IntentValue::PoseCapture { .. }
            | IntentValue::PoseApply { .. }
            | IntentValue::PoseReset { .. }
            | IntentValue::HttpClientRequest { .. }
            | IntentValue::HttpServerReply { .. } => None,
        }
    }

    fn recipient_component_id_mut(value: &mut IntentValue) -> Option<&mut ComponentId> {
        match value {
            IntentValue::SetColor { component_id, .. }
            | IntentValue::SetText { component_id, .. }
            | IntentValue::SetEmissiveIntensity { component_id, .. }
            | IntentValue::SetPosition { component_id, .. }
            | IntentValue::LookAt { component_id, .. }
            | IntentValue::GLTFArmatureVisible { component_id, .. }
            | IntentValue::SetLayoutAvailableWidth { component_id, .. }
            | IntentValue::SetLayoutAvailableHeight { component_id, .. }
            | IntentValue::SetLayoutInspect { component_id, .. }
            | IntentValue::SelectionSet { component_id, .. }
            | IntentValue::ToggleSet { component_id, .. }
            | IntentValue::SliderSet { component_id, .. }
            | IntentValue::CollisionVisualizationSet { component_id, .. }
            | IntentValue::SpringBoneVisualizationSet { component_id, .. }
            | IntentValue::ZoneVisualizationSet { component_id, .. }
            | IntentValue::CameraVisualizationSet { component_id, .. }
            | IntentValue::Detach { component_id }
            | IntentValue::RemoveSubtree { component_id }
            | IntentValue::AudioGraphRebuild { component_id }
            | IntentValue::RequestRaycast { component_id }
            | IntentValue::AudioLowPassSetCutoffHz { component_id, .. }
            | IntentValue::AudioBandPassSetCenterHz { component_id, .. }
            | IntentValue::OscillatorSetEnabled { component_id, .. }
            | IntentValue::OscillatorSetPitch { component_id, .. }
            | IntentValue::OscillatorScheduleSetPitch { component_id, .. }
            | IntentValue::AudioSchedulePlay { component_id, .. }
            | IntentValue::RegisterRenderable { component_id }
            | IntentValue::RemoveRenderable { component_id }
            | IntentValue::RegisterStencilClip { component_id }
            | IntentValue::UnregisterStencilClip { component_id }
            | IntentValue::RegisterRouter { component_id }
            | IntentValue::RegisterHttpServer { component_id }
            | IntentValue::RegisterHttpClient { component_id }
            | IntentValue::RegisterScrolling { component_id }
            | IntentValue::RegisterTransform { component_id }
            | IntentValue::UpdateTransform { component_id, .. }
            | IntentValue::SetTransformTrs { component_id, .. }
            | IntentValue::RemoveTransform { component_id }
            | IntentValue::RegisterCamera3d { component_id }
            | IntentValue::RegisterCamera2d { component_id }
            | IntentValue::MakeActiveCamera { component_id }
            | IntentValue::RegisterInput { component_id }
            | IntentValue::RegisterUv { component_id }
            | IntentValue::RegisterLight { component_id }
            | IntentValue::RegisterColor { component_id }
            | IntentValue::RegisterOpacity { component_id }
            | IntentValue::RegisterTransparentCutout { component_id }
            | IntentValue::RegisterBackgroundColor { component_id }
            | IntentValue::RegisterRendererSettings { component_id }
            | IntentValue::RegisterRenderGraph { component_id }
            | IntentValue::RegisterAmbientLight { component_id }
            | IntentValue::RegisterEmissive { component_id }
            | IntentValue::RegisterLightQuantization { component_id }
            | IntentValue::RegisterAnimeShading { component_id }
            | IntentValue::RegisterToonOutline { component_id }
            | IntentValue::RegisterTexture { component_id }
            | IntentValue::RegisterTextureFiltering { component_id }
            | IntentValue::RegisterText { component_id }
            | IntentValue::RegisterGLTF { component_id }
            | IntentValue::RegisterTextInput { component_id }
            | IntentValue::RegisterCollision { component_id }
            | IntentValue::RemoveCollision { component_id }
            | IntentValue::RegisterCollisionResponse { component_id }
            | IntentValue::RemoveCollisionResponse { component_id }
            | IntentValue::RegisterAvatarControl { component_id }
            | IntentValue::RegisterHumanoidBoneMap { component_id }
            | IntentValue::UnregisterHumanoidBoneMap { component_id }
            | IntentValue::HumanoidBoneMapGltfInitialized { component_id }
            | IntentValue::HumanoidBoneMapTopologyChanged { component_id }
            | IntentValue::RegisterAvatarBodyYaw { component_id }
            | IntentValue::RegisterIkChain { component_id }
            | IntentValue::RegisterSecondaryMotion { component_id }
            | IntentValue::RegisterJointRetargetBasis { component_id }
            | IntentValue::RetargetBasisConfigurationChanged { component_id }
            | IntentValue::JointRetargetBasisGltfInitialized { component_id }
            | IntentValue::UnregisterJointRetargetBasis { component_id }
            | IntentValue::SecondaryMotionConfigurationChanged { component_id }
            | IntentValue::SecondaryMotionTopologyChanged { component_id }
            | IntentValue::SecondaryMotionGltfInitialized { component_id }
            | IntentValue::UnregisterSecondaryMotion { component_id }
            | IntentValue::ResetSecondaryMotion { component_id }
            | IntentValue::RegisterXr { component_id }
            | IntentValue::RegisterInputXr { component_id }
            | IntentValue::RegisterControllerXr { component_id }
            | IntentValue::RegisterInputXrGamepad { component_id }
            | IntentValue::RemoveInputXr { component_id }
            | IntentValue::RemoveControllerXr { component_id }
            | IntentValue::RemoveInputXrGamepad { component_id }
            | IntentValue::RegisterEyeTracking { component_id }
            | IntentValue::RemoveEyeTracking { component_id }
            | IntentValue::RegisterRaycast { component_id }
            | IntentValue::RegisterRaycastable { component_id }
            | IntentValue::RegisterPointer { component_id }
            | IntentValue::RegisterGrabbable { component_id }
            | IntentValue::RegisterMountable { component_id }
            | IntentValue::RegisterDraggable { component_id }
            | IntentValue::RegisterSlider { component_id }
            | IntentValue::RemoveRaycast { component_id }
            | IntentValue::RemoveRaycastable { component_id }
            | IntentValue::RegisterAnimation { component_id }
            | IntentValue::SetAnimationState { component_id, .. }
            | IntentValue::StepAnimation { component_id, .. }
            | IntentValue::RegisterKeyframe { component_id }
            | IntentValue::RegisterAudioOutput { component_id }
            | IntentValue::AudioGraphDirtyImmediate { component_id }
            | IntentValue::RegisterAudioOscillator { component_id }
            | IntentValue::RegisterAudioClip { component_id }
            | IntentValue::RegisterAudioBufferSize { component_id }
            | IntentValue::RegisterClock { component_id }
            | IntentValue::RegisterTransformGizmo { component_id }
            | IntentValue::RegisterNormalVis { component_id }
            | IntentValue::RegisterEditor { component_id }
            | IntentValue::RegisterEditorUI { component_id }
            | IntentValue::ScheduleAudioOp { component_id, .. }
            | IntentValue::ScheduleAudioGraphSwap { component_id, .. }
            | IntentValue::ScheduleAudioPitchSetHz { component_id, .. }
            | IntentValue::ScheduleAudioOscillatorEnabled { component_id, .. }
            | IntentValue::ScheduleAudioGainSet { component_id, .. } => Some(component_id),

            IntentValue::RegisterSignalRouteUpward { .. }
            | IntentValue::RemoveSignalRouteUpward { .. }
            | IntentValue::Noop
            | IntentValue::RetryXrRuntime
            | IntentValue::RefreshEditorWorldPanel { .. }
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
            | IntentValue::InitializePoseCapture { .. }
            | IntentValue::PoseCapture { .. }
            | IntentValue::PoseApply { .. }
            | IntentValue::PoseReset { .. }
            | IntentValue::HttpClientRequest { .. }
            | IntentValue::HttpServerReply { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{
        ColorComponent, SignalRouteUpwardComponent, TransformComponent,
    };
    use crate::engine::ecs::system::PipelineSystem;
    use crate::engine::ecs::{IntentSignal, Signal};

    #[test]
    fn routing_rewrites_one_recipient_and_preserves_scope() {
        let mut world = World::default();
        let routed_parent = world.add_component(TransformComponent::new());
        let recipient = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let operator =
            world.add_component(SignalRouteUpwardComponent::new("set_color", "transform"));
        world.add_child(routed_parent, recipient).unwrap();
        world.add_child(recipient, operator).unwrap();

        let mut rx = RxWorld::default();
        PipelineSystem.register_signal_route_upward(&world, &mut rx, operator);
        let scope = operator;
        let signal = Signal::intent(
            scope,
            IntentSignal::now(IntentValue::SetColor {
                component_id: recipient,
                rgba: [0.0, 0.0, 0.0, 1.0],
            }),
        );

        let output = SignalPipelineProcessor.process_intent(&world, &rx, signal);
        assert_eq!(output.scope, scope);
        assert!(matches!(
            output.intent.map(|intent| intent.value),
            Some(IntentValue::SetColor { component_id, .. })
                if component_id == routed_parent
        ));
    }
}
