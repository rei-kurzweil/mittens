use crate::xr_host::{ApiResult, Report, Result};
use ash::{vk, vk::Handle};
use openxr as xr;
use std::{
    ffi::{CStr, CString},
    sync::Arc,
};

pub(crate) struct InstanceOwner {
    pub raw: ash::Instance,
    _entry: ash::Entry,
}
impl Drop for InstanceOwner {
    fn drop(&mut self) {
        unsafe {
            self.raw.destroy_instance(None);
        }
    }
}

pub(crate) struct Gpu {
    pub device: ash::Device,
    pub instance: Arc<InstanceOwner>,
    pub physical: vk::PhysicalDevice,
    pub queue: vk::Queue,
    pub family: u32,
    #[allow(dead_code)] // Used by the optional triangle diagnostic.
    pub dynamic: bool,
    #[allow(dead_code)] // The scene renderer deliberately uses classic passes.
    pub dynamic_api: Option<ash::khr::dynamic_rendering::Device>,
}
impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            if let Err(error) = self.device.device_wait_idle() {
                log::error!("vkDeviceWaitIdle during teardown: {error:?}");
            }
            self.device.destroy_device(None);
        }
    }
}

fn vk_version(version: xr::Version) -> u32 {
    vk::make_api_version(
        0,
        version.major() as u32,
        version.minor() as u32,
        version.patch(),
    )
}
fn version_string(version: u32) -> String {
    format!(
        "{}.{}.{}",
        vk::api_version_major(version),
        vk::api_version_minor(version),
        vk::api_version_patch(version)
    )
}
fn names(list: &str) -> Vec<CString> {
    list.split_whitespace()
        .map(|name| CString::new(name).expect("extension name"))
        .collect()
}

impl Gpu {
    pub(super) fn new(
        xr_instance: &xr::Instance,
        system: xr::SystemId,
        enable2: bool,
        requirements: xr::vulkan::Requirements,
        report: &mut Report,
    ) -> Result<Arc<Self>> {
        // All handles below are owned here; no window/surface/WSI device is created.
        unsafe {
            let entry = ash::Entry::load().api("load libvulkan.so")?;
            let loader_version = entry
                .try_enumerate_instance_version()
                .api("vkEnumerateInstanceVersion")?
                .unwrap_or(vk::API_VERSION_1_0);
            let api = vk::API_VERSION_1_2
                .min(loader_version)
                .min(vk_version(requirements.max_api_version_supported));
            if api < vk_version(requirements.min_api_version_supported) {
                return Err(format!(
                    "no Vulkan API intersection: loader={} runtime_min={} runtime_max={} probe_max=1.2",
                    version_string(loader_version),
                    requirements.min_api_version_supported,
                    requirements.max_api_version_supported
                ));
            }
            let instance_extensions = if enable2 {
                report.line("enable2: runtime supplies required Vulkan extensions through xrCreateVulkanInstanceKHR/xrCreateVulkanDeviceKHR");
                Vec::new()
            } else {
                let list = xr_instance
                    .vulkan_legacy_instance_extensions(system)
                    .api("xrGetVulkanInstanceExtensionsKHR")?;
                report.line(format!("required Vulkan instance extensions={list}"));
                names(&list)
            };
            let available_instance = entry
                .enumerate_instance_extension_properties(None)
                .api("vkEnumerateInstanceExtensionProperties")?;
            for required in &instance_extensions {
                if !available_instance
                    .iter()
                    .any(|p| CStr::from_ptr(p.extension_name.as_ptr()) == required.as_c_str())
                {
                    return Err(format!(
                        "required instance extension unavailable: {required:?}"
                    ));
                }
            }
            report.line(format!(
                "Vulkan loader={} requested_api={}",
                version_string(loader_version),
                version_string(api)
            ));
            let pointers = instance_extensions
                .iter()
                .map(|s| s.as_ptr())
                .collect::<Vec<_>>();
            let app = vk::ApplicationInfo::default()
                .application_name(c"Mittens Probe")
                .engine_name(c"Mittens")
                .api_version(api);
            let info = vk::InstanceCreateInfo::default()
                .application_info(&app)
                .enabled_extension_names(&pointers);
            let instance = if enable2 {
                let raw = xr_instance
                    .create_vulkan_instance(
                        system,
                        std::mem::transmute(entry.static_fn().get_instance_proc_addr),
                        &info as *const _ as _,
                    )
                    .api("xrCreateVulkanInstanceKHR")?
                    .map_err(vk::Result::from_raw)
                    .api("xrCreateVulkanInstanceKHR Vulkan result")?;
                ash::Instance::load(entry.static_fn(), vk::Instance::from_raw(raw as _))
            } else {
                entry.create_instance(&info, None).api("vkCreateInstance")?
            };
            let instance = Arc::new(InstanceOwner {
                raw: instance,
                _entry: entry,
            });
            let physical = vk::PhysicalDevice::from_raw(
                xr_instance
                    .vulkan_graphics_device(system, instance.raw.handle().as_raw() as _)
                    .api("xrGetVulkanGraphicsDeviceKHR/2KHR")? as _,
            );
            let properties = instance.raw.get_physical_device_properties(physical);
            let effective_api = api.min(properties.api_version);
            if effective_api < vk_version(requirements.min_api_version_supported) {
                return Err("runtime-selected GPU is below the required Vulkan minimum".into());
            }
            report.line(format!(
                "GPU={} physical={physical:?} API={} driver={} vendor={:#x} device={:#x}",
                CStr::from_ptr(properties.device_name.as_ptr()).to_string_lossy(),
                version_string(properties.api_version),
                properties.driver_version,
                properties.vendor_id,
                properties.device_id
            ));
            let supported = instance
                .raw
                .enumerate_device_extension_properties(physical)
                .api("vkEnumerateDeviceExtensionProperties")?
                .iter()
                .map(|p| {
                    CStr::from_ptr(p.extension_name.as_ptr())
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<Vec<_>>();
            for extension in &supported {
                report.line(format!("Vulkan device extension={extension}"));
            }
            let has = |name: &CStr| supported.iter().any(|s| s == &name.to_string_lossy());
            let mut dynamic_features = vk::PhysicalDeviceDynamicRenderingFeatures::default();
            let mut multiview_features = vk::PhysicalDeviceMultiviewFeatures::default();
            let mut local_read_features =
                vk::PhysicalDeviceDynamicRenderingLocalReadFeaturesKHR::default();
            let features = if effective_api >= vk::API_VERSION_1_1 {
                let mut query =
                    vk::PhysicalDeviceFeatures2::default().push_next(&mut multiview_features);
                if has(ash::khr::dynamic_rendering::NAME) {
                    query = query.push_next(&mut dynamic_features);
                }
                if has(ash::khr::dynamic_rendering_local_read::NAME) {
                    query = query.push_next(&mut local_read_features);
                }
                instance
                    .raw
                    .get_physical_device_features2(physical, &mut query);
                query.features
            } else {
                instance.raw.get_physical_device_features(physical)
            };
            let dynamic_dependencies = if effective_api < vk::API_VERSION_1_2 {
                vec![
                    ash::khr::dynamic_rendering::NAME,
                    ash::khr::depth_stencil_resolve::NAME,
                    ash::khr::create_renderpass2::NAME,
                ]
            } else {
                vec![ash::khr::dynamic_rendering::NAME]
            };
            let dynamic = dynamic_features.dynamic_rendering == vk::TRUE
                && dynamic_dependencies.iter().all(|name| has(name));
            report.line(format!("dynamicRendering={} usable={} multiview={} dynamic_rendering_local_read_extension={}",
                dynamic_features.dynamic_rendering, dynamic, multiview_features.multiview,
                has(ash::khr::dynamic_rendering_local_read::NAME)));
            report.line(format!(
                "dynamicRenderingLocalRead={}",
                local_read_features.dynamic_rendering_local_read
            ));
            if effective_api >= vk::API_VERSION_1_1 {
                let mut multiview = vk::PhysicalDeviceMultiviewProperties::default();
                let mut query = vk::PhysicalDeviceProperties2::default().push_next(&mut multiview);
                instance
                    .raw
                    .get_physical_device_properties2(physical, &mut query);
                report.line(format!(
                    "maxMultiviewViewCount={}",
                    multiview.max_multiview_view_count
                ));
            }
            if effective_api >= vk::API_VERSION_1_1
                && (effective_api >= vk::API_VERSION_1_2
                    || has(ash::khr::depth_stencil_resolve::NAME))
            {
                let mut resolve = vk::PhysicalDeviceDepthStencilResolveProperties::default();
                let mut query = vk::PhysicalDeviceProperties2::default().push_next(&mut resolve);
                instance
                    .raw
                    .get_physical_device_properties2(physical, &mut query);
                report.line(format!("depth_resolve_modes={:?} stencil_resolve_modes={:?} independent_resolve={} independent_resolve_none={}",
                    resolve.supported_depth_resolve_modes, resolve.supported_stencil_resolve_modes,
                    resolve.independent_resolve, resolve.independent_resolve_none));
            }
            let limits = properties.limits;
            report.line(format!("compute workgroup_invocations={} size={:?} count={:?} storage_buffers_per_stage={} stage_resources={} storage_range={} uniform_range={} sampled_images={}",
                limits.max_compute_work_group_invocations, limits.max_compute_work_group_size, limits.max_compute_work_group_count,
                limits.max_per_stage_descriptor_storage_buffers, limits.max_per_stage_resources, limits.max_storage_buffer_range,
                limits.max_uniform_buffer_range, limits.max_per_stage_descriptor_sampled_images));
            report.line(format!(
                "texture ASTC_LDR={} ETC2={} BC={} color_samples={:?} depth_samples={:?}",
                features.texture_compression_astc_ldr,
                features.texture_compression_etc2,
                features.texture_compression_bc,
                limits.framebuffer_color_sample_counts,
                limits.framebuffer_depth_sample_counts
            ));
            let queues = instance
                .raw
                .get_physical_device_queue_family_properties(physical);
            for (index, queue) in queues.iter().enumerate() {
                report.line(format!(
                    "queue_family={index} flags={:?} count={}",
                    queue.queue_flags, queue.queue_count
                ));
            }
            let family = queues
                .iter()
                .position(|q| q.queue_count > 0 && q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .ok_or("runtime GPU has no graphics queue")? as u32;
            report.line(format!(
                "engine_deformation_basic_limits_ok={} (probe does not require deformation)",
                queues[family as usize]
                    .queue_flags
                    .contains(vk::QueueFlags::COMPUTE)
                    && limits.max_compute_work_group_invocations >= 64
                    && limits.max_compute_work_group_size[0] >= 64
                    && limits.max_per_stage_descriptor_storage_buffers >= 8
            ));
            let memory = instance.raw.get_physical_device_memory_properties(physical);
            for (index, heap) in memory.memory_heaps[..memory.memory_heap_count as usize]
                .iter()
                .enumerate()
            {
                report.line(format!(
                    "memory_heap={index} bytes={} flags={:?}",
                    heap.size, heap.flags
                ));
            }
            for (index, ty) in memory.memory_types[..memory.memory_type_count as usize]
                .iter()
                .enumerate()
            {
                report.line(format!(
                    "memory_type={index} heap={} flags={:?}",
                    ty.heap_index, ty.property_flags
                ));
            }
            for format in [
                vk::Format::R8G8B8A8_SRGB,
                vk::Format::B8G8R8A8_SRGB,
                vk::Format::R8G8B8A8_UNORM,
                vk::Format::B8G8R8A8_UNORM,
                vk::Format::D32_SFLOAT_S8_UINT,
                vk::Format::D24_UNORM_S8_UINT,
            ] {
                let depth = matches!(
                    format,
                    vk::Format::D32_SFLOAT_S8_UINT | vk::Format::D24_UNORM_S8_UINT
                );
                let usage = if depth {
                    vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                } else {
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                };
                let support = instance.raw.get_physical_device_image_format_properties(
                    physical,
                    format,
                    vk::ImageType::TYPE_2D,
                    vk::ImageTiling::OPTIMAL,
                    usage,
                    vk::ImageCreateFlags::empty(),
                );
                report.line(format!(
                    "format={format:?} usage={usage:?} image_support={support:?}"
                ));
            }
            let mut extensions = if enable2 {
                Vec::new()
            } else {
                let list = xr_instance
                    .vulkan_legacy_device_extensions(system)
                    .api("xrGetVulkanDeviceExtensionsKHR")?;
                report.line(format!("required Vulkan device extensions={list}"));
                names(&list)
            };
            if dynamic {
                for name in dynamic_dependencies {
                    if !extensions.iter().any(|s| s.as_c_str() == name) {
                        extensions.push(name.to_owned());
                    }
                }
            }
            for extension in &extensions {
                if !has(extension) {
                    return Err(format!(
                        "required device extension unavailable: {extension:?}"
                    ));
                }
            }
            report.line(format!("application Vulkan device extensions={extensions:?} graphics_queue={family} renderer={}",
                if dynamic { "dynamic rendering" } else { "classic render pass" }));
            let pointers = extensions.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
            let queues = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(family)
                .queue_priorities(&[1.0])];
            let mut dynamic_enable =
                vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);
            let mut info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queues)
                .enabled_extension_names(&pointers);
            if dynamic {
                info = info.push_next(&mut dynamic_enable);
            }
            let device = if enable2 {
                let raw = xr_instance
                    .create_vulkan_device(
                        system,
                        std::mem::transmute(instance._entry.static_fn().get_instance_proc_addr),
                        physical.as_raw() as _,
                        &info as *const _ as _,
                    )
                    .api("xrCreateVulkanDeviceKHR")?
                    .map_err(vk::Result::from_raw)
                    .api("xrCreateVulkanDeviceKHR Vulkan result")?;
                ash::Device::load(instance.raw.fp_v1_0(), vk::Device::from_raw(raw as _))
            } else {
                instance
                    .raw
                    .create_device(physical, &info, None)
                    .api("vkCreateDevice")?
            };
            let queue = device.get_device_queue(family, 0);
            let dynamic_api =
                dynamic.then(|| ash::khr::dynamic_rendering::Device::new(&instance.raw, &device));
            report.line("Vulkan instance/runtime-selected GPU/device SUCCESS");
            Ok(Arc::new(Self {
                device,
                instance,
                physical,
                queue,
                family,
                dynamic,
                dynamic_api,
            }))
        }
    }
}
