use std::sync::Arc;

use vulkano::format::ClearValue;
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass};
use vulkano::swapchain::{Surface, Swapchain, SwapchainCreateInfo};
use vulkano::Validated;
use vulkano_util::context::VulkanoContext;
use winit::window::Window;

/// Swapchain + swapchain-dependent attachments (views/depth/framebuffers).
///
/// This intentionally owns *all* swapchain-dependent resources so recreation is localized.
pub(crate) struct VulkanoSwapchainState {
    pub surface: Arc<Surface>,
    pub swapchain: Arc<Swapchain>,
    pub swapchain_views: Vec<Arc<ImageView>>,
    pub depth_views: Vec<Arc<ImageView>>,
    pub render_pass: Arc<RenderPass>,
    pub framebuffers: Vec<Arc<Framebuffer>>,
}

impl VulkanoSwapchainState {
    pub(crate) const DEPTH_FORMAT: Format = Format::D32_SFLOAT;

    pub(crate) fn clear_values() -> Vec<Option<ClearValue>> {
        vec![
            Some(ClearValue::from([0.0f32, 0.0, 0.0, 1.0])),
            Some(ClearValue::Depth(1.0)),
        ]
    }

    pub(crate) fn new(context: &VulkanoContext, window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let device = context.device().clone();

        let surface = Surface::from_window(device.instance().clone(), window.clone())?;

        let surface_capabilities = device
            .physical_device()
            .surface_capabilities(&surface, Default::default())?;
        let image_format = device
            .physical_device()
            .surface_formats(&surface, Default::default())?
            .first()
            .ok_or("no supported surface formats")?
            .0;

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

        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    format: swapchain.image_format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                depth: {
                    format: Self::DEPTH_FORMAT,
                    samples: 1,
                    load_op: Clear,
                    store_op: DontCare,
                },
            },
            pass: {
                color: [color],
                depth_stencil: {depth},
            }
        )?;

        let extent = swapchain.image_extent();
        let (depth_views, framebuffers) = Self::create_swapchain_dependent(
            context,
            &swapchain_views,
            extent,
            render_pass.clone(),
        )?;

        Ok(Self {
            surface,
            swapchain,
            swapchain_views,
            depth_views,
            render_pass,
            framebuffers,
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
        let (depth_views, framebuffers) = Self::create_swapchain_dependent(
            context,
            &self.swapchain_views,
            extent,
            self.render_pass.clone(),
        )?;
        self.depth_views = depth_views;
        self.framebuffers = framebuffers;

        Ok(())
    }

    fn create_swapchain_dependent(
        context: &VulkanoContext,
        swapchain_views: &[Arc<ImageView>],
        extent: [u32; 2],
        render_pass: Arc<RenderPass>,
    ) -> Result<(Vec<Arc<ImageView>>, Vec<Arc<Framebuffer>>), Box<dyn std::error::Error>> {
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

        let framebuffers = swapchain_views
            .iter()
            .zip(depth_views.iter())
            .map(|(view, depth_view)| {
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![view.clone(), depth_view.clone()],
                        ..Default::default()
                    },
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

        Ok((depth_views, framebuffers))
    }
}
