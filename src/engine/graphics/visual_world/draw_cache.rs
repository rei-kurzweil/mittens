//! Retained draw-stream classification and per-view transparency preparation.
use super::*;

impl VisualWorld {
    pub fn prepare_draw_cache(&mut self) -> bool {
        #[cfg(test)]
        self.draw_cache_profile.prepare(self.dirty_draw_cache);
        if !self.dirty_draw_cache {
            return false;
        }

        self.background_order.clear();
        self.background_occluded_lit_order.clear();
        self.background_occluded_lit_emissive_order.clear();
        self.background_occluded_lit_emissive_batches.clear();
        self.draw_order.clear();
        self.emissive_draw_order.clear();
        self.outline_order.clear();
        self.cutout_order.clear();
        self.emissive_cutout_order.clear();
        self.refraction_order.clear();
        self.rough_transmission_order.clear();
        self.transparent_single_draw_order.clear();
        self.overlay_order.clear();
        self.stencil_clip_order.clear();
        // Opaque pass: exclude anything that is transparent.
        for i in 0..self.instances.len() {
            let inst = &self.instances[i];
            let capabilities = self
                .materials
                .resolved_capabilities(inst.material_instance, inst.renderable.material);
            if inst.toon_outline.is_some_and(|outline| outline.width > 0.0)
                && !inst.overlay
                && !inst.background
                && !inst.transparent_cutout
                && !Self::is_transparent(inst)
                && capabilities.material.transmission
                    == crate::engine::graphics::materials::MaterialTransmission::None
                && inst.stencil_ref == 0
            {
                self.outline_order.push(i as u32);
            }
            if inst.is_stencil_clip {
                self.stencil_clip_order.push(i as u32);
            }
            if inst.overlay {
                self.overlay_order.push(i as u32);
            } else if inst.background {
                if inst.background_occluded_lit {
                    self.background_occluded_lit_order.push(i as u32);
                } else {
                    self.background_order.push(i as u32);
                }
            } else if capabilities.material.transmission
                == crate::engine::graphics::materials::MaterialTransmission::Sharp
            {
                self.refraction_order.push(i as u32);
            } else if capabilities.material.transmission
                == crate::engine::graphics::materials::MaterialTransmission::Rough
            {
                self.rough_transmission_order.push(i as u32);
            } else if inst.transparent_cutout {
                self.cutout_order.push(i as u32);
            } else if !Self::is_transparent(inst) {
                self.draw_order.push(i as u32);
            } else if !inst.multiple_layers {
                self.transparent_single_draw_order.push(i as u32);
            }
        }
        self.stencil_clip_order
            .sort_by_key(|&i| self.instances[i as usize].stencil_ref);
        let has_stencil_clips = !self.stencil_clip_order.is_empty();

        self.outline_order.sort_by_key(|&i| {
            let instance = self.instances[i as usize];
            (
                u8::from(Self::uses_cached_deformation(&instance, &self.materials)),
                instance.renderable.mesh.0,
            )
        });
        Self::build_outline_render_stream(
            &self.materials,
            &self.instances,
            &self.outline_order,
            &mut self.outline_stream,
            &mut self.outline_stream_instances,
        );

        // Background pass: batch aggressively (order does not depend on view).
        // NOTE: Background instances are excluded from the normal opaque/transparent lists.
        self.background_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
                material_parameter(inst.renderable.material, inst.quant_steps).to_bits(),
                inst.material_instance.map(|id| id.data().as_ffi()),
            )
        });
        let instances = &self.instances;
        let background_draw_order = &self.background_order;
        Self::build_draw_batches_for_order(
            &self.materials,
            instances,
            background_draw_order,
            &mut self.background_batches,
        );

        // Background (occluded+lit) pass: same batching strategy.
        self.background_occluded_lit_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
                material_parameter(inst.renderable.material, inst.quant_steps).to_bits(),
                inst.material_instance.map(|id| id.data().as_ffi()),
            )
        });
        let background_occluded_lit_draw_order = &self.background_occluded_lit_order;
        Self::build_draw_batches_for_order(
            &self.materials,
            instances,
            background_occluded_lit_draw_order,
            &mut self.background_occluded_lit_batches,
        );

        if self
            .background_occluded_lit_order
            .iter()
            .any(|&i| Self::is_emissive_instance(&self.instances[i as usize], &self.materials))
        {
            self.background_occluded_lit_emissive_order.extend(
                self.background_occluded_lit_order
                    .iter()
                    .copied()
                    .filter(|&i| {
                        Self::is_emissive_instance(&self.instances[i as usize], &self.materials)
                    }),
            );
            Self::build_draw_batches_for_order(
                &self.materials,
                instances,
                &self.background_occluded_lit_emissive_order,
                &mut self.background_occluded_lit_emissive_batches,
            );
        }

        // Sort by (stencil_ref, material, tex, mesh, filtering) so stencil-clipped
        // instances group with their clip region. tex before mesh matches overlay convention
        // (untextured quads draw before textured glyphs at same depth).
        self.draw_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map_or(0, |t| t.0.wrapping_add(1));
            (
                inst.stencil_ref,
                r.material.0,
                tex,
                r.mesh.0,
                inst.texture_filtering as u8,
                material_parameter(inst.renderable.material, inst.quant_steps).to_bits(),
                inst.material_instance.map(|id| id.data().as_ffi()),
            )
        });

        // Build the DFS render stream for the opaque phase.
        Self::build_phase_render_stream(
            &self.materials,
            instances,
            &self.draw_order,
            has_stencil_clips,
            &mut self.opaque_stream,
            &mut self.opaque_stream_instances,
        );

        self.emissive_draw_order.extend(
            self.draw_order.iter().copied().filter(|&i| {
                Self::is_emissive_instance(&self.instances[i as usize], &self.materials)
            }),
        );
        let emissive_draw_order = &self.emissive_draw_order;
        Self::build_draw_batches_for_order(
            &self.materials,
            instances,
            emissive_draw_order,
            &mut self.emissive_draw_batches,
        );

        // Cutout pass: batch aggressively (order does not depend on view).
        self.cutout_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                inst.stencil_ref,
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
                material_parameter(inst.renderable.material, inst.quant_steps).to_bits(),
                inst.material_instance.map(|id| id.data().as_ffi()),
            )
        });
        Self::build_phase_render_stream(
            &self.materials,
            instances,
            &self.cutout_order,
            has_stencil_clips,
            &mut self.cutout_stream,
            &mut self.cutout_stream_instances,
        );

        self.refraction_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                inst.stencil_ref,
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
            )
        });
        Self::build_phase_render_stream(
            &self.materials,
            instances,
            &self.refraction_order,
            has_stencil_clips,
            &mut self.refraction_stream,
            &mut self.refraction_stream_instances,
        );

        self.rough_transmission_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                inst.stencil_ref,
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
            )
        });
        Self::build_phase_render_stream(
            &self.materials,
            instances,
            &self.rough_transmission_order,
            has_stencil_clips,
            &mut self.rough_transmission_stream,
            &mut self.rough_transmission_stream_instances,
        );

        self.emissive_cutout_order.extend(
            self.cutout_order.iter().copied().filter(|&i| {
                Self::is_emissive_instance(&self.instances[i as usize], &self.materials)
            }),
        );
        let emissive_cutout_order = &self.emissive_cutout_order;
        Self::build_draw_batches_for_order(
            &self.materials,
            instances,
            emissive_cutout_order,
            &mut self.emissive_cutout_batches,
        );

        // Single-layer transparent pass: batch aggressively (order does not depend on view).
        self.transparent_single_draw_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                inst.stencil_ref,
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
                material_parameter(inst.renderable.material, inst.quant_steps).to_bits(),
                inst.material_instance.map(|id| id.data().as_ffi()),
            )
        });
        Self::build_phase_render_stream(
            &self.materials,
            instances,
            &self.transparent_single_draw_order,
            has_stencil_clips,
            &mut self.transparent_single_stream,
            &mut self.transparent_single_stream_instances,
        );

        // Overlay pass: no-texture instances (e.g. text backgrounds) must draw before
        // textured glyphs so that depth-write from glyph edge pixels does not block
        // the background quad.  tex=0 for untextured, tex=handle+1 for textured, so
        // placing tex BEFORE mesh ensures untextured always sorts first regardless of
        // which MeshHandle was allocated earlier.
        self.overlay_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map_or(0, |t| t.0.wrapping_add(1));
            (
                inst.stencil_ref,
                r.material.0,
                tex,
                r.mesh.0,
                inst.texture_filtering as u8,
                material_parameter(inst.renderable.material, inst.quant_steps).to_bits(),
                inst.material_instance.map(|id| id.data().as_ffi()),
            )
        });
        // Build the DFS render stream for the overlay phase.
        Self::build_phase_render_stream(
            &self.materials,
            instances,
            &self.overlay_order,
            has_stencil_clips,
            &mut self.overlay_stream,
            &mut self.overlay_stream_instances,
        );

        self.dirty_draw_cache = false;
        true
    }

    pub fn prepare_transparent_multi_draw_cache_for_eye(
        &mut self,
        target: CameraTarget,
        eye: usize,
    ) {
        let view = self.camera_view_for_eye(target, eye);
        self.prepare_transparent_multi_draw_cache_for_view(view);
    }

    /// Rebuild multi-layer transparent draw order/batches for a specific view matrix.
    pub fn prepare_transparent_multi_draw_cache_for_view(&mut self, view: [[f32; 4]; 4]) {
        #[cfg(test)]
        self.draw_cache_profile.transparency();
        self.transparent_multi_draw_order.clear();

        for i in 0..self.instances.len() {
            let inst = &self.instances[i];
            if inst.overlay {
                continue;
            }
            if inst.background {
                continue;
            }
            if inst.transparent_cutout {
                continue;
            }
            if inst.multiple_layers && Self::is_transparent(inst) {
                self.transparent_multi_draw_order.push(i as u32);
            }
        }

        if self.transparent_multi_draw_order.is_empty() {
            self.transparent_multi_draw_batches.clear();
            return;
        }

        // Back-to-front for blending.
        self.transparent_multi_draw_order.sort_by(|&a, &b| {
            let ia = self.instances[a as usize];
            let ib = self.instances[b as usize];
            let za = Self::view_space_z(view, ia.transform.model);
            let zb = Self::view_space_z(view, ib.transform.model);
            za.partial_cmp(&zb)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(&b))
        });

        let instances = &self.instances;
        let transparent_draw_order = &self.transparent_multi_draw_order;
        Self::build_draw_batches_for_order(
            &self.materials,
            instances,
            transparent_draw_order,
            &mut self.transparent_multi_draw_batches,
        );
    }
}
