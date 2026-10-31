//! Bounded worker; no boolean or triangulation executes in the render loop.
use crate::engine::ecs::ComponentId;
use crate::engine::graphics::{csg_mesh::Solid, mesh::CpuMesh};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

#[derive(Debug)]
pub struct CutJob {
    pub owner: ComponentId,
    pub revision: u64,
    pub cells: Vec<(usize, Solid, Solid, [[f32; 4]; 4])>,
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
                        Ok(CutBatch { cells, debris })
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
