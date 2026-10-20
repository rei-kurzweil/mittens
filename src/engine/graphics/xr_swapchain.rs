use ash::vk;
use ash::vk::Handle as _;
use std::sync::{Arc, Mutex, MutexGuard};
use vulkano::VulkanObject;
use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::image::{ImageCreateInfo, ImageType, ImageUsage, SampleCount};

use super::xr_renderer::XrSwapchainImage;

/// OpenXR Vulkan swapchain and cached, non-owning render views.
///
/// This creates a stereo swapchain (array size = view count) and exposes the
/// runtime-owned VkImage handles for rendering.
pub struct XRSwapchain {
    // Keep views before their owner in drop order. In-flight renderer clones
    // also retain the owner if a session is stopped before batch retirement.
    render_images: Vec<Option<Arc<XrSwapchainImage>>>,
    swapchain: Arc<Mutex<openxr::Swapchain<openxr::Vulkan>>>,
    device: vk::Device,
    images: Vec<vk::Image>,
    extent: openxr::Extent2Di,
    view_count: u32,
    format: u32,
    acquired_image: Option<u32>,
    image_waited: bool,
    // Destroy runtime objects before releasing our last Vulkan device owner.
    device_owner: Option<Arc<Device>>,
}

/// Read-only borrow for composition-layer construction. Acquisition and release
/// go through XRSwapchain so render-image access can verify their ordering.
pub struct XRSwapchainGuard<'a>(MutexGuard<'a, openxr::Swapchain<openxr::Vulkan>>);

impl std::ops::Deref for XRSwapchainGuard<'_> {
    type Target = openxr::Swapchain<openxr::Vulkan>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl XRSwapchain {
    pub fn swapchain(&self) -> XRSwapchainGuard<'_> {
        XRSwapchainGuard(self.swapchain.lock().unwrap_or_else(|e| e.into_inner()))
    }

    pub fn acquire_image(&mut self) -> openxr::Result<u32> {
        if self.acquired_image.is_some() {
            return Err(openxr::sys::Result::ERROR_CALL_ORDER_INVALID);
        }
        let index = self
            .swapchain
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .acquire_image()?;
        self.acquired_image = Some(index);
        Ok(index)
    }

    pub fn wait_image(&mut self, timeout: openxr::Duration) -> openxr::Result<()> {
        if self.acquired_image.is_none() || self.image_waited {
            return Err(openxr::sys::Result::ERROR_CALL_ORDER_INVALID);
        }
        self.swapchain
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .wait_image(timeout)?;
        self.image_waited = true;
        Ok(())
    }

    pub fn release_image(&mut self) -> openxr::Result<()> {
        if !self.image_waited {
            return Err(openxr::sys::Result::ERROR_CALL_ORDER_INVALID);
        }
        self.swapchain
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .release_image()?;
        self.image_waited = false;
        self.acquired_image = None;
        Ok(())
    }

    pub fn images(&self) -> &[vk::Image] {
        &self.images
    }

    pub fn extent(&self) -> openxr::Extent2Di {
        self.extent
    }

    pub fn view_count(&self) -> u32 {
        self.view_count
    }

    pub fn format(&self) -> u32 {
        self.format
    }

    pub(crate) fn render_image(
        &mut self,
        device: Arc<Device>,
        image_index: u32,
    ) -> Result<Arc<XrSwapchainImage>, Box<dyn std::error::Error>> {
        if self.acquired_image != Some(image_index) || !self.image_waited {
            return Err("XR image must be acquired and waited before rendering".into());
        }
        if device.handle() != self.device {
            return Err("OpenXR image and renderer belong to different Vulkan devices".into());
        }
        if self.device_owner.is_none() {
            self.device_owner = Some(device.clone());
        }
        let index = image_index as usize;
        let handle = *self
            .images
            .get(index)
            .ok_or("XR image index out of range")?;
        let slot = self
            .render_images
            .get_mut(index)
            .ok_or("XR render image index out of range")?;
        if slot.is_none() {
            let format = Format::try_from(vk::Format::from_raw(self.format as i32))
                .map_err(|_| "unsupported OpenXR Vulkan image format")?;
            let create_info = ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format,
                extent: [self.extent.width as u32, self.extent.height as u32, 1],
                array_layers: self.view_count,
                samples: SampleCount::Sample1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
                ..Default::default()
            };
            // SAFETY: these handles were enumerated from this swapchain, created
            // on the checked device with the matching single-sample metadata.
            *slot = Some(Arc::new(unsafe {
                XrSwapchainImage::new(device, handle, create_info, self.swapchain.clone())
            }?));
        }
        Ok(slot.as_ref().unwrap().clone())
    }

    /// # Safety
    /// `device` must be the live Vulkan device used in `session`'s graphics
    /// binding, and must outlive the swapchain and all runtime operations on it.
    pub unsafe fn new(
        instance: &openxr::Instance,
        session: &openxr::Session<openxr::Vulkan>,
        system: openxr::SystemId,
        view_type: openxr::ViewConfigurationType,
        preferred_format: Option<u32>,
        device: vk::Device,
    ) -> Result<Self, String> {
        let views = instance
            .enumerate_view_configuration_views(system, view_type)
            .map_err(|e| format!("enumerate_view_configuration_views: {e:?}"))?;

        if views.is_empty() {
            return Err("enumerate_view_configuration_views returned 0 views".to_string());
        }

        let view_count = views.len() as u32;
        let extent = openxr::Extent2Di {
            width: views[0].recommended_image_rect_width as i32,
            height: views[0].recommended_image_rect_height as i32,
        };

        let formats = session
            .enumerate_swapchain_formats()
            .map_err(|e| format!("enumerate_swapchain_formats: {e:?}"))?;
        if formats.is_empty() {
            return Err("enumerate_swapchain_formats returned 0 formats".to_string());
        }

        // Prefer a renderer-provided format if the runtime supports it.
        // Otherwise, fall back to common 8-bit sRGB formats, then the first supported format.
        let format = preferred_format
            .filter(|f| formats.contains(f))
            .or_else(|| {
                [
                    vk::Format::R8G8B8A8_SRGB.as_raw() as u32,
                    vk::Format::B8G8R8A8_SRGB.as_raw() as u32,
                ]
                .into_iter()
                .find(|f| formats.contains(f))
            })
            .unwrap_or(formats[0]);

        let swapchain = session
            .create_swapchain(&openxr::SwapchainCreateInfo {
                create_flags: openxr::SwapchainCreateFlags::EMPTY,
                usage_flags: openxr::SwapchainUsageFlags::COLOR_ATTACHMENT
                    | openxr::SwapchainUsageFlags::TRANSFER_DST,
                format,
                // MSAA remains in engine-owned transient attachments and resolves
                // directly into the runtime's single-sample image.
                sample_count: 1,
                width: extent.width as u32,
                height: extent.height as u32,
                face_count: 1,
                array_size: view_count,
                mip_count: 1,
            })
            .map_err(|e| format!("create_swapchain: {e:?}"))?;

        let raw_images = swapchain
            .enumerate_images()
            .map_err(|e| format!("enumerate_images: {e:?}"))?;

        let images = raw_images
            .into_iter()
            .map(|h| vk::Image::from_raw(h))
            .collect::<Vec<_>>();

        Ok(Self {
            render_images: vec![None; images.len()],
            swapchain: Arc::new(Mutex::new(swapchain)),
            device,
            images,
            extent,
            view_count,
            format,
            acquired_image: None,
            image_waited: false,
            device_owner: None,
        })
    }
}
