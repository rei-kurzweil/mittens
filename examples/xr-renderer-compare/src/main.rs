//! Desktop OpenXR A/B demo using one frame loop and one shared custom scene.
mod backend;
#[path = "../../../android/src/graphics_device.rs"]
mod graphics_device;
mod options;
#[path = "../../../android/src/renderer/mod.rs"]
mod renderer;
mod scene;
mod timings;

// Host reporting adapter for the shared prototype modules. Android lifecycle
// and JNI are not compiled into this executable.
mod xr_host {
    pub type Result<T> = std::result::Result<T, String>;
    pub trait ApiResult<T> {
        fn api(self, op: &str) -> Result<T>;
    }
    impl<T, E: std::fmt::Debug> ApiResult<T> for std::result::Result<T, E> {
        fn api(self, op: &str) -> Result<T> {
            self.map_err(|e| format!("{op}: {e:?}"))
        }
    }
    pub struct Report;
    impl Report {
        pub fn line(&mut self, value: impl std::fmt::Display) {
            println!("{value}");
        }
    }
}
use ash::{vk, vk::Handle};
use backend::Backend;
use mittens_engine::engine::{
    Universe,
    graphics::{CameraData, XRSwapchain},
    user_input::InputState,
};
use openxr as xr;
use options::Options;
use renderer::view::EyeCamera;
use std::time::{Duration, Instant};
use timings::{Sample, Timings};
use xr_host::{ApiResult, Result};
const VIEW: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

struct App {
    // Borrowed image views are retired in Drop. Targets/session then drop before
    // the backend releases its graphics device; the scene has no engine XR driver.
    target: XRSwapchain,
    space: xr::Space,
    frames: xr::FrameStream<xr::Vulkan>,
    waiter: xr::FrameWaiter,
    session: xr::Session<xr::Vulkan>,
    backend: Backend,
    universe: Universe,
    instance: xr::Instance,
    running: bool,
    last_time: Option<i64>,
}
impl Drop for App {
    fn drop(&mut self) {
        if let Backend::Lean { renderer, .. } = &mut self.backend {
            renderer.take();
        }
        if self.running {
            let _ = self.session.request_exit();
        }
    }
}
impl App {
    fn new(options: &Options) -> Result<Self> {
        let entry = mittens_engine::engine::ecs::system::openxr_system::OpenXRSystem::load_entry()?;
        let available = entry
            .enumerate_extensions()
            .api("enumerate XR extensions")?;
        if !available.khr_vulkan_enable && !available.khr_vulkan_enable2 {
            return Err("runtime has no Vulkan binding".into());
        }
        if options.renderer == "desktop" && !available.khr_vulkan_enable {
            return Err("existing desktop backend requires XR_KHR_vulkan_enable".into());
        }
        let mut extensions = xr::ExtensionSet::default();
        extensions.khr_vulkan_enable = available.khr_vulkan_enable;
        extensions.khr_vulkan_enable2 = available.khr_vulkan_enable2;
        let instance = entry
            .create_instance(
                &xr::ApplicationInfo {
                    application_name: "Mittens Renderer Compare",
                    application_version: 1,
                    engine_name: "Mittens",
                    engine_version: 1,
                    api_version: xr::Version::new(1, 0, 34),
                },
                &extensions,
                &[],
            )
            .api("create desktop XR instance")?;
        println!(
            "runtime={:?}",
            instance.properties().api("XR runtime properties")?
        );
        let system = instance
            .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
            .api("get XR HMD")?;
        let mut backend = Backend::new(
            &options.renderer,
            &instance,
            system,
            available.khr_vulkan_enable2,
        )?;
        let gfx = backend.graphics();
        let (session, waiter, frames) = unsafe {
            instance.create_session::<xr::Vulkan>(
                system,
                &xr::vulkan::SessionCreateInfo {
                    instance: gfx.vk_instance,
                    physical_device: gfx.vk_physical_device,
                    device: gfx.vk_device,
                    queue_family_index: gfx.queue_family_index,
                    queue_index: gfx.queue_index,
                },
            )
        }
        .api("create desktop XR session")?;
        let space = session
            .create_reference_space(xr::ReferenceSpaceType::LOCAL, xr::Posef::IDENTITY)
            .api("create LOCAL space")?;
        let view_config = instance
            .enumerate_view_configuration_views(system, VIEW)
            .api("stereo configuration")?;
        if view_config.len() != 2 {
            return Err("comparison requires stereo views".into());
        }
        let blends = instance
            .enumerate_environment_blend_modes(system, VIEW)
            .api("blend modes")?;
        if !blends.contains(&xr::EnvironmentBlendMode::OPAQUE) {
            return Err("comparison requires opaque environment blending".into());
        }
        let target = unsafe {
            XRSwapchain::new(
                &instance,
                &session,
                system,
                VIEW,
                Some(vk::Format::R8G8B8A8_SRGB.as_raw() as u32),
                vk::Device::from_raw(gfx.vk_device as u64),
            )
        }?;
        let directory =
            std::env::temp_dir().join(format!("mittens-xr-compare-{}", std::process::id()));
        let universe = scene::load(&directory, options.grid)?;
        backend.initialize_scene(&target)?;
        println!(
            "renderer={} extent={:?} layers={} format={} samples=1 cubes={} build={} budget_hz={} (runtime refresh is unchanged)",
            options.renderer,
            target.extent(),
            target.view_count(),
            target.format(),
            options.grid * options.grid,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            options.hz
        );
        Ok(Self {
            target,
            space,
            frames,
            waiter,
            session,
            backend,
            universe,
            instance,
            running: false,
            last_time: None,
        })
    }
    fn run(&mut self, options: &Options) -> Result<Timings> {
        let started = Instant::now();
        let mut events = xr::EventDataBuffer::new();
        let mut ready_since = None;
        let mut sampling_since = None;
        let mut exit_since = None;
        let mut last_frame = None;
        let mut last_prediction = None;
        let mut timings = Timings::new(options.hz);
        loop {
            while let Some(event) = self
                .instance
                .poll_event(&mut events)
                .api("poll XR events")?
            {
                match event {
                    xr::Event::SessionStateChanged(event)
                        if event.session() == self.session.as_raw() =>
                    {
                        println!("session={:?}", event.state());
                        match event.state() {
                            xr::SessionState::READY if exit_since.is_none() => {
                                self.session.begin(VIEW).api("begin XR session")?;
                                self.running = true;
                            }
                            xr::SessionState::STOPPING => {
                                if self.running {
                                    self.session.end().api("end XR session")?;
                                    self.running = false;
                                }
                                return Ok(timings);
                            }
                            xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                                return Ok(timings);
                            }
                            _ => {}
                        }
                    }
                    xr::Event::InstanceLossPending(_) => return Ok(timings),
                    _ => {}
                }
            }
            if !self.running {
                if started.elapsed() > options.startup_timeout {
                    return Err("no running desktop XR session before startup timeout".into());
                }
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            if exit_since.is_none()
                && (sampling_since.is_some_and(|t: Instant| t.elapsed() >= options.seconds)
                    || (ready_since.is_none() && started.elapsed() >= options.startup_timeout))
            {
                if ready_since.is_none() {
                    println!(
                        "startup timeout: no visible rendered scene; requesting clean session stop without performance samples"
                    );
                }
                self.session.request_exit().api("request XR exit")?;
                exit_since = Some(Instant::now());
            }
            if exit_since.is_some_and(|t| t.elapsed() > Duration::from_secs(3)) {
                return Err("runtime did not stop after exit request".into());
            }
            let frame_start = Instant::now();
            let state = self.waiter.wait().api("xrWaitFrame")?;
            let wait = frame_start.elapsed();
            let active = Instant::now();
            self.frames.begin().api("xrBeginFrame")?;
            if !state.should_render || exit_since.is_some() {
                last_frame = None;
                last_prediction = None;
                self.frames
                    .end(
                        state.predicted_display_time,
                        xr::EnvironmentBlendMode::OPAQUE,
                        &[],
                    )
                    .api("xrEndFrame(empty)")?;
                continue;
            }
            let frame = self.render(state.predicted_display_time);
            let (views, mut sample, ready) = match frame {
                Ok(frame) => frame,
                Err(error) => {
                    let _ = self.frames.end(
                        state.predicted_display_time,
                        xr::EnvironmentBlendMode::OPAQUE,
                        &[],
                    );
                    return Err(error);
                }
            };
            let Some(views) = views else {
                last_frame = None;
                last_prediction = None;
                self.frames
                    .end(
                        state.predicted_display_time,
                        xr::EnvironmentBlendMode::OPAQUE,
                        &[],
                    )
                    .api("xrEndFrame(no poses)")?;
                continue;
            };
            let guard = self.target.swapchain();
            let projection = [0, 1].map(|eye| {
                xr::CompositionLayerProjectionView::new()
                    .pose(views[eye].pose)
                    .fov(views[eye].fov)
                    .sub_image(
                        xr::SwapchainSubImage::new()
                            .swapchain(&guard)
                            .image_array_index(eye as u32)
                            .image_rect(xr::Rect2Di {
                                offset: xr::Offset2Di::default(),
                                extent: self.target.extent(),
                            }),
                    )
            });
            let submit = Instant::now();
            self.frames
                .end(
                    state.predicted_display_time,
                    xr::EnvironmentBlendMode::OPAQUE,
                    &[&xr::CompositionLayerProjection::new()
                        .space(&self.space)
                        .views(&projection)],
                )
                .api("xrEndFrame(projection)")?;
            sample.submit = submit.elapsed();
            sample.wait = wait;
            sample.active = active.elapsed();
            sample.interval = last_frame.map_or(Duration::ZERO, |last| {
                frame_start.saturating_duration_since(last)
            });
            sample.predicted_period_ms = state.predicted_display_period.as_nanos() as f64 * 1e-6;
            sample.predicted_step_ms = last_prediction.map_or(0.0, |last| {
                (state.predicted_display_time.as_nanos() - last) as f64 * 1e-6
            });
            last_frame = Some(frame_start);
            last_prediction = Some(state.predicted_display_time.as_nanos());
            if ready && !sample.interval.is_zero() {
                let since = *ready_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= options.warmup {
                    if sampling_since.is_none() {
                        sampling_since = Some(Instant::now());
                        println!("sampling started; warmup excluded");
                    }
                    timings.record(sample);
                }
            }
        }
    }
    fn render(&mut self, time: xr::Time) -> Result<(Option<Vec<xr::View>>, Sample, bool)> {
        let mut sample = Sample::default();
        let (flags, views) = self
            .session
            .locate_views(VIEW, time, &self.space)
            .api("locate stereo views")?;
        if !flags
            .contains(xr::ViewStateFlags::ORIENTATION_VALID | xr::ViewStateFlags::POSITION_VALID)
        {
            return Ok((None, sample, false));
        }
        if views.len() != 2 {
            return Err("unexpected view count".into());
        }
        let update = Instant::now();
        let now = time.as_nanos();
        let dt = self.last_time.map_or(1.0 / 90.0, |last| {
            ((now - last) as f64 * 1e-9).clamp(0.0, 0.1) as f32
        });
        self.last_time = Some(now);
        self.universe.update(dt, &InputState::default());
        let cameras = [
            EyeCamera::from_xr(&views[0])?,
            EyeCamera::from_xr(&views[1])?,
        ];
        self.universe.visuals.set_xr_camera(
            cameras
                .iter()
                .map(|camera| CameraData {
                    view: camera.view,
                    proj: camera.proj,
                    transform: Default::default(),
                })
                .collect(),
        );
        sample.update = update.elapsed();
        let prepare = Instant::now();
        self.universe.systems.prepare_render(
            &mut self.universe.world,
            &mut self.universe.visuals,
            &mut self.universe.render_assets,
            self.backend.uploader(),
            &mut self.universe.command_queue,
        );
        self.universe.visuals.prepare_draw_cache();
        sample.prepare = prepare.elapsed();
        let ready = !self.universe.visuals.instances().is_empty()
            && self
                .universe
                .visuals
                .materials
                .instances()
                .all(|(_, record)| {
                    self.universe
                        .visuals
                        .materials
                        .definition(record.definition)
                        .and_then(|d| d.custom.as_ref())
                        .is_some_and(|d| matches!(d.compiled.get(), Some(Ok(_))))
                });
        let index = self.target.acquire_image().api("acquire XR image")?;
        self.target
            .wait_image(xr::Duration::INFINITE)
            .api("wait XR image")?;
        let render = Instant::now();
        self.backend.draw(
            &mut self.target,
            index,
            &mut self.universe.visuals,
            &cameras,
        )?;
        sample.render_complete = render.elapsed();
        self.target
            .release_image()
            .api("release completed XR image")?;
        Ok((Some(views), sample, ready))
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    mittens_engine::utils::logger::init();
    let mut app = App::new(&options)?;
    let timings = app.run(&options)?;
    timings.write(&options.output, &options.renderer)?;
    Ok(())
}
