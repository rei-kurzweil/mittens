use std::collections::HashMap;

use crate::engine::graphics::MeshUploader;
use crate::engine::graphics::mesh::CpuMesh;
use crate::engine::graphics::mesh::MeshFactory;
use crate::engine::graphics::primitives::{CpuMeshHandle, MeshHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinMeshType {
    Triangle2D,
    Quad2D,
    Cube,
    Tetrahedron,
}

/// Renderer-side asset registry used by ECS systems.
///
/// Design:
/// - ECS and gameplay code refer to geometry by `CpuMeshHandle` (CPU asset identity).
/// - The renderer owns GPU resources and returns `MeshHandle`.
/// - `RenderAssets` bridges the two and caches uploads.
#[derive(Debug, Default)]
pub struct RenderAssets {
    cpu_meshes: Vec<CpuMesh>,
    gpu_meshes: HashMap<CpuMeshHandle, MeshHandle>,

    /// Built-in CPU mesh handles (stable ids) keyed by mesh kind.
    ///
    /// These are pre-registered in `RenderAssets::new()` so scenes that refer to built-in
    /// meshes by numeric id can load without any explicit setup.
    builtin_meshes: HashMap<BuiltinMeshType, CpuMeshHandle>,
}

impl RenderAssets {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.register_builtin_meshes();
        s
    }

    /// Return the CPU mesh handle for a built-in mesh.
    ///
    /// Builtins are pre-registered, but this is also safe to call if a `RenderAssets` was
    /// constructed via `Default`.
    pub fn get_mesh(&mut self, mesh: BuiltinMeshType) -> CpuMeshHandle {
        self.ensure_builtin_mesh(mesh)
    }

    fn register_builtin_meshes(&mut self) {
        // Keep this order stable so serialized scenes that refer to built-in meshes by id
        // stay valid across runs.
        let _ = self.ensure_builtin_mesh(BuiltinMeshType::Triangle2D);
        let _ = self.ensure_builtin_mesh(BuiltinMeshType::Quad2D);
        let _ = self.ensure_builtin_mesh(BuiltinMeshType::Cube);
        let _ = self.ensure_builtin_mesh(BuiltinMeshType::Tetrahedron);
    }

    fn ensure_builtin_mesh(&mut self, mesh: BuiltinMeshType) -> CpuMeshHandle {
        if let Some(h) = self.builtin_meshes.get(&mesh).copied() {
            return h;
        }

        let cpu_mesh = match mesh {
            BuiltinMeshType::Triangle2D => MeshFactory::triangle_2d(),
            BuiltinMeshType::Quad2D => MeshFactory::quad_2d(),
            BuiltinMeshType::Cube => MeshFactory::cube(),
            BuiltinMeshType::Tetrahedron => MeshFactory::tetrahedron(),
        };

        let h = self.register_mesh(cpu_mesh);
        self.builtin_meshes.insert(mesh, h);
        h
    }

    /// Register CPU mesh data and get a stable CPU-side handle.
    ///
    /// If callers want reuse, they should keep and share this handle.
    pub fn register_mesh(&mut self, mesh: CpuMesh) -> CpuMeshHandle {
        let h = CpuMeshHandle(self.cpu_meshes.len() as u32);
        self.cpu_meshes.push(mesh);
        h
    }

    pub fn cpu_mesh(&self, h: CpuMeshHandle) -> Option<&CpuMesh> {
        self.cpu_meshes.get(h.0 as usize)
    }

    /// Get (or upload) a mesh into the renderer and return a renderer-owned `MeshHandle`.
    pub fn gpu_mesh_handle(
        &mut self,
        uploader: &mut dyn MeshUploader,
        cpu_mesh: CpuMeshHandle,
    ) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        if let Some(h) = self.gpu_meshes.get(&cpu_mesh).copied() {
            return Ok(h);
        }

        let mesh = self
            .cpu_mesh(cpu_mesh)
            .ok_or("RenderAssets: invalid CpuMeshHandle")?;
        let h = uploader.upload_mesh(mesh)?;
        self.gpu_meshes.insert(cpu_mesh, h);
        Ok(h)
    }
}
