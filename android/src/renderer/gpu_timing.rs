//! Timestamp the two eye passes in the existing submission; no extra GPU waits.
use crate::{graphics_device::Gpu, xr_host::Result};
use ash::vk;
use std::sync::Arc;

pub(super) struct GpuTiming {
    gpu: Arc<Gpu>,
    pool: vk::QueryPool,
    period_ns: f64,
    mask: u64,
}
impl GpuTiming {
    pub fn new(gpu: Arc<Gpu>) -> Result<Option<Self>> {
        let properties = unsafe {
            gpu.instance
                .raw
                .get_physical_device_queue_family_properties(gpu.physical)
        };
        let bits = properties
            .get(gpu.family as usize)
            .map_or(0, |family| family.timestamp_valid_bits);
        if bits == 0 {
            return Ok(None);
        }
        let period_ns = unsafe {
            gpu.instance
                .raw
                .get_physical_device_properties(gpu.physical)
        }
        .limits
        .timestamp_period as f64;
        if !period_ns.is_finite() || period_ns <= 0.0 {
            return Ok(None);
        }
        let pool = unsafe {
            gpu.device.create_query_pool(
                &vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::TIMESTAMP)
                    .query_count(2),
                None,
            )
        }
        .map_err(|e| format!("create GPU timestamp pool: {e:?}"))?;
        Ok(Some(Self {
            gpu,
            pool,
            period_ns,
            mask: if bits >= 64 {
                u64::MAX
            } else {
                (1u64 << bits) - 1
            },
        }))
    }
    pub unsafe fn start(&self, command: vk::CommandBuffer) {
        unsafe {
            self.gpu
                .device
                .cmd_reset_query_pool(command, self.pool, 0, 2);
            self.gpu.device.cmd_write_timestamp(
                command,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                self.pool,
                0,
            );
        }
    }
    pub unsafe fn end(&self, command: vk::CommandBuffer) {
        unsafe {
            self.gpu.device.cmd_write_timestamp(
                command,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.pool,
                1,
            );
        }
    }
    pub fn elapsed_ms(&self) -> Option<f64> {
        let mut data = [0u64; 2];
        unsafe {
            self.gpu.device.get_query_pool_results(
                self.pool,
                0,
                &mut data,
                vk::QueryResultFlags::TYPE_64,
            )
        }
        .ok()?;
        Some((data[1].wrapping_sub(data[0]) & self.mask) as f64 * self.period_ns / 1_000_000.0)
    }
}
impl Drop for GpuTiming {
    fn drop(&mut self) {
        unsafe {
            self.gpu.device.destroy_query_pool(self.pool, None);
        }
    }
}
