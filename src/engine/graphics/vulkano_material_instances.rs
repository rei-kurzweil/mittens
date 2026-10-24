// Included inside vulkano_backend. Mutable values never enter a descriptor key.
const MATERIAL_SNAPSHOT_LIMIT: usize = 8;
type MaterialResourceKey = (
    crate::engine::graphics::MaterialHandle,
    TextureHandle,
    TextureFiltering,
    u32,
);

struct MaterialSnapshot {
    revision: u64,
    buffer: Subbuffer<MaterialUBO>,
    set: Arc<DescriptorSet>,
}

struct GpuMaterialInstance {
    custom: bool,
    record: crate::engine::graphics::materials::MaterialInstance,
    bindings: crate::engine::graphics::material_cache::MaterialCache<
        MaterialResourceKey,
        Vec<MaterialSnapshot>,
    >,
    custom_snapshots: Vec<CustomMaterialSnapshot>,
}

#[derive(Debug, Default, Clone, Copy)]
struct MaterialUploadStats {
    allocations: u64,
    uploads: u64,
    snapshot_reuses: u64,
    last_logged_uploads: u64,
}

impl VulkanoState {
    fn log_material_upload_stats(&mut self) {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *ENABLED.get_or_init(|| std::env::var_os("CAT_DEBUG_MATERIAL_INSTANCES").is_some())
            && self.material_upload_stats.uploads
                >= self.material_upload_stats.last_logged_uploads + 256
        {
            self.material_upload_stats.last_logged_uploads = self.material_upload_stats.uploads;
            eprintln!(
                "[MaterialInstances] live={} ubo_descriptor_allocations={} uploads={} snapshot_reuses={}",
                self.material_instances.len(),
                self.material_upload_stats.allocations,
                self.material_upload_stats.uploads,
                self.material_upload_stats.snapshot_reuses
            );
        }
    }
    fn sync_material_instances(&mut self, visuals: &VisualWorld) {
        self.prepare_custom_programs(visuals);
        self.material_instances
            .retain(|id, _| visuals.materials.instance(id).is_some());
        for (id, record) in visuals.materials.instances() {
            let custom = visuals
                .materials
                .definition(record.definition)
                .is_some_and(|definition| definition.custom.is_some());
            if let Some(entry) = self.material_instances.get_mut(id) {
                if entry.record.definition != record.definition {
                    entry.bindings =
                        crate::engine::graphics::material_cache::MaterialCache::new(16);
                    entry.custom_snapshots.clear();
                }
                entry.record = record.clone();
                entry.custom = custom;
            } else {
                self.material_instances.insert(
                    id,
                    GpuMaterialInstance {
                        custom,
                        record: record.clone(),
                        custom_snapshots: Vec::new(),
                        bindings: crate::engine::graphics::material_cache::MaterialCache::new(16),
                    },
                );
            }
        }
    }

    fn mutable_material_set(
        &mut self,
        id: crate::engine::graphics::materials::MaterialInstanceId,
        material: crate::engine::graphics::MaterialHandle,
        texture: TextureHandle,
        filtering: TextureFiltering,
        quant_steps: f32,
    ) -> Result<Option<Arc<DescriptorSet>>, Box<dyn std::error::Error>> {
        let Some(view) = self.textures.get(&texture).map(|tex| tex.view.clone()) else {
            return Ok(None);
        };
        let sampler = self.sampler_for(filtering).clone();
        let entry = self
            .material_instances
            .get_mut(id)
            .ok_or("stale or unprepared MaterialInstanceId")?;
        let key = (material, texture, filtering, quant_steps.to_bits());
        if entry.bindings.get(&key).is_none() {
            entry.bindings.insert(key, Vec::new());
        }
        let slots = entry.bindings.get_mut(&key).unwrap();
        let revision = entry.record.revision;
        if let Some(slot) = slots.iter().find(|slot| slot.revision == revision) {
            self.material_upload_stats.snapshot_reuses += 1;
            return Ok(Some(slot.set.clone()));
        }
        let ubo = Self::create_material_ubo(material, quant_steps, entry.record.parameters);
        // A command buffer retains the descriptor Arc, including after recording
        // but before submission. Reuse only an exclusively cache-owned snapshot.
        // Buffer::write additionally checks Vulkano's CPU/GPU resource-use locks.
        if let Some(slot) = slots
            .iter_mut()
            .find(|slot| Arc::strong_count(&slot.set) == 1)
        {
            *slot.buffer.write()? = ubo;
            slot.revision = revision;
            self.material_upload_stats.uploads += 1;
            return Ok(Some(slot.set.clone()));
        }
        if slots.len() == MATERIAL_SNAPSHOT_LIMIT {
            return Err("all mutable material snapshots are retained by command buffers; complete/retire frames before recording another revision".into());
        }
        let buffer = Buffer::from_data(
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
            ubo,
        )?;
        let set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.set_layouts.material.clone(),
            [
                WriteDescriptorSet::buffer(0, buffer.clone()),
                WriteDescriptorSet::image_view_sampler(1, view, sampler),
            ],
            [],
        )?;
        slots.push(MaterialSnapshot {
            revision,
            buffer,
            set: set.clone(),
        });
        self.material_upload_stats.allocations += 1;
        self.material_upload_stats.uploads += 1;
        Ok(Some(set))
    }
}

#[cfg(test)]
mod material_instance_gpu_tests {
    use super::*;
    use crate::engine::ecs::{World, component::ShadingComponent};
    use crate::engine::graphics::{materials::MaterialRegistry, visual_world::AnimeShadingParams};

    #[test]
    #[ignore = "requires Vulkan; verifies real mutable UBO/descriptor allocation and retirement"]
    fn material_instance_gpu_snapshots_are_bounded_and_preserve_retained_revisions()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = RendererContext::new(None, None, None)?;
        let mut renderer = VulkanoState::new(
            context,
            RendererOutput::Xr(Format::B8G8R8A8_UNORM),
            MsaaMode::Off,
        )?;
        let mut world = World::default();
        let source = world.add_component(ShadingComponent::new());
        let mut visuals = VisualWorld::new();
        let mut materials = MaterialRegistry::default();
        let definition = materials.anime_definition;
        let mut params = AnimeShadingParams::default();
        let id = materials.upsert_source(source, definition, params);
        visuals.materials = materials;
        let program = crate::engine::graphics::MaterialHandle::ANIME_MESH;
        let key = (
            program,
            TextureHandle(0),
            TextureFiltering::Linear,
            3.0f32.to_bits(),
        );
        let pipeline = renderer.pipeline_anime_mesh.clone();
        let mut retained = Vec::new();
        for revision in 0..MATERIAL_SNAPSHOT_LIMIT {
            params.controls[3] = revision as f32 + 1.0;
            visuals.materials.upsert_source(source, definition, params);
            renderer.sync_material_instances(&visuals);
            retained.push(
                renderer
                    .get_or_create_material_set(
                        program,
                        TextureHandle(0),
                        TextureFiltering::Linear,
                        3.0,
                        Some(id),
                    )?
                    .unwrap(),
            );
        }
        params.controls[3] = 99.0;
        visuals.materials.upsert_source(source, definition, params);
        renderer.sync_material_instances(&visuals);
        assert!(
            renderer
                .get_or_create_material_set(
                    program,
                    TextureHandle(0),
                    TextureFiltering::Linear,
                    3.0,
                    Some(id)
                )
                .is_err(),
            "bounded pool applies backpressure instead of overwriting retained commands"
        );
        let slots = renderer.material_instances[id].bindings.get(&key).unwrap();
        for (index, slot) in slots.iter().enumerate() {
            assert_eq!(slot.buffer.read()?.anime_controls[3], index as f32 + 1.0);
        }
        retained.clear();
        let allocations = renderer.material_upload_stats.allocations;
        for edit in 0..10_000 {
            params.controls[3] = edit as f32 + 0.1;
            visuals.materials.upsert_source(source, definition, params);
            renderer.sync_material_instances(&visuals);
            drop(
                renderer
                    .get_or_create_material_set(
                        program,
                        TextureHandle(0),
                        TextureFiltering::Linear,
                        3.0,
                        Some(id),
                    )?
                    .unwrap(),
            );
        }
        assert_eq!(renderer.material_upload_stats.allocations, allocations);
        assert_eq!(
            renderer.material_upload_stats.allocations,
            MATERIAL_SNAPSHOT_LIMIT as u64
        );
        assert_eq!(
            renderer.cached_material_sets.len(),
            0,
            "animated values never enter the legacy immutable cache"
        );
        assert!(Arc::ptr_eq(&pipeline, &renderer.pipeline_anime_mesh));
        let still_retained = renderer
            .get_or_create_material_set(
                program,
                TextureHandle(0),
                TextureFiltering::Linear,
                3.0,
                Some(id),
            )?
            .unwrap();
        visuals.materials.remove_source(source);
        renderer.sync_material_instances(&visuals);
        assert!(renderer.material_instances.get(id).is_none());
        assert_eq!(
            Arc::strong_count(&still_retained),
            1,
            "retired descriptor remains owned by its last consumer"
        );
        assert!(
            renderer
                .get_or_create_material_set(
                    program,
                    TextureHandle(0),
                    TextureFiltering::Linear,
                    3.0,
                    Some(id)
                )
                .is_err()
        );
        Ok(())
    }
}
