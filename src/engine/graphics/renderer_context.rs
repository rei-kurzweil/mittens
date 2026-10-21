use std::sync::Arc;

use ash::vk::Handle as _;
use vulkano::device::physical::PhysicalDeviceType;
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo, QueueFlags,
};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::{Version, VulkanLibrary, VulkanObject};

/// Graphics resources shared by windowed and XR-only rendering. No window,
/// surface, or presentation swapchain is created here.
pub(crate) struct RendererContext {
    device: Arc<Device>,
    graphics_queue: Arc<Queue>,
    memory_allocator: Arc<StandardMemoryAllocator>,
}

impl RendererContext {
    pub(crate) fn new(
        surface_extensions: Option<InstanceExtensions>,
        xr_required: Option<(&[String], &[String])>,
        xr_runtime: Option<(&openxr::Instance, openxr::SystemId)>,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let mut instance_extensions = surface_extensions.unwrap_or_default();
        let mut device_extensions = DeviceExtensions {
            khr_swapchain: surface_extensions.is_some(),
            khr_dynamic_rendering: true,
            ..Default::default()
        };
        if let Some((instance_names, device_names)) = xr_required {
            let required_instance: InstanceExtensions =
                instance_names.iter().map(String::as_str).collect();
            let required_device: DeviceExtensions =
                device_names.iter().map(String::as_str).collect();
            // Vulkano's FromIterator ignores unknown names. Reject them rather
            // than silently creating graphics that do not satisfy the runtime.
            let known_instance: Vec<std::ffi::CString> = (&required_instance).into();
            let known_device: Vec<std::ffi::CString> = (&required_device).into();
            for (names, known) in [
                (instance_names, &known_instance),
                (device_names, &known_device),
            ] {
                for name in names {
                    if !known
                        .iter()
                        .any(|value| value.as_bytes() == name.as_bytes())
                    {
                        return Err(format!(
                            "unrecognized OpenXR-required Vulkan extension: {name}"
                        )
                        .into());
                    }
                }
            }
            instance_extensions = instance_extensions.union(&required_instance);
            device_extensions = device_extensions.union(&required_device);
        }

        let mut api_version = Version::V1_2;
        let mut minimum_api_version = Version::V1_0;
        if let Some((xr, system)) = xr_runtime {
            let requirements = xr.graphics_requirements::<openxr::Vulkan>(system)?;
            let to_version = |value: openxr::Version| Version {
                major: value.major() as u32,
                minor: value.minor() as u32,
                patch: value.patch(),
            };
            api_version = api_version.min(to_version(requirements.max_api_version_supported));
            minimum_api_version = to_version(requirements.min_api_version_supported);
            if api_version < minimum_api_version {
                return Err(
                    "OpenXR Vulkan API range does not include the renderer's Vulkan 1.2 path"
                        .into(),
                );
            }
        }
        let instance = Instance::new(
            VulkanLibrary::new()?,
            InstanceCreateInfo {
                max_api_version: Some(api_version),
                enabled_extensions: instance_extensions,
                ..Default::default()
            },
        )?;
        if instance.api_version() < minimum_api_version {
            return Err("Vulkan loader API version is below the OpenXR runtime minimum".into());
        }
        let required_physical_device = xr_runtime
            .map(|(xr, system)| unsafe {
                xr.vulkan_graphics_device(system, instance.handle().as_raw() as usize as _)
            })
            .transpose()?;
        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()?
            .filter(|physical| {
                required_physical_device
                    .is_none_or(|required| physical.handle().as_raw() as usize == required as usize)
                    && physical.supported_extensions().contains(&device_extensions)
                    && physical.supported_features().dynamic_rendering
                    && physical.api_version() >= minimum_api_version
            })
            .filter_map(|physical| {
                let family = physical
                    .queue_family_properties()
                    .iter()
                    .position(|queue| {
                        queue
                            .queue_flags
                            .contains(QueueFlags::GRAPHICS | QueueFlags::COMPUTE)
                    })?;
                Some((physical, family as u32))
            })
            .min_by_key(|(physical, _)| match physical.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                _ => 4,
            })
            .ok_or(
                "no compatible Vulkan graphics/compute device (including OpenXR GPU requirements)",
            )?;
        let (device, mut queues) = Device::new(
            physical_device,
            DeviceCreateInfo {
                enabled_extensions: device_extensions,
                enabled_features: DeviceFeatures {
                    dynamic_rendering: true,
                    ..Default::default()
                },
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )?;
        Ok(Arc::new(Self {
            memory_allocator: Arc::new(StandardMemoryAllocator::new_default(device.clone())),
            device,
            graphics_queue: queues.next().ok_or("Vulkan graphics queue missing")?,
        }))
    }

    pub(crate) fn device(&self) -> &Arc<Device> {
        &self.device
    }

    pub(crate) fn graphics_queue(&self) -> &Arc<Queue> {
        &self.graphics_queue
    }

    pub(crate) fn memory_allocator(&self) -> &Arc<StandardMemoryAllocator> {
        &self.memory_allocator
    }

    pub(crate) fn xr_graphics(&self) -> super::XrVulkanGraphics {
        super::XrVulkanGraphics {
            vk_instance: self.device.instance().handle().as_raw() as usize as _,
            vk_physical_device: self.device.physical_device().handle().as_raw() as usize as _,
            vk_device: self.device.handle().as_raw() as usize as _,
            vk_queue: self.graphics_queue.handle().as_raw() as usize as _,
            queue_family_index: self.graphics_queue.queue_family_index(),
            queue_index: 0,
        }
    }
}
