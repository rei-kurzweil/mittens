use std::collections::BTreeMap;
use std::sync::Arc;

use vulkano::descriptor_set::layout::{
    DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType,
};
use vulkano::device::Device;
use vulkano::shader::ShaderStages;

pub struct PipelineDescriptorSetLayouts {
    /// Set 0: global per-frame data shared by all pipelines.
    ///
    /// Bindings:
    /// - `binding=0`: camera UBO (view/proj + misc per-frame values)
    /// - `binding=1`: lights SSBO
    ///
    /// Note: the renderer may build multiple descriptor sets with this same layout
    /// (e.g. a foreground variant and a background variant). They share the same
    /// bindings, but may differ in the *values* written into the camera UBO.
    pub global: Arc<DescriptorSetLayout>,

    /// Set 1: material data (textures/params).
    pub material: Arc<DescriptorSetLayout>,

    /// Set 2: per-instance/object data (bones, per-instance lighting, etc).
    pub rig: Arc<DescriptorSetLayout>,
}

impl PipelineDescriptorSetLayouts {
    /// Creates a shared descriptor set layout used for global data.
    ///
    /// This layout is:
    /// - `set=0,binding=0` uniform buffer (camera UBO)
    /// - `set=0,binding=1` storage buffer (lights SSBO)
    pub fn new(device: Arc<Device>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut bindings = BTreeMap::new();

        let mut camera_binding =
            DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer);
        camera_binding.descriptor_count = 1;
        // Use a superset stage mask so we can read camera in VS/FS without changing layouts.
        camera_binding.stages = ShaderStages::VERTEX | ShaderStages::FRAGMENT;

        bindings.insert(0, camera_binding);

        // Global lighting buffer: `set=0,binding=1` storage buffer.
        // Intended for dozens of lights (too many for a typical UBO).
        let mut lights_binding =
            DescriptorSetLayoutBinding::descriptor_type(DescriptorType::StorageBuffer);
        lights_binding.descriptor_count = 1;
        lights_binding.stages = ShaderStages::FRAGMENT;
        bindings.insert(1, lights_binding);

        let global = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings,
                ..Default::default()
            },
        )?;

        // Set 1 (material):
        // - binding 0: uniform buffer (MaterialUBO)
        // - binding 1: combined image sampler (base color texture)
        let mut material_bindings = BTreeMap::new();
        let mut material_params =
            DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer);
        material_params.descriptor_count = 1;
        material_params.stages = ShaderStages::FRAGMENT;
        material_bindings.insert(0, material_params);

        let mut base_color_tex =
            DescriptorSetLayoutBinding::descriptor_type(DescriptorType::CombinedImageSampler);
        base_color_tex.descriptor_count = 1;
        base_color_tex.stages = ShaderStages::FRAGMENT;
        material_bindings.insert(1, base_color_tex);

        let material = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings: material_bindings,
                ..Default::default()
            },
        )?;

        // Set 2 (object/rig):
        // - binding 0: per-instance lighting/shade data (SSBO), indexed by `gl_InstanceIndex`.
        // - binding 1: placeholder storage buffer for bones.
        let mut rig_bindings = BTreeMap::new();

        let mut per_instance_lighting =
            DescriptorSetLayoutBinding::descriptor_type(DescriptorType::StorageBuffer);
        per_instance_lighting.descriptor_count = 1;
        per_instance_lighting.stages = ShaderStages::FRAGMENT;
        rig_bindings.insert(0, per_instance_lighting);

        let mut bones = DescriptorSetLayoutBinding::descriptor_type(DescriptorType::StorageBuffer);
        bones.descriptor_count = 1;
        bones.stages = ShaderStages::VERTEX;
        rig_bindings.insert(1, bones);

        let rig = DescriptorSetLayout::new(
            device,
            DescriptorSetLayoutCreateInfo {
                bindings: rig_bindings,
                ..Default::default()
            },
        )?;

        Ok(Self {
            global,
            material,
            rig,
        })
    }
}
