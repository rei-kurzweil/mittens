//! Opt-in diagnostic: try a larger swapchain without changing the normal scene.
use crate::xr_host::Report;
use ash::vk;
use openxr as xr;

pub(crate) fn test_2448(
    session: &xr::Session<xr::Vulkan>,
    format: vk::Format,
    report: &mut Report,
) {
    let result = session.create_swapchain(&xr::SwapchainCreateInfo {
        create_flags: xr::SwapchainCreateFlags::EMPTY,
        usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT | xr::SwapchainUsageFlags::SAMPLED,
        format: format.as_raw() as _,
        sample_count: 1,
        width: 2448,
        height: 2448,
        face_count: 1,
        array_size: 2,
        mip_count: 1,
    });
    match result {
        Ok(swapchain) => match swapchain.enumerate_images() {
            Ok(images) => report.line(format!("EXPERIMENT 2448x2448 stereo swapchain created, images={}; no rendering or presentation tested", images.len())),
            Err(error) => report.line(format!("EXPERIMENT 2448x2448 swapchain created but image enumeration failed: {error:?}")),
        },
        Err(error) => report.line(format!("EXPERIMENT 2448x2448 stereo swapchain creation failed: {error:?}")),
    }
}
