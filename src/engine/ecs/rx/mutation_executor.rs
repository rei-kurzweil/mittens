use crate::engine::ecs::component::{AnimationState, RenderableComponent, TextComponent, TransformComponent};
use crate::engine::ecs::system::SystemWorld;
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
                let children: Vec<ComponentId> = world.children_of(component).to_vec();
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

            systems.register_text(world, visuals, component, emit);
        }

        let Some(intent) = env.intent.as_ref() else {
            return;
        };

        match &intent.value {
            IntentValue::RegisterRenderable { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_renderable(world, visuals, component);
                }
            }
            IntentValue::RemoveRenderable { component_ids } => {
                for &component in component_ids.iter() {
                    systems.remove_renderable(world, visuals, component);
                }
            }
            IntentValue::RegisterStencilClip { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_stencil_clip(world, visuals, component);
                }
            }
            IntentValue::UnregisterStencilClip { component_ids } => {
                for &component in component_ids.iter() {
                    systems.unregister_stencil_clip(world, visuals, component);
                }
            }
            IntentValue::RegisterRouter { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_router(world, emit, component);
                }
            }
            IntentValue::RegisterScrolling { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_scrolling(world, emit, component);
                }
            }

            IntentValue::RegisterTransform { component_ids } => {
                for &component in component_ids.iter() {
                    systems.transform_changed(world, visuals, component);
                }
            }
            IntentValue::UpdateTransformWorld { component_ids } => {
                for &component in component_ids.iter() {
                    systems.transform_changed(world, visuals, component);
                }
            }
            IntentValue::UpdateTransform {
                component_ids,
                translation,
                rotation_quat_xyzw,
                scale,
            } => {
                let mut t = Transform::default();
                t.translation = *translation;
                t.rotation = *rotation_quat_xyzw;
                t.scale = *scale;
                t.recompute_model();

                for &component in component_ids.iter() {
                    systems.update_transform(world, visuals, component, t);
                }
            }
            IntentValue::RemoveTransform { component_ids } => {
                for &component in component_ids.iter() {
                    systems.remove_transform(world, visuals, component);
                }
            }

            IntentValue::RegisterCamera3d { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_camera(world, visuals, component);
                }
            }
            IntentValue::RegisterCamera2d { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_camera2d(world, visuals, component);
                }
            }
            IntentValue::MakeActiveCamera { component_ids } => {
                for &component in component_ids.iter() {
                    systems.make_active_camera(world, visuals, component);
                }
            }

            IntentValue::RegisterInput { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_input(component);
                }
            }
            IntentValue::RegisterUv { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_uv(world, visuals, component);
                }
            }

            IntentValue::RegisterLight { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_light(world, visuals, component);
                }
            }
            IntentValue::RegisterColor { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_color(world, visuals, component);
                }
            }
            IntentValue::RegisterOpacity { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_opacity(world, visuals, component);
                }
            }
            IntentValue::RegisterTransparentCutout { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_transparent_cutout(world, visuals, component);
                }
            }
            IntentValue::RegisterBackgroundColor { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_background_color(world, visuals, component);
                }
            }
            IntentValue::RegisterRendererSettings { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_renderer_settings(world, visuals, component);
                }
            }
            IntentValue::RegisterRenderGraph { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_render_graph(world, visuals, component);
                }
            }
            IntentValue::RegisterAmbientLight { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_ambient_light(world, visuals, component);
                }
            }
            IntentValue::RegisterEmissive { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_emissive(world, visuals, component);
                }
            }
            IntentValue::RegisterLightQuantization { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_light_quantization(world, visuals, component);
                }
            }

            IntentValue::RegisterTexture { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_texture(world, visuals, component);
                }
            }
            IntentValue::RegisterTextureFiltering { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_texture_filtering(world, visuals, component);
                }
            }

            IntentValue::RegisterText { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_text(world, visuals, component, emit);
                }
            }
            IntentValue::SetText {
                component_ids,
                text,
            } => {
                let mut text_cids = Vec::new();
                for &t in component_ids.iter() {
                    collect_text_targets(world, t, &mut text_cids);
                }
                text_cids.sort();
                text_cids.dedup();

                for text_cid in text_cids {
                    apply_set_text_to_component(systems, world, visuals, emit, text_cid, text);
                }
            }

            IntentValue::SetLayoutAvailableWidth { component_ids, width } => {
                use crate::engine::ecs::component::LayoutComponent;
                let width = *width;
                for &cid in component_ids.iter() {
                    if let Some(lo) = world.get_component_by_id_as_mut::<LayoutComponent>(cid) {
                        lo.available_width = width;
                        lo.dirty = true;
                    }
                }
            }

            IntentValue::RegisterCollision { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_collision(world, visuals, component);
                }
            }
            IntentValue::RemoveCollision { component_ids } => {
                for &component in component_ids.iter() {
                    systems.remove_collision(world, visuals, component);
                }
            }
            IntentValue::RegisterKineticResponse { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_kinetic_response(world, visuals, component);
                }
            }
            IntentValue::RemoveKineticResponse { component_ids } => {
                for &component in component_ids.iter() {
                    systems.remove_kinetic_response(world, visuals, component);
                }
            }

            IntentValue::RemoveSubtree { component_ids } => {
                let mut roots: Vec<ComponentId> = component_ids.iter().copied().collect();
                roots.sort();
                roots.dedup();
                for root in roots {
                    emit.push_intent_now(
                        root,
                        IntentValue::AudioGraphDirtyImmediate {
                            component_ids: vec![root],
                        },
                    );
                    // Best-effort: if the root is still attached, detach it first and publish
                    // a topology fact before deletion.
                    if let Some(old_parent) = world.parent_of(root) {
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
                }
            }

            IntentValue::RegisterOpenxr { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_openxr(world, visuals, component);
                }
            }
            IntentValue::RegisterInputXr { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_input_xr(world, visuals, component);
                }
            }
            IntentValue::RegisterControllerXr { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_controller_xr(world, visuals, component);
                }
            }
            IntentValue::RemoveInputXr { component_ids } => {
                for &component in component_ids.iter() {
                    systems.remove_input_xr(world, visuals, component);
                }
            }
            IntentValue::RemoveControllerXr { component_ids } => {
                for &component in component_ids.iter() {
                    systems.remove_controller_xr(world, visuals, component);
                }
            }

            IntentValue::RegisterRaycast { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_raycast(world, visuals, component);
                }
            }
            IntentValue::RegisterPointer { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_pointer(world, visuals, component, emit);
                }
            }
            IntentValue::RemoveRaycast { component_ids } => {
                for &component in component_ids.iter() {
                    systems.remove_raycast(world, visuals, component);
                }
            }

            IntentValue::RegisterAnimation { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_animation(world, visuals, component);
                }
            }
            IntentValue::SetAnimationState { component_ids, state } => {
                let state: AnimationState = state.clone();
                for &component in component_ids.iter() {
                    systems.set_animation_state(component, state.clone());
                }
            }
            IntentValue::RegisterKeyframe { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_keyframe(world, visuals, component);
                }
            }

            IntentValue::RegisterAudioOutput { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_audio_output(world, visuals, component);
                }
            }
            IntentValue::AudioGraphDirtyImmediate { component_ids } => {
                for &component in component_ids.iter() {
                    systems.audio_graph_dirty(world, visuals, component);
                }
            }
            IntentValue::RegisterAudioOscillator { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_audio_oscillator(world, visuals, component);
                }
            }
            IntentValue::RegisterAudioClip { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_audio_clip(world, visuals, component);
                }
            }
            IntentValue::RegisterAudioBufferSize { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_audio_buffer_size(world, visuals, component);
                }
            }

            IntentValue::RegisterClock { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_clock(world, visuals, component);
                }
            }

            IntentValue::RegisterTransformGizmo { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_transform_gizmo(world, visuals, component, emit);
                }
            }

            IntentValue::RegisterNormalVis { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_normal_vis(world, component);
                }
            }

            IntentValue::ReplExec { command } => {
                systems.queue_repl_command(command.clone());
            }

            IntentValue::RegisterEditor { component_ids } => {
                for &component in component_ids.iter() {
                    systems.register_editor(world, visuals, component, emit);
                }
            }

            IntentValue::RegisterAction { component_ids } => {
                for &component in component_ids.iter() {
                    crate::engine::ecs::system::action_system::register_action(
                        world, emit, component,
                    );
                }
            }

            IntentValue::RegisterSignalRouteUpward { component_ids } => {
                for &component in component_ids.iter() {
                    systems.pipeline.register_signal_route_upward(
                        world,
                        &mut systems.rx,
                        component,
                    );
                }
            }
            IntentValue::RemoveSignalRouteUpward { component_ids } => {
                for &component in component_ids.iter() {
                    systems
                        .pipeline
                        .remove_signal_route_upward(&mut systems.rx, component);
                }
            }

            IntentValue::ScheduleAudioOp {
                component_ids,
                beat,
                op,
            } => {
                for &component in component_ids.iter() {
                    systems.audio.schedule_audio_op(component, *beat, *op);
                }
            }
            IntentValue::ScheduleAudioGraphSwap {
                component_ids,
                beat,
            } => {
                for &component in component_ids.iter() {
                    systems.audio.schedule_graph_swap(&*world, component, *beat);
                }
            }
            IntentValue::ScheduleAudioPitchSetHz {
                component_ids,
                beat,
                frequency_hz,
            } => {
                for &component in component_ids.iter() {
                    systems.audio.schedule_audio_op(
                        component,
                        *beat,
                        AudioOp::SetHz(*frequency_hz),
                    );
                }
            }
            IntentValue::ScheduleAudioOscillatorEnabled {
                component_ids,
                beat,
                enabled,
            } => {
                for &component in component_ids.iter() {
                    systems.audio.schedule_audio_op(
                        component,
                        *beat,
                        AudioOp::SetEnabled(*enabled),
                    );
                }
            }
            IntentValue::ScheduleAudioGainSet {
                component_ids,
                beat,
                gain,
            } => {
                for &component in component_ids.iter() {
                    systems
                        .audio
                        .schedule_audio_op(component, *beat, AudioOp::SetGain(*gain));
                }
            }

            // Not executed by the mutation executor.
            _ => {}
        }
    }
}
