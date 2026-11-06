//! Ordered batch and clip programs, independent of per-view capture bindings.
use super::*;

impl VisualWorld {
    pub(super) fn build_draw_batches_for_order(
        materials: &MaterialRegistry,
        instances: &[VisualInstance],
        order: &[u32],
        out: &mut Vec<DrawBatch>,
    ) {
        out.clear();
        let mut cursor = 0usize;
        while cursor < order.len() {
            let idx0 = order[cursor] as usize;
            let inst0 = instances[idx0];
            let r0 = inst0.renderable;
            let material = r0.material;
            let mesh = r0.mesh;
            let texture = inst0.texture;
            // Legacy Mirror overrides the program while keeping inherited shading.
            // Custom reflective definitions use their resolved MaterialBase contract.
            let view_texture_source = (inst0.renderable.material
                == crate::engine::graphics::MaterialHandle::MIRROR
                || materials
                    .resolved_capabilities(inst0.material_instance, inst0.renderable.material)
                    .planar_reflection)
                .then_some(inst0.component);
            let texture_filtering = inst0.texture_filtering;
            let quant_steps = sanitize_quant_steps(inst0.quant_steps);
            let material_instance = inst0.material_instance;
            let stencil_ref = inst0.stencil_ref;

            let start = cursor;
            cursor += 1;

            while cursor < order.len() {
                let idx = order[cursor] as usize;
                let inst = instances[idx];
                let r = inst.renderable;
                if view_texture_source.is_none()
                    && r.material == material
                    && r.mesh == mesh
                    && inst.texture == texture
                    && inst.texture_filtering == texture_filtering
                    && sanitize_quant_steps(inst.quant_steps).to_bits() == quant_steps.to_bits()
                    && inst.material_instance == material_instance
                    && inst.stencil_ref == stencil_ref
                {
                    cursor += 1;
                } else {
                    break;
                }
            }

            out.push(DrawBatch {
                material,
                mesh,
                texture,
                view_texture_source,
                texture_filtering,
                quant_steps,
                material_instance,
                stencil_ref,
                start,
                count: cursor - start,
            });
        }
    }

    pub(super) fn uses_cached_deformation(
        instance: &VisualInstance,
        materials: &MaterialRegistry,
    ) -> bool {
        materials
            .resolved_capabilities(instance.material_instance, instance.renderable.material)
            .cached_deformation
    }

    pub(super) fn build_outline_render_stream(
        materials: &MaterialRegistry,
        instances: &[VisualInstance],
        order: &[u32],
        ops: &mut Vec<RenderOp>,
        stream_instances: &mut Vec<u32>,
    ) {
        use crate::engine::graphics::MaterialHandle;

        ops.clear();
        stream_instances.clear();
        stream_instances.extend_from_slice(order);

        let mut cursor = 0usize;
        while cursor < order.len() {
            let first = instances[order[cursor] as usize];
            let material = if Self::uses_cached_deformation(&first, materials) {
                MaterialHandle::SKINNED_TOON_MESH
            } else {
                MaterialHandle::TOON_MESH
            };
            let mesh = first.renderable.mesh;
            let start = cursor;
            cursor += 1;
            while cursor < order.len() {
                let instance = instances[order[cursor] as usize];
                let candidate_material = if Self::uses_cached_deformation(&instance, materials) {
                    MaterialHandle::SKINNED_TOON_MESH
                } else {
                    MaterialHandle::TOON_MESH
                };
                if candidate_material != material || instance.renderable.mesh != mesh {
                    break;
                }
                cursor += 1;
            }
            ops.push(RenderOp::DrawBatch(DrawBatch {
                material,
                mesh,
                texture: None,
                view_texture_source: None,
                texture_filtering: TextureFiltering::Linear,
                quant_steps: 1.0,
                material_instance: None,
                stencil_ref: 0,
                start,
                count: cursor - start,
            }));
        }
    }

    pub(super) fn filtered_phase_order(
        &self,
        order: &[u32],
        excluded_instance: Option<InstanceHandle>,
    ) -> Vec<u32> {
        let Some(excluded_index) =
            excluded_instance.and_then(|handle| self.handle_to_index.get(&handle).copied())
        else {
            return order.to_vec();
        };
        order
            .iter()
            .copied()
            .filter(|&idx| idx as usize != excluded_index)
            .collect()
    }

    pub(super) fn filtered_clip_sources_by_depth(
        &self,
        max_depth: u8,
        excluded_instance: Option<InstanceHandle>,
    ) -> Vec<Vec<u32>> {
        let mut clip_sources_by_depth = vec![Vec::new(); max_depth as usize + 1];
        let excluded_index =
            excluded_instance.and_then(|handle| self.handle_to_index.get(&handle).copied());
        for (index, inst) in self.instances.iter().enumerate() {
            if !inst.is_stencil_clip {
                continue;
            }
            if Some(index) == excluded_index {
                continue;
            }
            clip_sources_by_depth[inst.stencil_ref as usize].push(index as u32);
        }
        clip_sources_by_depth
    }

    /// Build the per-phase DFS render stream for the overlay pass.
    ///
    /// `overlay_order` must already be sorted by `(stencil_ref, material, tex, mesh, ...)`.
    ///
    /// Algorithm (one level at a time):
    /// - Non-clip instances at depth D → `DrawBatch` at stencil_ref D.
    /// - Clip sources at depth D → `EnterClip`, then their visual draw at stencil_ref D+1,
    ///   then all content at depth D+1 (recursed), then `ExitClip`.
    pub(super) fn build_phase_render_stream(
        materials: &MaterialRegistry,
        instances: &[VisualInstance],
        phase_order: &[u32],
        has_stencil_clips: bool,
        out_ops: &mut Vec<RenderOp>,
        out_instances: &mut Vec<u32>,
    ) {
        out_ops.clear();
        out_instances.clear();

        if phase_order.is_empty() {
            return;
        }

        if !has_stencil_clips {
            Self::append_stream_batches(
                materials,
                instances,
                phase_order,
                0,
                out_ops,
                out_instances,
            );
            return;
        }

        let max_depth = phase_order
            .iter()
            .map(|&i| instances[i as usize].stencil_ref)
            .chain(
                instances
                    .iter()
                    .filter(|inst| inst.is_stencil_clip)
                    .map(|inst| inst.stencil_ref),
            )
            .max()
            .unwrap_or(0);

        // Split into per-depth groups preserving sort order within each group.
        // Group[d] = (non_clip indices, clip_source indices) at stencil_ref == d.
        let depth_count = max_depth as usize + 1;
        let mut non_clip_by_depth: Vec<Vec<u32>> = vec![Vec::new(); depth_count];
        let mut phase_clip_sources_by_depth: Vec<Vec<u32>> = vec![Vec::new(); depth_count];
        let mut all_clip_sources_by_depth: Vec<Vec<u32>> = vec![Vec::new(); depth_count];

        for (index, inst) in instances.iter().enumerate() {
            if inst.is_stencil_clip {
                all_clip_sources_by_depth[inst.stencil_ref as usize].push(index as u32);
            }
        }

        for &idx in phase_order {
            let inst = &instances[idx as usize];
            let d = inst.stencil_ref as usize;
            if inst.is_stencil_clip {
                phase_clip_sources_by_depth[d].push(idx);
            } else {
                non_clip_by_depth[d].push(idx);
            }
        }

        Self::build_overlay_level(
            materials,
            0,
            max_depth,
            instances,
            &non_clip_by_depth,
            &all_clip_sources_by_depth,
            &phase_clip_sources_by_depth,
            out_ops,
            out_instances,
        );
    }

    pub(super) fn build_phase_render_stream_excluding(
        &self,
        phase_order: &[u32],
        excluded_instance: Option<InstanceHandle>,
        out_ops: &mut Vec<RenderOp>,
        out_instances: &mut Vec<u32>,
    ) {
        let filtered_order = self.filtered_phase_order(phase_order, excluded_instance);
        out_ops.clear();
        out_instances.clear();

        if filtered_order.is_empty() {
            return;
        }

        let excluded_index =
            excluded_instance.and_then(|handle| self.handle_to_index.get(&handle).copied());
        let has_stencil_clips = self.instances.iter().any(|inst| inst.is_stencil_clip);
        if !has_stencil_clips {
            Self::append_stream_batches(
                &self.materials,
                &self.instances,
                &filtered_order,
                0,
                out_ops,
                out_instances,
            );
            return;
        }

        let max_depth = filtered_order
            .iter()
            .map(|&i| self.instances[i as usize].stencil_ref)
            .chain(
                self.instances
                    .iter()
                    .enumerate()
                    .filter(|inst| {
                        let (index, inst) = inst;
                        inst.is_stencil_clip && Some(*index) != excluded_index
                    })
                    .map(|(_, inst)| inst.stencil_ref),
            )
            .max()
            .unwrap_or(0);

        let depth_count = max_depth as usize + 1;
        let mut non_clip_by_depth: Vec<Vec<u32>> = vec![Vec::new(); depth_count];
        let mut phase_clip_sources_by_depth: Vec<Vec<u32>> = vec![Vec::new(); depth_count];
        let all_clip_sources_by_depth =
            self.filtered_clip_sources_by_depth(max_depth, excluded_instance);

        for &idx in &filtered_order {
            let inst = &self.instances[idx as usize];
            let d = inst.stencil_ref as usize;
            if inst.is_stencil_clip {
                phase_clip_sources_by_depth[d].push(idx);
            } else {
                non_clip_by_depth[d].push(idx);
            }
        }

        Self::build_overlay_level(
            &self.materials,
            0,
            max_depth,
            &self.instances,
            &non_clip_by_depth,
            &all_clip_sources_by_depth,
            &phase_clip_sources_by_depth,
            out_ops,
            out_instances,
        );
    }

    pub(super) fn build_overlay_level(
        materials: &MaterialRegistry,
        depth: usize,
        max_depth: u8,
        instances: &[VisualInstance],
        non_clip_by_depth: &[Vec<u32>],
        all_clip_sources_by_depth: &[Vec<u32>],
        phase_clip_sources_by_depth: &[Vec<u32>],
        out_ops: &mut Vec<RenderOp>,
        out_instances: &mut Vec<u32>,
    ) {
        if depth >= non_clip_by_depth.len() {
            return;
        }

        // Draw non-clip instances at this depth.
        let non_clip = &non_clip_by_depth[depth];
        if !non_clip.is_empty() {
            Self::append_stream_batches(
                materials,
                instances,
                non_clip,
                depth as u8,
                out_ops,
                out_instances,
            );
        }

        let all_clip_sources = &all_clip_sources_by_depth[depth];
        if !all_clip_sources.is_empty() {
            let parent_ref = depth as u8;
            let new_ref = parent_ref.saturating_add(1);
            let phase_clip_sources = &phase_clip_sources_by_depth[depth];

            // Emit EnterClip for every clip source at this depth.
            for &src_idx in all_clip_sources {
                if !phase_clip_sources.contains(&src_idx) {
                    Self::append_clip_stream_instance(out_instances, src_idx);
                }
                out_ops.push(RenderOp::EnterClip {
                    instance_index: src_idx,
                    parent_ref,
                    new_ref,
                });
            }

            // Clip sources also draw as normal color instances, but at new_ref
            // (they are inside their own clip region after the INCR).
            if !phase_clip_sources.is_empty() {
                Self::append_stream_batches(
                    materials,
                    instances,
                    phase_clip_sources,
                    new_ref,
                    out_ops,
                    out_instances,
                );
            }

            // Recurse: all content at the next depth is inside the clip region.
            if depth + 1 <= max_depth as usize {
                Self::build_overlay_level(
                    materials,
                    depth + 1,
                    max_depth,
                    instances,
                    non_clip_by_depth,
                    all_clip_sources_by_depth,
                    phase_clip_sources_by_depth,
                    out_ops,
                    out_instances,
                );
            }

            // Emit ExitClip in reverse order (innermost-first, which here means
            // the last clip source entered is the first exited).
            for &src_idx in all_clip_sources.iter().rev() {
                out_ops.push(RenderOp::ExitClip {
                    instance_index: src_idx,
                    ref_value: new_ref,
                });
            }
        } else if depth + 1 <= max_depth as usize {
            // No clips here but deeper levels exist; keep recursing.
            Self::build_overlay_level(
                materials,
                depth + 1,
                max_depth,
                instances,
                non_clip_by_depth,
                all_clip_sources_by_depth,
                phase_clip_sources_by_depth,
                out_ops,
                out_instances,
            );
        }
    }

    pub(super) fn append_clip_stream_instance(out_instances: &mut Vec<u32>, instance_index: u32) {
        if !out_instances.contains(&instance_index) {
            out_instances.push(instance_index);
        }
    }

    /// Append `DrawBatch` ops for a pre-sorted slice of instance indices.
    ///
    /// `effective_ref` overrides the per-instance `stencil_ref` — necessary for clip
    /// sources that are visually drawn inside their own region (`new_ref`).
    pub(super) fn append_stream_batches(
        materials: &MaterialRegistry,
        instances: &[VisualInstance],
        indices: &[u32],
        effective_ref: u8,
        out_ops: &mut Vec<RenderOp>,
        out_instances: &mut Vec<u32>,
    ) {
        let mut cursor = 0usize;
        while cursor < indices.len() {
            let idx0 = indices[cursor] as usize;
            let inst0 = &instances[idx0];
            let r0 = inst0.renderable;
            let material = r0.material;
            let mesh = r0.mesh;
            let texture = inst0.texture;
            // Legacy Mirror overrides the program while keeping inherited shading.
            // Custom reflective definitions use their resolved MaterialBase contract.
            let view_texture_source = (inst0.renderable.material
                == crate::engine::graphics::MaterialHandle::MIRROR
                || materials
                    .resolved_capabilities(inst0.material_instance, inst0.renderable.material)
                    .planar_reflection)
                .then_some(inst0.component);
            let texture_filtering = inst0.texture_filtering;
            let quant_steps = sanitize_quant_steps(inst0.quant_steps);
            let material_instance = inst0.material_instance;

            let start = out_instances.len();
            out_instances.push(indices[cursor]);
            cursor += 1;

            while cursor < indices.len() {
                let idx = indices[cursor] as usize;
                let inst = &instances[idx];
                let r = inst.renderable;
                if view_texture_source.is_none()
                    && r.material == material
                    && r.mesh == mesh
                    && inst.texture == texture
                    && inst.texture_filtering == texture_filtering
                    && sanitize_quant_steps(inst.quant_steps).to_bits() == quant_steps.to_bits()
                    && inst.material_instance == material_instance
                {
                    out_instances.push(indices[cursor]);
                    cursor += 1;
                } else {
                    break;
                }
            }

            out_ops.push(RenderOp::DrawBatch(DrawBatch {
                material,
                mesh,
                texture,
                view_texture_source,
                texture_filtering,
                quant_steps,
                material_instance,
                stencil_ref: effective_ref,
                start,
                count: out_instances.len() - start,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::graphics::{
        MaterialHandle,
        primitives::{MeshHandle, TextureHandle},
    };

    fn register(visuals: &mut VisualWorld, id: u64, material: MaterialHandle) -> InstanceHandle {
        visuals.register(
            slotmap::KeyData::from_ffi(id).into(),
            GpuRenderable::new(MeshHandle::SQUARE, material),
            Transform::default(),
            [1.; 4],
            1.,
            false,
            false,
            false,
            false,
            false,
            0.,
            Some(TextureHandle(42)),
            3.,
        )
    }

    #[test]
    fn reflective_sources_stay_distinct_in_opaque_and_sorted_transparent_batches() {
        let mut visuals = VisualWorld::new();
        let handles = [
            register(&mut visuals, 1, MaterialHandle::MIRROR),
            register(&mut visuals, 2, MaterialHandle::MIRROR),
        ];
        let inherited = visuals.materials.upsert_source(
            slotmap::KeyData::from_ffi(99).into(),
            visuals.materials.anime_definition,
            AnimeShadingParams::default(),
        );
        for handle in handles {
            visuals.bind_material_instance(handle, Some(inherited));
        }
        visuals.prepare_draw_cache();
        let batches: Vec<_> = visuals
            .opaque_stream()
            .0
            .iter()
            .filter_map(|op| match op {
                RenderOp::DrawBatch(batch) => Some(batch),
                _ => None,
            })
            .collect();
        assert_eq!(batches.len(), 2);
        assert!(batches.iter().all(|batch| batch.count == 1));
        assert_ne!(
            batches[0].view_texture_source,
            batches[1].view_texture_source
        );
        for handle in handles {
            visuals.update_opacity_state(handle, 0.5, Some(true));
        }
        visuals.prepare_draw_cache();
        visuals.prepare_transparent_multi_draw_cache_for_view(Transform::default().model);
        assert_eq!(visuals.transparent_multi_draw_batches().len(), 2);
        assert!(
            visuals
                .transparent_multi_draw_batches()
                .iter()
                .all(|b| b.view_texture_source.is_some())
        );
    }

    #[test]
    fn custom_reflective_sources_keep_identity_inside_clip_programs() {
        use crate::engine::graphics::{
            custom_material::CustomMaterialDefinition, materials::MaterialBaseKind,
        };
        let mut visuals = VisualWorld::new();
        let source = slotmap::KeyData::from_ffi(99).into();
        let definition = visuals.materials.register_custom_definition(
            source,
            std::sync::Arc::new(CustomMaterialDefinition {
                base: MaterialBaseKind::PlanarReflective,
                fields: vec![],
                fragment_uri: "test.frag".into(),
                compiled: std::sync::OnceLock::new(),
            }),
        );
        let material = visuals
            .materials
            .definition(definition)
            .unwrap()
            .static_program;
        let instance = visuals.materials.upsert_source(source, definition, vec![]);
        let clip = register(&mut visuals, 10, MaterialHandle::UNLIT_MESH);
        visuals.register_stencil_clip(clip, 0);
        for id in [11, 12] {
            let handle = register(&mut visuals, id, material);
            visuals.bind_material_instance(handle, Some(instance));
            visuals.update_stencil_ref(handle, 1);
        }
        visuals.prepare_draw_cache();
        let batches: Vec<_> = visuals
            .opaque_stream()
            .0
            .iter()
            .filter_map(|op| match op {
                RenderOp::DrawBatch(batch) if batch.view_texture_source.is_some() => Some(batch),
                _ => None,
            })
            .collect();
        assert_eq!(batches.len(), 2);
        assert!(batches.iter().all(|b| b.count == 1 && b.stencil_ref == 1));
        assert_ne!(
            batches[0].view_texture_source,
            batches[1].view_texture_source
        );
    }

    #[test]
    fn unchanged_texture_keeps_draw_cache_but_replacement_invalidates() {
        let mut visuals = VisualWorld::new();
        let handle = register(&mut visuals, 1, MaterialHandle::UNLIT_MESH);
        visuals.prepare_draw_cache();
        assert!(visuals.update_texture(handle, Some(TextureHandle(42))));
        assert!(!visuals.prepare_draw_cache());
        assert!(visuals.update_texture(handle, Some(TextureHandle(43))));
        assert!(visuals.prepare_draw_cache());
        assert_eq!(
            visuals.opaque_stream().0.iter().find_map(|op| match op {
                RenderOp::DrawBatch(batch) => batch.texture,
                _ => None,
            }),
            Some(TextureHandle(43))
        );
    }
}
