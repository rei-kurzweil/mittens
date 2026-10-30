//! Select a renderer once; both use the same host frame loop and XR targets.
use crate::{
    graphics_device::Gpu,
    renderer::{SceneRenderer, view::EyeCamera},
    xr_host::{ApiResult, Report, Result},
};
use ash::{vk, vk::Handle};
use mittens_engine::engine::graphics::{
    self, RenderUploader, VisualWorld, VulkanoRenderer, XRSwapchain,
};
use std::sync::Arc;

pub enum Backend {
    Lean {
        renderer: Option<SceneRenderer>,
        gpu: Arc<Gpu>,
    },
    Desktop {
        completion: Completion,
        renderer: VulkanoRenderer,
    },
}
pub struct Completion {
    device: ash::Device,
    queue: vk::Queue,
    fence: vk::Fence,
    _entry: ash::Entry,
}
impl Drop for Completion {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_fence(self.fence, None);
            // The Vulkano renderer owns the device and instance, not these wrappers.
        }
    }
}
impl Backend {
    pub fn new(
        name: &str,
        instance: &openxr::Instance,
        system: openxr::SystemId,
        enable2: bool,
    ) -> Result<Self> {
        match name {
            "lean" => {
                let requirements = instance
                    .graphics_requirements::<openxr::Vulkan>(system)
                    .api("XR graphics requirements")?;
                Ok(Self::Lean {
                    renderer: None,
                    gpu: Gpu::new(instance, system, enable2, requirements, &mut Report)?,
                })
            }
            "desktop" => {
                instance
                    .graphics_requirements::<openxr::Vulkan>(system)
                    .api("XR graphics requirements")?;
                let mut renderer = VulkanoRenderer::new();
                renderer
                    .set_msaa_mode(graphics::MsaaMode::Off)
                    .api("disable MSAA for matched comparison")?;
                renderer
                    .init_xr_graphics(instance, system)
                    .api("desktop XR graphics")?;
                let handles = renderer
                    .xr_vulkan_graphics()
                    .ok_or("missing desktop graphics")?;
                let completion = unsafe {
                    let entry = ash::Entry::load().api("load Vulkan")?;
                    let raw_instance = ash::Instance::load(
                        entry.static_fn(),
                        vk::Instance::from_raw(handles.vk_instance as u64),
                    );
                    let device = ash::Device::load(
                        raw_instance.fp_v1_0(),
                        vk::Device::from_raw(handles.vk_device as u64),
                    );
                    let fence = device
                        .create_fence(&vk::FenceCreateInfo::default(), None)
                        .api("desktop completion fence")?;
                    Completion {
                        device,
                        queue: vk::Queue::from_raw(handles.vk_queue as u64),
                        fence,
                        _entry: entry,
                    }
                };
                Ok(Self::Desktop {
                    completion,
                    renderer,
                })
            }
            _ => Err("renderer must be lean or desktop".into()),
        }
    }
    pub fn graphics(&self) -> graphics::XrVulkanGraphics {
        match self {
            Self::Desktop { renderer, .. } => renderer.xr_vulkan_graphics().unwrap(),
            Self::Lean { gpu, .. } => graphics::XrVulkanGraphics {
                vk_instance: gpu.instance.raw.handle().as_raw() as _,
                vk_physical_device: gpu.physical.as_raw() as _,
                vk_device: gpu.device.handle().as_raw() as _,
                vk_queue: gpu.queue.as_raw() as _,
                queue_family_index: gpu.family,
                queue_index: 0,
            },
        }
    }
    pub fn initialize_scene(&mut self, target: &XRSwapchain) -> Result<()> {
        match self {
            Self::Lean { renderer, gpu } => {
                let extent = target.extent();
                let scene = SceneRenderer::new(
                    gpu.clone(),
                    vk::Format::from_raw(target.format() as i32),
                    vk::Extent2D {
                        width: extent.width as u32,
                        height: extent.height as u32,
                    },
                    target.images().iter().map(|image| image.as_raw()).collect(),
                )?;
                println!("{}", scene.attachment_policy());
                *renderer = Some(scene);
                Ok(())
            }
            Self::Desktop { renderer, .. } => {
                renderer
                    .init_xr_scene(target.format())
                    .api("desktop XR scene")?;
                println!(
                    "desktop GPU={:?} MSAA={:?}",
                    renderer.gpu_device_name(),
                    renderer.msaa_description()
                );
                Ok(())
            }
        }
    }
    pub fn uploader(&mut self) -> &mut dyn RenderUploader {
        match self {
            Self::Lean { renderer, .. } => renderer.as_mut().unwrap(),
            Self::Desktop { renderer, .. } => renderer,
        }
    }
    pub fn draw(
        &mut self,
        target: &mut XRSwapchain,
        index: u32,
        visuals: &mut VisualWorld,
        cameras: &[EyeCamera; 2],
    ) -> Result<()> {
        match self {
            Self::Lean { renderer, .. } => {
                renderer
                    .as_mut()
                    .unwrap()
                    .draw(index as usize, visuals, cameras)?;
                Ok(())
            }
            Self::Desktop {
                completion,
                renderer,
            } => {
                let result = (|| {
                    renderer
                        .begin_xr_frame(target, index)
                        .api("begin desktop eye batch")?;
                    let extent = target.extent();
                    for eye in 0..2 {
                        renderer
                            .submit_xr_eye(
                                visuals,
                                eye,
                                [extent.width as u32, extent.height as u32],
                            )
                            .api("submit desktop eye")?;
                    }
                    renderer
                        .with_xr_graphics_queue_lock(|| unsafe {
                            graphics::xr_renderer::submit_xr_completion_and_wait(
                                &completion.device,
                                completion.queue,
                                completion.fence,
                            )
                        })
                        .ok_or("missing graphics queue")?
                        .api("complete desktop eyes")?;
                    unsafe { renderer.finish_xr_batch_after_external_wait() }
                        .api("retire desktop eye batch")
                })();
                if result.is_err() {
                    let _ = renderer.wait_for_xr_batch_on_error();
                }
                result
            }
        }
    }
}
