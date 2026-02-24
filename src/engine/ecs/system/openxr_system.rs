use crate::engine::ecs::component::CameraXRComponent;
use crate::engine::ecs::component::OpenXRComponent;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::CameraData;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::VulkanoRenderer;
use crate::engine::graphics::XRSwapchain;
use crate::engine::graphics::XrVulkanGraphics;
use crate::engine::user_input::InputState;

use ash::vk::Handle as _;

pub struct OpenXRSystem {
    state: Option<OpenXRState>,
    last_init_error: Option<String>,
    vulkan_graphics: Option<XrVulkanGraphics>,
    preferred_swapchain_format: Option<u32>,
}

struct OpenXRState {
    #[allow(dead_code)]
    entry: openxr::Entry,
    #[allow(dead_code)]
    instance: openxr::Instance,
    #[allow(dead_code)]
    system: openxr::SystemId,
    events: openxr::EventDataBuffer,

    session: Option<OpenXRSessionState>,
    view_type: openxr::ViewConfigurationType,
    blend_mode: openxr::EnvironmentBlendMode,
}

struct OpenXRSessionState {
    session: openxr::Session<openxr::Vulkan>,
    frame_waiter: openxr::FrameWaiter,
    frame_stream: openxr::FrameStream<openxr::Vulkan>,
    reference_space: openxr::Space,
    running: bool,

    xr_swapchain: XRSwapchain,

    swapchain_image_initialized: Vec<bool>,

    did_log_format_mismatch: bool,

    vk_device: ash::Device,
    vk_queue: ash::vk::Queue,
    vk_command_pool: ash::vk::CommandPool,
    vk_command_buffer: ash::vk::CommandBuffer,
}

impl Default for OpenXRSystem {
    fn default() -> Self {
        Self {
            state: None,
            last_init_error: None,
            vulkan_graphics: None,
            preferred_swapchain_format: None,
        }
    }
}

impl std::fmt::Debug for OpenXRSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenXRSystem")
            .field("initialized", &self.state.is_some())
            .field("last_init_error", &self.last_init_error)
            .finish()
    }
}

impl OpenXRSystem {
    /// Hint: prefer this Vulkan `VkFormat` (as raw `u32`) when creating the OpenXR swapchain.
    ///
    /// This is used to match the window renderer's color attachment format, so we can copy
    /// rendered images into the OpenXR swapchain without format mismatch.
    pub fn set_preferred_swapchain_format(&mut self, format: u32) {
        self.preferred_swapchain_format = Some(format);
    }

    /// Returns the Vulkan instance/device extensions required by the active OpenXR runtime.
    ///
    /// This uses the `XR_KHR_vulkan_enable` query APIs (space-delimited strings). Even when the
    /// runtime supports `XR_KHR_vulkan_enable2`, SteamVR commonly still reports the required
    /// extension lists via these functions.
    pub fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        let state = self.state.as_ref()?;

        let instance_exts = state
            .instance
            .vulkan_legacy_instance_extensions(state.system)
            .ok()?;
        let device_exts = state
            .instance
            .vulkan_legacy_device_extensions(state.system)
            .ok()?;

        let split = |s: String| -> Vec<String> {
            s.split_whitespace()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect()
        };

        Some((split(instance_exts), split(device_exts)))
    }

    pub fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        self.vulkan_graphics = Some(gfx);

        // If OpenXR is already initialized, opportunistically create the session now.
        let Some(state) = self.state.as_mut() else {
            return;
        };

        if state.session.is_none() {
            if let Err(err) = Self::try_init_session(state, gfx, self.preferred_swapchain_format) {
                eprintln!("[OpenXR] Session init failed: {err}");
                self.last_init_error = Some(err);
            }
        }
    }

    pub fn register_openxr(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(cfg) = world.get_component_by_id_as::<OpenXRComponent>(component) else {
            return;
        };

        if !cfg.enabled {
            return;
        }

        if self.state.is_some() {
            return;
        }

        match Self::try_init_openxr() {
            Ok(state) => {
                println!("[OpenXR] Initialized.");
                self.state = Some(state);
                self.last_init_error = None;

                // If we already have Vulkan handles from the renderer, try to create a session.
                if let (Some(state), Some(gfx)) = (self.state.as_mut(), self.vulkan_graphics) {
                    if state.session.is_none() {
                        if let Err(err) =
                            Self::try_init_session(state, gfx, self.preferred_swapchain_format)
                        {
                            eprintln!("[OpenXR] Session init failed: {err}");
                            self.last_init_error = Some(err);
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("[OpenXR] Init failed: {err}");
                self.last_init_error = Some(err);
            }
        }
    }

    fn try_init_openxr() -> Result<OpenXRState, String> {
        // Prefer dynamically loading the OpenXR loader. This keeps us from requiring
        // special linker setup and matches typical Linux setups.
        let entry = unsafe { openxr::Entry::load().map_err(|e| format!("Entry::load: {e:?}"))? };

        let app_info = openxr::ApplicationInfo {
            application_name: "cat-engine",
            application_version: 1,
            engine_name: "cat-engine",
            engine_version: 1,
            api_version: openxr::Version::new(1, 0, 0),
        };

        let mut extensions = openxr::ExtensionSet::default();
        // Use the legacy Vulkan binding path for now (reuse an already-created VkInstance/VkDevice).
        // SteamVR can be stricter with XR_KHR_vulkan_enable2 unless you create Vulkan objects
        // via xrCreateVulkanInstanceKHR/xrCreateVulkanDeviceKHR.
        extensions.khr_vulkan_enable2 = false;
        // Needed for Vulkan session creation.
        extensions.khr_vulkan_enable = true;
        let layers: [&str; 0] = [];

        let instance = entry
            .create_instance(&app_info, &extensions, &layers)
            .map_err(|e| format!("create_instance: {e:?}"))?;

        // Best-effort runtime identification (helps debugging which OpenXR runtime is active).
        if let Ok(props) = instance.properties() {
            println!(
                "[OpenXR] Runtime: {} ({:?})",
                props.runtime_name, props.runtime_version
            );
        }

        let system = match instance.system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY) {
            Ok(system) => system,
            Err(openxr::sys::Result::ERROR_FORM_FACTOR_UNAVAILABLE) => {
                return Err(
                    "system(HMD): ERROR_FORM_FACTOR_UNAVAILABLE (no HMD detected / runtime not ready).\n\
                    Start an OpenXR runtime (e.g. SteamVR/Monado/ALVR) and ensure a headset is connected and the runtime is running before launching cat-engine.\n\
                    On Linux you can also check XR_RUNTIME_JSON points to an installed runtime manifest."
                        .to_string(),
                );
            }
            Err(e) => return Err(format!("system(HMD): {e:?}")),
        };

        // Pick a supported blend mode and stash common config.
        let view_type = openxr::ViewConfigurationType::PRIMARY_STEREO;
        let blend_mode = instance
            .enumerate_environment_blend_modes(system, view_type)
            .ok()
            .and_then(|m| m.first().copied())
            .unwrap_or(openxr::EnvironmentBlendMode::OPAQUE);

        Ok(OpenXRState {
            entry,
            instance,
            system,
            events: openxr::EventDataBuffer::new(),
            session: None,
            view_type,
            blend_mode,
        })
    }

    fn try_init_session(
        state: &mut OpenXRState,
        gfx: XrVulkanGraphics,
        preferred_swapchain_format: Option<u32>,
    ) -> Result<(), String> {
        // Log Vulkan version requirements (useful debugging).
        if let Ok(reqs) = state
            .instance
            .graphics_requirements::<openxr::Vulkan>(state.system)
        {
            println!(
                "[OpenXR] Vulkan requirements: min {:?}, max {:?}",
                reqs.min_api_version_supported, reqs.max_api_version_supported
            );
        }

        // SteamVR may validate that the VkPhysicalDevice matches the one it expects for the HMD.
        // If these differ (multi-GPU setups), create_session can fail.
        if let Ok(required_pd) = unsafe {
            state
                .instance
                .vulkan_graphics_device(state.system, gfx.vk_instance)
        } {
            if required_pd != gfx.vk_physical_device {
                return Err(format!(
                    "Vulkan physical device mismatch for OpenXR system.\n\
Engine/Vulkano physical device: {:?}\n\
OpenXR-required physical device: {:?}\n\
Fix: pick the OpenXR-required VkPhysicalDevice when creating the Vulkan device/queues.",
                    gfx.vk_physical_device, required_pd
                ));
            }
        }

        let info = openxr::vulkan::SessionCreateInfo {
            instance: gfx.vk_instance,
            physical_device: gfx.vk_physical_device,
            device: gfx.vk_device,
            queue_family_index: gfx.queue_family_index,
            queue_index: gfx.queue_index,
        };

        let (session, frame_waiter, frame_stream) = unsafe {
            state
                .instance
                .create_session::<openxr::Vulkan>(state.system, &info)
                .map_err(|e| {
                    let required_instance = state
                        .instance
                        .vulkan_legacy_instance_extensions(state.system)
                        .ok();
                    let required_device = state
                        .instance
                        .vulkan_legacy_device_extensions(state.system)
                        .ok();

                    let extra = match (required_instance, required_device) {
                        (Some(i), Some(d)) => format!(
                            "\n[OpenXR] Runtime-required Vulkan instance extensions: {i}\n[OpenXR] Runtime-required Vulkan device extensions: {d}"
                        ),
                        _ => String::new(),
                    };

                    format!(
                        "create_session(Vulkan): {e:?}.\n\
If this fails with Vulkan extension errors, the Vulkan instance/device created by Vulkano may be missing runtime-required extensions (from XR_KHR_vulkan_enable2)."
                    )
                    + extra.as_str()
                })?
        };

        let reference_space = session
            .create_reference_space(openxr::ReferenceSpaceType::LOCAL, openxr::Posef::IDENTITY)
            .map_err(|e| format!("create_reference_space(LOCAL): {e:?}"))?;

        let xr_swapchain = XRSwapchain::new(
            &state.instance,
            &session,
            state.system,
            state.view_type,
            preferred_swapchain_format,
        )?;

        let swapchain_image_initialized = vec![false; xr_swapchain.images().len()];

        // Minimal Vulkan command recording for XR bring-up: wrap the existing VkDevice via ash.
        let entry = unsafe { ash::Entry::load().map_err(|e| format!("ash::Entry::load: {e:?}"))? };

        let vk_instance = ash::vk::Instance::from_raw(gfx.vk_instance as usize as u64);
        let vk_device_handle = ash::vk::Device::from_raw(gfx.vk_device as usize as u64);

        let vk_instance = unsafe { ash::Instance::load(entry.static_fn(), vk_instance) };
        let vk_device = unsafe { ash::Device::load(vk_instance.fp_v1_0(), vk_device_handle) };
        let vk_queue =
            unsafe { vk_device.get_device_queue(gfx.queue_family_index, gfx.queue_index) };

        let vk_command_pool = unsafe {
            vk_device
                .create_command_pool(
                    &ash::vk::CommandPoolCreateInfo::default()
                        .queue_family_index(gfx.queue_family_index)
                        .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .map_err(|e| format!("create_command_pool: {e:?}"))?
        };

        let vk_command_buffer = unsafe {
            vk_device
                .allocate_command_buffers(
                    &ash::vk::CommandBufferAllocateInfo::default()
                        .command_pool(vk_command_pool)
                        .level(ash::vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .map_err(|e| format!("allocate_command_buffers: {e:?}"))?
                .into_iter()
                .next()
                .ok_or("allocate_command_buffers returned 0 command buffers")?
        };

        state.session = Some(OpenXRSessionState {
            session,
            frame_waiter,
            frame_stream,
            reference_space,
            running: false,

            xr_swapchain,

            swapchain_image_initialized,

            did_log_format_mismatch: false,

            vk_device,
            vk_queue,
            vk_command_pool,
            vk_command_buffer,
        });

        println!("[OpenXR] Session created (Vulkan)");
        Ok(())
    }

    fn first_enabled_camera_xr(world: &World) -> Option<ComponentId> {
        world
            .all_components()
            .filter_map(|id| {
                world
                    .get_component_by_id_as::<CameraXRComponent>(id)
                    .map(|c| (id, c.enabled))
            })
            .find(|(_, enabled)| *enabled)
            .map(|(id, _)| id)
    }

    fn mul_mat4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
        let mut c = [[0.0f32; 4]; 4];
        for col in 0..4 {
            for row in 0..4 {
                c[col][row] = a[0][row] * b[col][0]
                    + a[1][row] * b[col][1]
                    + a[2][row] * b[col][2]
                    + a[3][row] * b[col][3];
            }
        }
        c
    }

    fn transform_from_matrix_world(
        m: [[f32; 4]; 4],
    ) -> crate::engine::graphics::primitives::Transform {
        let mut t = crate::engine::graphics::primitives::Transform::default();
        t.model = m;
        t.matrix_world = m;
        t
    }

    fn mat4_from_pose(pose: openxr::Posef) -> [[f32; 4]; 4] {
        // IMPORTANT: This must match the engine's quaternion convention.
        // `Transform::recompute_model` is the canonical implementation.
        let q = pose.orientation;
        let p = pose.position;

        let mut t = crate::engine::graphics::primitives::Transform::default();
        t.translation = [p.x, p.y, p.z];
        t.rotation = [q.x, q.y, q.z, q.w];
        t.scale = [1.0, 1.0, 1.0];
        t.recompute_model();

        // OpenXR view poses are right-handed with -Z forward, which matches the engine's
        // Camera3D convention (forward -Z) and our projection math.
        //
        // Do not apply additional basis flips here unless we have a proven convention mismatch,
        // since flipping X/Z will also flip head translation direction and can break stereo.
        t.model
    }

    fn invert_affine_transform(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
        // Upper-left 3x3 in column-major.
        let c0 = [m[0][0], m[0][1], m[0][2]];
        let c1 = [m[1][0], m[1][1], m[1][2]];
        let c2 = [m[2][0], m[2][1], m[2][2]];

        // Row-major elements for determinant/cofactors.
        let a00 = c0[0];
        let a10 = c0[1];
        let a20 = c0[2];
        let a01 = c1[0];
        let a11 = c1[1];
        let a21 = c1[2];
        let a02 = c2[0];
        let a12 = c2[1];
        let a22 = c2[2];

        let det = a00 * (a11 * a22 - a12 * a21) - a01 * (a10 * a22 - a12 * a20)
            + a02 * (a10 * a21 - a11 * a20);

        if det.abs() < 1e-8 {
            return [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
        }
        let inv_det = 1.0 / det;

        // Inverse in row-major.
        let inv00 = (a11 * a22 - a12 * a21) * inv_det;
        let inv01 = (a02 * a21 - a01 * a22) * inv_det;
        let inv02 = (a01 * a12 - a02 * a11) * inv_det;

        let inv10 = (a12 * a20 - a10 * a22) * inv_det;
        let inv11 = (a00 * a22 - a02 * a20) * inv_det;
        let inv12 = (a02 * a10 - a00 * a12) * inv_det;

        let inv20 = (a10 * a21 - a11 * a20) * inv_det;
        let inv21 = (a01 * a20 - a00 * a21) * inv_det;
        let inv22 = (a00 * a11 - a01 * a10) * inv_det;

        let tx = m[3][0];
        let ty = m[3][1];
        let tz = m[3][2];

        let itx = -(inv00 * tx + inv01 * ty + inv02 * tz);
        let ity = -(inv10 * tx + inv11 * ty + inv12 * tz);
        let itz = -(inv20 * tx + inv21 * ty + inv22 * tz);

        [
            [inv00, inv10, inv20, 0.0],
            [inv01, inv11, inv21, 0.0],
            [inv02, inv12, inv22, 0.0],
            [itx, ity, itz, 1.0],
        ]
    }

    fn proj_from_fov_rh_zo(fov: openxr::Fovf, z_near: f32, z_far: f32) -> [[f32; 4]; 4] {
        let l = (fov.angle_left).tan() * z_near;
        let r = (fov.angle_right).tan() * z_near;
        let d = (fov.angle_down).tan() * z_near;
        let u = (fov.angle_up).tan() * z_near;

        let w = r - l;
        let h = u - d;
        let nf = 1.0 / (z_near - z_far);

        // Match the engine's column-major RH ZO layout used in Camera3D::perspective_rh_zo.
        // Now: +Y is up in clip space, matching OpenXR and GLTF conventions.
        [
            [2.0 * z_near / w, 0.0, 0.0, 0.0],
            [0.0, 2.0 * z_near / h, 0.0, 0.0],
            [(r + l) / w, (u + d) / h, z_far * nf, -1.0],
            [0.0, 0.0, (z_near * z_far) * nf, 0.0],
        ]
    }

    fn clear_xr_swapchain_image(
        sess: &OpenXRSessionState,
        image: ash::vk::Image,
        rgba: [f32; 4],
        was_initialized: bool,
    ) -> Result<(), ash::vk::Result> {
        let clear = ash::vk::ClearColorValue { float32: rgba };

        let old_layout = if was_initialized {
            ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
        } else {
            ash::vk::ImageLayout::UNDEFINED
        };

        let src_stage = if was_initialized {
            ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
        } else {
            ash::vk::PipelineStageFlags::TOP_OF_PIPE
        };

        let src_access = if was_initialized {
            ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE
        } else {
            ash::vk::AccessFlags::empty()
        };

        unsafe {
            sess.vk_device.reset_command_buffer(
                sess.vk_command_buffer,
                ash::vk::CommandBufferResetFlags::empty(),
            )?;

            sess.vk_device.begin_command_buffer(
                sess.vk_command_buffer,
                &ash::vk::CommandBufferBeginInfo::default()
                    .flags(ash::vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;

            let range = ash::vk::ImageSubresourceRange::default()
                .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(sess.xr_swapchain.view_count());

            // Transition UNDEFINED -> TRANSFER_DST_OPTIMAL.
            let barrier_to_transfer = ash::vk::ImageMemoryBarrier::default()
                .old_layout(old_layout)
                .new_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_access_mask(src_access)
                .dst_access_mask(ash::vk::AccessFlags::TRANSFER_WRITE)
                .image(image)
                .subresource_range(range);

            sess.vk_device.cmd_pipeline_barrier(
                sess.vk_command_buffer,
                src_stage,
                ash::vk::PipelineStageFlags::TRANSFER,
                ash::vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_to_transfer],
            );

            sess.vk_device.cmd_clear_color_image(
                sess.vk_command_buffer,
                image,
                ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &clear,
                &[range],
            );

            // Transition TRANSFER_DST_OPTIMAL -> COLOR_ATTACHMENT_OPTIMAL.
            let barrier_to_color = ash::vk::ImageMemoryBarrier::default()
                .old_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .src_access_mask(ash::vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .image(image)
                .subresource_range(range);

            sess.vk_device.cmd_pipeline_barrier(
                sess.vk_command_buffer,
                ash::vk::PipelineStageFlags::TRANSFER,
                ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                ash::vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_to_color],
            );

            sess.vk_device.end_command_buffer(sess.vk_command_buffer)?;

            let command_buffers = [sess.vk_command_buffer];
            let submit_info = ash::vk::SubmitInfo::default().command_buffers(&command_buffers);
            sess.vk_device
                .queue_submit(sess.vk_queue, &[submit_info], ash::vk::Fence::null())?;
            sess.vk_device.queue_wait_idle(sess.vk_queue)?;
        }

        Ok(())
    }
}

impl System for OpenXRSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        // Drain events; for now we just print them so you can see the runtime is alive.
        loop {
            let evt = match state.instance.poll_event(&mut state.events) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[OpenXR] poll_event error: {e:?}");
                    return;
                }
            };

            let Some(evt) = evt else {
                break;
            };

            // Avoid depending on Debug impls that might be missing.
            match evt {
                openxr::Event::InstanceLossPending(_) => {
                    eprintln!("[OpenXR] Event: InstanceLossPending");
                }
                openxr::Event::SessionStateChanged(e) => {
                    println!("[OpenXR] Event: SessionStateChanged -> {:?}", e.state());

                    if let Some(sess) = state.session.as_mut() {
                        match e.state() {
                            openxr::SessionState::READY => {
                                if !sess.running {
                                    if let Err(err) = sess.session.begin(state.view_type) {
                                        eprintln!("[OpenXR] session.begin failed: {err:?}");
                                    } else {
                                        sess.running = true;
                                    }
                                }
                            }
                            openxr::SessionState::STOPPING => {
                                if sess.running {
                                    if let Err(err) = sess.session.end() {
                                        eprintln!("[OpenXR] session.end failed: {err:?}");
                                    }
                                    sess.running = false;
                                }
                            }
                            openxr::SessionState::EXITING | openxr::SessionState::LOSS_PENDING => {
                                sess.running = false;
                            }
                            _ => {}
                        }
                    }
                }
                openxr::Event::EventsLost(e) => {
                    eprintln!("[OpenXR] Event: EventsLost ({})", e.lost_event_count());
                }
                _ => {
                    // Too noisy to print everything by default.
                }
            }
        }

        // Rendering is driven from Universe::render via `OpenXRSystem::render_xr`.
    }
}

impl OpenXRSystem {
    pub fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        let Some(sess) = state.session.as_mut() else {
            return;
        };
        if !sess.running {
            return;
        }

        let frame_state = match sess.frame_waiter.wait() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[OpenXR] wait_frame failed: {e:?}");
                return;
            }
        };

        if let Err(e) = sess.frame_stream.begin() {
            eprintln!("[OpenXR] begin_frame failed: {e:?}");
            return;
        }

        if !frame_state.should_render {
            let _ =
                sess.frame_stream
                    .end(frame_state.predicted_display_time, state.blend_mode, &[]);
            return;
        }

        let views = match sess.session.locate_views(
            state.view_type,
            frame_state.predicted_display_time,
            &sess.reference_space,
        ) {
            Ok((_flags, views)) => views,
            Err(e) => {
                eprintln!("[OpenXR] locate_views failed: {e:?}");
                let _ = sess.frame_stream.end(
                    frame_state.predicted_display_time,
                    state.blend_mode,
                    &[],
                );
                return;
            }
        };

        // Publish XR per-eye camera matrices into VisualWorld (CameraTarget::Xr).
        let rig_world = visuals
            .active_xr_camera()
            .or_else(|| Self::first_enabled_camera_xr(world))
            .and_then(|cid| TransformSystem::world_model(world, cid))
            .unwrap_or([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]);

        let mut eyes = Vec::with_capacity(views.len());
        for v in &views {
            let eye_from_space = Self::mat4_from_pose(v.pose);
            let world_from_eye = Self::mul_mat4(&rig_world, &eye_from_space);
            let view = Self::invert_affine_transform(&world_from_eye);
            let proj = Self::proj_from_fov_rh_zo(v.fov, 0.1, 100.0);
            eyes.push(CameraData {
                view,
                proj,
                transform: Self::transform_from_matrix_world(world_from_eye),
            });
        }
        visuals.set_xr_camera(eyes);

        // Acquire XR swapchain image.
        let image_index = {
            let swapchain = sess.xr_swapchain.swapchain_mut();
            match swapchain.acquire_image() {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("[OpenXR] acquire_image failed: {e:?}");
                    let _ = sess.frame_stream.end(
                        frame_state.predicted_display_time,
                        state.blend_mode,
                        &[],
                    );
                    return;
                }
            }
        };

        if let Err(e) = sess
            .xr_swapchain
            .swapchain_mut()
            .wait_image(openxr::Duration::INFINITE)
        {
            eprintln!("[OpenXR] wait_image failed: {e:?}");
        } else {
            // Render into offscreen Vulkano images (per eye), then copy into the OpenXR swapchain layers.
            let extent = sess.xr_swapchain.extent();
            let extent_u = [extent.width as u32, extent.height as u32];

            let image_index_usize = image_index as usize;
            let dst_was_initialized = sess
                .swapchain_image_initialized
                .get(image_index_usize)
                .copied()
                .unwrap_or(false);

            // Render both eyes first (blocking bring-up path).
            let view_count = sess.xr_swapchain.view_count() as usize;

            let xr_format = sess.xr_swapchain.format();
            let window_format = renderer.window_vk_format_raw();
            let format_matches = window_format.map(|f| f == xr_format).unwrap_or(true);

            // Copy requires compatible formats. If we couldn't match formats at swapchain creation,
            // fall back to a pink clear to prove we're submitting frames.
            let dst_image = sess.xr_swapchain.images()[image_index_usize];
            if !format_matches {
                if !sess.did_log_format_mismatch {
                    sess.did_log_format_mismatch = true;
                    eprintln!(
                        "[OpenXR] XR swapchain format mismatch (xr={} window={:?}); falling back to clear_color",
                        xr_format, window_format
                    );
                }

                if let Err(e) = Self::clear_xr_swapchain_image(
                    sess,
                    dst_image,
                    visuals.clear_color(),
                    dst_was_initialized,
                ) {
                    eprintln!("[OpenXR] clear XR image failed: {e:?}");
                } else if let Some(slot) =
                    sess.swapchain_image_initialized.get_mut(image_index_usize)
                {
                    *slot = true;
                }
            } else {
                for eye in 0..view_count.min(views.len()) {
                    if let Err(e) = renderer.render_xr_eye_offscreen(visuals, eye, extent_u) {
                        eprintln!("[OpenXR] render_xr_eye_offscreen failed: {e}");
                    }
                }

                if let Err(e) = Self::copy_offscreen_to_xr_layers(
                    sess,
                    renderer,
                    image_index_usize,
                    dst_image,
                    view_count,
                ) {
                    eprintln!("[OpenXR] copy to XR image failed: {e:?}");
                }
            }
        }

        if let Err(e) = sess.xr_swapchain.swapchain_mut().release_image() {
            eprintln!("[OpenXR] release_image failed: {e:?}");
        }

        // Submit a projection layer.
        if views.len() >= 2 {
            let rect = openxr::Rect2Di {
                offset: openxr::Offset2Di { x: 0, y: 0 },
                extent: sess.xr_swapchain.extent(),
            };

            let pv0 = openxr::CompositionLayerProjectionView::new()
                .pose(views[0].pose)
                .fov(views[0].fov)
                .sub_image(
                    openxr::SwapchainSubImage::new()
                        .swapchain(sess.xr_swapchain.swapchain())
                        .image_array_index(0)
                        .image_rect(rect),
                );

            let pv1 = openxr::CompositionLayerProjectionView::new()
                .pose(views[1].pose)
                .fov(views[1].fov)
                .sub_image(
                    openxr::SwapchainSubImage::new()
                        .swapchain(sess.xr_swapchain.swapchain())
                        .image_array_index(1)
                        .image_rect(rect),
                );

            let projection_views = [pv0, pv1];
            let layer = openxr::CompositionLayerProjection::new()
                .space(&sess.reference_space)
                .views(&projection_views);

            if let Err(e) = sess.frame_stream.end(
                frame_state.predicted_display_time,
                state.blend_mode,
                &[&layer],
            ) {
                eprintln!("[OpenXR] end_frame failed: {e:?}");
            }

            return;
        }

        if let Err(e) =
            sess.frame_stream
                .end(frame_state.predicted_display_time, state.blend_mode, &[])
        {
            eprintln!("[OpenXR] end_frame failed: {e:?}");
        }
    }

    fn copy_offscreen_to_xr_layers(
        sess: &mut OpenXRSessionState,
        renderer: &VulkanoRenderer,
        image_index: usize,
        dst_image: ash::vk::Image,
        view_count: usize,
    ) -> Result<(), ash::vk::Result> {
        let dst_was_initialized = sess
            .swapchain_image_initialized
            .get(image_index)
            .copied()
            .unwrap_or(false);

        let dst_old_layout = if dst_was_initialized {
            ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
        } else {
            ash::vk::ImageLayout::UNDEFINED
        };

        let dst_src_access = if dst_was_initialized {
            ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE
        } else {
            ash::vk::AccessFlags::empty()
        };

        unsafe {
            sess.vk_device.reset_command_buffer(
                sess.vk_command_buffer,
                ash::vk::CommandBufferResetFlags::empty(),
            )?;

            sess.vk_device.begin_command_buffer(
                sess.vk_command_buffer,
                &ash::vk::CommandBufferBeginInfo::default()
                    .flags(ash::vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;

            for eye in 0..view_count {
                let Some(src_image) = renderer.xr_offscreen_vk_image(eye) else {
                    continue;
                };

                let src_range = ash::vk::ImageSubresourceRange::default()
                    .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1);

                let dst_range = ash::vk::ImageSubresourceRange::default()
                    .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(eye as u32)
                    .layer_count(1);

                // src: COLOR_ATTACHMENT_OPTIMAL -> TRANSFER_SRC_OPTIMAL
                let barrier_src_to_copy = ash::vk::ImageMemoryBarrier::default()
                    .old_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .new_layout(ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .src_access_mask(ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .dst_access_mask(ash::vk::AccessFlags::TRANSFER_READ)
                    .image(src_image)
                    .subresource_range(src_range);

                // dst: UNDEFINED -> TRANSFER_DST_OPTIMAL (we overwrite whole layer)
                let barrier_dst_to_copy = ash::vk::ImageMemoryBarrier::default()
                    .old_layout(dst_old_layout)
                    .new_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_access_mask(dst_src_access)
                    .dst_access_mask(ash::vk::AccessFlags::TRANSFER_WRITE)
                    .image(dst_image)
                    .subresource_range(dst_range);

                sess.vk_device.cmd_pipeline_barrier(
                    sess.vk_command_buffer,
                    ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ash::vk::PipelineStageFlags::TRANSFER,
                    ash::vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier_src_to_copy, barrier_dst_to_copy],
                );

                let extent = sess.xr_swapchain.extent();
                let region = ash::vk::ImageCopy::default()
                    .src_subresource(
                        ash::vk::ImageSubresourceLayers::default()
                            .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                            .mip_level(0)
                            .base_array_layer(0)
                            .layer_count(1),
                    )
                    .dst_subresource(
                        ash::vk::ImageSubresourceLayers::default()
                            .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                            .mip_level(0)
                            .base_array_layer(eye as u32)
                            .layer_count(1),
                    )
                    .extent(ash::vk::Extent3D {
                        width: extent.width as u32,
                        height: extent.height as u32,
                        depth: 1,
                    });

                sess.vk_device.cmd_copy_image(
                    sess.vk_command_buffer,
                    src_image,
                    ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    dst_image,
                    ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[region],
                );

                // src: TRANSFER_SRC_OPTIMAL -> COLOR_ATTACHMENT_OPTIMAL (ready for next frame)
                let barrier_src_back = ash::vk::ImageMemoryBarrier::default()
                    .old_layout(ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .new_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .src_access_mask(ash::vk::AccessFlags::TRANSFER_READ)
                    .dst_access_mask(ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .image(src_image)
                    .subresource_range(src_range);

                // dst: TRANSFER_DST_OPTIMAL -> COLOR_ATTACHMENT_OPTIMAL (common OpenXR expectation)
                let barrier_dst_back = ash::vk::ImageMemoryBarrier::default()
                    .old_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .src_access_mask(ash::vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .image(dst_image)
                    .subresource_range(dst_range);

                sess.vk_device.cmd_pipeline_barrier(
                    sess.vk_command_buffer,
                    ash::vk::PipelineStageFlags::TRANSFER,
                    ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    ash::vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier_src_back, barrier_dst_back],
                );
            }

            sess.vk_device.end_command_buffer(sess.vk_command_buffer)?;

            let command_buffers = [sess.vk_command_buffer];
            let submit_info = ash::vk::SubmitInfo::default().command_buffers(&command_buffers);
            sess.vk_device
                .queue_submit(sess.vk_queue, &[submit_info], ash::vk::Fence::null())?;
            sess.vk_device.queue_wait_idle(sess.vk_queue)?;
        }

        if let Some(slot) = sess.swapchain_image_initialized.get_mut(image_index) {
            *slot = true;
        }

        Ok(())
    }
}
