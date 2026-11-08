//! Test-only opaque work counters and truck draw-order experiments.
//! Sorting happens only when a retained graphics command is recorded.
use super::*;
use crate::engine::graphics::visual_world::RenderOp;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use vulkano::query::{QueryControlFlags, QueryPipelineStatisticFlags};

pub(in super::super) struct MaterialDiagnostic {
    pool: Option<Arc<QueryPool>>,
    meshes: HashSet<MeshHandle>,
    order: String,
    native_truck_batches: RefCell<Vec<serde_json::Value>>,
    recorded_truck_batches: RefCell<Vec<serde_json::Value>>,
    geometry_order: RefCell<Vec<u64>>,
}

impl MaterialDiagnostic {
    pub(in super::super) fn new(context: &RendererContext) -> Result<Self, Box<dyn Error>> {
        let enabled = std::env::var("SHIP_PROFILE_MATERIAL_DIAGNOSTIC").as_deref() == Ok("1");
        let order = std::env::var("SHIP_PROFILE_TRUCK_ORDER").unwrap_or("native".into());
        assert!(matches!(
            order.as_str(),
            "native" | "first" | "last" | "geometry"
        ));
        assert!(
            enabled || order == "native",
            "draw reordering requires material diagnostics"
        );
        let pool = if enabled {
            assert!(
                context
                    .device()
                    .enabled_features()
                    .pipeline_statistics_query
            );
            Some(QueryPool::new(
                context.device().clone(),
                QueryPoolCreateInfo {
                    query_count: 2,
                    pipeline_statistics: QueryPipelineStatisticFlags::INPUT_ASSEMBLY_VERTICES
                        | QueryPipelineStatisticFlags::VERTEX_SHADER_INVOCATIONS
                        | QueryPipelineStatisticFlags::FRAGMENT_SHADER_INVOCATIONS,
                    ..QueryPoolCreateInfo::query_type(QueryType::PipelineStatistics)
                },
            )?)
        } else {
            None
        };
        Ok(Self {
            pool,
            meshes: HashSet::new(),
            order,
            native_truck_batches: RefCell::new(Vec::new()),
            recorded_truck_batches: RefCell::new(Vec::new()),
            geometry_order: RefCell::new(Vec::new()),
        })
    }

    pub(in super::super) fn select_truck(&mut self, scene: &Universe) {
        if self.pool.is_none() {
            return;
        }
        self.meshes = scene
            .visuals
            .instances()
            .iter()
            .filter(|instance| {
                let mut current = Some(instance.component);
                while let Some(id) = current {
                    if scene.world.component_label(id) == Some("pickup_painted_body") {
                        return true;
                    }
                    current = scene.world.parent_of(id);
                }
                false
            })
            .map(|instance| instance.renderable.mesh)
            .collect();
        assert!(
            !self.meshes.is_empty(),
            "truck body not found for material diagnostic"
        );
    }

    fn truck_batches(&self, ops: &[RenderOp]) -> Vec<serde_json::Value> {
        ops.iter()
            .enumerate()
            .filter_map(|(index, op)| {
                let RenderOp::DrawBatch(batch) = op else {
                    return None;
                };
                self.meshes.contains(&batch.mesh).then(|| {
                    serde_json::json!({
                        "op":index, "material":batch.material.0, "mesh":batch.mesh.0,
                        "instances":batch.count, "stencil":batch.stencil_ref,
                    })
                })
            })
            .collect()
    }

    // Deliberately excludes material IDs and component IDs: equivalent custom and
    // built-in materials must not change the diagnostic ordering key.
    fn geometry_key(op: &RenderOp, visuals: &VisualWorld, indices: &[u32]) -> Vec<u32> {
        let RenderOp::DrawBatch(batch) = op else {
            return vec![u32::MAX];
        };
        let mut key = vec![
            batch.mesh.0,
            batch.texture.map_or(0, |t| t.0),
            batch.count as u32,
        ];
        for index in &indices[batch.start..batch.start + batch.count] {
            let instance = &visuals.instances()[*index as usize];
            key.extend(
                instance
                    .transform
                    .model
                    .iter()
                    .flatten()
                    .map(|v| v.to_bits()),
            );
            key.extend(instance.color.iter().map(|v| v.to_bits()));
            key.push(instance.opacity.to_bits());
        }
        key
    }

    pub(in super::super) fn reorder(
        &self,
        ops: &[RenderOp],
        visuals: &VisualWorld,
        indices: &[u32],
    ) -> Option<Vec<RenderOp>> {
        if self.pool.is_none() {
            return None;
        }
        *self.native_truck_batches.borrow_mut() = self.truck_batches(ops);
        if self.order == "native" {
            *self.recorded_truck_batches.borrow_mut() = self.truck_batches(ops);
            return None;
        }
        let mut reordered = ops.to_vec();
        // Never move a draw through clip entry/exit operations or clipped draws.
        // Batch starts still reference the original, unchanged instance stream.
        let is_unclipped =
            |op: &RenderOp| matches!(op, RenderOp::DrawBatch(b) if b.stencil_ref == 0);
        let mut start = 0;
        while start < reordered.len() {
            if !is_unclipped(&reordered[start]) {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < reordered.len() && is_unclipped(&reordered[end]) {
                end += 1;
            }
            if self.order == "geometry" {
                reordered[start..end]
                    .sort_by_cached_key(|op| Self::geometry_key(op, visuals, indices));
            } else {
                reordered[start..end].sort_by_key(|op| {
                    let RenderOp::DrawBatch(batch) = op else {
                        unreachable!()
                    };
                    let truck = self.meshes.contains(&batch.mesh);
                    if self.order == "first" { !truck } else { truck }
                });
            }
            start = end;
        }
        *self.recorded_truck_batches.borrow_mut() = self.truck_batches(&reordered);
        *self.geometry_order.borrow_mut() = reordered
            .iter()
            .map(|op| {
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                Self::geometry_key(op, visuals, indices).hash(&mut hash);
                hash.finish()
            })
            .collect();
        Some(reordered)
    }

    pub(in super::super) fn reset(
        &self,
        commands: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        eye: usize,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(pool) = &self.pool {
            unsafe {
                commands.reset_query_pool(pool.clone(), eye as u32..eye as u32 + 1)?;
            }
        }
        Ok(())
    }

    pub(in super::super) fn begin(
        &self,
        commands: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        eye: usize,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(pool) = &self.pool {
            unsafe {
                commands.begin_query(pool.clone(), eye as u32, QueryControlFlags::empty())?;
            }
        }
        Ok(())
    }

    pub(in super::super) fn end(
        &self,
        commands: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        eye: usize,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(pool) = &self.pool {
            commands.end_query(pool.clone(), eye as u32)?;
        }
        Ok(())
    }

    pub(in super::super) fn statistics(&self) -> Result<serde_json::Value, Box<dyn Error>> {
        let Some(pool) = &self.pool else {
            return Ok(serde_json::Value::Null);
        };
        let mut values = [0u64; 6];
        if !pool.get_results(0..2, &mut values, QueryResultFlags::empty())? {
            return Err("opaque pipeline statistics unavailable after GPU completion".into());
        }
        Ok(serde_json::json!({
            "input_vertices":values[0] + values[3],
            "vertex_invocations":values[1] + values[4],
            "fragment_invocations":values[2] + values[5],
        }))
    }

    pub(in super::super) fn description(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled":self.pool.is_some(), "order":self.order,
            "native_truck_batches":*self.native_truck_batches.borrow(),
            "recorded_truck_batches":*self.recorded_truck_batches.borrow(),
            "geometry_order":*self.geometry_order.borrow(),
        })
    }
}
