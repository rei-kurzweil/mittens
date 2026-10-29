use mittens_engine::engine::ecs::system::openxr_system::OpenXRSystem;
use openxr as xr;
use std::{
    ffi::CStr,
    fs::File,
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

pub(super) type Result<T> = std::result::Result<T, String>;
pub(super) trait ApiResult<T> {
    fn api(self, operation: &str) -> Result<T>;
}
impl<T, E: std::fmt::Debug> ApiResult<T> for std::result::Result<T, E> {
    fn api(self, operation: &str) -> Result<T> {
        self.map_err(|error| format!("{operation}: {error:?}"))
    }
}

pub(super) struct Report(Option<File>);
impl Report {
    fn new(directory: Option<PathBuf>) -> Self {
        let file = directory.and_then(|path| {
            let path = path.join("xr-vulkan-probe.txt");
            log::info!("probe_report path={}", path.display());
            File::create(path)
                .map_err(|error| log::error!("probe report: {error}"))
                .ok()
        });
        Self(file)
    }
    pub(super) fn line(&mut self, message: impl std::fmt::Display) {
        log::info!("probe {message}");
        if let Some(file) = self.0.as_mut() {
            if let Err(error) = writeln!(file, "{message}").and_then(|_| file.flush()) {
                log::error!("probe report write failed: {error}");
                self.0 = None;
            }
        }
    }
}

pub(super) struct Worker {
    directory: Option<PathBuf>,
    stop: Arc<AtomicBool>,
    resumed: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    pub(super) fn new(directory: Option<PathBuf>) -> Self {
        Self {
            directory,
            stop: Arc::new(AtomicBool::new(false)),
            resumed: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }
    pub(super) fn resume(&mut self) {
        self.resumed.store(true, Ordering::Release);
        if self.thread.is_none() {
            match spawn(
                self.directory.clone(),
                self.stop.clone(),
                self.resumed.clone(),
            ) {
                Ok(thread) => self.thread = Some(thread),
                Err(error) => log::error!("XR thread startup failed: {error}"),
            }
        }
    }
    pub(super) fn pause(&self) {
        self.resumed.store(false, Ordering::Release);
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            log::info!("joining XR worker before releasing Android context");
            if thread.join().is_err() {
                log::error!("XR worker panicked; see host_panic log");
            }
            log::info!("XR worker joined");
        }
    }
}

pub(super) fn spawn(
    directory: Option<PathBuf>,
    stop: Arc<AtomicBool>,
    resumed: Arc<AtomicBool>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("mittens-xr".into())
        .spawn(move || {
            let mut report = Report::new(directory);
            let result = (|| {
                // The Activity/context outlives this worker; android_main joins it
                // before returning. Attach the graphics thread for runtime JNI calls.
                let context = ndk_context::android_context();
                let vm = unsafe { jni::JavaVM::from_raw(context.vm().cast()) }
                    .api("JavaVM::from_raw")?;
                let _attachment = vm.attach_current_thread().api("attach XR thread to JVM")?;
                run(&mut report, &stop, &resumed)
            })();
            match result {
                Ok(()) => report.line("worker_exit clean"),
                Err(error) => report.line(format!("FAILED {error}")),
            }
        })
}

fn run(report: &mut Report, stop: &AtomicBool, resumed: &AtomicBool) -> Result<()> {
    report.line("loader=Khronos-1.1.36 api_request=OpenXR-1.0.34");
    // Shared engine hook performs Android loader initialization before enumeration.
    let entry = OpenXRSystem::load_entry()?;
    report.line("xrInitializeLoaderKHR(Android) SUCCESS");
    enumerate_extensions(&entry, report)?;
    let available = entry
        .enumerate_extensions()
        .api("xrEnumerateInstanceExtensionProperties")?;
    if !available.khr_android_create_instance {
        return Err("XR_KHR_android_create_instance not advertised".into());
    }
    let enable2 = available.khr_vulkan_enable2;
    if !enable2 && !available.khr_vulkan_enable {
        return Err("runtime advertises neither Vulkan binding".into());
    }
    let mut extensions = xr::ExtensionSet::default();
    extensions.khr_android_create_instance = true;
    extensions.khr_vulkan_enable2 = enable2;
    extensions.khr_vulkan_enable = !enable2;
    let instance = entry
        .create_instance(
            &xr::ApplicationInfo {
                application_name: "Mittens Vulkan Probe",
                application_version: 1,
                engine_name: "Mittens",
                engine_version: 1,
                api_version: xr::Version::new(1, 0, 34),
            },
            &extensions,
            &[],
        )
        .api("xrCreateInstance(Android)")?;
    let properties = instance.properties().api("xrGetInstanceProperties")?;
    report.line(format!(
        "runtime={} version={} binding={}",
        properties.runtime_name,
        properties.runtime_version,
        if enable2 {
            "XR_KHR_vulkan_enable2"
        } else {
            "XR_KHR_vulkan_enable"
        }
    ));
    let system = instance
        .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
        .api("xrGetSystem(HMD)")?;
    let system_properties = instance
        .system_properties(system)
        .api("xrGetSystemProperties")?;
    report.line(format!(
        "system={system:?} properties={system_properties:?}"
    ));
    for view in instance
        .enumerate_view_configurations(system)
        .api("xrEnumerateViewConfigurations")?
    {
        report.line(format!(
            "view_configuration={view:?} views={:?} blend_modes={:?}",
            instance
                .enumerate_view_configuration_views(system, view)
                .api("xrEnumerateViewConfigurationViews")?,
            instance
                .enumerate_environment_blend_modes(system, view)
                .api("xrEnumerateEnvironmentBlendModes")?
        ));
    }
    let requirements = instance
        .graphics_requirements::<xr::Vulkan>(system)
        .api("xrGetVulkanGraphicsRequirementsKHR/2KHR")?;
    report.line(format!(
        "Vulkan API min={} max={}",
        requirements.min_api_version_supported, requirements.max_api_version_supported
    ));
    report.line("OpenXR instance/HMD capability stage SUCCESS");
    #[cfg(feature = "vulkan-probe")]
    {
        let mut probe =
            crate::vulkan_probe::Probe::new(&instance, system, enable2, requirements, report)?;
        probe.run(&instance, stop, resumed, report)?;
    }
    #[cfg(not(feature = "vulkan-probe"))]
    {
        let _ = (stop, resumed);
        report.line("instance-only probe complete; Vulkan session not requested");
    }
    Ok(())
}

fn enumerate_extensions(entry: &xr::Entry, report: &mut Report) -> Result<()> {
    // ExtensionSet::names is private in openxr 0.19; enumerate raw names so
    // vendor extensions unknown to that crate are also preserved in the report.
    unsafe {
        let enumerate = entry.fp().enumerate_instance_extension_properties;
        let mut count = 0;
        let result = enumerate(std::ptr::null(), 0, &mut count, std::ptr::null_mut());
        if result.into_raw() < 0 {
            return Err(format!(
                "xrEnumerateInstanceExtensionProperties(count): {result:?}"
            ));
        }
        let mut properties = (0..count)
            .map(|_| xr::sys::ExtensionProperties {
                ty: xr::sys::ExtensionProperties::TYPE,
                next: std::ptr::null_mut(),
                extension_name: [0; xr::sys::MAX_EXTENSION_NAME_SIZE],
                extension_version: 0,
            })
            .collect::<Vec<_>>();
        let result = enumerate(std::ptr::null(), count, &mut count, properties.as_mut_ptr());
        if result.into_raw() < 0 {
            return Err(format!(
                "xrEnumerateInstanceExtensionProperties(names): {result:?}"
            ));
        }
        for property in properties.iter().take(count as usize) {
            report.line(format!(
                "XR extension={} version={}",
                CStr::from_ptr(property.extension_name.as_ptr()).to_string_lossy(),
                property.extension_version
            ));
        }
    }
    Ok(())
}
