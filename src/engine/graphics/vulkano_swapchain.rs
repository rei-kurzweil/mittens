use std::sync::Arc;

use vulkano::Validated;
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage, SampleCount};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::swapchain::{Surface, Swapchain, SwapchainCreateInfo};
use vulkano_util::context::VulkanoContext;
use winit::window::Window;

/// Swapchain + swapchain-dependent attachments (views/depth).
///
/// This intentionally owns *all* swapchain-dependent resources so recreation is localized.
pub(crate) struct VulkanoSwapchainState {
    pub surface: Arc<Surface>,
    pub swapchain: Arc<Swapchain>,
    pub swapchain_views: Vec<Arc<ImageView>>,

    /// Multisample configuration used by pipelines and attachments.
    pub msaa_samples: SampleCount,

    /// Multisampled color attachments, one per swapchain image.
    ///
    /// When `msaa_samples` is `SampleCount::Sample1`, this is empty and the swapchain images are
    /// rendered to directly.
    pub msaa_color_views: Vec<Arc<ImageView>>,

    pub depth_views: Vec<Arc<ImageView>>,
}

impl VulkanoSwapchainState {
    pub(crate) const DEPTH_FORMAT: Format = Format::D32_SFLOAT;

    pub(crate) fn new(
        context: &VulkanoContext,
        window: Arc<Window>,
        msaa_samples: SampleCount,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let device = context.device().clone();

        let surface = Surface::from_window(device.instance().clone(), window.clone())?;

        let surface_capabilities = device
            .physical_device()
            .surface_capabilities(&surface, Default::default())?;

        let surface_formats = device
            .physical_device()
            .surface_formats(&surface, Default::default())?;

        if surface_formats.is_empty() {
            return Err("no supported surface formats".into());
        }

        // Prefer common 8-bit sRGB formats so the window swapchain can match
        // the OpenXR swapchain format on common runtimes (e.g. SteamVR).
        let preferred_formats = [Format::R8G8B8A8_SRGB, Format::B8G8R8A8_SRGB];

        let image_format = preferred_formats
            .into_iter()
            .find_map(|fmt| {
                surface_formats
                    .iter()
                    .find(|(f, _)| *f == fmt)
                    .map(|(f, _)| *f)
            })
            .unwrap_or(surface_formats[0].0);

        let mut min_image_count = 2u32.max(surface_capabilities.min_image_count);
        if let Some(max_image_count) = surface_capabilities.max_image_count {
            min_image_count = min_image_count.min(max_image_count);
        }

        let (swapchain, images) = Swapchain::new(device.clone(), surface.clone(), {
            SwapchainCreateInfo {
                // Keep swapchain buffering as low as possible (prefer 2) while
                // respecting surface min/max limits.
                min_image_count,
                image_format,
                image_extent: window.inner_size().into(),
                image_usage: vulkano::image::ImageUsage::COLOR_ATTACHMENT,
                composite_alpha: surface_capabilities
                    .supported_composite_alpha
                    .into_iter()
                    .next()
                    .ok_or("no supported composite alpha")?,
                ..Default::default()
            }
        })?;

        let swapchain_views = images
            .into_iter()
            .map(|image| ImageView::new_default(image).map_err(|e| e.into()))
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

        let extent = swapchain.image_extent();
        let depth_views = Self::create_depth_views(context, &swapchain_views, extent, msaa_samples)?;
        let msaa_color_views =
            Self::create_msaa_color_views(context, &swapchain_views, extent, image_format, msaa_samples)?;

        Ok(Self {
            surface,
            swapchain,
            swapchain_views,
            msaa_samples,
            msaa_color_views,
            depth_views,
        })
    }

    pub(crate) fn recreate(
        &mut self,
        context: &VulkanoContext,
        window: &Window,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let new_dimensions = window.inner_size();
        if new_dimensions.width == 0 || new_dimensions.height == 0 {
            // Avoid recreating with a zero-sized swapchain while minimized.
            return Ok(());
        }

        let (new_swapchain, new_images) = match self.swapchain.recreate(SwapchainCreateInfo {
            image_extent: new_dimensions.into(),
            ..self.swapchain.create_info()
        }) {
            Ok(r) => r,
            Err(e) => {
                // Caller controls retry behavior; we only translate errors.
                return Err(Box::new(Validated::unwrap(e)));
            }
        };

        self.swapchain = new_swapchain;
        self.swapchain_views = new_images
            .into_iter()
            .map(|image| ImageView::new_default(image).map_err(|e| e.into()))
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

        let extent = self.swapchain.image_extent();
        self.depth_views = Self::create_depth_views(
            context,
            &self.swapchain_views,
            extent,
            self.msaa_samples,
        )?;
        self.msaa_color_views = Self::create_msaa_color_views(
            context,
            &self.swapchain_views,
            extent,
            self.swapchain.image_format(),
            self.msaa_samples,
        )?;

        Ok(())
    }

    fn create_depth_views(
        context: &VulkanoContext,
        swapchain_views: &[Arc<ImageView>],
        extent: [u32; 2],
        samples: SampleCount,
    ) -> Result<Vec<Arc<ImageView>>, Box<dyn std::error::Error>> {
        // Depth buffer: one image per swapchain image.
        let memory_allocator = context.memory_allocator().clone();

        let depth_views = swapchain_views
            .iter()
            .map(|_| {
                let image = Image::new(
                    memory_allocator.clone(),
                    ImageCreateInfo {
                        image_type: ImageType::Dim2d,
                        format: Self::DEPTH_FORMAT,
                        extent: [extent[0], extent[1], 1],
                        samples,
                        usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                        ..Default::default()
                    },
                )?;

                ImageView::new_default(image)
                    .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

        Ok(depth_views)
    }

    fn create_msaa_color_views(
        context: &VulkanoContext,
        swapchain_views: &[Arc<ImageView>],
        extent: [u32; 2],
        format: Format,
        samples: SampleCount,
    ) -> Result<Vec<Arc<ImageView>>, Box<dyn std::error::Error>> {
        if samples == SampleCount::Sample1 {
            return Ok(Vec::new());
        }

        let memory_allocator = context.memory_allocator().clone();

        let views = swapchain_views
            .iter()
            .map(|_| {
                let image = Image::new(
                    memory_allocator.clone(),
                    ImageCreateInfo {
                        image_type: ImageType::Dim2d,
                        format,
                        extent: [extent[0], extent[1], 1],
                        samples,
                        usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                        ..Default::default()
                    },
                )?;

                ImageView::new_default(image)
                    .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

        Ok(views)
    }
}
