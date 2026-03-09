use crate::engine::ecs::component::{RenderableComponent, TextComponent, TransformComponent};
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
            if world.get_component_by_id_as::<TextComponent>(target).is_some() {
                out.push(target);
                return;
            }

            let mut stack = vec![target];
            while let Some(node) = stack.pop() {
                for &ch in world.children_of(node) {
                    stack.push(ch);
                }

                if world.get_component_by_id_as::<TextComponent>(node).is_some() {
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
                    if world.get_component_by_id_as::<TransformComponent>(ch).is_none() {
                        continue;
                    }

                    let has_renderable_child = world.children_of(ch).iter().any(|&gch| {
                        world.get_component_by_id_as::<RenderableComponent>(gch).is_some()
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
            IntentValue::RegisterRenderable { component } => {
                systems.register_renderable(world, visuals, *component);
            }
            IntentValue::RemoveRenderable { component } => {
                systems.remove_renderable(world, visuals, *component);
            }

            IntentValue::RegisterTransform { component } => {
                systems.transform_changed(world, visuals, *component);
            }
            IntentValue::UpdateTransform {
                component,
                translation,
                rotation_quat_xyzw,
                scale,
            } => {
                let mut t = Transform::default();
                t.translation = *translation;
                t.rotation = *rotation_quat_xyzw;
                t.scale = *scale;
                t.recompute_model();
                systems.update_transform(world, visuals, *component, t);
            }
            IntentValue::RemoveTransform { component } => {
                systems.remove_transform(world, visuals, *component);
            }

            IntentValue::RegisterCamera3d { component } => {
                systems.register_camera(world, visuals, *component);
            }
            IntentValue::RegisterCamera2d { component } => {
                systems.register_camera2d(world, visuals, *component);
            }
            IntentValue::MakeActiveCamera { component } => {
                systems.make_active_camera(world, visuals, *component);
            }

            IntentValue::RegisterInput { component } => {
                systems.register_input(*component);
            }
            IntentValue::RegisterUv { component } => {
                systems.register_uv(world, visuals, *component);
            }

            IntentValue::RegisterLight { component } => {
                systems.register_light(world, visuals, *component);
            }
            IntentValue::RegisterColor { component } => {
                systems.register_color(world, visuals, *component);
            }
            IntentValue::RegisterOpacity { component } => {
                systems.register_opacity(world, visuals, *component);
            }
            IntentValue::RegisterTransparentCutout { component } => {
                systems.register_transparent_cutout(world, visuals, *component);
            }
            IntentValue::RegisterBackgroundColor { component } => {
                systems.register_background_color(world, visuals, *component);
            }
            IntentValue::RegisterAmbientLight { component } => {
                systems.register_ambient_light(world, visuals, *component);
            }
            IntentValue::RegisterEmissive { component } => {
                systems.register_emissive(world, visuals, *component);
            }
            IntentValue::RegisterLightQuantization { component } => {
                systems.register_light_quantization(world, visuals, *component);
            }

            IntentValue::RegisterTexture { component } => {
                systems.register_texture(world, visuals, *component);
            }
            IntentValue::RegisterTextureFiltering { component } => {
                systems.register_texture_filtering(world, visuals, *component);
            }

            IntentValue::RegisterText { component } => {
                systems.register_text(world, visuals, *component, emit);
            }
            IntentValue::SetText { target, text } => {
                let mut text_cids = Vec::new();
                for &t in target.iter() {
                    collect_text_targets(world, t, &mut text_cids);
                }
                text_cids.sort();
                text_cids.dedup();

                for text_cid in text_cids {
                    apply_set_text_to_component(systems, world, visuals, emit, text_cid, text);
                }
            }

            IntentValue::RegisterCollision { component } => {
                systems.register_collision(world, visuals, *component);
            }
            IntentValue::RemoveCollision { component } => {
                systems.remove_collision(world, visuals, *component);
            }
            IntentValue::RegisterKineticResponse { component } => {
                systems.register_kinetic_response(world, visuals, *component);
            }
            IntentValue::RemoveKineticResponse { component } => {
                systems.remove_kinetic_response(world, visuals, *component);
            }

            IntentValue::RemoveSubtree { target } => {
                let mut roots: Vec<ComponentId> = target.iter().copied().collect();
                roots.sort();
                roots.dedup();
                for root in roots {
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

            IntentValue::RegisterOpenxr { component } => {
                systems.register_openxr(world, visuals, *component);
            }
            IntentValue::RegisterControllerXr { component } => {
                systems.register_controller_xr(world, visuals, *component);
            }
            IntentValue::RemoveControllerXr { component } => {
                systems.remove_controller_xr(world, visuals, *component);
            }

            IntentValue::RegisterRaycast { component } => {
                systems.register_raycast(world, visuals, *component);
            }
            IntentValue::RemoveRaycast { component } => {
                systems.remove_raycast(world, visuals, *component);
            }

            IntentValue::RegisterAnimation { component } => {
                systems.register_animation(world, visuals, *component);
            }
            IntentValue::RegisterKeyframe { component } => {
                systems.register_keyframe(world, visuals, *component);
            }

            IntentValue::RegisterAudioOutput { component } => {
                systems.register_audio_output(world, visuals, *component);
            }
            IntentValue::AudioGraphDirtyImmediate { component } => {
                systems.audio_graph_dirty(world, visuals, *component);
            }
            IntentValue::RegisterAudioOscillator { component } => {
                systems.register_audio_oscillator(world, visuals, *component);
            }
            IntentValue::RegisterAudioBufferSize { component } => {
                systems.register_audio_buffer_size(world, visuals, *component);
            }

            IntentValue::RegisterClock { component } => {
                systems.register_clock(world, visuals, *component);
            }

            IntentValue::RegisterTransformGizmo { component } => {
                systems.register_transform_gizmo(world, visuals, *component, emit);
            }

            IntentValue::RegisterEditor { component } => {
                systems.register_editor(world, visuals, *component, emit);
            }

            IntentValue::RegisterAction { component } => {
                crate::engine::ecs::system::action_system::register_action(world, emit, *component);
            }

            IntentValue::ScheduleAudioOp { component, beat, op } => {
                systems.audio.schedule_audio_op(*component, *beat, *op);
            }
            IntentValue::ScheduleAudioGraphSwap { component, beat } => {
                systems.audio.schedule_graph_swap(&*world, *component, *beat);
            }
            IntentValue::ScheduleAudioPitchSetHz {
                component,
                beat,
                frequency_hz,
            } => {
                systems
                    .audio
                    .schedule_audio_op(*component, *beat, AudioOp::SetHz(*frequency_hz));
            }
            IntentValue::ScheduleAudioOscillatorEnabled {
                component,
                beat,
                enabled,
            } => {
                systems
                    .audio
                    .schedule_audio_op(*component, *beat, AudioOp::SetEnabled(*enabled));
            }
            IntentValue::ScheduleAudioGainSet {
                component,
                beat,
                gain,
            } => {
                systems
                    .audio
                    .schedule_audio_op(*component, *beat, AudioOp::SetGain(*gain));
            }

            // Not executed by the mutation executor.
            _ => {}
        }
    }
}
