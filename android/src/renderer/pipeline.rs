//! The mobile opaque surface state and shared fragment interface.
use crate::{
    graphics_device::Gpu,
    xr_host::{ApiResult, Result},
};
use ash::vk;
use std::{io::Cursor, sync::Arc};

pub(super) struct Pipeline {
    gpu: Arc<Gpu>,
    pub handle: vk::Pipeline,
}
impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            self.gpu.device.destroy_pipeline(self.handle, None);
        }
    }
}
struct Shader {
    gpu: Arc<Gpu>,
    handle: vk::ShaderModule,
}
impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            self.gpu.device.destroy_shader_module(self.handle, None);
        }
    }
}
impl Shader {
    fn new(gpu: Arc<Gpu>, words: &[u32]) -> Result<Self> {
        let handle = unsafe {
            gpu.device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(words), None)
        }
        .api("vkCreateShaderModule(surface)")?;
        Ok(Self { gpu, handle })
    }
}
impl Pipeline {
    pub fn new(
        gpu: Arc<Gpu>,
        pass: vk::RenderPass,
        layout: vk::PipelineLayout,
        fragment: &[u32],
        sample_count: vk::SampleCountFlags,
        background: bool,
        transparent: bool,
    ) -> Result<Self> {
        let words = ash::util::read_spv(&mut Cursor::new(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/surface.vert.spv"
        ))))
        .api("read mobile surface vertex SPIR-V")?;
        let vertex_shader = Shader::new(gpu.clone(), &words)?;
        let fragment_shader = Shader::new(gpu.clone(), fragment)?;
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vertex_shader.handle)
                .name(c"main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fragment_shader.handle)
                .name(c"main"),
        ];
        // CpuVertex is repr(C): position, UV, normal. Instance data is supplied
        // separately; custom fragments keep the engine's MeshSurfaceV1 ABI.
        let bindings = [
            vk::VertexInputBindingDescription {
                binding: 0,
                stride: 32,
                input_rate: vk::VertexInputRate::VERTEX,
            },
            vk::VertexInputBindingDescription {
                binding: 1,
                stride: 80,
                input_rate: vk::VertexInputRate::INSTANCE,
            },
        ];
        let mut attributes = vec![
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                location: 5,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: 12,
            },
            vk::VertexInputAttributeDescription {
                location: 8,
                binding: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 20,
            },
        ];
        for column in 0..4 {
            attributes.push(vk::VertexInputAttributeDescription {
                location: column + 1,
                binding: 1,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: column * 16,
            });
        }
        attributes.push(vk::VertexInputAttributeDescription {
            location: 6,
            binding: 1,
            format: vk::Format::R32G32B32A32_SFLOAT,
            offset: 64,
        });
        let vertex = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&bindings)
            .vertex_attribute_descriptions(&attributes);
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(if background {
                vk::CullModeFlags::NONE
            } else {
                vk::CullModeFlags::BACK
            })
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let samples =
            vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(sample_count);
        let depth = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(!background && !transparent)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
        let colors = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(transparent)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)];
        let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&colors);
        let states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&states);
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex)
            .input_assembly_state(&assembly)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&samples)
            .depth_stencil_state(&depth)
            .color_blend_state(&blend)
            .dynamic_state(&dynamic)
            .layout(layout)
            .render_pass(pass)
            .subpass(0);
        let handle = match unsafe {
            gpu.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
        } {
            Ok(pipelines) => pipelines[0],
            Err((partial, error)) => {
                for handle in partial {
                    unsafe {
                        gpu.device.destroy_pipeline(handle, None);
                    }
                }
                return Err(format!(
                    "vkCreateGraphicsPipelines(custom surface): {error:?}"
                ));
            }
        };
        Ok(Self { gpu, handle })
    }
}
