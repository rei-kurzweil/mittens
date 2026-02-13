use crate::engine::ecs::component::{
    Action, ActionComponent, ActionMethod, AudioOscillatorComponent, ColorComponent,
    RenderableComponent, TextComponent, TransformComponent,
};
use crate::engine::ecs::component::{AudioBandPassFilterComponent, AudioLowPassFilterComponent};
use crate::engine::ecs::component::{MusicNote, MusicNoteComponent, NotePitch};
use crate::engine::ecs::system::MusicSystem;
use crate::engine::ecs::system::audio_system::AudioOp;
use crate::engine::ecs::{CommandQueue, ComponentCodec, ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;
use slotmap::KeyData;

#[derive(Debug, Default)]
pub struct ActionSystem;

impl ActionSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn execute_action_component(
        &mut self,
        world: &mut World,
        queue: &mut CommandQueue,
        action: &ActionComponent,
    ) {
        // Legacy callers don't provide beat context; treat "scheduled" offsets as immediate.
        self.execute(world, queue, 0.0, &action.action);
    }

    pub fn execute(
        &mut self,
        world: &mut World,
        queue: &mut CommandQueue,
        beat_now: f64,
        action: &Action,
    ) {
        match &action.method {
            ActionMethod::Noop => {}
            ActionMethod::Print => {
                let _msg = action.params.get(0).and_then(|v| v.as_str()).unwrap_or("");
                //println!("[ActionSystem] print targets={:?} msg={}", action.target, msg);
            }
            ActionMethod::SetColor => {
                let Some(rgba) = action.params.get(0).and_then(parse_rgba) else {
                    println!(
                        "[ActionSystem] set_color: missing/invalid rgba params={:?}",
                        action.params
                    );
                    return;
                };

                let mut color_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_color_targets(world, target, &mut color_cids);
                }
                color_cids.sort();
                color_cids.dedup();

                for color_cid in color_cids {
                    if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(color_cid) {
                        c.rgba = rgba;
                        queue.queue_register_color(color_cid);
                    }
                }
            }
            ActionMethod::SetText => {
                let Some(text) = action.params.get(0).and_then(|v| v.as_str()) else {
                    println!(
                        "[ActionSystem] set_text: missing/invalid text params={:?}",
                        action.params
                    );
                    return;
                };

                let mut text_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_text_targets(world, target, &mut text_cids);
                }
                text_cids.sort();
                text_cids.dedup();

                for text_cid in text_cids {
                    queue.queue_set_text(text_cid, text.to_string());
                }
            }
            ActionMethod::SetPosition => {
                let Some(pos) = action.params.get(0).and_then(parse_vec3_f32) else {
                    println!(
                        "[ActionSystem] set_position: missing/invalid position params={:?}",
                        action.params
                    );
                    return;
                };

                let mut transform_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_transform_targets(world, target, &mut transform_cids);
                }
                transform_cids.sort();
                transform_cids.dedup();

                for transform_cid in transform_cids {
                    if let Some(t) =
                        world.get_component_by_id_as_mut::<TransformComponent>(transform_cid)
                    {
                        t.set_position(queue, pos[0], pos[1], pos[2]);
                    }
                }
            }
            ActionMethod::Attach => {
                let Some(child) = action.params.get(0).and_then(parse_component_id) else {
                    println!(
                        "[ActionSystem] attach: missing/invalid child params={:?}",
                        action.params
                    );
                    return;
                };

                for &parent in action.target.iter() {
                    if let Err(e) = world.add_child(parent, child) {
                        println!("[ActionSystem] attach failed: {e}");
                        continue;
                    }

                    if world.is_initialized(parent) {
                        world.init_component_tree(child, queue);
                    }

                    // Topology changes alter world transform composition. Our TransformSystem is
                    // event-driven, so explicitly queue a transform refresh on the moved subtree.
                    queue_topology_transform_refresh(world, queue, child);
                    queue_topology_transform_refresh(world, queue, parent);

                    // Topology change may affect audio compilation.
                    queue.queue_audio_graph_dirty(parent);
                    queue.queue_audio_graph_dirty(child);
                }
            }
            ActionMethod::AttachClone => {
                let Some(prefab_root) = action.params.get(0).and_then(parse_component_id) else {
                    println!(
                        "[ActionSystem] attach_clone: missing/invalid prefab_root params={:?}",
                        action.params
                    );
                    return;
                };

                let node = match ComponentCodec::encode_subtree_node(&*world, prefab_root) {
                    Ok(n) => n,
                    Err(e) => {
                        println!("[ActionSystem] attach_clone failed: {e}");
                        return;
                    }
                };

                for &parent in action.target.iter() {
                    let new_root = match ComponentCodec::decode_subtree_node_with_new_guids(
                        world,
                        Some(parent),
                        &node,
                    ) {
                        Ok(id) => id,
                        Err(e) => {
                            println!("[ActionSystem] attach_clone decode failed: {e}");
                            continue;
                        }
                    };

                    if world.is_initialized(parent) {
                        world.init_component_tree(new_root, queue);
                    }

                    queue_topology_transform_refresh(world, queue, new_root);
                    queue_topology_transform_refresh(world, queue, parent);

                    queue.queue_audio_graph_dirty(parent);
                    queue.queue_audio_graph_dirty(new_root);
                }
            }
            ActionMethod::Detach => {
                for &child in action.target.iter() {
                    // Topology change may affect audio compilation. Mark before detaching so
                    // we can still find an AudioOutput ancestor.
                    queue.queue_audio_graph_dirty(child);

                    let old_parent = world.parent_of(child);
                    world.detach_from_parent(child);
                    if let Some(p) = old_parent {
                        queue.queue_audio_graph_dirty(p);
                    }

                    queue_topology_transform_refresh(world, queue, child);
                    if let Some(p) = old_parent {
                        queue_topology_transform_refresh(world, queue, p);
                    }
                }
            }
            ActionMethod::RemoveChild => {
                let Some(index) = action.params.get(0).and_then(parse_usize) else {
                    println!(
                        "[ActionSystem] remove_child: missing/invalid index params={:?}",
                        action.params
                    );
                    return;
                };

                for &parent in action.target.iter() {
                    let child = world.children_of(parent).get(index).copied();
                    let Some(child) = child else {
                        // No-op when index is out of range (common when a parent has only marker children).
                        continue;
                    };

                    queue.queue_audio_graph_dirty(child);
                    queue.queue_audio_graph_dirty(parent);

                    world.detach_from_parent(child);
                    queue.queue_remove_subtree(child);

                    queue_topology_transform_refresh(world, queue, parent);
                }
            }
            ActionMethod::RemoveChildren => {
                for &parent in action.target.iter() {
                    let children: Vec<ComponentId> = world.children_of(parent).to_vec();
                    if children.is_empty() {
                        continue;
                    }

                    queue.queue_audio_graph_dirty(parent);
                    for child in children {
                        queue.queue_audio_graph_dirty(child);
                        world.detach_from_parent(child);
                        queue.queue_remove_subtree(child);
                    }

                    queue_topology_transform_refresh(world, queue, parent);
                }
            }
            ActionMethod::RemoveSubtree => {
                for &root in action.target.iter() {
                    // Topology change may affect audio compilation. Mark before removal so we
                    // can still walk to an AudioOutput ancestor.
                    queue.queue_audio_graph_dirty(root);
                    queue.queue_remove_subtree(root);
                }
            }
            ActionMethod::AudioGraphRebuild => {
                // Mark graphs dirty; AudioSystem will batch recompile + schedule a graph swap
                // once after CommandQueue flush for this frame.
                for &target in action.target.iter() {
                    queue.queue_audio_graph_dirty(target);
                }
            }
            ActionMethod::AudioLowPassSetCutoffHz => {
                let Some(cutoff_hz) = action.params.get(0).and_then(parse_f32) else {
                    println!(
                        "[ActionSystem] audio_low_pass_set_cutoff_hz: missing/invalid cutoff_hz params={:?}",
                        action.params
                    );
                    return;
                };

                for &target in action.target.iter() {
                    if let Some(c) =
                        world.get_component_by_id_as_mut::<AudioLowPassFilterComponent>(target)
                    {
                        c.cutoff_hz = if cutoff_hz.is_finite() {
                            cutoff_hz.max(0.0)
                        } else {
                            c.cutoff_hz
                        };

                        // Cutoff changes don't require rebuilding the entire graph; update the
                        // compiled RT node in-place at the current beat.
                        queue.queue_schedule_audio_op(
                            target,
                            beat_now,
                            AudioOp::SetLowPassCutoffHz(c.cutoff_hz),
                        );
                    }
                }
            }
            ActionMethod::AudioBandPassSetCenterHz => {
                let Some(center_hz) = action.params.get(0).and_then(parse_f32) else {
                    println!(
                        "[ActionSystem] audio_band_pass_set_center_hz: missing/invalid center_hz params={:?}",
                        action.params
                    );
                    return;
                };

                for &target in action.target.iter() {
                    if let Some(c) =
                        world.get_component_by_id_as_mut::<AudioBandPassFilterComponent>(target)
                    {
                        c.center_hz = if center_hz.is_finite() {
                            center_hz.max(0.0)
                        } else {
                            c.center_hz
                        };

                        queue.queue_schedule_audio_op(
                            target,
                            beat_now,
                            AudioOp::SetBandPassCenterHz(c.center_hz),
                        );
                    }
                }
            }
            ActionMethod::OscillatorSetEnabled => {
                let Some(enabled) = action.params.get(0).and_then(parse_bool) else {
                    println!(
                        "[ActionSystem] oscillator_set_enabled: missing/invalid enabled params={:?}",
                        action.params
                    );
                    return;
                };

                let mut osc_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_oscillator_targets(world, target, &mut osc_cids);
                }
                osc_cids.sort();
                osc_cids.dedup();

                for osc_cid in osc_cids {
                    if let Some(c) =
                        world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                    {
                        for osc in c.oscillators.iter_mut() {
                            osc.enabled = enabled;
                        }
                        queue.queue_register_audio_oscillator(osc_cid);
                    }
                }
            }
            ActionMethod::OscillatorSetPitch => {
                let Some(frequency_hz) = action.params.get(0).and_then(parse_f32) else {
                    println!(
                        "[ActionSystem] oscillator_set_pitch: missing/invalid frequency params={:?}",
                        action.params
                    );
                    return;
                };
                if !frequency_hz.is_finite() {
                    println!(
                        "[ActionSystem] oscillator_set_pitch: non-finite frequency_hz={frequency_hz}"
                    );
                    return;
                }

                let mut osc_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_oscillator_targets(world, target, &mut osc_cids);
                }
                osc_cids.sort();
                osc_cids.dedup();

                for osc_cid in osc_cids {
                    if let Some(c) =
                        world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                    {
                        for osc in c.oscillators.iter_mut() {
                            osc.frequency = frequency_hz;
                            osc.music_note_applied = true;
                        }
                        queue.queue_register_audio_oscillator(osc_cid);
                    }
                }
            }
            ActionMethod::OscillatorScheduleSetPitch => {
                let Some(beat_offset) = action.params.get(0).and_then(parse_f64) else {
                    println!(
                        "[ActionSystem] oscillator_schedule_set_pitch: missing/invalid beat params={:?}",
                        action.params
                    );
                    return;
                };
                let Some(frequency_hz) = action.params.get(1).and_then(parse_f32) else {
                    println!(
                        "[ActionSystem] oscillator_schedule_set_pitch: missing/invalid frequency params={:?}",
                        action.params
                    );
                    return;
                };

                let beat = beat_now + beat_offset;

                let mut osc_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_oscillator_targets(world, target, &mut osc_cids);
                }
                osc_cids.sort();
                osc_cids.dedup();

                for osc_cid in osc_cids {
                    queue.queue_schedule_audio_pitch_set_hz(osc_cid, beat, frequency_hz);
                }
            }
            ActionMethod::OscillatorScheduleSetNote => {
                let Some(beat_offset) = action.params.get(0).and_then(parse_f64) else {
                    println!(
                        "[ActionSystem] oscillator_schedule_set_note: missing/invalid beat params={:?}",
                        action.params
                    );
                    return;
                };
                let Some(pitch) = action.params.get(1).and_then(parse_pitch) else {
                    println!(
                        "[ActionSystem] oscillator_schedule_set_note: missing/invalid pitch params={:?}",
                        action.params
                    );
                    return;
                };
                let Some(octave) = action.params.get(2).and_then(parse_u16) else {
                    println!(
                        "[ActionSystem] oscillator_schedule_set_note: missing/invalid octave params={:?}",
                        action.params
                    );
                    return;
                };

                let duration_beats = action
                    .params
                    .get(3)
                    .and_then(parse_f32)
                    .unwrap_or(0.25)
                    .max(0.0) as f64;

                let beat = beat_now + beat_offset;

                let note = MusicNote::from_pitch(duration_beats as f32, pitch, octave);
                let frequency_hz = MusicSystem::frequency_hz_for_note(note);

                let mut osc_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_oscillator_targets(world, target, &mut osc_cids);
                }
                osc_cids.sort();
                osc_cids.dedup();

                for osc_cid in osc_cids {
                    queue.queue_schedule_audio_oscillator_enabled(osc_cid, beat, true);
                    queue.queue_schedule_audio_pitch_set_hz(osc_cid, beat, frequency_hz);

                    if duration_beats.is_finite() && duration_beats > 0.0 {
                        queue.queue_schedule_audio_oscillator_enabled(
                            osc_cid,
                            beat + duration_beats,
                            false,
                        );
                    }
                }
            }
            ActionMethod::OscillatorScheduleMusicNote => {
                let Some(beat_offset) = action.params.get(0).and_then(parse_f64) else {
                    println!(
                        "[ActionSystem] oscillator_schedule_music_note: missing/invalid beat params={:?}",
                        action.params
                    );
                    return;
                };

                let Some(note) = action.params.get(1).and_then(parse_music_note) else {
                    println!(
                        "[ActionSystem] oscillator_schedule_music_note: missing/invalid note params={:?}",
                        action.params
                    );
                    return;
                };

                // Velocity is part of MusicNote.
                let velocity = note.velocity();
                let velocity = if velocity.is_finite() {
                    velocity.max(0.0)
                } else {
                    1.0
                };

                let frequency_hz = MusicSystem::frequency_hz_for_note(note);
                let duration_beats = note.duration_beats() as f64;

                let beat = beat_now + beat_offset;

                let mut osc_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_oscillator_targets(world, target, &mut osc_cids);
                }
                osc_cids.sort();
                osc_cids.dedup();

                for osc_cid in osc_cids {
                    queue.queue_schedule_audio_oscillator_enabled(osc_cid, beat, true);
                    queue.queue_schedule_audio_pitch_set_hz(osc_cid, beat, frequency_hz);
                    queue.queue_schedule_audio_gain_set(osc_cid, beat, velocity);

                    if duration_beats.is_finite() && duration_beats > 0.0 {
                        queue.queue_schedule_audio_oscillator_enabled(
                            osc_cid,
                            beat + duration_beats,
                            false,
                        );

                        // Reset gain after note-off for predictable one-shot behavior.
                        queue.queue_schedule_audio_gain_set(osc_cid, beat + duration_beats, 1.0);
                    }
                }
            }
            ActionMethod::MusicSetNote => {
                let Some(note) = action.params.get(0).and_then(parse_music_note) else {
                    println!(
                        "[ActionSystem] music_set_note: missing/invalid note params={:?}",
                        action.params
                    );
                    return;
                };

                let mut osc_cids = Vec::new();
                for &target in action.target.iter() {
                    collect_oscillator_targets(world, target, &mut osc_cids);
                }
                osc_cids.sort();
                osc_cids.dedup();

                for osc_cid in osc_cids {
                    // 1) Update note component under this oscillator (if present).
                    if let Some(note_cid) = find_first_music_note_component(world, osc_cid) {
                        if let Some(nc) =
                            world.get_component_by_id_as_mut::<MusicNoteComponent>(note_cid)
                        {
                            nc.note = note;
                        }
                    }

                    // 2) Apply computed frequency to oscillator(s).
                    let freq = MusicSystem::frequency_hz_for_note(note);
                    if let Some(c) =
                        world.get_component_by_id_as_mut::<AudioOscillatorComponent>(osc_cid)
                    {
                        for osc in c.oscillators.iter_mut() {
                            osc.frequency = freq;
                            osc.music_note_applied = true;
                        }
                        queue.queue_register_audio_oscillator(osc_cid);
                    }
                }
            }
            ActionMethod::CommandQueue { command_name } => {
                println!(
                    "[ActionSystem] command_queue '{}' targets={:?} params={:?}",
                    command_name, action.target, action.params
                );
            }
        }
    }
}

fn parse_vec3_f32(v: &serde_json::Value) -> Option<[f32; 3]> {
    let arr = v.as_array()?;
    if arr.len() != 3 {
        return None;
    }
    let x = arr[0].as_f64()? as f32;
    let y = arr[1].as_f64()? as f32;
    let z = arr[2].as_f64()? as f32;
    if !(x.is_finite() && y.is_finite() && z.is_finite()) {
        return None;
    }
    Some([x, y, z])
}

fn collect_transform_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
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

fn queue_topology_transform_refresh(world: &World, queue: &mut CommandQueue, cid: ComponentId) {
    // If this node is a TransformComponent, refreshing it updates cached world matrices
    // for its whole subtree.
    if let Some(t) = world.get_component_by_id_as::<TransformComponent>(cid) {
        queue.queue_update_transform(cid, t.transform);
        return;
    }

    // Otherwise, refresh the nearest ancestor transform (if any).
    let mut cur = cid;
    while let Some(p) = world.parent_of(cur) {
        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(p) {
            queue.queue_update_transform(p, t.transform);
            return;
        }
        cur = p;
    }
}

impl crate::engine::ecs::system::System for ActionSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // Event-driven: executed by AnimationSystem when keyframes fire.
    }
}

fn parse_rgba(v: &serde_json::Value) -> Option<[f32; 4]> {
    // Accept either a JSON array [r,g,b,a] or object {r,g,b,a} in the future.
    let arr = v.as_array()?;
    if arr.len() != 4 {
        return None;
    }
    let mut rgba = [0.0; 4];
    for i in 0..4 {
        rgba[i] = arr[i].as_f64()? as f32;
    }
    Some(rgba)
}

fn parse_bool(v: &serde_json::Value) -> Option<bool> {
    if let Some(b) = v.as_bool() {
        return Some(b);
    }
    if let Some(i) = v.as_i64() {
        return Some(i != 0);
    }
    if let Some(f) = v.as_f64() {
        return Some(f != 0.0);
    }
    None
}

fn parse_f32(v: &serde_json::Value) -> Option<f32> {
    if let Some(f) = v.as_f64() {
        return Some(f as f32);
    }
    if let Some(i) = v.as_i64() {
        return Some(i as f32);
    }
    if let Some(u) = v.as_u64() {
        return Some(u as f32);
    }
    None
}

fn parse_component_id(v: &serde_json::Value) -> Option<ComponentId> {
    let ffi = if let Some(u) = v.as_u64() {
        u
    } else if let Some(i) = v.as_i64() {
        u64::try_from(i).ok()?
    } else {
        return None;
    };
    Some(KeyData::from_ffi(ffi).into())
}

fn parse_f64(v: &serde_json::Value) -> Option<f64> {
    if let Some(f) = v.as_f64() {
        return Some(f);
    }
    if let Some(i) = v.as_i64() {
        return Some(i as f64);
    }
    if let Some(u) = v.as_u64() {
        return Some(u as f64);
    }
    None
}

fn parse_u16(v: &serde_json::Value) -> Option<u16> {
    if let Some(u) = v.as_u64() {
        return u16::try_from(u).ok();
    }
    if let Some(i) = v.as_i64() {
        return u16::try_from(i).ok();
    }
    if let Some(f) = v.as_f64() {
        if f.is_finite() && f >= 0.0 {
            return u16::try_from(f as u64).ok();
        }
    }
    None
}

fn parse_usize(v: &serde_json::Value) -> Option<usize> {
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).ok();
    }
    if let Some(i) = v.as_i64() {
        return usize::try_from(i).ok();
    }
    if let Some(f) = v.as_f64() {
        if f.is_finite() && f >= 0.0 {
            return usize::try_from(f as u64).ok();
        }
    }
    None
}

fn parse_pitch(v: &serde_json::Value) -> Option<NotePitch> {
    serde_json::from_value::<NotePitch>(v.clone()).ok()
}

fn parse_music_note(v: &serde_json::Value) -> Option<MusicNote> {
    serde_json::from_value::<MusicNote>(v.clone()).ok()
}

fn find_first_music_note_component(world: &World, target: ComponentId) -> Option<ComponentId> {
    // Find the first MusicNoteComponent anywhere in the subtree.
    if world
        .get_component_by_id_as::<MusicNoteComponent>(target)
        .is_some()
    {
        return Some(target);
    }

    let mut stack = vec![target];
    while let Some(node) = stack.pop() {
        for &ch in world.children_of(node) {
            if world
                .get_component_by_id_as::<MusicNoteComponent>(ch)
                .is_some()
            {
                return Some(ch);
            }
            stack.push(ch);
        }
    }
    None
}

fn collect_color_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
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

fn collect_oscillator_targets(world: &World, target: ComponentId, out: &mut Vec<ComponentId>) {
    if world
        .get_component_by_id_as::<AudioOscillatorComponent>(target)
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
            .get_component_by_id_as::<AudioOscillatorComponent>(node)
            .is_some()
        {
            out.push(node);
        }
    }
}

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
