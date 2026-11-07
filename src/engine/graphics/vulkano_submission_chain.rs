// Preserve access provenance through a sequence of primary command buffers.
// Vulkano 0.35 returns Unknown immediately for a buffer/image absent from its
// most recent command buffer. Forward Unknown to the preceding future, without
// overriding a denial or claiming access ourselves.
use std::ops::Range;
use vulkano::device::{Device, DeviceOwned, Queue};
use vulkano::image::ImageLayout;
use vulkano::swapchain::Swapchain;
use vulkano::sync::future::{AccessCheckError, SubmitAnyBuilder};

struct PreviousFuture {
    inner: std::sync::Mutex<Box<dyn GpuFuture>>,
    device: Arc<Device>,
}
// One unique submission-facing owner. The additional Arc in AccessChain is used
// only for access queries; it cannot build or submit the previous work separately.
struct SubmissionPrevious(Arc<PreviousFuture>);
struct AccessChain {
    current: Box<dyn GpuFuture>,
    previous: Arc<PreviousFuture>,
}
fn chain_primary(
    prior: Box<dyn GpuFuture>,
    queue: Arc<Queue>,
    commands: PrimaryCommands,
) -> Result<Box<dyn GpuFuture>, Box<dyn std::error::Error>> {
    let device = prior.device().clone();
    let previous = Arc::new(PreviousFuture {
        inner: std::sync::Mutex::new(prior),
        device,
    });
    let current = SubmissionPrevious(previous.clone())
        .then_execute(queue, commands)?
        .boxed();
    Ok(AccessChain { current, previous }.boxed())
}
unsafe impl DeviceOwned for SubmissionPrevious {
    fn device(&self) -> &Arc<Device> {
        &self.0.device
    }
}
// SAFETY: All submission, completion and access operations delegate to the unique
// owned future. The shared state is private and has one submission-facing owner.
unsafe impl GpuFuture for SubmissionPrevious {
    fn cleanup_finished(&mut self) {
        self.0.inner.lock().unwrap().cleanup_finished();
    }
    unsafe fn build_submission(&self) -> Result<SubmitAnyBuilder, Validated<VulkanError>> {
        unsafe { self.0.inner.lock().unwrap().build_submission() }
    }
    fn flush(&self) -> Result<(), Validated<VulkanError>> {
        self.0.inner.lock().unwrap().flush()
    }
    unsafe fn signal_finished(&self) {
        unsafe {
            self.0.inner.lock().unwrap().signal_finished();
        }
    }
    fn queue(&self) -> Option<Arc<Queue>> {
        self.0.inner.lock().unwrap().queue()
    }
    fn queue_change_allowed(&self) -> bool {
        self.0.inner.lock().unwrap().queue_change_allowed()
    }
    fn check_buffer_access(
        &self,
        buffer: &Buffer,
        range: Range<DeviceSize>,
        exclusive: bool,
        queue: &Queue,
    ) -> Result<(), AccessCheckError> {
        self.0
            .inner
            .lock()
            .unwrap()
            .check_buffer_access(buffer, range, exclusive, queue)
    }
    fn check_image_access(
        &self,
        image: &Image,
        range: Range<DeviceSize>,
        exclusive: bool,
        layout: ImageLayout,
        queue: &Queue,
    ) -> Result<(), AccessCheckError> {
        self.0
            .inner
            .lock()
            .unwrap()
            .check_image_access(image, range, exclusive, layout, queue)
    }
    fn check_swapchain_image_acquired(
        &self,
        swapchain: &Swapchain,
        image: u32,
        before: bool,
    ) -> Result<(), AccessCheckError> {
        self.0
            .inner
            .lock()
            .unwrap()
            .check_swapchain_image_acquired(swapchain, image, before)
    }
}
unsafe impl DeviceOwned for AccessChain {
    fn device(&self) -> &Arc<Device> {
        self.current.device()
    }
}
// SAFETY: No work is added, removed or independently submitted here. Unknown
// access is resolved only by asking the actual preceding work on the same queue;
// explicit denials and image-layout mismatches are always preserved.
unsafe impl GpuFuture for AccessChain {
    fn cleanup_finished(&mut self) {
        self.current.cleanup_finished();
    }
    unsafe fn build_submission(&self) -> Result<SubmitAnyBuilder, Validated<VulkanError>> {
        unsafe { self.current.build_submission() }
    }
    fn flush(&self) -> Result<(), Validated<VulkanError>> {
        self.current.flush()
    }
    unsafe fn signal_finished(&self) {
        unsafe {
            self.current.signal_finished();
        }
    }
    fn queue(&self) -> Option<Arc<Queue>> {
        self.current.queue()
    }
    fn queue_change_allowed(&self) -> bool {
        self.current.queue_change_allowed()
    }
    fn check_buffer_access(
        &self,
        buffer: &Buffer,
        range: Range<DeviceSize>,
        exclusive: bool,
        queue: &Queue,
    ) -> Result<(), AccessCheckError> {
        match self
            .current
            .check_buffer_access(buffer, range.clone(), exclusive, queue)
        {
            Err(AccessCheckError::Unknown) => self
                .previous
                .inner
                .lock()
                .unwrap()
                .check_buffer_access(buffer, range, exclusive, queue),
            result => result,
        }
    }
    fn check_image_access(
        &self,
        image: &Image,
        range: Range<DeviceSize>,
        exclusive: bool,
        layout: ImageLayout,
        queue: &Queue,
    ) -> Result<(), AccessCheckError> {
        match self
            .current
            .check_image_access(image, range.clone(), exclusive, layout, queue)
        {
            Err(AccessCheckError::Unknown) => self
                .previous
                .inner
                .lock()
                .unwrap()
                .check_image_access(image, range, exclusive, layout, queue),
            result => result,
        }
    }
    fn check_swapchain_image_acquired(
        &self,
        swapchain: &Swapchain,
        image: u32,
        before: bool,
    ) -> Result<(), AccessCheckError> {
        self.current
            .check_swapchain_image_acquired(swapchain, image, before)
    }
}
