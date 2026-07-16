use crate::engine::ecs::{ComponentId, IntentValue, Signal, SignalEmitter, World};
use crate::engine::graphics::RenderAssets;

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
                | IntentValue::SpawnComponentTree { .. }
                | IntentValue::Print { .. }
                | IntentValue::SetColor { .. }
                | IntentValue::SetPosition { .. }
                | IntentValue::LookAt { .. }
                | IntentValue::GLTFArmatureVisible { .. }
                | IntentValue::SelectionSet { .. }
                | IntentValue::Attach { .. }
                | IntentValue::QueryFindComponent { .. }
                | IntentValue::QueryFindAllComponents { .. }
                | IntentValue::AttachClone { .. }
                | IntentValue::Detach { .. }
                | IntentValue::RemoveChild { .. }
                | IntentValue::RemoveChildren { .. }
                | IntentValue::AudioGraphRebuild { .. }
                | IntentValue::RequestRaycast { .. }
                | IntentValue::AudioLowPassSetCutoffHz { .. }
                | IntentValue::AudioBandPassSetCenterHz { .. }
                | IntentValue::OscillatorSetEnabled { .. }
                | IntentValue::OscillatorSetPitch { .. }
                | IntentValue::OscillatorScheduleSetPitch { .. }
                | IntentValue::AudioSchedulePlay { .. }
        )
    }

    /// Execute an intent signal, emitting follow-up mutation signals via `emit`.
    ///
    pub fn execute(
        &mut self,
        world: &mut World,
        render_assets: &mut RenderAssets,
        emit: &mut dyn SignalEmitter,
        env: &Signal,
    ) {
        handle_intent_signal(world, render_assets, emit, env);
    }
}

fn handle_intent_signal(
    world: &mut World,
    render_assets: &mut RenderAssets,
    emit: &mut dyn SignalEmitter,
    env: &Signal,
) {
    use crate::engine::ecs::component::{
        AudioBandPassFilterComponent, AudioClipComponent, AudioClipLoadState,
        AudioLowPassFilterComponent, AudioOscillatorComponent, ColorComponent, MusicNoteComponent,
        RayCastComponent, TransformComponent,
    };
    use crate::engine::ecs::system::MusicSystem;
    use crate::engine::ecs::system::audio_system::AudioOp;
    use crate::engine::ecs::{ComponentId, EventSignal, IntentValue};

    let beat_now = 0.0;

    let Some(intent) = env.intent.as_ref() else {
        return;
    };

    match &intent.value {
        IntentValue::Noop => {}

        IntentValue::SpawnComponentTree { root, parent } => {
            match crate::scripting::component_registry::with_live_render_assets(
                render_assets,
                || crate::scripting::component_registry::spawn_tree(root, *parent, world, emit),
            ) {
                Ok(id) => println!("[SpawnComponentTree] spawned root {id:?}"),
                Err(e) => println!("[SpawnComponentTree] error: {e}"),
            }
        }

        IntentValue::Print { .. } => {}

        IntentValue::SetColor {
            component_ids,
            rgba,
        } => {
            let mut color_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_color_targets(world, t, &mut color_cids);
            }
            color_cids.sort();
            for color_cid in color_cids {
                if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(color_cid) {
                    c.rgba = *rgba;
                    emit.push_intent_now(
                        color_cid,
                        IntentValue::RegisterColor {
                            component_ids: vec![color_cid],
                        },
                    );
                }
            }
        }

        IntentValue::SetText { .. } => {
            // Executed by the mutation executor.
        }

        IntentValue::SetPosition {
            component_ids,
            position,
        } => {
            let mut transform_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_transform_targets(world, t, &mut transform_cids);
            }
            transform_cids.sort();
            transform_cids.dedup();
            for transform_cid in transform_cids {
                if let Some(t) =
                    world.get_component_by_id_as_mut::<TransformComponent>(transform_cid)
                {
                    t.set_position(emit, position[0], position[1], position[2]);
                }
            }
        }

        IntentValue::LookAt {
            component_ids,
            target_world,
        } => {
            let mut transform_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_transform_targets(world, t, &mut transform_cids);
            }
            transform_cids.sort();
            transform_cids.dedup();
            for transform_cid in transform_cids {
                let Some(world_position) =
                    crate::engine::ecs::system::TransformSystem::world_position(
                        world,
                        transform_cid,
                    )
                else {
                    continue;
                };

                let Some(desired_world_rotation) =
                    TransformComponent::look_at_world_rotation(world_position, *target_world)
                else {
                    continue;
                };

                let parent_world_rotation = world
                    .parent_of(transform_cid)
                    .and_then(|parent| {
                        crate::engine::ecs::system::TransformSystem::world_model(world, parent)
                    })
                    .map(crate::utils::math::mat_to_quat)
                    .unwrap_or([0.0, 0.0, 0.0, 1.0]);
                let local_rotation =
                    crate::utils::math::quat_normalize(crate::utils::math::quat_mul(
                        crate::utils::math::quat_conjugate(parent_world_rotation),
                        desired_world_rotation,
                    ));

                let Some(transform) = world
                    .get_component_by_id_as::<TransformComponent>(transform_cid)
                    .map(|t| t.transform)
                else {
                    continue;
                };

                emit.push_intent_now(
                    transform_cid,
                    IntentValue::UpdateTransform {
                        component_ids: vec![transform_cid],
                        translation: transform.translation,
                        rotation_quat_xyzw: local_rotation,
                        scale: transform.scale,
                    },
                );
            }
        }

        IntentValue::GLTFArmatureVisible {
            component_ids,
            visible,
        } => {
            for &component_id in component_ids {
                if let Some(gltf) = world
                    .get_component_by_id_as_mut::<crate::engine::ecs::component::GLTFComponent>(
                        component_id,
                    )
                {
                    gltf.armature_visible = *visible;
                }
            }
        }

        IntentValue::SelectionSet {
            component_ids,
            entries,
            primary,
        } => {
            for &selection_root in component_ids.iter() {
                crate::engine::ecs::system::selection_system::apply_selection_set(
                    world,
                    emit,
                    selection_root,
                    entries.clone(),
                    *primary,
                );
            }
        }

        IntentValue::Attach { parents, child } => {
            for &parent in parents.iter() {
                let old_parent = world.parent_of(*child);
                if let Err(e) = world.add_child(parent, *child) {
                    println!("[IntentExecutor] attach failed: {e}");
                    continue;
                }

                emit.push_event(
                    *child,
                    EventSignal::ParentChanged {
                        child: *child,
                        old_parent,
                        new_parent: Some(parent),
                    },
                );

                if world.is_initialized(parent) {
                    world.init_component_tree(*child, emit);
                }

                emit_topology_transform_refresh(world, emit, *child);
                emit_topology_transform_refresh(world, emit, parent);

                emit.push_intent_now(
                    parent,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![parent],
                    },
                );
                emit.push_intent_now(
                    *child,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![*child],
                    },
                );
            }
        }

        IntentValue::QueryFindComponent {
            root,
            selector,
            reply,
        } => {
            let _ = reply.send(world.find_component(*root, selector));
        }

        IntentValue::QueryFindAllComponents {
            root,
            selector,
            reply,
        } => {
            let _ = reply.send(world.find_all_components(*root, selector));
        }

        IntentValue::AttachClone {
            parents,
            prefab_root,
        } => {
            // Encode prefab subtree → MMS ComponentExpression AST → MaterializedCE.
            // Cloning then drops into the same `spawn_tree` path that MMS source uses.
            let ce_ast = match crate::scripting::component_registry::subtree_to_ce_ast(
                &*world,
                *prefab_root,
            ) {
                Ok(ce) => ce,
                Err(e) => {
                    println!("[IntentExecutor] attach_clone encode failed: {e}");
                    return;
                }
            };
            let materialized =
                match crate::scripting::component_registry::ce_ast_to_materialized(&ce_ast) {
                    Ok(m) => m,
                    Err(e) => {
                        println!("[IntentExecutor] attach_clone materialize failed: {e}");
                        return;
                    }
                };

            for &parent in parents.iter() {
                let new_root = match crate::scripting::component_registry::with_live_render_assets(
                    render_assets,
                    || {
                        crate::scripting::component_registry::spawn_tree(
                            &materialized,
                            Some(parent),
                            world,
                            emit,
                        )
                    },
                ) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("[IntentExecutor] attach_clone spawn failed: {e}");
                        continue;
                    }
                };

                if world.get_component_record(new_root).is_none() {
                    println!("[IntentExecutor] attach_clone: new root missing after spawn");
                    continue;
                }

                // spawn_tree already attached + initialized the subtree if
                // the parent was initialized. Just emit the topology signals.

                emit.push_event(
                    new_root,
                    EventSignal::ParentChanged {
                        child: new_root,
                        old_parent: None,
                        new_parent: Some(parent),
                    },
                );

                emit_topology_transform_refresh(world, emit, new_root);
                emit_topology_transform_refresh(world, emit, parent);

                emit.push_intent_now(
                    parent,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![parent],
                    },
                );
                emit.push_intent_now(
                    new_root,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![new_root],
                    },
                );
            }
        }

        IntentValue::Detach { component_ids } => {
            for &child in component_ids.iter() {
                let old_parent = world.parent_of(child);
                world.detach_from_parent(child);

                emit.push_event(
                    child,
                    EventSignal::ParentChanged {
                        child,
                        old_parent,
                        new_parent: None,
                    },
                );

                if let Some(p) = old_parent {
                    emit.push_intent_now(
                        p,
                        IntentValue::AudioGraphDirtyImmediate {
                            component_ids: vec![p],
                        },
                    );
                }

                emit_topology_transform_refresh(world, emit, child);
                if let Some(p) = old_parent {
                    emit_topology_transform_refresh(world, emit, p);
                }
            }
        }

        IntentValue::RemoveChild { parents, index } => {
            for &parent in parents.iter() {
                let child = world.children_of(parent).get(*index).copied();
                let Some(child) = child else {
                    continue;
                };

                emit.push_intent_now(
                    parent,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![parent],
                    },
                );
                emit.push_intent_now(
                    child,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![child],
                    },
                );

                world.detach_from_parent(child);

                emit.push_event(
                    child,
                    EventSignal::ParentChanged {
                        child,
                        old_parent: Some(parent),
                        new_parent: None,
                    },
                );

                emit.push_intent_now(
                    child,
                    IntentValue::RemoveSubtree {
                        component_ids: vec![child],
                    },
                );

                emit_topology_transform_refresh(world, emit, parent);
            }
        }

        IntentValue::RemoveChildren { parents } => {
            for &parent in parents.iter() {
                let children: Vec<ComponentId> = world.children_of(parent).to_vec();
                if children.is_empty() {
                    continue;
                }

                emit.push_intent_now(
                    parent,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![parent],
                    },
                );
                for child in children {
                    emit.push_intent_now(
                        child,
                        IntentValue::AudioGraphDirtyImmediate {
                            component_ids: vec![child],
                        },
                    );

                    world.detach_from_parent(child);
                    emit.push_event(
                        child,
                        EventSignal::ParentChanged {
                            child,
                            old_parent: Some(parent),
                            new_parent: None,
                        },
                    );

                    emit.push_intent_now(
                        child,
                        IntentValue::RemoveSubtree {
                            component_ids: vec![child],
                        },
                    );
                }

                emit_topology_transform_refresh(world, emit, parent);
            }
        }

        IntentValue::AudioGraphRebuild { component_ids } => {
            for &t in component_ids.iter() {
                emit.push_intent_now(
                    t,
                    IntentValue::AudioGraphDirtyImmediate {
                        component_ids: vec![t],
                    },
                );
            }
        }

        IntentValue::RequestRaycast { component_ids } => {
            let mut raycast_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_raycast_targets(world, t, &mut raycast_cids);
            }
            raycast_cids.sort();
            raycast_cids.dedup();

            for rcid in raycast_cids {
                if let Some(rc) = world.get_component_by_id_as_mut::<RayCastComponent>(rcid) {
                    rc.cast_requests = rc.cast_requests.saturating_add(1);
                }
            }
        }

        IntentValue::AudioLowPassSetCutoffHz {
            component_ids,
            cutoff_hz,
        } => {
            for &t in component_ids.iter() {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioLowPassFilterComponent>(t)
                {
                    c.cutoff_hz = if cutoff_hz.is_finite() {
                        cutoff_hz.max(0.0)
                    } else {
                        c.cutoff_hz
                    };
                    emit.push_intent_now(
                        t,
                        IntentValue::ScheduleAudioOp {
                            component_ids: vec![t],
                            beat: beat_now,
                            op: AudioOp::SetLowPassCutoffHz(c.cutoff_hz),
                        },
                    );
                }
            }
        }

        IntentValue::AudioBandPassSetCenterHz {
            component_ids,
            center_hz,
        } => {
            for &t in component_ids.iter() {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioBandPassFilterComponent>(t)
                {
                    c.center_hz = if center_hz.is_finite() {
                        center_hz.max(0.0)
                    } else {
                        c.center_hz
                    };
                    emit.push_intent_now(
                        t,
                        IntentValue::ScheduleAudioOp {
                            component_ids: vec![t],
                            beat: beat_now,
                            op: AudioOp::SetBandPassCenterHz(c.center_hz),
                        },
                    );
                }
            }
        }

        IntentValue::OscillatorSetEnabled {
            component_ids,
            enabled,
        } => {
            let mut osc_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                if let Some(c) =
                    world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                {
                    for osc in c.oscillators.iter_mut() {
                        osc.enabled = *enabled;
                    }
                    emit.push_intent_now(
                        osc_cid,
                        IntentValue::RegisterAudioOscillator {
                            component_ids: vec![osc_cid],
                        },
                    );
                }
            }
        }

        IntentValue::OscillatorSetPitch {
            component_ids,
            frequency_hz,
        } => {
            if !frequency_hz.is_finite() {
                println!(
                    "[IntentExecutor] oscillator_set_pitch: non-finite frequency_hz={frequency_hz}"
                );
                return;
            }

            let mut osc_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                if let Some(c) =
                    world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                {
                    for osc in c.oscillators.iter_mut() {
                        osc.frequency = *frequency_hz;
                    }
                    emit.push_intent_now(
                        osc_cid,
                        IntentValue::RegisterAudioOscillator {
                            component_ids: vec![osc_cid],
                        },
                    );
                }
            }
        }

        IntentValue::OscillatorScheduleSetPitch {
            component_ids,
            beat_offset,
            beat_context,
            frequency_hz,
        } => {
            let beat = beat_context.unwrap_or(beat_now) + *beat_offset;

            let mut osc_cids = Vec::new();
            for &t in component_ids.iter() {
                collect_oscillator_targets(world, t, &mut osc_cids);
            }
            osc_cids.sort();
            osc_cids.dedup();
            for osc_cid in osc_cids {
                emit.push_intent_now(
                    osc_cid,
                    IntentValue::ScheduleAudioPitchSetHz {
                        component_ids: vec![osc_cid],
                        beat,
                        frequency_hz: *frequency_hz,
                    },
                );
            }
        }

        IntentValue::AudioSchedulePlay {
            component_ids,
            beat_offset,
            beat_context,
            note,
            gain,
            rate: _,
            duration,
        } => {
            let beat = beat_context.unwrap_or(beat_now) + *beat_offset;

            // Per-source semantics (oscillator path): set pitch from note (if any),
            // gate on, set gain, gate off after duration. See docs/spec/audio-sources.md §4.
            let frequency_hz = note
                .as_ref()
                .map(|n| MusicSystem::frequency_hz_for_note(*n));

            let velocity_gain = gain.or_else(|| note.as_ref().map(|n| n.velocity()));
            let final_gain = velocity_gain.map(|g| if g.is_finite() { g.max(0.0) } else { 1.0 });

            let note_duration = note.as_ref().map(|n| n.duration_beats() as f64);
            let stop_after = duration.or(note_duration);

            // Resolve every input id to a concrete audio source id (osc or clip).
            let mut source_cids = Vec::new();
            for &t in component_ids.iter() {
                // MusicNoteComponent: full resolution chain (cache →
                // target_source → context voice). See §6.6.
                let mut resolved_via_note: Option<ComponentId> = None;
                if world
                    .get_component_by_id_as::<MusicNoteComponent>(t)
                    .is_some()
                {
                    let mut taken: MusicNoteComponent = world
                        .get_component_by_id_as::<MusicNoteComponent>(t)
                        .cloned()
                        .unwrap();
                    resolved_via_note = taken.resolve_target(world);
                    if let Some(slot) = world.get_component_by_id_as_mut::<MusicNoteComponent>(t) {
                        slot.target_resolved = taken.target_resolved;
                    }
                }
                if let Some(via_note) = resolved_via_note {
                    source_cids.push(via_note);
                    continue;
                }

                let before = source_cids.len();
                collect_audio_source_targets(world, t, &mut source_cids);
                // Cache the ancestor-walk result back into the
                // MusicNoteComponent so subsequent fires skip the walk.
                if let Some(&found) = source_cids[before..].first() {
                    if let Some(mn) = world.get_component_by_id_as_mut::<MusicNoteComponent>(t) {
                        if mn.target_resolved.is_none() {
                            mn.target_resolved = Some(found);
                        }
                    }
                }
            }
            source_cids.sort();
            source_cids.dedup();

            for src_cid in source_cids {
                // Dispatch by source kind. Oscillator → gate+pitch+gain schedule;
                // Clip → cursor reset (stub log in phase 4).
                if world
                    .get_component_by_id_as::<AudioOscillatorComponent>(src_cid)
                    .is_some()
                {
                    emit.push_intent_now(
                        src_cid,
                        IntentValue::ScheduleAudioOscillatorEnabled {
                            component_ids: vec![src_cid],
                            beat,
                            enabled: true,
                        },
                    );
                    if let Some(frequency_hz) = frequency_hz {
                        emit.push_intent_now(
                            src_cid,
                            IntentValue::ScheduleAudioPitchSetHz {
                                component_ids: vec![src_cid],
                                beat,
                                frequency_hz,
                            },
                        );
                    }
                    if let Some(g) = final_gain {
                        emit.push_intent_now(
                            src_cid,
                            IntentValue::ScheduleAudioGainSet {
                                component_ids: vec![src_cid],
                                beat,
                                gain: g,
                            },
                        );
                    }

                    if let Some(dur) = stop_after {
                        if dur.is_finite() && dur > 0.0 {
                            emit.push_intent_now(
                                src_cid,
                                IntentValue::ScheduleAudioOscillatorEnabled {
                                    component_ids: vec![src_cid],
                                    beat: beat + dur,
                                    enabled: false,
                                },
                            );
                            if final_gain.is_some() {
                                emit.push_intent_now(
                                    src_cid,
                                    IntentValue::ScheduleAudioGainSet {
                                        component_ids: vec![src_cid],
                                        beat: beat + dur,
                                        gain: 1.0,
                                    },
                                );
                            }
                        }
                    }
                } else if let Some(clip) =
                    world.get_component_by_id_as::<AudioClipComponent>(src_cid)
                {
                    match &clip.load_state {
                        AudioClipLoadState::Failed(reason) => {
                            eprintln!(
                                "[AudioSchedulePlay] skip clip {:?} ({}): {}",
                                src_cid, clip.uri, reason
                            );
                        }
                        AudioClipLoadState::Pending | AudioClipLoadState::Loaded => {
                            // Schedule playback via the same SetEnabled
                            // ops the oscillator path uses. The RT thread
                            // resets the clip's cursor on SetEnabled(true)
                            // (docs/spec/audio-sources.md §4).
                            emit.push_intent_now(
                                src_cid,
                                IntentValue::ScheduleAudioOscillatorEnabled {
                                    component_ids: vec![src_cid],
                                    beat,
                                    enabled: true,
                                },
                            );
                            if let Some(g) = final_gain {
                                emit.push_intent_now(
                                    src_cid,
                                    IntentValue::ScheduleAudioGainSet {
                                        component_ids: vec![src_cid],
                                        beat,
                                        gain: g,
                                    },
                                );
                            }
                            if let Some(dur) = stop_after {
                                if dur.is_finite() && dur > 0.0 {
                                    emit.push_intent_now(
                                        src_cid,
                                        IntentValue::ScheduleAudioOscillatorEnabled {
                                            component_ids: vec![src_cid],
                                            beat: beat + dur,
                                            enabled: false,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        _ => {}
    }
}

fn collect_raycast_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    use crate::engine::ecs::component::RayCastComponent;

    if world
        .get_component_by_id_as::<RayCastComponent>(target)
        .is_some()
    {
        out.push(target);
        return;
    }

    // Subtree search: collect all RayCastComponents under this target.
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<RayCastComponent>(node)
            .is_some()
        {
            out.push(node);
            continue;
        }
        for &ch in world.children_of(node).iter() {
            stack.push(ch);
        }
    }
}

fn collect_transform_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    use crate::engine::ecs::component::TransformComponent;

    if world
        .get_component_by_id_as::<TransformComponent>(target)
        .is_some()
    {
        out.push(target);
        return;
    }

    // Subtree search: pick the first TransformComponent encountered per branch.
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<TransformComponent>(node)
            .is_some()
        {
            out.push(node);
            continue;
        }
        for &ch in world.children_of(node).iter() {
            stack.push(ch);
        }
    }
}

fn emit_topology_transform_refresh(world: &World, emit: &mut dyn SignalEmitter, cid: ComponentId) {
    use crate::engine::ecs::IntentValue;
    use crate::engine::ecs::component::{TransformComponent, TransformParentComponent};

    // If this node is a TransformComponent, refreshing it updates cached world matrices
    // for its whole subtree.
    if let Some(t) = world.get_component_by_id_as::<TransformComponent>(cid) {
        let _ = t;
        emit.push_intent_now(
            cid,
            IntentValue::UpdateTransformWorld {
                component_ids: vec![cid],
            },
        );
        return;
    }

    if world
        .get_component_by_id_as::<TransformParentComponent>(cid)
        .is_some()
    {
        emit.push_intent_now(
            cid,
            IntentValue::UpdateTransformWorld {
                component_ids: vec![cid],
            },
        );
        return;
    }

    // Otherwise, refresh the nearest ancestor transform (if any).
    let mut cur = cid;
    while let Some(p) = world.parent_of(cur) {
        if world
            .get_component_by_id_as::<TransformParentComponent>(p)
            .is_some()
        {
            emit.push_intent_now(
                p,
                IntentValue::UpdateTransformWorld {
                    component_ids: vec![p],
                },
            );
            return;
        }
        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(p) {
            let _ = t;
            emit.push_intent_now(
                p,
                IntentValue::UpdateTransformWorld {
                    component_ids: vec![p],
                },
            );
            return;
        }
        cur = p;
    }
}

fn collect_color_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    use crate::engine::ecs::component::{ColorComponent, RenderableComponent};

    // 1) Direct ColorComponent target.
    if world
        .get_component_by_id_as::<ColorComponent>(target)
        .is_some()
    {
        out.push(target);
        return;
    }

    // 2) RenderableComponent target -> find immediate ColorComponent child.
    if world
        .get_component_by_id_as::<RenderableComponent>(target)
        .is_some()
    {
        for &ch in world.children_of(target) {
            if world.get_component_by_id_as::<ColorComponent>(ch).is_some() {
                out.push(ch);
                return;
            }
        }
        return;
    }

    // 3) Generic subtree target (e.g. TransformComponent): search for renderables and their color children.
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            stack.push(ch);
        }

        if world
            .get_component_by_id_as::<RenderableComponent>(node)
            .is_some()
        {
            for &ch in world.children_of(node) {
                if world.get_component_by_id_as::<ColorComponent>(ch).is_some() {
                    out.push(ch);
                }
            }
        }
    }
}

fn is_audio_source(world: &World, id: ComponentId) -> bool {
    use crate::engine::ecs::component::{AudioClipComponent, AudioOscillatorComponent};
    world
        .get_component_by_id_as::<AudioOscillatorComponent>(id)
        .is_some()
        || world
            .get_component_by_id_as::<AudioClipComponent>(id)
            .is_some()
}

/// Collect ids of `AudioSource`s reachable from `target`.
///
/// Treats `AudioOscillatorComponent` and `AudioClipComponent` as peer
/// source variants (§5 of docs/spec/audio-sources.md). Search order:
/// the target itself, then the subtree, then fall back to walking
/// ancestors for the nearest source (§6.6 rank 5).
fn collect_audio_source_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    if is_audio_source(world, target) {
        out.push(target);
        return;
    }

    let before = out.len();
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            stack.push(ch);
        }
        if is_audio_source(world, node) {
            out.push(node);
        }
    }

    if out.len() == before {
        let mut cur = target;
        while let Some(p) = world.parent_of(cur) {
            if is_audio_source(world, p) {
                out.push(p);
                return;
            }
            cur = p;
        }
    }
}

fn collect_oscillator_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    use crate::engine::ecs::component::AudioOscillatorComponent;

    if world
        .get_component_by_id_as::<AudioOscillatorComponent>(target)
        .is_some()
    {
        out.push(target);
        return;
    }

    let before = out.len();
    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            stack.push(ch);
        }

        if world
            .get_component_by_id_as::<AudioOscillatorComponent>(node)
            .is_some()
        {
            out.push(node);
        }
    }

    if out.len() == before {
        let mut cur = target;
        while let Some(p) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<AudioOscillatorComponent>(p)
                .is_some()
            {
                out.push(p);
                return;
            }
            cur = p;
        }
    }
}
