//! Samplers for uploaded material textures, including their mip levels.
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::image::sampler::{
    Filter, LOD_CLAMP_NONE, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode,
};

pub(crate) fn create(
    device: Arc<Device>,
) -> Result<(Arc<Sampler>, Arc<Sampler>, Arc<Sampler>), Box<dyn std::error::Error>> {
    let linear = Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear())?;
    let nearest = Sampler::new(
        device.clone(),
        SamplerCreateInfo {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Nearest,
            mipmap_mode: SamplerMipmapMode::Nearest,
            address_mode: [SamplerAddressMode::Repeat; 3],
            lod: 0.0..=LOD_CLAMP_NONE,
            ..Default::default()
        },
    )?;
    let nearest_mag = Sampler::new(
        device,
        SamplerCreateInfo {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Linear,
            mipmap_mode: SamplerMipmapMode::Linear,
            address_mode: [SamplerAddressMode::Repeat; 3],
            lod: 0.0..=LOD_CLAMP_NONE,
            ..Default::default()
        },
    )?;
    Ok((linear, nearest, nearest_mag))
}
