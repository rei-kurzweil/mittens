//! Bounded worker; no boolean or triangulation executes in the render loop.
use crate::engine::ecs::ComponentId;
use crate::engine::graphics::{csg_mesh::Solid, mesh::CpuMesh};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

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
    sender: SyncSender<CutJob>,
    receiver: Receiver<CutResult>,
}
impl Default for CsgSystem {
    fn default() -> Self {
        let (sender, jobs) = mpsc::sync_channel::<CutJob>(8);
        let (results, receiver) = mpsc::sync_channel(8);
        std::thread::Builder::new()
            .name("mittens-csg".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let started = std::time::Instant::now();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        use crate::engine::graphics::csg_mesh::Operation;
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
                            owner: job.owner,
                            revision: job.revision,
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
        self.sender.try_send(job).map_err(|e| match e {
            TrySendError::Full(job) | TrySendError::Disconnected(job) => job,
        })
    }
    pub fn poll(&self) -> Option<CutResult> {
        self.receiver.try_recv().ok()
    }
}
