use crate::engine::ecs::component::{
    CameraXRComponent, ControllerHand, ControllerPoseKind, InputVRComponent, VRHandComponent,
    VrComponent,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::vr_backend::{VrBackend, VrBackendKind};
use crate::engine::ecs::system::vr_types::{XrGamepadState, XrHandGamepadState, XrInputState};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::{CameraData, VisualWorld, VulkanoRenderer, XrVulkanGraphics};
use crate::engine::user_input::InputState;
use crate::utils::math;

use ash::vk::Handle as _;
use openvr_sys as ovr_sys;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::mem::{self, MaybeUninit};
use std::time::Instant;

const OPENVR_AXIS_NONE: i32 = 0;
const OPENVR_AXIS_TRACKPAD: i32 = 1;
const OPENVR_AXIS_JOYSTICK: i32 = 2;
const OPENVR_AXIS_TRIGGER: i32 = 3;
const OPENVR_TRIGGER_PRESS_THRESHOLD: f32 = 0.7;

#[derive(Clone, Copy, Debug, Default)]
struct OpenVrAxisBindings {
    thumbstick_axis: Option<usize>,
    trigger_axis: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default)]
struct ControllerPoseCache {
    left: Option<[[f32; 4]; 4]>,
    right: Option<[[f32; 4]; 4]>,
}

struct OpenVRState {
    runtime_initialized: bool,
    system: &'static ovr_sys::VR_IVRSystem_FnTable,
    compositor: &'static ovr_sys::VR_IVRCompositor_FnTable,
    overlay: &'static ovr_sys::VR_IVROverlay_FnTable,
    overlay_handle: ovr_sys::VROverlayHandle_t,
    axis_bindings: HashMap<u32, OpenVrAxisBindings>,
    recommended_render_target_size: [u32; 2],
    head_pose: Option<[[f32; 4]; 4]>,
    controller_pose_cache: ControllerPoseCache,
}

impl OpenVRState {
    fn new(
        system: &'static ovr_sys::VR_IVRSystem_FnTable,
        compositor: &'static ovr_sys::VR_IVRCompositor_FnTable,
        overlay: &'static ovr_sys::VR_IVROverlay_FnTable,
        overlay_handle: ovr_sys::VROverlayHandle_t,
    ) -> Self {
        Self {
            runtime_initialized: true,
            system,
            compositor,
            overlay,
            overlay_handle,
            axis_bindings: HashMap::new(),
            recommended_render_target_size: [0, 0],
            head_pose: None,
            controller_pose_cache: ControllerPoseCache::default(),
        }
    }
}

impl Drop for OpenVRState {
    fn drop(&mut self) {
        unsafe {
            let _ = self.overlay.DestroyOverlay.unwrap()(self.overlay_handle);
            if self.runtime_initialized {
                ovr_sys::VR_ShutdownInternal();
            }
        }
    }
}

pub struct OpenVRSystem {
    state: Option<OpenVRState>,
    last_init_error: Option<String>,
    vulkan_graphics: Option<XrVulkanGraphics>,
    input_xr_components: HashSet<ComponentId>,
    controller_components: HashSet<ComponentId>,
    last_init_attempt_instant: Option<Instant>,
    last_render_instant: Option<Instant>,
    last_render_dt_sec: Option<f32>,
    did_log_first_submit: bool,
    last_render_skip_reason: Option<&'static str>,
    xr_input_state: XrInputState,
    xr_gamepad_state: XrGamepadState,
}

impl Default for OpenVRSystem {
    fn default() -> Self {
        Self {
            state: None,
            last_init_error: None,
            vulkan_graphics: None,
            input_xr_components: HashSet::new(),
            controller_components: HashSet::new(),
            last_init_attempt_instant: None,
            last_render_instant: None,
            last_render_dt_sec: None,
            did_log_first_submit: false,
            last_render_skip_reason: None,
            xr_input_state: XrInputState::default(),
            xr_gamepad_state: XrGamepadState::default(),
        }
    }
}

impl std::fmt::Debug for OpenVRSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenVRSystem")
            .field("state_initialized", &self.state.is_some())
            .field("last_init_error", &self.last_init_error)
            .field("input_xr_components", &self.input_xr_components)
            .field("controller_components", &self.controller_components)
            .field("xr_input_state", &self.xr_input_state)
            .field("xr_gamepad_state", &self.xr_gamepad_state)
            .finish()
    }
}

impl OpenVRSystem {
    fn load_interface<T>(version: &[u8]) -> Result<&'static T, String> {
        let version = CStr::from_bytes_with_nul(version)
            .map_err(|err| format!("OpenVR interface version decode failed: {err}"))?;
        let mut magic = b"FnTable:".to_vec();
        magic.extend_from_slice(version.to_bytes());
        magic.push(0);

        let mut error = ovr_sys::EVRInitError_VRInitError_None;
        let interface = unsafe { ovr_sys::VR_GetGenericInterface(magic.as_ptr().cast(), &mut error) as *const T };
        if error != ovr_sys::EVRInitError_VRInitError_None || interface.is_null() {
            return Err(format!(
                "OpenVR interface load failed for {}: {error:?}",
                version.to_string_lossy()
            ));
        }

        Ok(unsafe { &*interface })
    }

    fn load_system_interface() -> Result<&'static ovr_sys::VR_IVRSystem_FnTable, String> {
        Self::load_interface(ovr_sys::IVRSystem_Version)
    }

    fn load_compositor_interface() -> Result<&'static ovr_sys::VR_IVRCompositor_FnTable, String> {
        Self::load_interface(ovr_sys::IVRCompositor_Version)
    }

    fn load_overlay_interface() -> Result<&'static ovr_sys::VR_IVROverlay_FnTable, String> {
        Self::load_interface(ovr_sys::IVROverlay_Version)
    }

    fn get_string<F: FnMut(*mut std::os::raw::c_char, u32) -> u32>(mut f: F) -> Option<CString> {
        let n = f(std::ptr::null_mut(), 0);
        if n == 0 {
            return None;
        }

        let mut storage: Vec<u8> = Vec::with_capacity(n as usize);
        unsafe {
            storage.set_len(n as usize);
        }

        let n2 = f(storage.as_mut_ptr().cast(), n);
        if n2 != n {
            return None;
        }

        storage.truncate((n - 1) as usize);
        Some(unsafe { CString::from_vec_unchecked(storage) })
    }

    fn compositor_vulkan_instance_extensions_required(
        compositor: &'static ovr_sys::VR_IVRCompositor_FnTable,
    ) -> Vec<CString> {
        let temp = match Self::get_string(|ptr, n| unsafe {
            compositor.GetVulkanInstanceExtensionsRequired.unwrap()(ptr, n)
        }) {
            Some(x) => x,
            None => return Vec::new(),
        };
        temp.as_bytes()
            .split(|&x| x == b' ')
            .map(|x| CString::new(x.to_vec()).expect("extension name contained null byte"))
            .collect()
    }

    fn compositor_vulkan_device_extensions_required(
        compositor: &'static ovr_sys::VR_IVRCompositor_FnTable,
        physical_device: *mut openvr::VkPhysicalDevice_T,
    ) -> Vec<CString> {
        let temp = match Self::get_string(|ptr, n| unsafe {
            compositor.GetVulkanDeviceExtensionsRequired.unwrap()(physical_device.cast(), ptr, n)
        }) {
            Some(x) => x,
            None => return Vec::new(),
        };
        temp.as_bytes()
            .split(|&x| x == b' ')
            .map(|x| CString::new(x.to_vec()).expect("extension name contained null byte"))
            .collect()
    }

    fn system_recommended_render_target_size(
        system: &'static ovr_sys::VR_IVRSystem_FnTable,
    ) -> (u32, u32) {
        let mut width = MaybeUninit::<u32>::uninit();
        let mut height = MaybeUninit::<u32>::uninit();
        unsafe {
            system.GetRecommendedRenderTargetSize.unwrap()(width.as_mut_ptr(), height.as_mut_ptr());
            (width.assume_init(), height.assume_init())
        }
    }

    fn system_projection_matrix(
        system: &'static ovr_sys::VR_IVRSystem_FnTable,
        eye: openvr::Eye,
        near_z: f32,
        far_z: f32,
    ) -> [[f32; 4]; 4] {
        unsafe { system.GetProjectionMatrix.unwrap()(eye as ovr_sys::EVREye, near_z, far_z) }.m
    }

    fn system_eye_to_head_transform(
        system: &'static ovr_sys::VR_IVRSystem_FnTable,
        eye: openvr::Eye,
    ) -> [[f32; 4]; 3] {
        unsafe { system.GetEyeToHeadTransform.unwrap()(eye as ovr_sys::EVREye) }.m
    }

    fn system_device_to_absolute_tracking_pose(
        system: &'static ovr_sys::VR_IVRSystem_FnTable,
        origin: openvr::TrackingUniverseOrigin,
        predicted_seconds_to_photons_from_now: f32,
    ) -> openvr::TrackedDevicePoses {
        let mut result = MaybeUninit::<openvr::TrackedDevicePoses>::uninit();
        unsafe {
            system.GetDeviceToAbsoluteTrackingPose.unwrap()(
                origin as ovr_sys::ETrackingUniverseOrigin,
                predicted_seconds_to_photons_from_now,
                result.as_mut_ptr().cast(),
                openvr::MAX_TRACKED_DEVICE_COUNT as u32,
            );
            result.assume_init()
        }
    }

    fn system_tracked_device_index_for_controller_role(
        system: &'static ovr_sys::VR_IVRSystem_FnTable,
        role: openvr::TrackedControllerRole,
    ) -> Option<openvr::TrackedDeviceIndex> {
        let index =
            unsafe { system.GetTrackedDeviceIndexForControllerRole.unwrap()(role as ovr_sys::ETrackedControllerRole) };
        let index = openvr::TrackedDeviceIndex(index);
        if index == openvr::tracked_device_index::INVALID {
            None
        } else {
            Some(index)
        }
    }

    fn system_int32_tracked_device_property(
        system: &'static ovr_sys::VR_IVRSystem_FnTable,
        device: openvr::TrackedDeviceIndex,
        property: openvr::TrackedDeviceProperty,
    ) -> Result<i32, openvr::system::TrackedPropertyError> {
        let mut error = MaybeUninit::<openvr::system::TrackedPropertyError>::uninit();
        let value = unsafe {
            system.GetInt32TrackedDeviceProperty.unwrap()(
                device.0,
                property.0,
                error.as_mut_ptr().cast(),
            )
        };
        let error = unsafe { error.assume_init() };
        if error == openvr::system::tracked_property_error::SUCCESS {
            Ok(value)
        } else {
            Err(error)
        }
    }

    fn system_controller_state(
        system: &'static ovr_sys::VR_IVRSystem_FnTable,
        device: openvr::TrackedDeviceIndex,
    ) -> Option<openvr::ControllerState> {
        let mut state = MaybeUninit::<openvr::ControllerState>::uninit();
        let ok = unsafe {
            system.GetControllerState.unwrap()(
                device.0,
                state.as_mut_ptr().cast(),
                mem::size_of::<openvr::ControllerState>() as u32,
            )
        };
        if ok {
            Some(unsafe { state.assume_init() })
        } else {
            None
        }
    }

    fn overlay_relative_to_hmd_transform() -> ovr_sys::HmdMatrix34_t {
        ovr_sys::HmdMatrix34_t {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, -1.2],
            ],
        }
    }

    fn create_overlay_handle(
        overlay: &'static ovr_sys::VR_IVROverlay_FnTable,
    ) -> Result<ovr_sys::VROverlayHandle_t, String> {
        let key = CString::new(format!(
            "cat-engine.openvr.overlay.{}",
            std::process::id()
        ))
        .map_err(|err| format!("overlay key creation failed: {err}"))?;
        let friendly =
            CString::new("Cat Engine OpenVR").map_err(|err| format!("overlay name failed: {err}"))?;
        let mut handle = ovr_sys::VROverlayHandle_t::default();

        let err = unsafe {
            overlay.CreateOverlay.unwrap()(
                key.as_ptr().cast_mut(),
                friendly.as_ptr().cast_mut(),
                &mut handle,
            )
        };
        if err != ovr_sys::EVROverlayError_VROverlayError_None {
            return Err(format!("CreateOverlay failed: {err:?}"));
        }

        let err = unsafe { overlay.SetOverlayAlpha.unwrap()(handle, 1.0) };
        if err != ovr_sys::EVROverlayError_VROverlayError_None {
            return Err(format!("SetOverlayAlpha failed: {err:?}"));
        }
        let err = unsafe { overlay.SetOverlayWidthInMeters.unwrap()(handle, 1.4) };
        if err != ovr_sys::EVROverlayError_VROverlayError_None {
            return Err(format!("SetOverlayWidthInMeters failed: {err:?}"));
        }
        let err = unsafe { overlay.SetOverlayCurvature.unwrap()(handle, 0.0) };
        if err != ovr_sys::EVROverlayError_VROverlayError_None {
            return Err(format!("SetOverlayCurvature failed: {err:?}"));
        }

        let transform = Self::overlay_relative_to_hmd_transform();
        let err = unsafe {
            overlay.SetOverlayTransformTrackedDeviceRelative.unwrap()(
                handle,
                openvr::tracked_device_index::HMD.0,
                (&raw const transform).cast_mut(),
            )
        };
        if err != ovr_sys::EVROverlayError_VROverlayError_None {
            return Err(format!(
                "SetOverlayTransformTrackedDeviceRelative failed: {err:?}"
            ));
        }

        let bounds = ovr_sys::VRTextureBounds_t {
            uMin: 0.0,
            vMin: 0.0,
            uMax: 1.0,
            vMax: 1.0,
        };
        let err = unsafe {
            overlay.SetOverlayTextureBounds.unwrap()(
                handle,
                (&raw const bounds).cast_mut(),
            )
        };
        if err != ovr_sys::EVROverlayError_VROverlayError_None {
            return Err(format!("SetOverlayTextureBounds failed: {err:?}"));
        }

        let err = unsafe { overlay.ShowOverlay.unwrap()(handle) };
        if err != ovr_sys::EVROverlayError_VROverlayError_None {
            return Err(format!("ShowOverlay failed: {err:?}"));
        }

        Ok(handle)
    }

    fn log_render_skip_reason(&mut self, reason: &'static str) {
        if self.last_render_skip_reason != Some(reason) {
            self.last_render_skip_reason = Some(reason);
            eprintln!("[OpenVR] render_xr skipped: {reason}");
        }
    }

    fn clear_render_skip_reason(&mut self) {
        self.last_render_skip_reason = None;
    }

    pub fn initialize_runtime(&mut self) -> Result<(), String> {
        if self.state.is_some() {
            return Ok(());
        }

        self.last_init_attempt_instant = Some(Instant::now());

        if unsafe { !ovr_sys::VR_IsRuntimeInstalled() } {
            let err = "OpenVR runtime is not installed".to_string();
            self.last_init_error = Some(err.clone());
            return Err(err);
        }

        let mut error = ovr_sys::EVRInitError_VRInitError_None;
        unsafe {
            ovr_sys::VR_InitInternal(
                &mut error,
                ovr_sys::EVRApplicationType_VRApplication_Overlay,
            );
        }
        if error != ovr_sys::EVRInitError_VRInitError_None {
            let err = format!("VR_InitInternal failed: {error:?}");
            self.last_init_error = Some(err.clone());
            return Err(err);
        }

        let system = match Self::load_system_interface() {
            Ok(system) => system,
            Err(err) => {
                unsafe { ovr_sys::VR_ShutdownInternal() };
                self.last_init_error = Some(err.clone());
                return Err(err);
            }
        };
        let compositor = match Self::load_compositor_interface() {
            Ok(compositor) => compositor,
            Err(err) => {
                unsafe { ovr_sys::VR_ShutdownInternal() };
                self.last_init_error = Some(err.clone());
                return Err(err);
            }
        };
        let overlay = Self::load_overlay_interface()?;
        let overlay_handle = Self::create_overlay_handle(overlay)?;
        unsafe {
            compositor.SetTrackingSpace.unwrap()(
                ovr_sys::ETrackingUniverseOrigin_TrackingUniverseStanding,
            );
        }

        println!(
            "[OpenVR] Initialized (hmd_present={})",
            unsafe { ovr_sys::VR_IsHmdPresent() }
        );
        let mut state = OpenVRState::new(system, compositor, overlay, overlay_handle);
        let (width, height) = Self::system_recommended_render_target_size(state.system);
        state.recommended_render_target_size = [width, height];
        self.state = Some(state);
        self.last_init_error = None;
        Ok(())
    }

    pub fn last_init_error(&self) -> Option<&str> {
        self.last_init_error.as_deref()
    }

    pub fn xr_input_state(&self) -> &XrInputState {
        &self.xr_input_state
    }

    pub fn xr_gamepad_state(&self) -> &XrGamepadState {
        &self.xr_gamepad_state
    }

    pub fn set_preferred_swapchain_format(&mut self, _format: u32) {}

    pub fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        let state = self.state.as_ref()?;
        let instance = Self::compositor_vulkan_instance_extensions_required(state.compositor)
            .into_iter()
            .filter_map(|value| value.into_string().ok())
            .collect::<Vec<_>>();
        let device = Self::compositor_vulkan_device_extensions_required(
            state.compositor,
            self.vulkan_graphics?
                .vk_physical_device
                .cast_mut()
                .cast::<openvr::VkPhysicalDevice_T>(),
        )
        .into_iter()
        .filter_map(|value| value.into_string().ok())
        .collect::<Vec<_>>();
        Some((instance, device))
    }

    pub fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        self.vulkan_graphics = Some(gfx);
    }

    pub fn register_vr(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(cfg) = world.get_component_by_id_as::<VrComponent>(component) else {
            return;
        };

        if !cfg.enabled || self.state.is_some() {
            return;
        }

        let previous_error = self.last_init_error.clone();
        if let Err(err) = self.initialize_runtime() {
            let changed = previous_error.as_deref() != Some(err.as_str());
            if changed {
                eprintln!("[OpenVR] initialize_runtime failed in register_vr: {err}");
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

    pub fn register_input_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.input_xr_components.insert(component);
    }

    pub fn remove_controller_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.controller_components.remove(&component);
    }

    pub fn remove_input_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.input_xr_components.remove(&component);
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

    fn input_xr_ancestor(world: &World, cid: ComponentId) -> Option<ComponentId> {
        let mut cur = cid;
        loop {
            if world
                .get_component_by_id_as::<InputVRComponent>(cur)
                .is_some()
            {
                return Some(cur);
            }
            let parent = world.parent_of(cur)?;
            cur = parent;
        }
    }

    fn transform_child_of(world: &World, component: ComponentId) -> Option<ComponentId> {
        world.children_of(component).iter().copied().find(|&cid| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(cid)
                .is_some()
        })
    }

    fn transform_parent_world(world: &World, transform_cid: ComponentId) -> [[f32; 4]; 4] {
        world
            .parent_of(transform_cid)
            .and_then(|parent| TransformSystem::world_model(world, parent))
            .unwrap_or([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
    }

    fn xr_rig_origin_world(world: &World, visuals: &VisualWorld) -> [[f32; 4]; 4] {
        let Some(camera_cid) = visuals
            .active_xr_camera()
            .or_else(|| Self::first_enabled_camera_xr(world))
        else {
            return [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
        };

        if let Some(input_xr_cid) = Self::input_xr_ancestor(world, camera_cid) {
            if let Some(driven_transform) = Self::transform_child_of(world, input_xr_cid) {
                return Self::transform_parent_world(world, driven_transform);
            }
        }

        TransformSystem::world_model(world, camera_cid).unwrap_or([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
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

    fn matrix4_from_openvr_pose(matrix: &[[f32; 4]; 3]) -> [[f32; 4]; 4] {
        [
            [matrix[0][0], matrix[1][0], matrix[2][0], 0.0],
            [matrix[0][1], matrix[1][1], matrix[2][1], 0.0],
            [matrix[0][2], matrix[1][2], matrix[2][2], 0.0],
            [matrix[0][3], matrix[1][3], matrix[2][3], 1.0],
        ]
    }

    fn matrix4_from_openvr_proj(matrix: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
        [
            [matrix[0][0], matrix[1][0], matrix[2][0], matrix[3][0]],
            [matrix[0][1], matrix[1][1], matrix[2][1], matrix[3][1]],
            [matrix[0][2], matrix[1][2], matrix[2][2], matrix[3][2]],
            [matrix[0][3], matrix[1][3], matrix[2][3], matrix[3][3]],
        ]
    }

    fn invert_affine_transform(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
        let c0 = [m[0][0], m[0][1], m[0][2]];
        let c1 = [m[1][0], m[1][1], m[1][2]];
        let c2 = [m[2][0], m[2][1], m[2][2]];

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
        let i00 = (a11 * a22 - a12 * a21) * inv_det;
        let i01 = (a02 * a21 - a01 * a22) * inv_det;
        let i02 = (a01 * a12 - a02 * a11) * inv_det;
        let i10 = (a12 * a20 - a10 * a22) * inv_det;
        let i11 = (a00 * a22 - a02 * a20) * inv_det;
        let i12 = (a02 * a10 - a00 * a12) * inv_det;
        let i20 = (a10 * a21 - a11 * a20) * inv_det;
        let i21 = (a01 * a20 - a00 * a21) * inv_det;
        let i22 = (a00 * a11 - a01 * a10) * inv_det;

        let t = [m[3][0], m[3][1], m[3][2]];
        let it = [
            -(i00 * t[0] + i01 * t[1] + i02 * t[2]),
            -(i10 * t[0] + i11 * t[1] + i12 * t[2]),
            -(i20 * t[0] + i21 * t[1] + i22 * t[2]),
        ];

        [
            [i00, i10, i20, 0.0],
            [i01, i11, i21, 0.0],
            [i02, i12, i22, 0.0],
            [it[0], it[1], it[2], 1.0],
        ]
    }

    fn transform_from_matrix_world(
        m: [[f32; 4]; 4],
    ) -> crate::engine::graphics::primitives::Transform {
        let mut t = crate::engine::graphics::primitives::Transform::default();
        t.model = m;
        t.matrix_world = m;
        t
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
                    let p_local = math::mat4_mul_vec4(
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
                return Some(math::mat_to_quat(t.transform.matrix_world));
            }
            cur = parent;
        }
        None
    }

    fn apply_pose_to_transform(
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        transform_cid: ComponentId,
        desired_world: [[f32; 4]; 4],
    ) {
        let desired_world_pos = [
            desired_world[3][0],
            desired_world[3][1],
            desired_world[3][2],
        ];
        let desired_world_rot = math::mat_to_quat(desired_world);
        let local_translation =
            Self::world_to_local_translation(world, transform_cid, desired_world_pos);
        let parent_world_rot =
            Self::parent_world_rotation_quat(world, transform_cid).unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let local_rotation =
            math::quat_mul(math::quat_conjugate(parent_world_rot), desired_world_rot);

        let Some(t) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(
                transform_cid,
            )
        else {
            return;
        };

        t.transform.translation = local_translation;
        t.transform.rotation = local_rotation;
        t.transform.recompute_model();

        let transform = t.transform;
        emit.push_intent_now(
            transform_cid,
            IntentValue::UpdateTransform {
                component_ids: vec![transform_cid],
                translation: transform.translation,
                rotation_quat_xyzw: transform.rotation,
                scale: transform.scale,
            },
        );
    }

    fn button_mask(button: u32) -> u64 {
        1u64 << button
    }

    fn resolve_axis_bindings(state: &mut OpenVRState, device: openvr::TrackedDeviceIndex) -> OpenVrAxisBindings {
        if let Some(bindings) = state.axis_bindings.get(&device.0).copied() {
            return bindings;
        }

        let axis_type_properties = [
            openvr::property::Axis0Type_Int32,
            openvr::property::Axis1Type_Int32,
            openvr::property::Axis2Type_Int32,
            openvr::property::Axis3Type_Int32,
            openvr::property::Axis4Type_Int32,
        ];

        let mut bindings = OpenVrAxisBindings::default();
        for (axis_index, property) in axis_type_properties.into_iter().enumerate() {
            let Ok(axis_type) = Self::system_int32_tracked_device_property(state.system, device, property) else {
                continue;
            };

            match axis_type {
                OPENVR_AXIS_JOYSTICK if bindings.thumbstick_axis.is_none() => {
                    bindings.thumbstick_axis = Some(axis_index);
                }
                OPENVR_AXIS_TRACKPAD if bindings.thumbstick_axis.is_none() => {
                    bindings.thumbstick_axis = Some(axis_index);
                }
                OPENVR_AXIS_TRIGGER if bindings.trigger_axis.is_none() => {
                    bindings.trigger_axis = Some(axis_index);
                }
                OPENVR_AXIS_NONE | OPENVR_AXIS_TRACKPAD | OPENVR_AXIS_JOYSTICK | OPENVR_AXIS_TRIGGER => {}
                _ => {}
            }
        }

        state.axis_bindings.insert(device.0, bindings);
        bindings
    }

    fn gamepad_hand_state(
        state: &mut OpenVRState,
        device: openvr::TrackedDeviceIndex,
        hand: ControllerHand,
    ) -> (XrHandGamepadState, bool) {
        let Some(controller_state) = Self::system_controller_state(state.system, device) else {
            return (XrHandGamepadState::default(), false);
        };

        let bindings = Self::resolve_axis_bindings(state, device);
        let trigger_value = bindings
            .trigger_axis
            .map(|axis| controller_state.axis[axis].x.clamp(0.0, 1.0));
        let trigger_button_down =
            (controller_state.button_pressed & Self::button_mask(openvr::button_id::STEAM_VR_TRIGGER))
                != 0;
        let trigger_pressed =
            trigger_value.map(|value| (trigger_button_down || value >= OPENVR_TRIGGER_PRESS_THRESHOLD, value));

        let grip_down = (controller_state.button_pressed & Self::button_mask(openvr::button_id::GRIP)) != 0;
        let a_button = (controller_state.button_pressed & Self::button_mask(openvr::button_id::A)) != 0;
        let app_button =
            (controller_state.button_pressed & Self::button_mask(openvr::button_id::APPLICATION_MENU)) != 0;

        let mut hand_state = XrHandGamepadState {
            thumbstick: bindings
                .thumbstick_axis
                .map(|axis| [controller_state.axis[axis].x, controller_state.axis[axis].y]),
            trigger_value,
            trigger_pressed,
            grip_value: Some(if grip_down { 1.0 } else { 0.0 }),
            grip_pressed: Some((grip_down, if grip_down { 1.0 } else { 0.0 })),
            button_a: None,
            button_b: None,
            button_x: None,
            button_y: None,
        };

        match hand {
            ControllerHand::Left => {
                hand_state.button_x = Some((a_button, if a_button { 1.0 } else { 0.0 }));
                hand_state.button_y = Some((app_button, if app_button { 1.0 } else { 0.0 }));
            }
            ControllerHand::Right => {
                hand_state.button_a = Some((a_button, if a_button { 1.0 } else { 0.0 }));
                hand_state.button_b = Some((app_button, if app_button { 1.0 } else { 0.0 }));
            }
        }

        (hand_state, true)
    }

    fn sync_runtime_state_from_poses(
        &mut self,
        poses: &openvr::TrackedDevicePoses,
    ) {
        let Some(state) = self.state.as_mut() else {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            return;
        };

        state.head_pose = None;
        state.controller_pose_cache = ControllerPoseCache::default();

        let hmd_pose = poses[openvr::tracked_device_index::HMD.0 as usize];
        if hmd_pose.pose_is_valid() {
            state.head_pose = Some(Self::matrix4_from_openvr_pose(
                hmd_pose.device_to_absolute_tracking(),
            ));
        }

        let left_device = state
            .system;
        let left_device =
            Self::system_tracked_device_index_for_controller_role(left_device, openvr::TrackedControllerRole::LeftHand);
        let right_device = state
            .system;
        let right_device =
            Self::system_tracked_device_index_for_controller_role(right_device, openvr::TrackedControllerRole::RightHand);

        if let Some(device) = left_device {
            let pose = poses[device.0 as usize];
            if pose.pose_is_valid() {
                state.controller_pose_cache.left =
                    Some(Self::matrix4_from_openvr_pose(pose.device_to_absolute_tracking()));
            }
        }

        if let Some(device) = right_device {
            let pose = poses[device.0 as usize];
            if pose.pose_is_valid() {
                state.controller_pose_cache.right =
                    Some(Self::matrix4_from_openvr_pose(pose.device_to_absolute_tracking()));
            }
        }

        let prev_trigger_down = self.xr_input_state.trigger_down;
        self.xr_input_state = XrInputState::default();
        self.xr_gamepad_state = XrGamepadState {
            active: state.head_pose.is_some(),
            hands: [XrHandGamepadState::default(), XrHandGamepadState::default()],
            head_pose_rotation: state.head_pose.map(math::mat_to_quat),
        };

        let left_trigger_down = left_device
            .and_then(|device| {
                let (hand_state, active) = Self::gamepad_hand_state(state, device, ControllerHand::Left);
                self.xr_gamepad_state.hands[0] = hand_state;
                if active {
                    self.xr_gamepad_state.active = true;
                }
                hand_state.trigger_pressed.map(|(down, _)| down)
            })
            .unwrap_or(false);
        let right_trigger_down = right_device
            .and_then(|device| {
                let (hand_state, active) =
                    Self::gamepad_hand_state(state, device, ControllerHand::Right);
                self.xr_gamepad_state.hands[1] = hand_state;
                if active {
                    self.xr_gamepad_state.active = true;
                }
                hand_state.trigger_pressed.map(|(down, _)| down)
            })
            .unwrap_or(false);

        self.xr_input_state.trigger_down = [left_trigger_down, right_trigger_down];
        for hand in 0..2 {
            self.xr_input_state.trigger_pressed[hand] =
                self.xr_input_state.trigger_down[hand] && !prev_trigger_down[hand];
            self.xr_input_state.trigger_released[hand] =
                !self.xr_input_state.trigger_down[hand] && prev_trigger_down[hand];
        }
    }

    fn sync_runtime_state(&mut self) {
        let Some(state) = self.state.as_mut() else {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            return;
        };

        let poses =
            Self::system_device_to_absolute_tracking_pose(state.system, openvr::TrackingUniverseOrigin::Standing, 0.0);
        self.sync_runtime_state_from_poses(&poses);
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        _input: &InputState,
        emit: &mut dyn SignalEmitter,
        _dt_sec: f32,
    ) {
        self.sync_runtime_state();
        visuals.set_xr_frame_dt_sec(None);

        let Some(state) = self.state.as_ref() else {
            return;
        };

        let rig_world = Self::xr_rig_origin_world(world, visuals);

        let input_xr_ids: Vec<ComponentId> = self.input_xr_components.iter().copied().collect();
        for input_xr_cid in input_xr_ids {
            let Some(cfg) = world.get_component_by_id_as::<InputVRComponent>(input_xr_cid) else {
                self.input_xr_components.remove(&input_xr_cid);
                continue;
            };

            if !cfg.enabled {
                continue;
            }

            let Some(head_pose) = state.head_pose else {
                continue;
            };
            let Some(transform_cid) = Self::transform_child_of(world, input_xr_cid) else {
                continue;
            };

            let desired_world = Self::mul_mat4(
                &Self::transform_parent_world(world, transform_cid),
                &head_pose,
            );
            Self::apply_pose_to_transform(world, emit, transform_cid, desired_world);
        }

        let controller_ids: Vec<ComponentId> = self.controller_components.iter().copied().collect();
        for controller_cid in controller_ids {
            let Some(cfg) = world.get_component_by_id_as::<VRHandComponent>(controller_cid) else {
                self.controller_components.remove(&controller_cid);
                continue;
            };

            if !cfg.enabled {
                continue;
            }

            let pose = match (cfg.hand, cfg.pose) {
                (ControllerHand::Left, ControllerPoseKind::Aim | ControllerPoseKind::Grip) => {
                    state.controller_pose_cache.left
                }
                (ControllerHand::Right, ControllerPoseKind::Aim | ControllerPoseKind::Grip) => {
                    state.controller_pose_cache.right
                }
            };

            let Some(pose) = pose else {
                continue;
            };
            let Some(transform_cid) = Self::transform_child_of(world, controller_cid) else {
                continue;
            };

            let desired_world = Self::mul_mat4(&rig_world, &pose);
            Self::apply_pose_to_transform(world, emit, transform_cid, desired_world);
        }
    }

    pub fn last_render_dt_sec(&self) -> Option<f32> {
        self.last_render_dt_sec
    }

    pub fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    ) {
        if self.state.is_none() {
            let should_retry = self
                .last_init_attempt_instant
                .map(|last| last.elapsed().as_secs_f32() >= 2.0)
                .unwrap_or(true);
            if should_retry {
                let previous_error = self.last_init_error.clone();
                if let Err(err) = self.initialize_runtime() {
                    let changed = previous_error.as_deref() != Some(err.as_str());
                    self.log_render_skip_reason("backend state not initialized");
                    if changed {
                        eprintln!("[OpenVR] initialize_runtime failed: {err}");
                    }
                }
            } else if self.last_init_error.is_some() {
                self.log_render_skip_reason("backend state not initialized");
            }
        }

        if self.state.is_none() {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            visuals.set_xr_frame_dt_sec(None);
            visuals.set_xr_camera(Vec::new());
            return;
        }
        let Some(gfx) = self.vulkan_graphics else {
            self.log_render_skip_reason("missing Vulkan graphics handles");
            visuals.set_xr_frame_dt_sec(None);
            return;
        };

        let now = Instant::now();
        let dt_sec = self
            .last_render_instant
            .map(|prev| now.saturating_duration_since(prev).as_secs_f32());
        self.last_render_instant = Some(now);
        if let Some(dt_sec) = dt_sec {
            self.last_render_dt_sec = Some(dt_sec);
            visuals.set_xr_frame_dt_sec(Some(dt_sec));
        }

        if visuals
            .active_xr_camera()
            .or_else(|| Self::first_enabled_camera_xr(world))
            .is_none()
        {
            self.log_render_skip_reason("no active CameraXR");
            visuals.set_xr_frame_dt_sec(None);
            visuals.set_xr_camera(Vec::new());
            return;
        }

        self.sync_runtime_state();

        let Some(state) = self.state.as_ref() else {
            self.log_render_skip_reason("backend state disappeared after pose sync");
            visuals.set_xr_camera(Vec::new());
            return;
        };
        let Some(head_pose) = state.head_pose else {
            self.log_render_skip_reason("missing valid HMD pose");
            visuals.set_xr_camera(Vec::new());
            return;
        };

        let rig_world = Self::xr_rig_origin_world(world, visuals);
        let world_from_head = Self::mul_mat4(&rig_world, &head_pose);
        let eye_to_head_left =
            Self::matrix4_from_openvr_pose(&Self::system_eye_to_head_transform(state.system, openvr::Eye::Left));
        let eye_to_head_right =
            Self::matrix4_from_openvr_pose(&Self::system_eye_to_head_transform(state.system, openvr::Eye::Right));
        let head_from_eye_left = Self::invert_affine_transform(&eye_to_head_left);
        let head_from_eye_right = Self::invert_affine_transform(&eye_to_head_right);
        let world_from_eye_left = Self::mul_mat4(&world_from_head, &head_from_eye_left);
        let world_from_eye_right = Self::mul_mat4(&world_from_head, &head_from_eye_right);
        let view_left = Self::invert_affine_transform(&world_from_eye_left);
        let view_right = Self::invert_affine_transform(&world_from_eye_right);
        let proj_left =
            Self::matrix4_from_openvr_proj(Self::system_projection_matrix(state.system, openvr::Eye::Left, 0.1, 100.0));
        let proj_right =
            Self::matrix4_from_openvr_proj(Self::system_projection_matrix(state.system, openvr::Eye::Right, 0.1, 100.0));

        visuals.set_xr_camera(vec![
            CameraData {
                view: view_left,
                proj: proj_left,
                transform: Self::transform_from_matrix_world(world_from_eye_left),
            },
            CameraData {
                view: view_right,
                proj: proj_right,
                transform: Self::transform_from_matrix_world(world_from_eye_right),
            },
        ]);

        let extent = state.recommended_render_target_size;
        if let Err(err) = renderer.render_xr_eye_offscreen(visuals, 0, extent) {
            eprintln!("[OpenVR] render_xr_eye_offscreen failed for overlay eye: {err}");
            return;
        }

        let Some(format) = renderer.window_vk_format_raw() else {
            self.log_render_skip_reason("missing XR offscreen Vulkan format");
            return;
        };
        let Some(left_image) = renderer.xr_offscreen_vk_image(0) else {
            self.log_render_skip_reason("missing left XR offscreen image");
            return;
        };
        let mut vk_texture = ovr_sys::VRVulkanTextureData_t {
            m_nImage: left_image.as_raw(),
            m_pDevice: gfx.vk_device.cast_mut().cast(),
            m_pPhysicalDevice: gfx.vk_physical_device.cast_mut().cast(),
            m_pInstance: gfx.vk_instance.cast_mut().cast(),
            m_pQueue: gfx.vk_queue.cast_mut().cast(),
            m_nQueueFamilyIndex: gfx.queue_family_index,
            m_nWidth: extent[0],
            m_nHeight: extent[1],
            m_nFormat: format,
            m_nSampleCount: 1,
        };
        let mut texture = ovr_sys::Texture_t {
            handle: (&raw mut vk_texture).cast(),
            eType: ovr_sys::ETextureType_TextureType_Vulkan,
            eColorSpace: ovr_sys::EColorSpace_ColorSpace_Auto,
        };
        let overlay_result = unsafe {
            state
                .overlay
                .SetOverlayTexture
                .unwrap()(state.overlay_handle, &mut texture)
        };
        if overlay_result != ovr_sys::EVROverlayError_VROverlayError_None {
            self.log_render_skip_reason("overlay texture upload failed");
            eprintln!("[OpenVR] SetOverlayTexture failed: {overlay_result:?}");
            return;
        }

        self.clear_render_skip_reason();
        if !self.did_log_first_submit {
            self.did_log_first_submit = true;
            eprintln!(
                "[OpenVR] uploaded Vulkan texture to HMD-relative overlay (extent={}x{}, format={})",
                extent[0], extent[1], format
            );
        }
    }
}

impl VrBackend for OpenVRSystem {
    fn kind(&self) -> VrBackendKind {
        VrBackendKind::OpenVR
    }

    fn initialize_runtime(&mut self) -> Result<(), String> {
        OpenVRSystem::initialize_runtime(self)
    }

    fn last_init_error(&self) -> Option<&str> {
        OpenVRSystem::last_init_error(self)
    }

    fn xr_input_state(&self) -> &XrInputState {
        OpenVRSystem::xr_input_state(self)
    }

    fn xr_gamepad_state(&self) -> &XrGamepadState {
        OpenVRSystem::xr_gamepad_state(self)
    }

    fn set_preferred_swapchain_format(&mut self, format: u32) {
        OpenVRSystem::set_preferred_swapchain_format(self, format)
    }

    fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        OpenVRSystem::required_vulkan_extensions(self)
    }

    fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        OpenVRSystem::set_vulkan_graphics(self, gfx)
    }

    fn register_vr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::register_vr(self, world, visuals, component)
    }

    fn register_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::register_controller_xr(self, world, visuals, component)
    }

    fn register_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::register_input_xr(self, world, visuals, component)
    }

    fn remove_controller_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::remove_controller_xr(self, world, visuals, component)
    }

    fn remove_input_xr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        OpenVRSystem::remove_input_xr(self, world, visuals, component)
    }

    fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        emit: &mut dyn SignalEmitter,
        dt_sec: f32,
    ) {
        OpenVRSystem::tick_with_queue(self, world, visuals, input, emit, dt_sec)
    }

    fn last_render_dt_sec(&self) -> Option<f32> {
        OpenVRSystem::last_render_dt_sec(self)
    }

    fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    ) {
        OpenVRSystem::render_xr(self, world, visuals, renderer)
    }

    fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        dt_sec: f32,
    ) {
        System::tick(self, world, visuals, input, dt_sec)
    }
}

impl System for OpenVRSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
    }
}
