//! Bounded worker; no boolean or triangulation executes in the render loop.
use crate::engine::ecs::ComponentId;
use crate::engine::graphics::{csg_mesh::Solid, mesh::CpuMesh};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, SyncSender, TrySendError},
};

/// Geometry is already resolved by the caller (for example from a GLB mesh).
/// Mesh and cutter use the same local coordinate frame. Import and validation
/// run on the CSG worker; the source asset remains immutable and shareable.
#[derive(Debug)]
pub struct MeshCutJob {
    pub owner: ComponentId,
    pub revision: u64,
    pub mesh: Arc<CpuMesh>,
    pub cutter: Solid,
}

#[derive(Debug)]
enum WorkerJob {
    Cells(CutJob),
    Mesh(MeshCutJob),
}

#[derive(Debug)]
pub struct CutJob {
    pub owner: ComponentId,
    pub revision: u64,
    pub cells: Vec<(usize, Solid, Solid, [[f32; 4]; 4])>,
    pub render_cells: Vec<(usize, Solid, [[f32; 4]; 4])>,
}
#[derive(Debug)]
pub struct CutCell {
    pub index: usize,
    pub solid: Solid,
    pub mesh: CpuMesh,
}
#[derive(Debug)]
pub struct CutBatch {
    pub cells: Vec<CutCell>,
    pub debris: Vec<CpuMesh>,
    pub render_mesh: Option<CpuMesh>,
}
#[derive(Debug)]
pub struct CutResult {
    pub owner: ComponentId,
    pub revision: u64,
    pub result: Result<CutBatch, String>,
    pub elapsed: std::time::Duration,
}
#[derive(Debug)]
pub struct CsgSystem {
    sender: SyncSender<WorkerJob>,
    receiver: Receiver<CutResult>,
}
impl Default for CsgSystem {
    fn default() -> Self {
        let (sender, jobs) = mpsc::sync_channel::<WorkerJob>(8);
        let (results, receiver) = mpsc::sync_channel(8);
        std::thread::Builder::new()
            .name("mittens-csg".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let (owner, revision) = match &job {
                        WorkerJob::Cells(job) => (job.owner, job.revision),
                        WorkerJob::Mesh(job) => (job.owner, job.revision),
                    };
                    let started = std::time::Instant::now();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        use crate::engine::graphics::csg_mesh::Operation;
                        let job = match job {
                            WorkerJob::Cells(job) => job,
                            WorkerJob::Mesh(job) => {
                                let solid = Solid::from_mesh(&job.mesh)?;
                                let identity = crate::utils::math::mat4_identity();
                                CutJob {
                                    owner: job.owner,
                                    revision: job.revision,
                                    cells: vec![(0, solid.clone(), job.cutter, identity)],
                                    render_cells: vec![(0, solid, identity)],
                                }
                            }
                        };
                        let mut cells = Vec::new();
                        let mut removed: Option<Solid> = None;
                        for (index, solid, cutter, to_owner) in job.cells {
                            let piece = solid.boolean(&cutter, Operation::Intersect)?;
                            if piece.empty() {
                                continue;
                            }
                            let solid = solid.boolean(&cutter, Operation::Subtract)?;
                            let mesh = solid.mesh();
                            cells.push(CutCell { index, solid, mesh });
                            let piece = piece.transformed(to_owner);
                            removed = Some(match removed {
                                Some(previous) => previous.boolean(&piece, Operation::Union)?,
                                None => piece,
                            });
                        }
                        let debris = match removed {
                            Some(solid) => solid.fragments()?.iter().map(Solid::mesh).collect(),
                            None => Vec::new(),
                        };
                        let render_mesh = (!cells.is_empty())
                            .then(|| batch_render_mesh(job.render_cells, &cells));
                        Ok(CutBatch {
                            cells,
                            debris,
                            render_mesh,
                        })
                    }))
                    .unwrap_or_else(|_| {
                        Err("CSG backend panicked; previous geometry retained".into())
                    });
                    if results
                        .send(CutResult {
                            owner,
                            revision,
                            result,
                            elapsed: started.elapsed(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("start CSG worker");
        Self { sender, receiver }
    }
}
/// Batch the surviving chunks for drawing without performing a whole-source
/// boolean. Spatial queries keep their individual precise cell meshes.
fn batch_render_mesh(sources: Vec<(usize, Solid, [[f32; 4]; 4])>, changes: &[CutCell]) -> CpuMesh {
    use crate::utils::math::{mat4_inverse, mat4_mul_vec4, vec3_normalize};
    let changes: std::collections::HashMap<_, _> = changes.iter().map(|c| (c.index, c)).collect();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (index, old, model) in sources {
        let solid = changes.get(&index).map_or(&old, |c| &c.solid);
        if solid.empty() {
            continue;
        }
        let mesh = solid.mesh();
        let inverse = mat4_inverse(model).expect("valid retained grid frame");
        let base = vertices.len() as u32;
        vertices.extend(mesh.vertices.into_iter().map(|mut v| {
            let p = mat4_mul_vec4(model, [v.pos[0], v.pos[1], v.pos[2], 1.]);
            v.pos = [p[0], p[1], p[2]];
            v.normal = vec3_normalize(std::array::from_fn(|i| {
                (0..3).map(|j| inverse[i][j] * v.normal[j]).sum()
            }));
            v
        }));
        indices.extend(mesh.indices_u32.into_iter().map(|i| base + i));
    }
    CpuMesh::new(vertices, indices)
}
impl CsgSystem {
    pub fn submit(&self, job: CutJob) -> Result<(), CutJob> {
        self.sender
            .try_send(WorkerJob::Cells(job))
            .map_err(|e| match e {
                TrySendError::Full(WorkerJob::Cells(job))
                | TrySendError::Disconnected(WorkerJob::Cells(job)) => job,
                _ => unreachable!("try_send returns the submitted job variant"),
            })
    }
    /// Submit a small rigid mesh/chunk for geometry-only subtraction. The
    /// returned cell's Solid can be retained for later cuts via `submit`.
    /// Rejected imports return an error result; no render/collision state is
    /// changed here. As with cell jobs, the caller owns publication/rollback.
    pub fn submit_mesh(&self, job: MeshCutJob) -> Result<(), MeshCutJob> {
        self.sender
            .try_send(WorkerJob::Mesh(job))
            .map_err(|e| match e {
                TrySendError::Full(WorkerJob::Mesh(job))
                | TrySendError::Disconnected(WorkerJob::Mesh(job)) => job,
                _ => unreachable!("try_send returns the submitted job variant"),
            })
    }
    pub fn poll(&self) -> Option<CutResult> {
        self.receiver.try_recv().ok()
    }
}

#[cfg(test)]
#[path = "csg_system_tests.rs"]
mod tests;
