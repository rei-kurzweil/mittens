//! Native eye capability detection and the XR_HTC_eye_tracker adapter.
//!
//! The host reports every eye-related extension it sees, but adapts only the
//! backend proven on a Vive Focus 3 with the eye tracker module:
//! `XR_HTC_eye_tracker` geometric data (per-eye openness), normalized into the
//! engine's provider-neutral `NativeEyeSample`. Gaze and pupil queries exist
//! in the same extension and are left for later. A missing extension, no
//! system support (module unplugged), a missing function or a failed tracker
//! creation is reported and leaves the session running without native eyes.
//!
//! The ABI is not in the `openxr` 0.19 bindings. It follows HTC's Unity
//! plugin (`ViveEyeTrackerHelper.cs`, `OpenXRUtils.cs` in
//! ViveSoftware/VIVE-OpenXR-Unity) as used by the ALVR fork's device-tested
//! probe (rei-kurzweil/wave-eye-tracking-notes; ALVR `feature/htc-eye-tracker`
//! 5b3c5f1). Info structs keep ALVR's trailing `time` field, which the tested
//! runtime accepted; HTC's own layout has only `type` and `next`.
use crate::xr_host::Report;
use mittens_engine::engine::ecs::system::external_xr_frame::NativeEyeSample;
use openxr as xr;
use std::ffi::{CStr, c_void};
use xr::sys::{Bool32, Result as XrResult, Session, Space, StructureType, Time};

const EXTENSION: &str = "XR_HTC_eye_tracker";
const CREATE_INFO: i32 = 1000326001;
const SYSTEM_PROPERTIES: i32 = 1000326005;
const GEOMETRIC_INFO: i32 = 1000326009;
const GEOMETRIC_DATA: i32 = 1000326010;

#[repr(transparent)]
#[derive(Clone, Copy)]
struct EyeTrackerHTC(u64);

#[repr(C)]
struct EyeTrackerCreateInfoHTC {
    ty: StructureType,
    next: *const c_void,
}

#[repr(C)]
struct SystemEyeTrackingPropertiesHTC {
    ty: StructureType,
    next: *mut c_void,
    supports_eye_tracking: Bool32,
}

#[repr(C)]
struct EyeGeometricDataInfoHTC {
    ty: StructureType,
    next: *const c_void,
    time: Time,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SingleEyeGeometricDataHTC {
    is_valid: Bool32,
    eye_openness: f32,
    eye_wide: f32,
    eye_squeeze: f32,
}

#[repr(C)]
struct EyeGeometricDataHTC {
    ty: StructureType,
    next: *mut c_void,
    time: Time,
    eyes: [SingleEyeGeometricDataHTC; 2],
}

type CreateEyeTracker =
    unsafe extern "system" fn(Session, *const EyeTrackerCreateInfoHTC, *mut EyeTrackerHTC) -> XrResult;
type DestroyEyeTracker = unsafe extern "system" fn(EyeTrackerHTC) -> XrResult;
type GetEyeGeometricData = unsafe extern "system" fn(
    EyeTrackerHTC,
    *const EyeGeometricDataInfoHTC,
    *mut EyeGeometricDataHTC,
) -> XrResult;

/// What the runtime and system advertise, gathered before session creation.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EyeCapabilities {
    pub eye_tracker_enabled: bool,
    pub supports_eye_tracking: bool,
}

/// Enable the extensions this host can adapt. Others are reported only.
/// `openxr` 0.19 appends the NUL to `other` names itself.
pub(crate) fn enable(available: &xr::ExtensionSet, extensions: &mut xr::ExtensionSet) {
    if available.other.iter().any(|name| name == EXTENSION) {
        extensions.other.push(EXTENSION.into());
    }
}

/// Query system support for the enabled backend and report all candidates.
pub(crate) fn probe(
    instance: &xr::Instance,
    system: xr::SystemId,
    available: &xr::ExtensionSet,
    enabled: &xr::ExtensionSet,
    report: &mut Report,
) -> EyeCapabilities {
    let mut capabilities = EyeCapabilities {
        eye_tracker_enabled: enabled.other.iter().any(|name| name == EXTENSION),
        ..Default::default()
    };
    let mut result = None;
    if capabilities.eye_tracker_enabled {
        let mut eye = SystemEyeTrackingPropertiesHTC {
            ty: StructureType::from_raw(SYSTEM_PROPERTIES),
            next: std::ptr::null_mut(),
            supports_eye_tracking: xr::sys::FALSE,
        };
        let status = unsafe {
            let mut properties: xr::sys::SystemProperties = std::mem::zeroed();
            properties.ty = xr::sys::SystemProperties::TYPE;
            properties.next = &mut eye as *mut _ as *mut c_void;
            (instance.fp().get_system_properties)(instance.as_raw(), system, &mut properties)
        };
        capabilities.supports_eye_tracking =
            status.into_raw() >= 0 && bool::from(eye.supports_eye_tracking);
        result = Some(status);
    }
    report.line(format!(
        "native eyes: {EXTENSION} advertised={} enabled={} xrGetSystemProperties={result:?} \
         supportsEyeTracking={}; XR_HTC_facial_tracking advertised={}; \
         XR_EXT_eye_gaze_interaction advertised={} (not adapted)",
        available.other.iter().any(|name| name == EXTENSION),
        capabilities.eye_tracker_enabled,
        capabilities.supports_eye_tracking,
        available.htc_facial_tracking,
        available.ext_eye_gaze_interaction,
    ));
    capabilities
}

unsafe fn lookup<T: Copy>(instance: &xr::Instance, name: &CStr, report: &mut Report) -> Option<T> {
    let mut function = None;
    let result = unsafe {
        (instance.entry().fp().get_instance_proc_addr)(
            instance.as_raw(),
            name.as_ptr(),
            &mut function,
        )
    };
    match function {
        Some(function) if result.into_raw() >= 0 => {
            // All HTC entry points are `extern "system"` function pointers.
            Some(unsafe { std::mem::transmute_copy::<xr::sys::pfn::VoidFunction, T>(&function) })
        }
        _ => {
            report.line(format!("native eyes: {name:?} unavailable: {result:?}"));
            None
        }
    }
}

/// A created eye tracker. Destroy it before its session.
pub(crate) struct NativeEyes {
    tracker: EyeTrackerHTC,
    destroy: DestroyEyeTracker,
    geometric: GetEyeGeometricData,
    active: Option<bool>,
}

impl NativeEyes {
    pub(crate) fn new<G>(
        instance: &xr::Instance,
        session: &xr::Session<G>,
        capabilities: EyeCapabilities,
        report: &mut Report,
    ) -> Option<Self> {
        if !capabilities.supports_eye_tracking {
            report.line("native eyes: no eye provider; XREyeTracking falls back to other sources");
            return None;
        }
        let (create, destroy, geometric) = unsafe {
            (
                lookup::<CreateEyeTracker>(instance, c"xrCreateEyeTrackerHTC", report)?,
                lookup::<DestroyEyeTracker>(instance, c"xrDestroyEyeTrackerHTC", report)?,
                lookup::<GetEyeGeometricData>(instance, c"xrGetEyeGeometricDataHTC", report)?,
            )
        };
        let info = EyeTrackerCreateInfoHTC {
            ty: StructureType::from_raw(CREATE_INFO),
            next: std::ptr::null(),
        };
        let mut tracker = EyeTrackerHTC(0);
        let result = unsafe { create(session.as_raw(), &info, &mut tracker) };
        if result.into_raw() < 0 {
            report.line(format!("native eyes: xrCreateEyeTrackerHTC failed: {result:?}"));
            return None;
        }
        report.line("native eyes: XR_HTC_eye_tracker created (per-eye openness)");
        Some(Self {
            tracker,
            destroy,
            geometric,
            active: None,
        })
    }

    /// Per-eye closure at `time` (the frame's predicted display time), or
    /// `None` when the query fails or neither eye is valid.
    pub(crate) fn sample(&mut self, time: xr::Time, report: &mut Report) -> Option<NativeEyeSample> {
        let info = EyeGeometricDataInfoHTC {
            ty: StructureType::from_raw(GEOMETRIC_INFO),
            next: std::ptr::null(),
            time,
        };
        let empty = SingleEyeGeometricDataHTC {
            is_valid: xr::sys::FALSE,
            eye_openness: 0.0,
            eye_wide: 0.0,
            eye_squeeze: 0.0,
        };
        // HTC's wrapper seeds the output time with the predicted display time.
        let mut data = EyeGeometricDataHTC {
            ty: StructureType::from_raw(GEOMETRIC_DATA),
            next: std::ptr::null_mut(),
            time,
            eyes: [empty; 2],
        };
        let result = unsafe { (self.geometric)(self.tracker, &info, &mut data) };
        // HTC: openness 0 = normally closed, 1 = normally open; a valid zero
        // is a closed eye, an invalid eye is missing data.
        let closure = |eye: &SingleEyeGeometricDataHTC| {
            bool::from(eye.is_valid).then(|| (1.0 - eye.eye_openness).clamp(0.0, 1.0))
        };
        let sample = (result.into_raw() >= 0)
            .then(|| [closure(&data.eyes[0]), closure(&data.eyes[1])])
            .filter(|closure| closure.iter().any(Option::is_some));
        let active = sample.is_some();
        if self.active != Some(active) {
            // Log transitions only, never per-frame values.
            report.line(format!(
                "native eyes: openness valid={active} (xrGetEyeGeometricDataHTC {result:?})"
            ));
            self.active = Some(active);
        }
        sample.map(|closure| NativeEyeSample { closure })
    }
}

impl Drop for NativeEyes {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.tracker) };
    }
}
