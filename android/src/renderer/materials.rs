//! Retained custom definitions/parameters. No legacy material handle switches.
use super::{
    pipeline::Pipeline,
    resources::{Buffer, Image},
};
use crate::{
    graphics_device::Gpu,
    xr_host::{ApiResult, Result},
};
use ash::vk;
use mittens_engine::engine::graphics::{
    TextureHandle,
    materials::{
        MaterialDefinitionId, MaterialEmission, MaterialInstanceId, MaterialRegistry,
        MaterialTransmission,
    },
    visual_world::TextureFiltering,
};
use std::{collections::HashMap, sync::Arc};

pub(super) type BindingKey = (MaterialInstanceId, Option<TextureHandle>, TextureFiltering);
struct Binding {
    set: vk::DescriptorSet,
    buffer: Buffer,
    definition: MaterialDefinitionId,
    revision: u64,
}
pub(super) struct Materials {
    gpu: Arc<Gpu>,
    pub layout: vk::PipelineLayout,
    layouts: [vk::DescriptorSetLayout; 2],
    pool: vk::DescriptorPool,
    samplers: Vec<vk::Sampler>,
    cameras: Vec<Buffer>,
    pub camera_sets: [vk::DescriptorSet; 2],
    pipelines: HashMap<MaterialDefinitionId, Pipeline>,
    bindings: HashMap<BindingKey, Binding>,
}
impl Drop for Materials {
    fn drop(&mut self) {
        unsafe {
            self.pipelines.clear();
            self.gpu.device.destroy_descriptor_pool(self.pool, None);
            self.gpu.device.destroy_pipeline_layout(self.layout, None);
            for layout in self.layouts {
                self.gpu.device.destroy_descriptor_set_layout(layout, None);
            }
            for sampler in self.samplers.drain(..) {
                self.gpu.device.destroy_sampler(sampler, None);
            }
        }
    }
}
impl Materials {
    // Explicit first-slice capacity, including texture/filter variants. Released
    // engine material instances return their descriptor sets to this pool.
    const LIMIT: usize = 256;
    pub fn new(gpu: Arc<Gpu>) -> Result<Self> {
        let mut materials = Self {
            gpu,
            layout: vk::PipelineLayout::null(),
            layouts: [vk::DescriptorSetLayout::null(); 2],
            pool: vk::DescriptorPool::null(),
            samplers: Vec::new(),
            cameras: Vec::new(),
            camera_sets: [vk::DescriptorSet::null(); 2],
            pipelines: HashMap::new(),
            bindings: HashMap::new(),
        };
        unsafe {
            let device = &materials.gpu.device;
            let uniform = vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT);
            materials.layouts[0] = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&[uniform]),
                    None,
                )
                .api("camera descriptor layout")?;
            let texture = vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT);
            materials.layouts[1] = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&[uniform, texture]),
                    None,
                )
                .api("material descriptor layout")?;
            materials.layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default().set_layouts(&materials.layouts),
                    None,
                )
                .api("mobile surface pipeline layout")?;
            let sizes = [
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: Self::LIMIT as u32 + 2,
                },
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    descriptor_count: Self::LIMIT as u32,
                },
            ];
            materials.pool = device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
                        .max_sets(Self::LIMIT as u32 + 2)
                        .pool_sizes(&sizes),
                    None,
                )
                .api("mobile descriptor pool")?;
            for (min, mag) in [
                (vk::Filter::LINEAR, vk::Filter::LINEAR),
                (vk::Filter::NEAREST, vk::Filter::NEAREST),
                (vk::Filter::LINEAR, vk::Filter::NEAREST),
            ] {
                materials.samplers.push(
                    device
                        .create_sampler(
                            &vk::SamplerCreateInfo::default()
                                .min_filter(min)
                                .mag_filter(mag)
                                .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                                .max_lod(0.0),
                            None,
                        )
                        .api("surface sampler")?,
                );
            }
            for eye in 0..2 {
                let buffer = Buffer::new(
                    materials.gpu.clone(),
                    208,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                )?;
                let set = materials.allocate(materials.layouts[0])?;
                materials.write_uniform(set, &buffer);
                materials.cameras.push(buffer);
                materials.camera_sets[eye] = set;
            }
        }
        Ok(materials)
    }
    fn allocate(&self, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet> {
        unsafe {
            self.gpu.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(self.pool)
                    .set_layouts(&[layout]),
            )
        }
        .api("allocate mobile descriptors")
        .map(|sets| sets[0])
    }
    fn write_uniform(&self, set: vk::DescriptorSet, buffer: &Buffer) {
        let info = [vk::DescriptorBufferInfo::default()
            .buffer(buffer.handle)
            .offset(0)
            .range(buffer.capacity as u64)];
        unsafe {
            self.gpu.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&info)],
                &[],
            );
        }
    }
    pub fn update_camera(&mut self, eye: usize, data: &[u8]) -> Result<()> {
        self.cameras[eye].write(data)
    }
    pub fn collect(&mut self, registry: &MaterialRegistry) -> Result<()> {
        let device = &self.gpu.device;
        let pool = self.pool;
        // This runs only after completion of the prior frame.
        let mut error = None;
        self.bindings.retain(|key, binding| {
            if registry.instance(key.0).is_some() {
                return true;
            }
            if let Err(e) = unsafe { device.free_descriptor_sets(pool, &[binding.set]) } {
                error = Some(e);
            }
            false
        });
        if let Some(error) = error {
            return Err(format!("free retired material descriptors: {error:?}"));
        }
        self.pipelines
            .retain(|id, _| registry.definition(*id).is_some());
        Ok(())
    }
    pub fn prepare(
        &mut self,
        registry: &MaterialRegistry,
        key: BindingKey,
        pass: vk::RenderPass,
        texture: &Image,
    ) -> Result<bool> {
        let record = registry
            .instance(key.0)
            .ok_or("missing custom material instance")?;
        let definition = registry
            .definition(record.definition)
            .ok_or("missing material definition")?;
        let custom = definition
            .custom
            .as_ref()
            .ok_or("mobile renderer accepts custom materials only; legacy material requested")?;
        if definition.base != registry.surface_base
            || definition.capabilities.emission != MaterialEmission::None
            || definition.capabilities.transmission != MaterialTransmission::None
        {
            return Err("mobile renderer supports only opaque/albedo MaterialBase()".into());
        }
        let Some(compiled) = custom.compiled.get() else {
            return Ok(false);
        };
        let compiled = compiled
            .as_ref()
            .map_err(|error| format!("custom fragment: {error}"))?;
        if compiled
            .descriptor_bindings
            .iter()
            .any(|binding| !matches!(binding, (0, 0) | (1, 0) | (1, 1)))
        {
            return Err("opaque/albedo material may use camera, numeric inputs and base texture; lights are not supported yet".into());
        }
        if !self.pipelines.contains_key(&record.definition) {
            if self.pipelines.len() >= Self::LIMIT {
                return Err("mobile custom pipeline capacity (256) exceeded".into());
            }
            self.pipelines.insert(
                record.definition,
                Pipeline::new(self.gpu.clone(), pass, self.layout, &compiled.words)?,
            );
        }
        if self
            .bindings
            .get(&key)
            .is_some_and(|binding| binding.definition != record.definition)
        {
            let previous = self.bindings.remove(&key).unwrap();
            unsafe {
                self.gpu
                    .device
                    .free_descriptor_sets(self.pool, &[previous.set])
            }
            .api("release material descriptors after definition change")?;
        }
        if let Some(binding) = self.bindings.get_mut(&key) {
            if binding.revision != record.revision {
                binding.buffer.write(&compiled.pack(&record.values)?)?;
                binding.revision = record.revision;
            }
            return Ok(true);
        }
        if self.bindings.len() >= Self::LIMIT {
            return Err("mobile material binding capacity (256) exceeded".into());
        }
        let packed = compiled.pack(&record.values)?;
        let mut buffer = Buffer::new(
            self.gpu.clone(),
            packed.len(),
            vk::BufferUsageFlags::UNIFORM_BUFFER,
        )?;
        buffer.write(&packed)?;
        let set = self.allocate(self.layouts[1])?;
        self.write_uniform(set, &buffer);
        let sampler = self.samplers[match key.2 {
            TextureFiltering::Linear => 0,
            TextureFiltering::Nearest => 1,
            TextureFiltering::NearestMagnification => 2,
        }];
        let image = [vk::DescriptorImageInfo::default()
            .sampler(sampler)
            .image_view(texture.view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        unsafe {
            self.gpu.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&image)],
                &[],
            );
        }
        self.bindings.insert(
            key,
            Binding {
                set,
                buffer,
                definition: record.definition,
                revision: record.revision,
            },
        );
        Ok(true)
    }
    pub fn binding(
        &self,
        registry: &MaterialRegistry,
        key: BindingKey,
    ) -> Option<(vk::Pipeline, vk::DescriptorSet)> {
        let binding = self.bindings.get(&key)?;
        if binding.definition != registry.instance(key.0)?.definition {
            return None;
        }
        Some((self.pipelines[&binding.definition].handle, binding.set))
    }
    pub fn counts(&self) -> (usize, usize) {
        (self.pipelines.len(), self.bindings.len())
    }
}
