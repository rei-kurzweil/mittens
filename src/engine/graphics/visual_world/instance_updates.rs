//! Visual registration, material binding and instance mutation.
use super::*;

impl VisualWorld {
    pub fn register(
        &mut self,
        cid: ComponentId,
        renderable: GpuRenderable,
        transform: Transform,
        color: [f32; 4],
        opacity: f32,
        multiple_layers: bool,
        transparent_cutout: bool,
        background: bool,
        background_occluded_lit: bool,
        overlay: bool,
        emissive: f32,
        texture: Option<crate::engine::graphics::TextureHandle>,
        quant_steps: f32,
    ) -> InstanceHandle {
        let handle = InstanceHandle(self.next_handle);
        self.next_handle = self.next_handle.wrapping_add(1);

        let idx = self.instances.len();
        self.instances.push(VisualInstance {
            component: cid,
            renderable,
            transform,
            color,
            opacity: if opacity.is_finite() {
                opacity.clamp(0.0, 1.0)
            } else {
                1.0
            },
            multiple_layers,
            transparent_cutout,
            background,
            background_occluded_lit,
            overlay,
            emissive: if emissive.is_finite() {
                emissive.max(0.0)
            } else {
                0.0
            },
            texture,
            texture_filtering: TextureFiltering::default(),
            quant_steps: sanitize_quant_steps(quant_steps),
            material_instance: None,
            toon_outline: None,
            transmission: [1.5, 0.1, 1.0, 0.02],
            transmission_roughness: 0.0,

            bones_base: 0,
            bones_count: 0,
            deformed_base: 0,
            deformed_count: 0,
            deformation_dirty: false,

            stencil_ref: 0,
            is_stencil_clip: false,
        });
        self.handle_to_index.insert(handle, idx);
        self.component_to_handle.insert(cid, handle);
        self.morph_inputs.push(Vec::new());

        visual_topology_dirty!(self, Register);
        visual_data_dirty!(self, Register);
        handle
    }

    pub fn update_quant_steps(&mut self, handle: InstanceHandle, quant_steps: f32) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            let q = sanitize_quant_steps(quant_steps);
            if self.instances[idx].quant_steps.to_bits() == q.to_bits() {
                return true;
            }
            self.instances[idx].quant_steps = q;
            // Quantization affects material UBO selection => batching.
            visual_topology_dirty!(self, UpdateQuantSteps);
            true
        } else {
            false
        }
    }

    pub fn bind_material_instance(
        &mut self,
        handle: InstanceHandle,
        material: Option<MaterialInstanceId>,
    ) -> bool {
        if material.is_some_and(|id| self.materials.instance(id).is_none()) {
            return false;
        }
        let Some(&idx) = self.handle_to_index.get(&handle) else {
            return false;
        };
        if self.instances[idx].material_instance != material {
            if let Some(previous) = self.instances[idx].material_instance {
                if let Some(consumers) = self.material_consumers.get_mut(previous) {
                    consumers.retain(|&consumer| consumer != handle);
                }
            }
            if let Some(id) = material {
                if self.material_consumers.get(id).is_none() {
                    self.material_consumers.insert(id, Vec::new());
                }
                self.material_consumers.get_mut(id).unwrap().push(handle);
            }
            self.instances[idx].material_instance = material;
            visual_topology_dirty!(self, BindMaterialInstance);
        }
        true
    }

    pub fn anime_shading(&self, handle: InstanceHandle) -> Option<AnimeShadingParams> {
        let instance = self.instance(handle)?;
        Some(
            instance
                .material_instance
                .and_then(|id| self.materials.instance(id))
                .and_then(|record| {
                    self.materials
                        .definition(record.definition)
                        .map(|definition| {
                            AnimeShadingParams::from_material(
                                &definition.parameter_schema,
                                &record.values,
                            )
                        })
                })
                .unwrap_or_default(),
        )
    }

    pub fn remove_material_source(&mut self, source: ComponentId) {
        let Some(id) = self.materials.remove_source(source) else {
            return;
        };
        let consumers = self.material_consumers.remove(id).unwrap_or_default();
        for consumer in consumers {
            if let Some(&index) = self.handle_to_index.get(&consumer) {
                let instance = &mut self.instances[index];
                instance.material_instance = None;
                instance.renderable.material = match instance.renderable.material {
                    crate::engine::graphics::MaterialHandle::ANIME_MESH => {
                        crate::engine::graphics::MaterialHandle::TOON_MESH
                    }
                    crate::engine::graphics::MaterialHandle::SKINNED_ANIME_MESH => {
                        crate::engine::graphics::MaterialHandle::SKINNED_TOON_MESH
                    }
                    other if other.0 >= 1024 => crate::engine::graphics::MaterialHandle::TOON_MESH,
                    other => other,
                };
                visual_topology_dirty!(self, RemoveMaterialSource);
            }
        }
    }

    pub(crate) fn material_consumer_components(
        &self,
        id: MaterialInstanceId,
    ) -> impl Iterator<Item = ComponentId> + '_ {
        self.material_consumers
            .get(id)
            .into_iter()
            .flatten()
            .filter_map(|&handle| self.instance(handle).map(|instance| instance.component))
    }

    pub fn update_toon_outline(
        &mut self,
        handle: InstanceHandle,
        outline: Option<ToonOutlineParams>,
    ) -> bool {
        let Some(&idx) = self.handle_to_index.get(&handle) else {
            return false;
        };
        if self.instances[idx].toon_outline == outline {
            return true;
        }
        self.instances[idx].toon_outline = outline;
        visual_topology_dirty!(self, UpdateToonOutline);
        visual_data_dirty!(self, UpdateToonOutline);
        true
    }

    pub fn remove(&mut self, handle: InstanceHandle) -> bool {
        if let Some(idx) = self.handle_to_index.remove(&handle) {
            // Free any skin allocation before removing the instance.
            let old_base = self.instances[idx].bones_base;
            let old_count = self.instances[idx].bones_count;
            if old_count != 0 {
                self.bones_free_range(old_base, old_count);
            }
            self.deformation_ranges.free(DeformationRange {
                base: self.instances[idx].deformed_base,
                vertex_count: self.instances[idx].deformed_count,
            });

            if let Some(id) = self.instances[idx].material_instance {
                if let Some(consumers) = self.material_consumers.get_mut(id) {
                    consumers.retain(|&consumer| consumer != handle);
                }
            }
            self.instances.swap_remove(idx);

            if idx < self.instances.len() {
                // NOTE: This is O(n). Consider storing index->handle too if it becomes hot.
                if let Some((moved_handle, _)) = self
                    .handle_to_index
                    .iter()
                    .find(|(_, i)| **i == self.instances.len())
                {
                    self.handle_to_index.insert(*moved_handle, idx);
                }
            }

            self.component_to_handle.retain(|_, &mut h| h != handle);

            visual_topology_dirty!(self, Remove);
            visual_data_dirty!(self, Remove);
            true
        } else {
            false
        }
    }

    pub fn update_transform(&mut self, handle: InstanceHandle, transform: Transform) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].transform = transform;
            visual_data_dirty!(self, UpdateTransform);
            // transform-only doesn’t affect batching by (material, mesh)
            true
        } else {
            false
        }
    }

    pub fn update_model(&mut self, handle: InstanceHandle, model: TransformMatrix) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].transform.model = model;
            self.instances[idx].transform.matrix_world = model;
            visual_data_dirty!(self, UpdateModel);
            // model-only doesn’t affect batching by (material, mesh)
            true
        } else {
            false
        }
    }

    pub fn update_color(&mut self, handle: InstanceHandle, color: [f32; 4]) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].color = color;
            visual_data_dirty!(self, UpdateColor);
            // Color alpha can change transparent/opaque classification.
            visual_topology_dirty!(self, UpdateColor);
            true
        } else {
            false
        }
    }

    pub fn update_opacity(&mut self, handle: InstanceHandle, opacity: f32) -> bool {
        self.update_opacity_state(handle, opacity, None)
    }

    pub fn update_opacity_state(
        &mut self,
        handle: InstanceHandle,
        opacity: f32,
        multiple_layers: impl Into<Option<bool>>,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            let o = if opacity.is_finite() {
                opacity.clamp(0.0, 1.0)
            } else {
                1.0
            };
            let mut changed = false;

            if (self.instances[idx].opacity - o).abs() >= f32::EPSILON {
                self.instances[idx].opacity = o;
                changed = true;
            }

            if let Some(ml) = multiple_layers.into() {
                if self.instances[idx].multiple_layers != ml {
                    self.instances[idx].multiple_layers = ml;
                    changed = true;
                }
            }

            if !changed {
                return true;
            }
            visual_data_dirty!(self, UpdateOpacityState);
            // Opacity changes can change transparent/opaque classification.
            visual_topology_dirty!(self, UpdateOpacityState);
            true
        } else {
            false
        }
    }

    pub fn update_transparent_cutout(&mut self, handle: InstanceHandle, enabled: bool) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            if self.instances[idx].transparent_cutout == enabled {
                return true;
            }
            self.instances[idx].transparent_cutout = enabled;
            // Cutout changes affect pass classification.
            visual_topology_dirty!(self, UpdateTransparentCutout);
            true
        } else {
            false
        }
    }

    pub fn update_emissive(&mut self, handle: InstanceHandle, emissive: f32) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].emissive = if emissive.is_finite() {
                emissive.max(0.0)
            } else {
                0.0
            };
            visual_data_dirty!(self, UpdateEmissive);
            true
        } else {
            false
        }
    }

    pub fn update_material(
        &mut self,
        handle: InstanceHandle,
        material: crate::engine::graphics::MaterialHandle,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            if self.instances[idx].renderable.material == material {
                return true;
            }
            self.instances[idx].renderable.material = material;
            visual_topology_dirty!(self, UpdateMaterial);
            true
        } else {
            false
        }
    }

    pub fn update_transmission(&mut self, handle: InstanceHandle, options: [f32; 4]) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].transmission = options;
            visual_data_dirty!(self, UpdateTransmission);
            true
        } else {
            false
        }
    }

    pub fn update_transmission_roughness(
        &mut self,
        handle: InstanceHandle,
        roughness: f32,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].transmission_roughness = if roughness.is_finite() {
                roughness.clamp(0.0, 1.0)
            } else {
                0.0
            };
            visual_data_dirty!(self, UpdateTransmissionRoughness);
            true
        } else {
            false
        }
    }

    pub fn update_texture(
        &mut self,
        handle: InstanceHandle,
        texture: Option<crate::engine::graphics::TextureHandle>,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            if self.instances[idx].texture == texture {
                return true;
            }
            self.instances[idx].texture = texture;
            // Texture affects batching (descriptor binding), but not instance vertex data.
            visual_topology_dirty!(self, UpdateTexture);
            true
        } else {
            false
        }
    }

    pub fn update_texture_filtering(
        &mut self,
        handle: InstanceHandle,
        filtering: TextureFiltering,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].texture_filtering = filtering;
            // Filtering affects batching (sampler binding), but not instance vertex data.
            visual_topology_dirty!(self, UpdateTextureFiltering);
            true
        } else {
            false
        }
    }

    pub fn update(
        &mut self,
        handle: InstanceHandle,
        renderable: GpuRenderable,
        transform: Transform,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            let deformation_input_changed = self.instances[idx].renderable.mesh != renderable.mesh;
            // Preserve per-instance color when updating renderable/transform.
            let color = self.instances[idx].color;
            let opacity = self.instances[idx].opacity;
            let multiple_layers = self.instances[idx].multiple_layers;
            let transparent_cutout = self.instances[idx].transparent_cutout;
            let background = self.instances[idx].background;
            let background_occluded_lit = self.instances[idx].background_occluded_lit;
            let overlay = self.instances[idx].overlay;
            let emissive = self.instances[idx].emissive;
            let texture = self.instances[idx].texture;
            let texture_filtering = self.instances[idx].texture_filtering;
            let quant_steps = self.instances[idx].quant_steps;
            let material_instance = self.instances[idx].material_instance;
            let toon_outline = self.instances[idx].toon_outline;
            let transmission = self.instances[idx].transmission;
            let transmission_roughness = self.instances[idx].transmission_roughness;
            let bones_base = self.instances[idx].bones_base;
            let bones_count = self.instances[idx].bones_count;
            let deformed_base = self.instances[idx].deformed_base;
            let deformed_count = self.instances[idx].deformed_count;
            let deformation_dirty =
                self.instances[idx].deformation_dirty || deformation_input_changed;
            let stencil_ref = self.instances[idx].stencil_ref;
            let is_stencil_clip = self.instances[idx].is_stencil_clip;
            self.instances[idx] = VisualInstance {
                component: self.instances[idx].component,
                renderable,
                transform,
                color,
                opacity,
                multiple_layers,
                transparent_cutout,
                background,
                background_occluded_lit,
                overlay,
                emissive,
                texture,
                texture_filtering,
                quant_steps,
                material_instance,
                toon_outline,
                transmission,
                transmission_roughness,
                bones_base,
                bones_count,
                deformed_base,
                deformed_count,
                deformation_dirty,
                stencil_ref,
                is_stencil_clip,
            };
            visual_topology_dirty!(self, Update); // renderable changes likely affect sort/batch
            visual_data_dirty!(self, Update);
            true
        } else {
            false
        }
    }
}
