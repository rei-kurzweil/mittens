// Retained host uploads. View/purpose identity separates reservations before submit;
// Vulkano's range locks prohibit CPU writes while a prior GPU use is pending.
use vulkano::sync::HostAccessError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum UploadView {
    Window(usize),
    Eye(usize),
    Mirror(
        ComponentId,
        crate::engine::graphics::visual_world::MirrorViewerFamily,
        usize,
        Option<crate::engine::graphics::primitives::InstanceHandle>,
    ),
}
#[derive(Clone, Copy)]
#[repr(usize)]
enum InstanceUpload {
    Opaque,
    Outline,
    Background,
    LitBackground,
    Cutout,
    Overlay,
    Refraction,
    RoughTransmission,
    Emission,
    LitEmission,
    CutoutEmission,
    Transparent,
    SortedTransparent,
}
const INSTANCE_UPLOADS: usize = 13;
#[derive(Default)]
struct ViewUploads {
    instances: [Option<Subbuffer<[InstanceData]>>; INSTANCE_UPLOADS],
    cameras: [Option<Subbuffer<CameraUBO>>; 2],
    lights: Option<Subbuffer<LightsSSBO>>,
}
#[derive(Default, Clone, Copy, serde::Serialize)]
struct UploadStats {
    allocations: u64,
    reuses: u64,
    busy_replacements: u64,
}
impl UploadStats {
    fn delta(self, before: Self) -> Self {
        Self {
            allocations: self.allocations - before.allocations,
            reuses: self.reuses - before.reuses,
            busy_replacements: self.busy_replacements - before.busy_replacements,
        }
    }
}
struct RetainedUploads {
    views: crate::engine::graphics::material_cache::MaterialCache<UploadView, ViewUploads>,
    active: Option<UploadView>,
    enabled: bool,
    stats: UploadStats,
}
impl Default for RetainedUploads {
    fn default() -> Self {
        #[cfg(test)]
        let enabled = std::env::var("SHIP_PROFILE_RETAIN_UPLOADS").as_deref() != Ok("0");
        #[cfg(not(test))]
        let enabled = true;
        Self {
            views: crate::engine::graphics::material_cache::MaterialCache::new(64),
            active: None,
            enabled,
            stats: Default::default(),
        }
    }
}
impl RetainedUploads {
    fn begin_view(&mut self, kind: &RenderViewKind, window_image: usize) {
        let view = match *kind {
            RenderViewKind::Window => UploadView::Window(window_image),
            RenderViewKind::XrEye { eye } => UploadView::Eye(eye),
            RenderViewKind::Mirror {
                mirror_component,
                family,
                view_index,
                excluded_instance,
                ..
            } => UploadView::Mirror(mirror_component, family, view_index, excluded_instance),
        };
        self.active = Some(view);
        if self.views.get(&view).is_none() {
            self.views.insert(view, ViewUploads::default());
        }
    }
    fn view(&mut self) -> &mut ViewUploads {
        self.views
            .get_mut(&self.active.expect("begin view before uploads"))
            .unwrap()
    }

    fn instances(
        &mut self,
        allocator: Arc<StandardMemoryAllocator>,
        role: InstanceUpload,
        count: usize,
        mut value: impl FnMut(usize) -> InstanceData,
    ) -> Result<(Subbuffer<[InstanceData]>, bool), Box<dyn std::error::Error>> {
        let count = count.max(1);
        let existing = if self.enabled {
            self.view().instances[role as usize].clone()
        } else {
            None
        };
        let mut busy = false;
        if let Some(buffer) = existing.filter(|buffer| buffer.len() >= count as u64) {
            match buffer.write() {
                Ok(mut data) => {
                    for (index, dst) in data[..count].iter_mut().enumerate() {
                        *dst = value(index);
                    }
                    drop(data);
                    self.stats.reuses += 1;
                    return Ok((buffer.clone().slice(0..count as u64), false));
                }
                Err(HostAccessError::AccessConflict(_)) => {
                    busy = true;
                }
                Err(error) => return Err(error.into()),
            }
        }
        let capacity = if self.enabled {
            count.next_power_of_two()
        } else {
            count
        };
        let buffer = Buffer::new_slice::<InstanceData>(
            allocator,
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            capacity as u64,
        )?;
        {
            let mut data = buffer.write()?;
            for (index, dst) in data[..count].iter_mut().enumerate() {
                *dst = value(index);
            }
        }
        self.stats.allocations += 1;
        self.stats.busy_replacements += u64::from(busy);
        if self.enabled {
            self.view().instances[role as usize] = Some(buffer.clone());
        }
        Ok((buffer.slice(0..count as u64), true))
    }
    fn camera(
        &mut self,
        allocator: Arc<StandardMemoryAllocator>,
        background: bool,
        value: CameraUBO,
    ) -> Result<Subbuffer<CameraUBO>, Box<dyn std::error::Error>> {
        let existing = if self.enabled {
            self.view().cameras[usize::from(background)].clone()
        } else {
            None
        };
        let (buffer, allocated, busy) =
            write_retained_scalar(existing, allocator, BufferUsage::UNIFORM_BUFFER, value)?;
        self.record(allocated, busy);
        if self.enabled {
            self.view().cameras[usize::from(background)] = Some(buffer.clone());
        }
        Ok(buffer)
    }
    fn lights(
        &mut self,
        allocator: Arc<StandardMemoryAllocator>,
        value: LightsSSBO,
    ) -> Result<Subbuffer<LightsSSBO>, Box<dyn std::error::Error>> {
        let existing = if self.enabled {
            self.view().lights.clone()
        } else {
            None
        };
        let (buffer, allocated, busy) =
            write_retained_scalar(existing, allocator, BufferUsage::STORAGE_BUFFER, value)?;
        self.record(allocated, busy);
        if self.enabled {
            self.view().lights = Some(buffer.clone());
        }
        Ok(buffer)
    }
    fn record(&mut self, allocated: bool, busy: bool) {
        self.stats.allocations += u64::from(allocated);
        self.stats.reuses += u64::from(!allocated);
        self.stats.busy_replacements += u64::from(busy);
    }
}
fn write_retained_scalar<T: BufferContents + Copy>(
    existing: Option<Subbuffer<T>>,
    allocator: Arc<StandardMemoryAllocator>,
    usage: BufferUsage,
    value: T,
) -> Result<(Subbuffer<T>, bool, bool), Box<dyn std::error::Error>> {
    let mut busy = false;
    if let Some(buffer) = existing {
        match buffer.write() {
            Ok(mut data) => {
                *data = value;
                drop(data);
                return Ok((buffer.clone(), false, false));
            }
            Err(HostAccessError::AccessConflict(_)) => {
                busy = true;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok((
        Buffer::from_data(
            allocator,
            BufferCreateInfo {
                usage,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            value,
        )?,
        true,
        busy,
    ))
}

struct RigBinding {
    output: Subbuffer<[GpuDeformedVertex]>,
    set: Arc<DescriptorSet>,
}
impl VulkanoState {
    fn retained_rig_set(
        &mut self,
        output: Subbuffer<[GpuDeformedVertex]>,
    ) -> Result<Arc<DescriptorSet>, Box<dyn std::error::Error>> {
        if self.retained_uploads.borrow().enabled
            && let Some(binding) = self.retained_rig.as_ref()
        {
            if Arc::ptr_eq(binding.output.buffer(), output.buffer())
                && binding.output.offset() == output.offset()
                && binding.output.size() == output.size()
            {
                return Ok(binding.set.clone());
            }
        }
        let lighting = Buffer::from_data(
            self.context.memory_allocator().clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            DummyPerInstanceLightingSSBO::default(),
        )?;
        let set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.set_layouts.rig.clone(),
            [
                WriteDescriptorSet::buffer(0, lighting),
                WriteDescriptorSet::buffer(1, output.clone()),
            ],
            [],
        )?;
        self.retained_rig = Some(RigBinding {
            output,
            set: set.clone(),
        });
        Ok(set)
    }
}

#[cfg(test)]
mod retained_upload_tests {
    use super::*;
    #[test]
    #[ignore = "requires Vulkan; validates upload reuse, range conflicts, view separation and capacity growth"]
    fn retained_uploads_preserve_reserved_data_and_reuse_completed_storage()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = RendererContext::new(None, None, None)?;
        let allocator = context.memory_allocator().clone();
        let mut cache = RetainedUploads::default();
        cache.enabled = true;
        cache.begin_view(&RenderViewKind::XrEye { eye: 0 }, 0);
        let value = |color| InstanceData {
            i_color: [color, 0., 0., 1.],
            ..Default::default()
        };
        let (first, new) =
            cache.instances(allocator.clone(), InstanceUpload::Opaque, 1, |_| value(1.))?;
        assert!(new);
        let (same, new) =
            cache.instances(allocator.clone(), InstanceUpload::Opaque, 1, |_| value(2.))?;
        assert!(!new && Arc::ptr_eq(first.buffer(), same.buffer()));
        let reservation = same.write()?;
        let (replacement, new) =
            cache.instances(allocator.clone(), InstanceUpload::Opaque, 1, |_| value(3.))?;
        assert!(new && !Arc::ptr_eq(replacement.buffer(), same.buffer()));
        assert_eq!(
            reservation[0].i_color[0], 2.,
            "a reserved allocation must not be overwritten"
        );
        drop(reservation);
        let (grown, new) =
            cache.instances(allocator.clone(), InstanceUpload::Opaque, 3, |_| value(4.))?;
        assert!(new && !Arc::ptr_eq(replacement.buffer(), grown.buffer()));
        let (smaller, new) =
            cache.instances(allocator.clone(), InstanceUpload::Opaque, 2, |_| value(5.))?;
        assert!(!new && Arc::ptr_eq(grown.buffer(), smaller.buffer()));
        cache.begin_view(&RenderViewKind::XrEye { eye: 1 }, 0);
        let (right, new) =
            cache.instances(allocator.clone(), InstanceUpload::Opaque, 2, |_| value(6.))?;
        assert!(new && !Arc::ptr_eq(right.buffer(), smaller.buffer()));
        let (transparent, new) =
            cache.instances(allocator, InstanceUpload::Transparent, 2, |_| value(7.))?;
        assert!(new && !Arc::ptr_eq(right.buffer(), transparent.buffer()));
        assert_eq!(cache.stats.busy_replacements, 1);
        Ok(())
    }
}
