//! Opt-in GPU pass timestamps. Diagnostic boundaries can perturb GPU overlap.
use super::*;
const POINTS: u32 = 8;
const NAMES: [&str; 7] = [
    "background",
    "outline",
    "opaque",
    "cutout",
    "transparency_resolve",
    "postprocess",
    "debug",
];
pub(super) struct GpuPassQueries {
    pool: Arc<QueryPool>,
}
impl GpuPassQueries {
    pub(super) fn new(context: &RendererContext) -> Result<Option<Self>, Box<dyn Error>> {
        if std::env::var("SHIP_PROFILE_GPU_PASSES").as_deref() != Ok("1")
            || context.device().physical_device().queue_family_properties()
                [context.graphics_queue().queue_family_index() as usize]
                .timestamp_valid_bits
                .is_none()
        {
            return Ok(None);
        }
        Ok(Some(Self {
            pool: QueryPool::new(
                context.device().clone(),
                QueryPoolCreateInfo {
                    query_count: POINTS * 2,
                    ..QueryPoolCreateInfo::query_type(QueryType::Timestamp)
                },
            )?,
        }))
    }
}
impl ProfileState {
    pub(crate) fn gpu_pass_timestamp(
        &self,
        commands: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        eye: usize,
        point: u32,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(passes) = &self.gpu_passes {
            unsafe {
                if point == 0 {
                    commands.reset_query_pool(
                        passes.pool.clone(),
                        eye as u32 * POINTS..(eye as u32 + 1) * POINTS,
                    )?;
                }
                commands.write_timestamp(
                    passes.pool.clone(),
                    eye as u32 * POINTS + point,
                    if point == 0 {
                        PipelineStage::TopOfPipe
                    } else {
                        PipelineStage::BottomOfPipe
                    },
                )?;
            }
        }
        Ok(())
    }
    pub(crate) fn gpu_pass_ms(&self) -> Result<BTreeMap<&'static str, f64>, Box<dyn Error>> {
        let Some(passes) = &self.gpu_passes else {
            return Ok(BTreeMap::new());
        };
        let mut times = [0u64; POINTS as usize * 2];
        if !passes
            .pool
            .get_results(0..POINTS * 2, &mut times, QueryResultFlags::empty())?
        {
            return Err("GPU pass queries unavailable after completion fence".into());
        }
        Ok(NAMES
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                let value = (0..2)
                    .map(|eye| {
                        (times[eye * POINTS as usize + i + 1]
                            .wrapping_sub(times[eye * POINTS as usize + i])
                            & self.timestamp_mask) as f64
                            * self.timestamp_period
                            / 1e6
                    })
                    .sum();
                (name, value)
            })
            .collect())
    }
}
