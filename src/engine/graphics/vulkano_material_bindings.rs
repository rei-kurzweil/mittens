// Fixed material uniforms and immutable/mutable descriptor selection.
impl VulkanoState {
    fn create_material_ubo(
        material: crate::engine::graphics::MaterialHandle,
        quant_steps: f32,
        anime_shading: crate::engine::graphics::visual_world::AnimeShadingParams,
    ) -> MaterialUBO {
        let quant_steps = material_parameter(material, quant_steps);

        let mut ubo = match material {
            crate::engine::graphics::MaterialHandle::TOON_MESH
            | crate::engine::graphics::MaterialHandle::ANIME_MESH
            | crate::engine::graphics::MaterialHandle::SKINNED_ANIME_MESH => MaterialUBO {
                base_color: [1.0, 1.0, 1.0, 1.0],
                quant_steps,
                emissive: 0,
                _pad0: 0,
                _pad1: 0,
                ..Default::default()
            },
            crate::engine::graphics::MaterialHandle::SKINNED_TOON_MESH => MaterialUBO {
                base_color: [1.0, 1.0, 1.0, 1.0],
                quant_steps,
                emissive: 0,
                _pad0: 0,
                _pad1: 0,
                ..Default::default()
            },
            crate::engine::graphics::MaterialHandle::EMISSIVE_TOON_MESH => MaterialUBO {
                base_color: [1.0, 1.0, 1.0, 1.0],
                quant_steps,
                emissive: 0,
                _pad0: 0,
                _pad1: 0,
                ..Default::default()
            },
            crate::engine::graphics::MaterialHandle::SKINNED_EMISSIVE_TOON_MESH => MaterialUBO {
                base_color: [1.0, 1.0, 1.0, 1.0],
                quant_steps,
                emissive: 0,
                _pad0: 0,
                _pad1: 0,
                ..Default::default()
            },
            crate::engine::graphics::MaterialHandle::GRID_MESH => MaterialUBO {
                base_color: [1.0, 1.0, 1.0, 1.0],
                quant_steps,
                emissive: 1,
                _pad0: 0,
                _pad1: 0,
                ..Default::default()
            },
            crate::engine::graphics::MaterialHandle::UNLIT_MESH => MaterialUBO {
                base_color: [1.0, 1.0, 1.0, 1.0],
                quant_steps,
                // The unlit shader does not use this UBO, but it is neither lit nor emissive.
                emissive: 0,
                _pad0: 0,
                _pad1: 0,
                ..Default::default()
            },
            crate::engine::graphics::MaterialHandle::MIRROR => MaterialUBO {
                base_color: [1.0, 1.0, 1.0, 1.0],
                quant_steps: 1.0,
                emissive: 1,
                _pad0: 0,
                _pad1: 0,
                ..Default::default()
            },
            crate::engine::graphics::MaterialHandle::REFRACTION_MESH
            | crate::engine::graphics::MaterialHandle::SKINNED_REFRACTION_MESH
            | crate::engine::graphics::MaterialHandle::ROUGH_TRANSMISSION_MESH
            | crate::engine::graphics::MaterialHandle::SKINNED_ROUGH_TRANSMISSION_MESH => {
                MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps: 1.0,
                    emissive: 0,
                    _pad0: 0,
                    _pad1: 0,
                    ..Default::default()
                }
            }
            _ => MaterialUBO::default(),
        };
        ubo.anime_shade_color_strength = anime_shading.shade_color_strength;
        ubo.anime_rim_color = anime_shading.rim_color;
        ubo.anime_controls = anime_shading.controls;
        ubo
    }

    fn get_or_create_material_set(
        &mut self,
        material: crate::engine::graphics::MaterialHandle,
        texture_handle: TextureHandle,
        filtering: TextureFiltering,
        quant_steps: f32,
        material_instance: Option<crate::engine::graphics::materials::MaterialInstanceId>,
    ) -> Result<Option<Arc<DescriptorSet>>, Box<dyn std::error::Error>> {
        if let Some(id) = material_instance {
            if self
                .custom_programs
                .get(&material)
                .is_some_and(Result::is_ok)
            {
                let result = self.custom_material_set(id, material, texture_handle, filtering);
                self.log_material_upload_stats();
                return result;
            }
            let result =
                self.mutable_material_set(id, material, texture_handle, filtering, quant_steps);
            self.log_material_upload_stats();
            return result;
        }
        let anime_shading = crate::engine::graphics::visual_world::AnimeShadingParams::default();
        match material {
            crate::engine::graphics::MaterialHandle::TOON_MESH
            | crate::engine::graphics::MaterialHandle::UNLIT_MESH
            | crate::engine::graphics::MaterialHandle::SKINNED_TOON_MESH
            | crate::engine::graphics::MaterialHandle::EMISSIVE_TOON_MESH
            | crate::engine::graphics::MaterialHandle::SKINNED_EMISSIVE_TOON_MESH
            | crate::engine::graphics::MaterialHandle::GRID_MESH
            | crate::engine::graphics::MaterialHandle::MIRROR
            | crate::engine::graphics::MaterialHandle::REFRACTION_MESH
            | crate::engine::graphics::MaterialHandle::SKINNED_REFRACTION_MESH
            | crate::engine::graphics::MaterialHandle::ROUGH_TRANSMISSION_MESH
            | crate::engine::graphics::MaterialHandle::SKINNED_ROUGH_TRANSMISSION_MESH => {}
            crate::engine::graphics::MaterialHandle::ANIME_MESH
            | crate::engine::graphics::MaterialHandle::SKINNED_ANIME_MESH => {}
            _ => return Ok(None),
        }

        let Some(tex) = self.textures.get(&texture_handle) else {
            return Ok(None);
        };

        let material_key = (
            MaterialResourceKey::new(material, texture_handle, filtering, quant_steps),
            anime_shading.key_bits(),
        );
        if let Some(set) = self.cached_material_sets.get(&material_key) {
            return Ok(Some(set.clone()));
        }

        let material_ubo = Self::create_material_ubo(material, quant_steps, anime_shading);
        let material_buffer: Subbuffer<MaterialUBO> = Buffer::from_data(
            self.context.memory_allocator().clone(),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            material_ubo,
        )?;

        let sampler = self.sampler_for(filtering).clone();
        let set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.set_layouts.material.clone(),
            [
                WriteDescriptorSet::buffer(0, material_buffer),
                WriteDescriptorSet::image_view_sampler(1, tex.view.clone(), sampler),
            ],
            [],
        )?;

        // Dropping the cache's Arc is safe: recorded/submitted Vulkan
        // command buffers retain their own descriptor/resource references.
        self.cached_material_sets.insert(material_key, set.clone());
        static DEBUG_CACHE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *DEBUG_CACHE.get_or_init(|| std::env::var_os("CAT_DEBUG_MATERIAL_CACHE").is_some())
            && self.cached_material_sets.misses % 256 == 0
        {
            eprintln!(
                "[MaterialCache] retained={} capacity=512 allocations={} hits={} evictions={}",
                self.cached_material_sets.len(),
                self.cached_material_sets.misses,
                self.cached_material_sets.hits,
                self.cached_material_sets.evictions
            );
        }
        Ok(Some(set))
    }
    fn retire_runtime_resources(&mut self, visuals: &mut VisualWorld) {
        let live: std::collections::HashSet<_> = visuals.runtime_texture_handles().collect();
        let mut retired: std::collections::HashSet<_> = visuals
            .take_retired_runtime_textures()
            .into_iter()
            .collect();
        retired.extend(self.managed_runtime_textures.difference(&live).copied());
        // A selector may have been re-registered before this draw boundary.
        retired.retain(|handle| !live.contains(handle));
        self.managed_runtime_textures = live;
        for texture in &retired {
            self.textures.remove(texture);
            self.pending_runtime_texture_updates.remove(texture);
        }
        if !retired.is_empty() {
            self.cached_material_sets
                .retain(|(key, _), _| !retired.contains(&key.texture()));
            for (_, entry) in self.material_instances.iter_mut() {
                entry
                    .bindings
                    .retain(|key, _| !retired.contains(&key.texture()));
                entry
                    .custom_snapshots
                    .retain(|snapshot| !retired.contains(&snapshot.texture));
            }
        }
        // Dropping cache ownership is safe: recorded/submitted commands keep
        // the image, descriptor and buffer Arcs until their GPU work retires.
        self.mirror_offscreen.retain(|key, _| {
            visuals
                .mirrors()
                .iter()
                .any(|mirror| key.starts_with(&mirror.capture_prefix))
        });
    }

    fn apply_pending_runtime_texture_updates(&mut self) {
        if self.pending_runtime_texture_updates.is_empty() {
            return;
        }

        for (handle, texture) in self.pending_runtime_texture_updates.drain() {
            self.textures.insert(handle, texture);
            self.cached_material_sets
                .retain(|(key, _), _| key.texture() != handle);
            for (_, entry) in self.material_instances.iter_mut() {
                entry.bindings.retain(|key, _| key.texture() != handle);
            }
        }
    }
}

#[cfg(test)]
mod fixed_band_tests {
    use super::*;
    #[test]
    fn legacy_lighting_uniform_is_fixed_while_grid_spacing_is_preserved() {
        use crate::engine::graphics::{MaterialHandle, visual_world::AnimeShadingParams};
        for program in [
            MaterialHandle::TOON_MESH,
            MaterialHandle::SKINNED_TOON_MESH,
            MaterialHandle::EMISSIVE_TOON_MESH,
            MaterialHandle::ANIME_MESH,
        ] {
            for authored in [1., 9., 64., f32::NAN] {
                assert_eq!(
                    VulkanoState::create_material_ubo(
                        program,
                        authored,
                        AnimeShadingParams::default()
                    )
                    .quant_steps,
                    3.
                );
            }
        }
        assert_eq!(
            VulkanoState::create_material_ubo(
                MaterialHandle::GRID_MESH,
                -0.25,
                AnimeShadingParams::default()
            )
            .quant_steps,
            -0.25
        );
    }
}
