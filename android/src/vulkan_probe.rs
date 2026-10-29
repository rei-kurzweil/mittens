//! Small standalone OpenXR renderer; no engine compute/material requirements.
mod gpu;
mod renderer;

use crate::xr_probe::{ApiResult, Report, Result};
use ash::{vk, vk::Handle};
use gpu::Gpu;
use openxr as xr;
use renderer::Renderer;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const VIEW: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

pub(super) struct Probe {
    // Declaration order is destruction order: views/framebuffers before the XR
    // images, all XR session references before the shared Vulkan device.
    renderer: Renderer,
    swapchain: xr::Swapchain<xr::Vulkan>,
    space: xr::Space,
    frames: xr::FrameStream<xr::Vulkan>,
    waiter: xr::FrameWaiter,
    session: xr::Session<xr::Vulkan>,
    _gpu: Arc<Gpu>,
    extent: xr::Extent2Di,
    blend: xr::EnvironmentBlendMode,
    running: bool,
    rendered: u64,
}

impl Probe {
    pub(super) fn new(
        instance: &xr::Instance,
        system: xr::SystemId,
        enable2: bool,
        requirements: xr::vulkan::Requirements,
        report: &mut Report,
    ) -> Result<Self> {
        let gpu = Gpu::new(instance, system, enable2, requirements, report)?;
        // Exactly these handles were created for the runtime-selected GPU.
        let (session, waiter, frames) = unsafe {
            instance.create_session::<xr::Vulkan>(
                system,
                &xr::vulkan::SessionCreateInfo {
                    instance: gpu.instance.raw.handle().as_raw() as _,
                    physical_device: gpu.physical.as_raw() as _,
                    device: gpu.device.handle().as_raw() as _,
                    queue_family_index: gpu.family,
                    queue_index: 0,
                },
            )
        }
        .api("xrCreateSession(Vulkan)")?;
        report.line("xrCreateSession(Vulkan) SUCCESS");
        report.line(format!(
            "reference_spaces={:?}",
            session
                .enumerate_reference_spaces()
                .api("xrEnumerateReferenceSpaces")?
        ));
        let space = session
            .create_reference_space(xr::ReferenceSpaceType::LOCAL, xr::Posef::IDENTITY)
            .api("xrCreateReferenceSpace(LOCAL)")?;
        let formats = session
            .enumerate_swapchain_formats()
            .api("xrEnumerateSwapchainFormats")?;
        report.line(format!("XR swapchain formats={formats:?}"));
        let format = [
            vk::Format::R8G8B8A8_SRGB,
            vk::Format::B8G8R8A8_SRGB,
            vk::Format::R8G8B8A8_UNORM,
            vk::Format::B8G8R8A8_UNORM,
        ]
        .into_iter()
        .find(|format| formats.contains(&(format.as_raw() as _)))
        .ok_or("no supported RGBA/BGRA color swapchain format")?;
        let views = instance
            .enumerate_view_configuration_views(system, VIEW)
            .api("xrEnumerateViewConfigurationViews(stereo)")?;
        if views.len() != 2 {
            return Err(format!("expected 2 stereo views, found {}", views.len()));
        }
        // Deliberately keep the graphics probe cheap; this is not the engine's
        // eventual render resolution or mobile quality policy.
        let width = views
            .iter()
            .map(|v| v.recommended_image_rect_width)
            .max()
            .unwrap()
            .min(1024)
            .min(views.iter().map(|v| v.max_image_rect_width).min().unwrap());
        let height = views
            .iter()
            .map(|v| v.recommended_image_rect_height)
            .max()
            .unwrap()
            .min(1024)
            .min(views.iter().map(|v| v.max_image_rect_height).min().unwrap());
        let swapchain = session
            .create_swapchain(&xr::SwapchainCreateInfo {
                create_flags: xr::SwapchainCreateFlags::EMPTY,
                usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                    | xr::SwapchainUsageFlags::SAMPLED,
                format: format.as_raw() as _,
                sample_count: 1,
                width,
                height,
                face_count: 1,
                array_size: 2,
                mip_count: 1,
            })
            .api("xrCreateSwapchain(stereo array)")?;
        let images = swapchain
            .enumerate_images()
            .api("xrEnumerateSwapchainImages(Vulkan)")?;
        report.line(format!(
            "XR swapchain format={format:?} extent={width}x{height} samples=1 layers=2 images={}",
            images.len()
        ));
        let blend_modes = instance
            .enumerate_environment_blend_modes(system, VIEW)
            .api("xrEnumerateEnvironmentBlendModes")?;
        let blend = if blend_modes.contains(&xr::EnvironmentBlendMode::OPAQUE) {
            xr::EnvironmentBlendMode::OPAQUE
        } else {
            *blend_modes.first().ok_or("no environment blend modes")?
        };
        let renderer = Renderer::new(gpu.clone(), format, vk::Extent2D { width, height }, images)?;
        report.line(format!("graphics resources SUCCESS; blend={blend:?}; left=dim red, right=dim blue; first 90 rendered frames clear-only, then RGB triangle"));
        Ok(Self {
            renderer,
            swapchain,
            space,
            frames,
            waiter,
            session,
            _gpu: gpu,
            extent: xr::Extent2Di {
                width: width as i32,
                height: height as i32,
            },
            blend,
            running: false,
            rendered: 0,
        })
    }

    pub(super) fn run(
        &mut self,
        instance: &xr::Instance,
        stop: &AtomicBool,
        resumed: &AtomicBool,
        report: &mut Report,
    ) -> Result<()> {
        let mut buffer = xr::EventDataBuffer::new();
        let mut exit_requested = false;
        let mut stopping_since = None;
        loop {
            while let Some(event) = instance.poll_event(&mut buffer).api("xrPollEvent")? {
                match event {
                    xr::Event::SessionStateChanged(event)
                        if event.session() == self.session.as_raw() =>
                    {
                        report.line(format!("session_state={:?}", event.state()));
                        match event.state() {
                            xr::SessionState::READY if !stop.load(Ordering::Acquire) => {
                                self.session.begin(VIEW).api("xrBeginSession(stereo)")?;
                                self.running = true;
                                exit_requested = false;
                            }
                            xr::SessionState::STOPPING => {
                                if self.running {
                                    self.session.end().api("xrEndSession")?;
                                    self.running = false;
                                }
                            }
                            xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                    xr::Event::InstanceLossPending(_) => {
                        report.line("instance loss pending");
                        return Ok(());
                    }
                    _ => {}
                }
            }
            let stopping = stop.load(Ordering::Acquire);
            if stopping && !self.running {
                return Ok(());
            }
            if stopping && !exit_requested {
                self.session.request_exit().api("xrRequestExitSession")?;
                report.line("xrRequestExitSession SUCCESS; waiting for STOPPING");
                exit_requested = true;
                stopping_since = Some(Instant::now());
            }
            if stopping_since.is_some_and(|time| time.elapsed() > Duration::from_secs(3)) {
                return Err(
                    "runtime did not deliver STOPPING within 3 seconds after xrRequestExitSession"
                        .into(),
                );
            }
            if !self.running {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            self.frame(!stopping && resumed.load(Ordering::Acquire), report)?;
        }
    }

    fn frame(&mut self, active: bool, report: &mut Report) -> Result<()> {
        let state = self.waiter.wait().api("xrWaitFrame")?;
        self.frames.begin().api("xrBeginFrame")?;
        let result = self.render_frame(
            state.predicted_display_time,
            active && state.should_render,
            report,
        );
        // A failed frame must still be paired with xrEndFrame before teardown.
        match result {
            Ok(Some(views)) => {
                let projection_views = [0, 1].map(|eye| {
                    xr::CompositionLayerProjectionView::new()
                        .pose(views[eye].pose)
                        .fov(views[eye].fov)
                        .sub_image(
                            xr::SwapchainSubImage::new()
                                .swapchain(&self.swapchain)
                                .image_array_index(eye as u32)
                                .image_rect(xr::Rect2Di {
                                    offset: xr::Offset2Di { x: 0, y: 0 },
                                    extent: self.extent,
                                }),
                        )
                });
                self.frames
                    .end(
                        state.predicted_display_time,
                        self.blend,
                        &[&xr::CompositionLayerProjection::new()
                            .space(&self.space)
                            .views(&projection_views)],
                    )
                    .api("xrEndFrame(projection)")?;
            }
            Ok(None) => self
                .frames
                .end(state.predicted_display_time, self.blend, &[])
                .api("xrEndFrame(empty)")?,
            Err(error) => {
                if let Err(end) = self
                    .frames
                    .end(state.predicted_display_time, self.blend, &[])
                {
                    report.line(format!("xrEndFrame after render failure: {end:?}"));
                }
                return Err(error);
            }
        }
        Ok(())
    }

    fn render_frame(
        &mut self,
        time: xr::Time,
        render: bool,
        report: &mut Report,
    ) -> Result<Option<Vec<xr::View>>> {
        if !render {
            return Ok(None);
        }
        let (flags, views) = self
            .session
            .locate_views(VIEW, time, &self.space)
            .api("xrLocateViews(stereo)")?;
        if !flags
            .contains(xr::ViewStateFlags::ORIENTATION_VALID | xr::ViewStateFlags::POSITION_VALID)
        {
            return Ok(None);
        }
        if views.len() != 2 {
            return Err("xrLocateViews returned an unexpected view count".into());
        }
        let index = self
            .swapchain
            .acquire_image()
            .api("xrAcquireSwapchainImage")?;
        self.swapchain
            .wait_image(xr::Duration::INFINITE)
            .api("xrWaitSwapchainImage")?;
        let triangle = self.rendered >= 90;
        // Only release after successful fence completion. On failure teardown
        // waits for the device before destroying the still-acquired XR image.
        self.renderer.draw(index as usize, triangle)?;
        self.swapchain
            .release_image()
            .api("xrReleaseSwapchainImage")?;
        if self.rendered == 0 || self.rendered == 90 || self.rendered % 900 == 0 {
            report.line(format!(
                "rendered_frame={} mode={} GPU_complete_before_release=true",
                self.rendered,
                if triangle {
                    "stereo_triangle"
                } else {
                    "stereo_clear"
                }
            ));
        }
        self.rendered += 1;
        Ok(Some(views))
    }
}
