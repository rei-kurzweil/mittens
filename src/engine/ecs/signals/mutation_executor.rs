use crate::engine::ecs::component::{
    AnimationState, LayoutComponent, RenderableComponent, TextComponent, TransformComponent,
};
use crate::engine::ecs::system::SystemWorld;
use crate::engine::ecs::system::pose_capture_system::{
    pose_assets_root, reconcile_pose_capture_at_root,
};
use crate::engine::ecs::system::text_system::TextSystem;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, Signal, SignalEmitter, World};
use crate::engine::graphics::VisualWorld;

/// Built-in executor for low-level engine mutations expressed as intents.
///
/// This is the “mutation executor” layer: it applies canonical engine side effects
/// like register/remove/update and other internal operations.
#[derive(Debug, Default)]
pub struct RxMutationExecutor;

impl RxMutationExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(
        &mut self,
        systems: &mut SystemWorld,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
        env: &Signal,
    ) {
        use crate::engine::ecs::system::audio_system::AudioOp;
        use crate::engine::graphics::primitives::Transform;

        fn collect_text_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
            if world
                .get_component_by_id_as::<TextComponent>(target)
                .is_some()
            {
                out.push(target);
                return;
            }

            let mut stack = vec![target];
            while let Some(node) = stack.pop() {
                for &ch in world.children_of(node) {
                    stack.push(ch);
                }

                if world
                    .get_component_by_id_as::<TextComponent>(node)
                    .is_some()
                {
                    out.push(node);
                }
            }
        }

        fn apply_set_text_to_component(
            systems: &mut SystemWorld,
            world: &mut World,
            visuals: &mut VisualWorld,
            emit: &mut dyn SignalEmitter,
            component: ComponentId,
            text: &String,
        ) {
            // Update text payload and force a rebuild.
            if let Some(tc) = world.get_component_by_id_as_mut::<TextComponent>(component) {
                tc.text = text.clone();
                tc.mark_unbuilt();

                // Best-effort: delete glyph transform children (keep style components).
                let glyph_parent =
                    TextSystem::owned_text_block(world, component).unwrap_or(component);
                let children: Vec<ComponentId> = world.children_of(glyph_parent).to_vec();
                for ch in children {
                    if world
                        .get_component_by_id_as::<TransformComponent>(ch)
                        .is_none()
                    {
                        continue;
                    }

                    let has_renderable_child = world.children_of(ch).iter().any(|&gch| {
                        world
                            .get_component_by_id_as::<RenderableComponent>(gch)
                            .is_some()
                    });
                    if has_renderable_child {
                        // This is very likely a glyph root.
                        systems.remove_subtree_immediate(world, visuals, ch);
                    }
                }
            }

            // Editable values can change an auto-sized input's intrinsic
            // dimensions. Preserve existing plain-Text SetText behavior, but
            // re-run enclosing layout roots for generated text owned by a
            // TextInput so its background and siblings follow the edit.
            let mut ancestor = world.parent_of(component);
            let mut text_input_owned = false;
            while let Some(id) = ancestor {
                if world
                    .get_component_by_id_as::<crate::engine::ecs::component::TextInputComponent>(id)
                    .is_some()
                {
                    text_input_owned = true;
                    break;
                }
                ancestor = world.parent_of(id);
            }
            if text_input_owned {
                let mut current = Some(component);
                while let Some(id) = current {
                    if let Some(layout) = world
                        .get_component_by_id_as_mut::<crate::engine::ecs::component::LayoutComponent>(
                            id,
                        )
                    {
                        layout.mark_dirty();
                    }
                    current = world.parent_of(id);
                }
            }

            systems.register_text(world, visuals, component, emit);
        }

        let Some(intent) = env.intent.as_ref() else {
            return;
        };

        match &intent.value {
            IntentValue::RefreshEditorWorldPanel { editor_root } => {
                systems
                    .editor_inspector
                    .refresh_world_panel_after_topology_change(world, emit, *editor_root);
            }
            IntentValue::RegisterRenderable { component_id } => {
                let component = *component_id;
                systems.register_renderable(world, visuals, component);
            }
            IntentValue::RemoveRenderable { component_id } => {
                let component = *component_id;
                systems.remove_renderable(world, visuals, component);
            }
            IntentValue::RegisterStencilClip { component_id } => {
                let component = *component_id;
                systems.register_stencil_clip(world, visuals, component);
            }
            IntentValue::UnregisterStencilClip { component_id } => {
                let component = *component_id;
                systems.unregister_stencil_clip(world, visuals, component);
            }
            IntentValue::RegisterRouter { component_id } => {
                let component = *component_id;
                systems.register_router(world, emit, component);
            }
            IntentValue::RegisterHttpServer { component_id } => {
                let component = *component_id;
                systems.register_http_server(world, emit, component);
            }
            IntentValue::RegisterHttpClient { component_id } => {
                let component = *component_id;
                systems.register_http_client(world, emit, component);
            }
            IntentValue::HttpClientRequest {
                component_id,
                method,
                url,
                headers,
                body_text,
            } => {
                systems.http_client.issue_request(
                    *component_id,
                    method.clone(),
                    url.clone(),
                    headers.clone(),
                    body_text.clone(),
                );
            }
            IntentValue::HttpServerReply {
                component_id,
                request_id,
                status,
                headers,
                body_text,
            } => {
                systems.http_server.deliver_reply(
                    *component_id,
                    *request_id,
                    *status,
                    headers.clone(),
                    body_text.clone(),
                );
            }
            IntentValue::RegisterScrolling { component_id } => {
                let component = *component_id;
                systems.register_scrolling(world, emit, component);
            }

            IntentValue::RegisterTransform { component_id } => {
                let component = *component_id;
                systems.transform_changed(world, visuals, component);
            }
            IntentValue::UpdateTransformWorld { component_id } => {
                let component = *component_id;
                systems.transform_changed(world, visuals, component);
            }
            IntentValue::UpdateTransform {
                component_id,
                translation,
                rotation_quat_xyzw,
                scale,
            } => {
                let mut t = Transform::default();
                t.translation = *translation;
                t.rotation = *rotation_quat_xyzw;
                t.scale = *scale;
                t.recompute_model();

                {
                    let component = *component_id;
                    systems.update_transform(world, visuals, component, t);
                }
            }
            IntentValue::SetTransformTrs {
                component_id,
                space,
                value,
            } => {
                let local = match space {
                    crate::engine::transform::TransformSpace::Local => {
                        value.normalized().map_err(|error| error.to_string())
                    }
                    crate::engine::transform::TransformSpace::World => {
                        crate::engine::ecs::system::TransformSystem::world_to_local_trs(
                            world,
                            &systems.transform_stream,
                            *component_id,
                            *value,
                        )
                        .map_err(|error| error.to_string())
                    }
                };
                match local.and_then(|value| Transform::try_from(value).map_err(|e| e.to_string()))
                {
                    Ok(transform) => {
                        systems.update_transform(world, visuals, *component_id, transform);
                    }
                    Err(error) => {
                        eprintln!("SetTransformTrs ignored for {:?}: {}", component_id, error)
                    }
                }
            }
            IntentValue::RemoveTransform { component_id } => {
                let component = *component_id;
                systems.remove_transform(world, visuals, component);
            }

            IntentValue::RegisterCamera3d { component_id } => {
                let component = *component_id;
                systems.register_camera(world, visuals, component);
            }
            IntentValue::RegisterCamera2d { component_id } => {
                let component = *component_id;
                systems.register_camera2d(world, visuals, component);
            }
            IntentValue::MakeActiveCamera { component_id } => {
                let component = *component_id;
                systems.make_active_camera(world, visuals, component);
            }

            IntentValue::RegisterInput { component_id } => {
                let component = *component_id;
                systems.register_input(component);
            }
            IntentValue::RegisterUv { component_id } => {
                let component = *component_id;
                systems.register_uv(world, visuals, component);
            }

            IntentValue::RegisterLight { component_id } => {
                let component = *component_id;
                systems.register_light(world, visuals, component);
            }
            IntentValue::RegisterColor { component_id } => {
                let component = *component_id;
                systems.register_color(world, visuals, component);
            }
            IntentValue::RegisterOpacity { component_id } => {
                let component = *component_id;
                systems.register_opacity(world, visuals, component);
            }
            IntentValue::RegisterTransparentCutout { component_id } => {
                let component = *component_id;
                systems.register_transparent_cutout(world, visuals, component);
            }
            IntentValue::RegisterBackgroundColor { component_id } => {
                let component = *component_id;
                systems.register_background_color(world, visuals, component);
            }
            IntentValue::RegisterRendererSettings { component_id } => {
                let component = *component_id;
                systems.register_renderer_settings(world, visuals, component);
            }
            IntentValue::RegisterRenderGraph { component_id } => {
                let component = *component_id;
                systems.register_render_graph(world, visuals, component);
            }
            IntentValue::RegisterAmbientLight { component_id } => {
                let component = *component_id;
                systems.register_ambient_light(world, visuals, component);
            }
            IntentValue::RegisterEmissive { component_id } => {
                let component = *component_id;
                systems.register_emissive(world, visuals, component);
            }
            IntentValue::RegisterLightQuantization { component_id } => {
                let component = *component_id;
                systems.register_light_quantization(world, visuals, component);
            }
            IntentValue::RegisterAnimeShading { component_id } => {
                let component = *component_id;
                systems.register_anime_shading(world, visuals, component);
            }
            IntentValue::RegisterToonOutline { component_id } => {
                let component = *component_id;
                systems.register_toon_outline(world, visuals, component);
            }

            IntentValue::RegisterTexture { component_id } => {
                let component = *component_id;
                systems.register_texture(world, visuals, component);
            }
            IntentValue::RegisterTextureFiltering { component_id } => {
                let component = *component_id;
                systems.register_texture_filtering(world, visuals, component);
            }

            IntentValue::RegisterText { component_id } => {
                let component = *component_id;
                systems.register_text(world, visuals, component, emit);
            }
            IntentValue::RegisterGLTF { component_id } => {
                let component = *component_id;
                systems.gltf.register_component(component);
            }
            IntentValue::RegisterTextInput { component_id } => {
                let component = *component_id;
                systems.register_text_input(world, component, emit);
            }
            IntentValue::SetText { component_id, text } => {
                let mut text_cids = Vec::new();
                {
                    let t = *component_id;
                    collect_text_targets(world, t, &mut text_cids);
                }
                text_cids.sort();
                text_cids.dedup();

                for text_cid in text_cids {
                    apply_set_text_to_component(systems, world, visuals, emit, text_cid, text);
                }
            }

            IntentValue::SetEmissiveIntensity {
                component_id,
                intensity,
            } => {
                let intensity = (*intensity).max(0.0);
                {
                    let cid = *component_id;
                    systems.update_emissive_intensity(world, visuals, cid, intensity);
                }
            }

            IntentValue::SetLayoutAvailableWidth {
                component_id,
                width,
            } => {
                use crate::engine::ecs::component::LayoutComponent;
                let width = *width;
                {
                    let cid = *component_id;
                    if let Some(lo) = world.get_component_by_id_as_mut::<LayoutComponent>(cid) {
                        lo.set_available_width_dimension(width);
                    }
                }
            }

            IntentValue::SetLayoutAvailableHeight {
                component_id,
                height,
            } => {
                use crate::engine::ecs::component::LayoutComponent;
                let height = *height;
                {
                    let cid = *component_id;
                    if let Some(lo) = world.get_component_by_id_as_mut::<LayoutComponent>(cid) {
                        lo.set_available_height_dimension(height);
                    }
                }
            }

            IntentValue::SetLayoutInspect {
                component_id,
                enabled,
            } => {
                use crate::engine::ecs::component::LayoutComponent;
                let enabled = *enabled;
                {
                    let cid = *component_id;
                    if let Some(lo) = world.get_component_by_id_as_mut::<LayoutComponent>(cid) {
                        if lo.inspect != enabled {
                            lo.inspect = enabled;
                            lo.dirty = true;
                        }
                    }
                }
            }

            IntentValue::TextInputSetFocus { .. }
            | IntentValue::TextInputClearFocus
            | IntentValue::TextInputInsertText { .. }
            | IntentValue::TextInputBackspace
            | IntentValue::TextInputDeleteForward
            | IntentValue::TextInputMoveCaret { .. }
            | IntentValue::TextInputMoveCaretTo { .. } => {
                systems.text_input.execute_intent(world, emit, env);
            }

            IntentValue::RegisterCollision { component_id } => {
                let component = *component_id;
                systems.register_collision(world, visuals, component);
            }
            IntentValue::RemoveCollision { component_id } => {
                let component = *component_id;
                systems.remove_collision(world, visuals, component);
            }
            IntentValue::RegisterCollisionResponse { component_id } => {
                let component = *component_id;
                systems.register_collision_response(world, visuals, component);
            }
            IntentValue::RemoveCollisionResponse { component_id } => {
                let component = *component_id;
                systems.remove_collision_response(world, visuals, component);
            }
            IntentValue::RegisterAvatarControl { component_id } => {
                let component = *component_id;
                systems.register_avatar_control(component);
            }
            IntentValue::RegisterHumanoidBoneMap { component_id } => {
                systems
                    .humanoid_bone_map
                    .register_component(world, *component_id);
            }
            IntentValue::UnregisterHumanoidBoneMap { component_id } => {
                systems
                    .humanoid_bone_map
                    .unregister_component(world, *component_id);
            }
            IntentValue::HumanoidBoneMapGltfInitialized { component_id } => {
                systems
                    .humanoid_bone_map
                    .gltf_initialized(world, *component_id);
            }
            IntentValue::HumanoidBoneMapTopologyChanged { component_id } => {
                systems
                    .humanoid_bone_map
                    .topology_changed(world, *component_id);
            }
            IntentValue::RegisterAvatarBodyYaw { component_id } => {
                let component = *component_id;
                systems.avatar_body_yaw.register(component);
            }
            IntentValue::RegisterIkChain { component_id } => {
                let component = *component_id;
                systems.ik.register(component);
            }
            IntentValue::RegisterSecondaryMotion { component_id } => {
                let component = *component_id;
                systems.secondary_motion.register(world, component);
            }
            IntentValue::RegisterJointRetargetBasis { component_id }
            | IntentValue::RetargetBasisConfigurationChanged { component_id } => {
                systems
                    .joint_basis_retargeting
                    .register_component(world, *component_id);
            }
            IntentValue::JointRetargetBasisGltfInitialized { component_id } => {
                systems
                    .joint_basis_retargeting
                    .gltf_initialized(world, *component_id);
            }
            IntentValue::UnregisterJointRetargetBasis { component_id } => {
                systems
                    .joint_basis_retargeting
                    .remove_definition(world, *component_id);
            }
            IntentValue::SecondaryMotionConfigurationChanged { component_id } => {
                let component = *component_id;
                systems
                    .secondary_motion
                    .configuration_changed(world, component);
            }
            IntentValue::SecondaryMotionTopologyChanged { component_id } => {
                let component = *component_id;
                systems.secondary_motion.topology_changed(world, component);
            }
            IntentValue::SecondaryMotionGltfInitialized { component_id } => {
                let component = *component_id;
                systems.secondary_motion.gltf_initialized(world, component);
            }
            IntentValue::UnregisterSecondaryMotion { component_id } => {
                let component = *component_id;
                systems.secondary_motion.component_removed(world, component);
            }
            IntentValue::ResetSecondaryMotion { component_id } => {
                let component = *component_id;
                systems.secondary_motion.reset(world, component);
            }

            IntentValue::RemoveSubtree { component_id } => {
                let root = *component_id;
                {
                    let mut retained_ancestors = Vec::new();
                    let mut current = world.parent_of(root);
                    while let Some(ancestor) = current {
                        retained_ancestors.push(ancestor);
                        current = world.parent_of(ancestor);
                    }
                    emit.push_intent_now(
                        root,
                        IntentValue::AudioGraphDirtyImmediate { component_id: root },
                    );
                    // Best-effort: if the root is still attached, detach it first and publish
                    // a topology fact before deletion.
                    if let Some(old_parent) = world.parent_of(root) {
                        let mut current = Some(old_parent);
                        while let Some(id) = current {
                            if let Some(layout) =
                                world.get_component_by_id_as_mut::<LayoutComponent>(id)
                            {
                                layout.mark_dirty();
                            }
                            current = world.parent_of(id);
                        }
                        world.detach_from_parent(root);
                        emit.push_event(
                            root,
                            EventSignal::ParentChanged {
                                child: root,
                                old_parent: Some(old_parent),
                                new_parent: None,
                            },
                        );
                    }
                    systems.remove_subtree_immediate(world, visuals, root);
                    for ancestor in retained_ancestors {
                        if world.get_component_record(ancestor).is_some() {
                            crate::engine::ecs::system::selection_system::refresh_existing_selection_overlay(
                                world, emit, ancestor,
                            );
                        }
                    }
                }
            }

            IntentValue::RegisterXr { component_id } => {
                let component = *component_id;
                systems.register_xr(world, visuals, component);
            }
            IntentValue::RegisterInputXr { component_id } => {
                let component = *component_id;
                systems.register_input_xr(world, visuals, component);
            }
            IntentValue::RegisterControllerXr { component_id } => {
                let component = *component_id;
                systems.register_controller_xr(world, visuals, component, emit);
            }
            IntentValue::RegisterInputXrGamepad { component_id } => {
                let component = *component_id;
                systems.register_input_xr_gamepad(world, visuals, component);
            }
            IntentValue::RemoveInputXr { component_id } => {
                let component = *component_id;
                systems.remove_input_xr(world, visuals, component);
            }
            IntentValue::RemoveControllerXr { component_id } => {
                let component = *component_id;
                systems.remove_controller_xr(world, visuals, component);
            }
            IntentValue::RemoveInputXrGamepad { component_id } => {
                let component = *component_id;
                systems.remove_input_xr_gamepad(world, visuals, component);
            }
            IntentValue::RegisterEyeTracking { component_id } => {
                systems.register_eye_tracking(world, *component_id);
            }
            IntentValue::RemoveEyeTracking { component_id } => {
                systems.remove_eye_tracking(*component_id);
            }

            IntentValue::RegisterRaycast { component_id } => {
                let component = *component_id;
                systems.register_raycast(world, visuals, component);
            }
            IntentValue::RegisterRaycastable { component_id } => {
                systems.register_raycastable(world, visuals, *component_id);
            }
            IntentValue::RegisterPointer { component_id } => {
                let component = *component_id;
                systems.register_pointer(world, visuals, component, emit);
            }
            IntentValue::RegisterGrabbable { component_id } => {
                let component = *component_id;
                systems.grabbable.register(world, component, emit);
            }
            IntentValue::RegisterMountable { component_id } => {
                let component = *component_id;
                systems.attachment.register(world, component, emit);
            }
            IntentValue::RegisterDraggable { component_id } => {
                let component = *component_id;
                systems.draggable.register(world, component, emit);
            }
            IntentValue::RegisterSlider { component_id } => {
                systems.slider.register(world, *component_id, emit);
            }
            IntentValue::RemoveRaycast { component_id } => {
                let component = *component_id;
                systems.remove_raycast(world, visuals, component);
            }
            IntentValue::RemoveRaycastable { component_id } => {
                systems.remove_raycastable(world, visuals, *component_id);
            }

            IntentValue::RegisterAnimation { component_id } => {
                let component = *component_id;
                systems.register_animation(world, visuals, component);
            }
            IntentValue::SetAnimationState {
                component_id,
                state,
            } => {
                let state: AnimationState = state.clone();
                {
                    let component = *component_id;
                    systems.set_animation_state(component, state.clone());
                }
            }
            IntentValue::StepAnimation {
                component_id,
                direction,
            } => {
                systems.step_animation(*component_id, *direction);
            }
            IntentValue::RegisterKeyframe { component_id } => {
                let component = *component_id;
                systems.register_keyframe(world, visuals, component);
            }

            IntentValue::RegisterAudioOutput { component_id } => {
                let component = *component_id;
                systems.register_audio_output(world, visuals, component);
            }
            IntentValue::AudioGraphDirtyImmediate { component_id } => {
                let component = *component_id;
                systems.audio_graph_dirty(world, visuals, component);
            }
            IntentValue::RegisterAudioOscillator { component_id } => {
                let component = *component_id;
                systems.register_audio_oscillator(world, visuals, component);
            }
            IntentValue::RegisterAudioClip { component_id } => {
                let component = *component_id;
                systems.register_audio_clip(world, visuals, component);
            }
            IntentValue::RegisterAudioBufferSize { component_id } => {
                let component = *component_id;
                systems.register_audio_buffer_size(world, visuals, component);
            }

            IntentValue::RegisterClock { component_id } => {
                let component = *component_id;
                systems.register_clock(world, visuals, component);
            }

            IntentValue::RegisterTransformGizmo { component_id } => {
                let component = *component_id;
                systems.register_transform_gizmo(world, visuals, component, emit);
            }

            IntentValue::RegisterNormalVis { component_id } => {
                let component = *component_id;
                systems.register_normal_vis(world, component);
            }

            IntentValue::ReplExec { command } => {
                systems.queue_repl_command(command.clone());
            }

            IntentValue::RegisterEditor { component_id } => {
                let component = *component_id;
                systems.register_editor(world, visuals, render_assets, component, emit);
            }

            IntentValue::RegisterEditorUI { component_id } => {
                let component = *component_id;
                systems.register_editor_ui(world, visuals, render_assets, component, emit);
            }
            IntentValue::CollisionVisualizationSet {
                component_id,
                scope_roots,
                mode,
            } => {
                let owner = component_id;
                if let Some(mode) = mode {
                    systems
                        .collision_visualization
                        .set_request(*owner, scope_roots.clone(), *mode);
                } else {
                    systems.collision_visualization.remove_request(*owner);
                }
            }
            IntentValue::SpringBoneVisualizationSet {
                component_id,
                scope_roots,
                visible,
            } => {
                let owner = component_id;
                if *visible {
                    systems
                        .spring_bone_visualization
                        .set_request(*owner, scope_roots.clone());
                } else {
                    systems.spring_bone_visualization.remove_request(*owner);
                }
            }
            IntentValue::ZoneVisualizationSet {
                component_id,
                scope_roots,
                visible,
            } => {
                let owner = component_id;
                if *visible {
                    systems
                        .zone_visualization
                        .set_request(*owner, scope_roots.clone());
                } else {
                    systems.zone_visualization.remove_request(*owner);
                }
            }
            IntentValue::CameraVisualizationSet {
                component_id,
                scope_roots,
                visible,
            } => {
                let owner = component_id;
                if *visible {
                    systems
                        .camera_visualization
                        .set_request(*owner, scope_roots.clone());
                } else {
                    systems.camera_visualization.remove_request(*owner);
                }
            }

            IntentValue::RegisterSignalRouteUpward { component_id } => {
                let component = *component_id;
                systems
                    .pipeline
                    .register_signal_route_upward(world, &mut systems.rx, component);
            }
            IntentValue::RemoveSignalRouteUpward { component_id } => {
                let component = *component_id;
                systems
                    .pipeline
                    .remove_signal_route_upward(&mut systems.rx, component);
            }

            IntentValue::ScheduleAudioOp {
                component_id,
                beat,
                op,
            } => {
                let component = *component_id;
                systems.audio.schedule_audio_op(component, *beat, *op);
            }
            IntentValue::ScheduleAudioGraphSwap { component_id, beat } => {
                let component = *component_id;
                systems.audio.schedule_graph_swap(&*world, component, *beat);
            }
            IntentValue::ScheduleAudioPitchSetHz {
                component_id,
                beat,
                frequency_hz,
            } => {
                let component = *component_id;
                systems
                    .audio
                    .schedule_audio_op(component, *beat, AudioOp::SetHz(*frequency_hz));
            }
            IntentValue::ScheduleAudioOscillatorEnabled {
                component_id,
                beat,
                enabled,
            } => {
                let component = *component_id;
                systems
                    .audio
                    .schedule_audio_op(component, *beat, AudioOp::SetEnabled(*enabled));
            }
            IntentValue::ScheduleAudioGainSet {
                component_id,
                beat,
                gain,
            } => {
                let component = *component_id;
                systems
                    .audio
                    .schedule_audio_op(component, *beat, AudioOp::SetGain(*gain));
            }

            IntentValue::InitializePoseCapture { target } => {
                if let Err(error) =
                    reconcile_pose_capture_at_root(world, emit, *target, &pose_assets_root())
                {
                    eprintln!("[PoseCaptureSystem] initialization failed: {error}");
                }
            }
            IntentValue::PoseCapture { target, pose_name } => {
                systems.pose_capture.handle_capture(
                    world,
                    emit,
                    env.scope,
                    *target,
                    pose_name.clone(),
                );
            }
            IntentValue::PoseApply { target, pose, mode } => {
                if let Err(error) = systems
                    .pose_capture
                    .handle_apply(world, emit, *target, *pose, *mode)
                {
                    eprintln!("[PoseCaptureSystem] {error}");
                }
            }
            IntentValue::PoseReset { target } => {
                if let Err(error) = systems.pose_capture.handle_reset(world, emit, *target) {
                    eprintln!("[PoseCaptureSystem] {error}");
                }
            }

            // Not executed by the mutation executor.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::ecs::component::{
        RaycastableComponent, RenderableComponent, TransformComponent,
    };
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::ecs::{CommandQueue, IntentValue, SignalEmitter, World};
    use crate::engine::graphics::{RenderAssets, VisualWorld};

    #[test]
    fn raycastable_intents_update_bvh_through_queue_flush() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let root = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::square());
        let marker = world.add_component(RaycastableComponent::disabled());
        world.add_child(root, renderable).unwrap();
        world.add_child(renderable, marker).unwrap();

        // Model a live surface whose marker is enabled after renderable creation,
        // as when a visible grid enters Paint. Repeat to cover re-entry.
        for _ in 0..2 {
            world
                .get_component_by_id_as_mut::<RaycastableComponent>(marker)
                .unwrap()
                .enable = true;
            queue.push_intent_now(
                marker,
                IntentValue::RegisterRaycastable {
                    component_id: marker,
                },
            );
            queue.flush(&mut world, &mut systems, &mut visuals, &mut assets);
            systems.bvh.flush_pending(&world);
            let hits = systems.bvh.raycast_renderables_candidates(
                [0.0, 0.0, 2.0],
                [0.0, 0.0, -1.0],
                10.0,
                64,
            );
            assert_eq!(hits.len(), 1, "registration must reach the active executor");
            assert_eq!(hits[0].0, renderable);

            world
                .get_component_by_id_as_mut::<RaycastableComponent>(marker)
                .unwrap()
                .enable = false;
            queue.push_intent_now(
                marker,
                IntentValue::RemoveRaycastable {
                    component_id: marker,
                },
            );
            queue.flush(&mut world, &mut systems, &mut visuals, &mut assets);
            systems.bvh.flush_pending(&world);
            assert!(
                systems.bvh.debug_renderable_bounds(renderable).is_none(),
                "removal must remove the indexed leaf"
            );
        }
    }
}
