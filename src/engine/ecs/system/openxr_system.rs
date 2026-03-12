use crate::engine::ecs::component::CameraXRComponent;
use crate::engine::ecs::component::OpenXRComponent;
use crate::engine::ecs::component::{ControllerHand, ControllerPoseKind, ControllerXRComponent};
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::CameraData;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::VulkanoRenderer;
use crate::engine::graphics::XRSwapchain;
use crate::engine::graphics::XrVulkanGraphics;
use crate::engine::graphics::xr_renderer;
use crate::engine::user_input::InputState;
use crate::utils::math;

use ash::vk::Handle as _;

use std::collections::HashSet;

pub struct OpenXRSystem {
    state: Option<OpenXRState>,
    last_init_error: Option<String>,
    vulkan_graphics: Option<XrVulkanGraphics>,
    preferred_swapchain_format: Option<u32>,

    controller_components: HashSet<ComponentId>,
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

    #[allow(dead_code)]
    vk_command_pool: ash::vk::CommandPool,
    vk_command_buffer: ash::vk::CommandBuffer,

    controller_input: Option<ControllerInput>,
    controller_pose_cache: ControllerPoseCache,
}

#[derive(Debug, Default, Clone, Copy)]
struct ControllerPoseCache {
    left_aim: Option<openxr::Posef>,
    right_aim: Option<openxr::Posef>,
    left_grip: Option<openxr::Posef>,
    right_grip: Option<openxr::Posef>,
}

#[allow(dead_code)]
struct ControllerInput {
    action_set: openxr::ActionSet,
    aim_pose: openxr::Action<openxr::Posef>,
    grip_pose: openxr::Action<openxr::Posef>,

    left: openxr::Path,
    right: openxr::Path,

    left_aim_space: openxr::Space,
    right_aim_space: openxr::Space,
    left_grip_space: openxr::Space,
    right_grip_space: openxr::Space,
}

impl Default for OpenXRSystem {
    fn default() -> Self {
        Self {
            state: None,
            last_init_error: None,
            vulkan_graphics: None,
            preferred_swapchain_format: None,

            controller_components: HashSet::new(),
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

    pub fn register_controller_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.controller_components.insert(component);
    }

    pub fn remove_controller_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.controller_components.remove(&component);
    }

    fn nearest_ancestor_transform(world: &World, start: ComponentId) -> Option<ComponentId> {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(start)
            .is_some()
        {
            return Some(start);
        }

        let mut cur = start;
        while let Some(parent) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
                .is_some()
            {
                return Some(parent);
            }
            cur = parent;
        }
        None
    }

    fn pump_events(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        // Drain events; for now we just print key session state transitions.
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
    }

    /// Like `tick`, but also queues transform updates for registered ControllerXRComponents.
    ///
    /// Controller poses are sourced from the last cache update performed during `render_xr`.
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        _input: &InputState,
        emit: &mut dyn SignalEmitter,
        _dt_sec: f32,
    ) {
        self.pump_events();

        let Some(state) = self.state.as_ref() else {
            return;
        };
        let Some(sess) = state.session.as_ref() else {
            return;
        };
        if !sess.running {
            return;
        }

        // Compose controller poses with the XR rig's world transform, matching `render_xr`.
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

        let cache = sess.controller_pose_cache;

        let controller_ids: Vec<ComponentId> = self.controller_components.iter().copied().collect();
        for controller_cid in controller_ids {
            let Some(cfg) = world.get_component_by_id_as::<ControllerXRComponent>(controller_cid)
            else {
                self.controller_components.remove(&controller_cid);
                continue;
            };

            if !cfg.enabled {
                continue;
            }

            let pose = match (cfg.hand, cfg.pose) {
                (ControllerHand::Left, ControllerPoseKind::Aim) => cache.left_aim,
                (ControllerHand::Right, ControllerPoseKind::Aim) => cache.right_aim,
                (ControllerHand::Left, ControllerPoseKind::Grip) => cache.left_grip,
                (ControllerHand::Right, ControllerPoseKind::Grip) => cache.right_grip,
            };

            let Some(pose) = pose else {
                continue;
            };

            let Some(tcid) = Self::nearest_ancestor_transform(world, controller_cid) else {
                continue;
            };

            // Pose is in OpenXR reference space; convert to engine world by applying rig transform.
            let world_from_controller = Self::mul_mat4(&rig_world, &Self::mat4_from_pose(pose));
            let desired_world_pos = [
                world_from_controller[3][0],
                world_from_controller[3][1],
                world_from_controller[3][2],
            ];
            let desired_world_rot = Self::quat_from_mat4(&world_from_controller);

            let local_translation =
                Self::world_to_local_translation(world, tcid, desired_world_pos);
            let parent_world_rot =
                Self::parent_world_rotation_quat(world, tcid).unwrap_or([0.0, 0.0, 0.0, 1.0]);
            let local_rotation =
                math::quat_mul(math::quat_conjugate(parent_world_rot), desired_world_rot);

            let Some(t) = world
                .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(
                    tcid,
                )
            else {
                continue;
            };

            // Convert world-space target into local-space values relative to the nearest parent
            // transform above `tcid` (if any), matching how transform chains are composed.
            t.transform.translation = local_translation;
            t.transform.rotation = local_rotation;
            t.transform.recompute_model();

            let transform = t.transform;
            emit.push_intent_now(
                tcid,
                IntentValue::UpdateTransform {
                    component_ids: vec![tcid],
                    translation: transform.translation,
                    rotation_quat_xyzw: transform.rotation,
                    scale: transform.scale,
                },
            );
        }
    }

    fn world_to_local_translation(
        world: &World,
        transform_cid: ComponentId,
        desired_world: [f32; 3],
    ) -> [f32; 3] {
        let mut cur = transform_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
            {
                if let Some(inv) = math::mat4_inverse(t.transform.matrix_world) {
                    let p_local = Self::mat4_mul_vec4(
                        inv,
                        [desired_world[0], desired_world[1], desired_world[2], 1.0],
                    );
                    return [p_local[0], p_local[1], p_local[2]];
                }
                break;
            }
            cur = parent;
        }

        desired_world
    }

    fn parent_world_rotation_quat(world: &World, transform_cid: ComponentId) -> Option<[f32; 4]> {
        let mut cur = transform_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
            {
                return Some(Self::quat_from_mat4(&t.transform.matrix_world));
            }
            cur = parent;
        }
        None
    }

    fn mat4_mul_vec4(m: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
        [
            m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2] + m[3][0] * v[3],
            m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2] + m[3][1] * v[3],
            m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2] + m[3][2] * v[3],
            m[0][3] * v[0] + m[1][3] * v[1] + m[2][3] * v[2] + m[3][3] * v[3],
        ]
    }

    /// Extract a unit quaternion from the rotation part of a column-major 4x4 matrix.
    ///
    /// Best-effort: normalizes basis vectors to remove uniform/non-uniform scale.
    fn quat_from_mat4(m: &[[f32; 4]; 4]) -> [f32; 4] {
        let mut x = [m[0][0], m[0][1], m[0][2]];
        let mut y = [m[1][0], m[1][1], m[1][2]];
        let mut z = [m[2][0], m[2][1], m[2][2]];

        let nx = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
        let ny = (y[0] * y[0] + y[1] * y[1] + y[2] * y[2]).sqrt();
        let nz = (z[0] * z[0] + z[1] * z[1] + z[2] * z[2]).sqrt();
        if nx > 0.0 {
            x[0] /= nx;
            x[1] /= nx;
            x[2] /= nx;
        }
        if ny > 0.0 {
            y[0] /= ny;
            y[1] /= ny;
            y[2] /= ny;
        }
        if nz > 0.0 {
            z[0] /= nz;
            z[1] /= nz;
            z[2] /= nz;
        }

        // Convert column-major basis into row-major rotation entries.
        let r00 = x[0];
        let r01 = y[0];
        let r02 = z[0];
        let r10 = x[1];
        let r11 = y[1];
        let r12 = z[1];
        let r20 = x[2];
        let r21 = y[2];
        let r22 = z[2];

        let trace = r00 + r11 + r22;
        let (qx, qy, qz, qw) = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            ((r21 - r12) / s, (r02 - r20) / s, (r10 - r01) / s, 0.25 * s)
        } else if r00 > r11 && r00 > r22 {
            let s = (1.0 + r00 - r11 - r22).sqrt() * 2.0;
            (0.25 * s, (r01 + r10) / s, (r02 + r20) / s, (r21 - r12) / s)
        } else if r11 > r22 {
            let s = (1.0 + r11 - r00 - r22).sqrt() * 2.0;
            ((r01 + r10) / s, 0.25 * s, (r12 + r21) / s, (r02 - r20) / s)
        } else {
            let s = (1.0 + r22 - r00 - r11).sqrt() * 2.0;
            ((r02 + r20) / s, (r12 + r21) / s, 0.25 * s, (r10 - r01) / s)
        };

        let len = (qx * qx + qy * qy + qz * qz + qw * qw).sqrt();
        if len > 0.0 {
            [qx / len, qy / len, qz / len, qw / len]
        } else {
            [0.0, 0.0, 0.0, 1.0]
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

        let controller_input = match Self::try_init_controller_input(&state.instance, &session) {
            Ok(v) => Some(v),
            Err(err) => {
                eprintln!("[OpenXR] Controller input init failed: {err}");
                None
            }
        };

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

            controller_input,
            controller_pose_cache: ControllerPoseCache::default(),
        });

        println!("[OpenXR] Session created (Vulkan)");
        Ok(())
    }

    fn try_init_controller_input(
        instance: &openxr::Instance,
        session: &openxr::Session<openxr::Vulkan>,
    ) -> Result<ControllerInput, String> {
        let left = instance
            .string_to_path("/user/hand/left")
            .map_err(|e| format!("string_to_path(/user/hand/left): {e:?}"))?;
        let right = instance
            .string_to_path("/user/hand/right")
            .map_err(|e| format!("string_to_path(/user/hand/right): {e:?}"))?;

        let action_set = instance
            .create_action_set("cat_engine", "Cat Engine", 0)
            .map_err(|e| format!("create_action_set: {e:?}"))?;

        let subaction_paths = [left, right];
        let aim_pose = action_set
            .create_action::<openxr::Posef>("aim_pose", "Aim Pose", &subaction_paths)
            .map_err(|e| format!("create_action(aim_pose): {e:?}"))?;
        let grip_pose = action_set
            .create_action::<openxr::Posef>("grip_pose", "Grip Pose", &subaction_paths)
            .map_err(|e| format!("create_action(grip_pose): {e:?}"))?;

        // Create spaces for each subaction path.
        let left_aim_space = aim_pose
            .create_space(session.clone(), left, openxr::Posef::IDENTITY)
            .map_err(|e| format!("aim_pose.create_space(left): {e:?}"))?;
        let right_aim_space = aim_pose
            .create_space(session.clone(), right, openxr::Posef::IDENTITY)
            .map_err(|e| format!("aim_pose.create_space(right): {e:?}"))?;
        let left_grip_space = grip_pose
            .create_space(session.clone(), left, openxr::Posef::IDENTITY)
            .map_err(|e| format!("grip_pose.create_space(left): {e:?}"))?;
        let right_grip_space = grip_pose
            .create_space(session.clone(), right, openxr::Posef::IDENTITY)
            .map_err(|e| format!("grip_pose.create_space(right): {e:?}"))?;

        // Attach the action set so sync_actions can be called.
        session
            .attach_action_sets(&[&action_set])
            .map_err(|e| format!("attach_action_sets: {e:?}"))?;

        // Best-effort bindings for common interaction profiles.
        // Runtimes will ignore profiles they don't support.
        let profiles = [
            "/interaction_profiles/khr/simple_controller",
            "/interaction_profiles/oculus/touch_controller",
            "/interaction_profiles/htc/vive_controller",
            "/interaction_profiles/valve/index_controller",
            "/interaction_profiles/microsoft/motion_controller",
        ];

        let left_aim_path = instance
            .string_to_path("/user/hand/left/input/aim/pose")
            .map_err(|e| format!("string_to_path(left aim): {e:?}"))?;
        let right_aim_path = instance
            .string_to_path("/user/hand/right/input/aim/pose")
            .map_err(|e| format!("string_to_path(right aim): {e:?}"))?;
        let left_grip_path = instance
            .string_to_path("/user/hand/left/input/grip/pose")
            .map_err(|e| format!("string_to_path(left grip): {e:?}"))?;
        let right_grip_path = instance
            .string_to_path("/user/hand/right/input/grip/pose")
            .map_err(|e| format!("string_to_path(right grip): {e:?}"))?;

        let bindings = [
            openxr::Binding::new(&aim_pose, left_aim_path),
            openxr::Binding::new(&aim_pose, right_aim_path),
            openxr::Binding::new(&grip_pose, left_grip_path),
            openxr::Binding::new(&grip_pose, right_grip_path),
        ];

        for profile_str in profiles {
            let Ok(profile) = instance.string_to_path(profile_str) else {
                continue;
            };
            // Not all runtimes support every profile; treat as best-effort.
            let _ = instance.suggest_interaction_profile_bindings(profile, &bindings);
        }

        Ok(ControllerInput {
            action_set,
            aim_pose,
            grip_pose,
            left,
            right,
            left_aim_space,
            right_aim_space,
            left_grip_space,
            right_grip_space,
        })
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

}

impl System for OpenXRSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        self.pump_events();
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

        // Update controller pose cache at the same predicted time as views.
        if let Some(ci) = sess.controller_input.as_ref() {
            // Sync actions (best-effort).
            let active = openxr::ActiveActionSet::new(&ci.action_set);
            let _ = sess.session.sync_actions(&[active]);

            let update_pose = |space: &openxr::Space, base: &openxr::Space, t: openxr::Time| {
                space.locate(base, t).ok()
            };

            if let Some(loc) = update_pose(
                &ci.left_aim_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            ) {
                if loc
                    .location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
                {
                    sess.controller_pose_cache.left_aim = Some(loc.pose);
                }
            }
            if let Some(loc) = update_pose(
                &ci.right_aim_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            ) {
                if loc
                    .location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
                {
                    sess.controller_pose_cache.right_aim = Some(loc.pose);
                }
            }
            if let Some(loc) = update_pose(
                &ci.left_grip_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            ) {
                if loc
                    .location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
                {
                    sess.controller_pose_cache.left_grip = Some(loc.pose);
                }
            }
            if let Some(loc) = update_pose(
                &ci.right_grip_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            ) {
                if loc
                    .location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
                {
                    sess.controller_pose_cache.right_grip = Some(loc.pose);
                }
            }
        }

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

                if let Err(e) = xr_renderer::clear_xr_swapchain_image(
                    &sess.vk_device,
                    sess.vk_queue,
                    sess.vk_command_buffer,
                    sess.xr_swapchain.view_count(),
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

                if let Err(e) = xr_renderer::copy_offscreen_to_xr_layers(
                    &sess.vk_device,
                    sess.vk_queue,
                    sess.vk_command_buffer,
                    &sess.xr_swapchain,
                    renderer,
                    dst_image,
                    dst_was_initialized,
                    view_count,
                ) {
                    eprintln!("[OpenXR] copy to XR image failed: {e:?}");
                } else if let Some(slot) =
                    sess.swapchain_image_initialized.get_mut(image_index_usize)
                {
                    *slot = true;
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

}
