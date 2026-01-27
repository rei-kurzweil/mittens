use ash::vk;
use ash::vk::Handle as _;

/// Simple OpenXR Vulkan swapchain wrapper for bring-up.
///
/// This creates a stereo swapchain (array size = view count) and exposes the
/// runtime-owned VkImage handles for rendering.
pub struct XRSwapchain {
    swapchain: openxr::Swapchain<openxr::Vulkan>,
    images: Vec<vk::Image>,
    extent: openxr::Extent2Di,
    view_count: u32,
    format: u32,
}

impl XRSwapchain {
    pub fn swapchain(&self) -> &openxr::Swapchain<openxr::Vulkan> {
        &self.swapchain
    }

    pub fn swapchain_mut(&mut self) -> &mut openxr::Swapchain<openxr::Vulkan> {
        &mut self.swapchain
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

    pub fn new(
        instance: &openxr::Instance,
        session: &openxr::Session<openxr::Vulkan>,
        system: openxr::SystemId,
        view_type: openxr::ViewConfigurationType,
        preferred_format: Option<u32>,
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
                sample_count: views[0].recommended_swapchain_sample_count,
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
            swapchain,
            images,
            extent,
            view_count,
            format,
        })
    }
}
