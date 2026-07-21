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
    Sphere,
    Cone,
    Circle2D,
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

    /// Imported CPU meshes keyed by a stable string (e.g. "{gltf_name}:{mesh_name_or_index}:{prim_index}").
    imported_meshes: HashMap<String, CpuMeshHandle>,

    /// Built-in CPU mesh handles (stable ids) keyed by mesh kind.
    ///
    /// These are pre-registered in `RenderAssets::new()` so scenes that refer to built-in
    /// meshes by numeric id can load without any explicit setup.
    builtin_meshes: HashMap<BuiltinMeshType, CpuMeshHandle>,

    /// Procedural unit wireframe boxes keyed by the exact authored thickness bits.
    wireframe_box_meshes: HashMap<u32, CpuMeshHandle>,
    /// Procedural unit wireframe squares keyed by the exact authored thickness bits.
    wireframe_square_meshes: HashMap<u32, CpuMeshHandle>,
    capsule_y_meshes: HashMap<(u32, u32), CpuMeshHandle>,
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
        // Appended to preserve existing numeric ids.
        let _ = self.ensure_builtin_mesh(BuiltinMeshType::Sphere);
        let _ = self.ensure_builtin_mesh(BuiltinMeshType::Cone);
        let _ = self.ensure_builtin_mesh(BuiltinMeshType::Circle2D);
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
            BuiltinMeshType::Sphere => MeshFactory::sphere(),
            BuiltinMeshType::Cone => MeshFactory::cone(32),
            BuiltinMeshType::Circle2D => MeshFactory::circle_2d(0.45, 0.5, 64),
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

    /// Return a shared unit wireframe-box mesh for the requested relative edge thickness.
    pub fn wireframe_box_mesh(&mut self, thickness: f32) -> CpuMeshHandle {
        let thickness = thickness.clamp(1.0e-4, 1.0);
        let key = thickness.to_bits();
        if let Some(handle) = self.wireframe_box_meshes.get(&key).copied() {
            return handle;
        }
        let handle = self.register_mesh(MeshFactory::wireframe_box(thickness));
        self.wireframe_box_meshes.insert(key, handle);
        handle
    }

    /// Return a shared unit wireframe-square mesh for the requested relative edge thickness.
    pub fn wireframe_square_mesh(&mut self, thickness: f32) -> CpuMeshHandle {
        let thickness = thickness.clamp(1.0e-4, 0.5);
        let key = thickness.to_bits();
        if let Some(handle) = self.wireframe_square_meshes.get(&key).copied() {
            return handle;
        }
        let handle = self.register_mesh(MeshFactory::wireframe_square(thickness));
        self.wireframe_square_meshes.insert(key, handle);
        handle
    }

    /// Shared exact upright capsule mesh, keyed by normalized shape dimensions.
    pub fn capsule_y_mesh(&mut self, radius: f32, half_segment: f32) -> CpuMeshHandle {
        let radius = radius.max(0.0);
        let half_segment = half_segment.max(0.0);
        let key = (radius.to_bits(), half_segment.to_bits());
        if let Some(handle) = self.capsule_y_meshes.get(&key).copied() {
            return handle;
        }
        let handle = self.register_mesh(MeshFactory::capsule_y(radius, half_segment, 32, 12));
        self.capsule_y_meshes.insert(key, handle);
        handle
    }

    /// Register an imported mesh and index it by `key` for later lookup.
    pub fn register_imported_mesh(
        &mut self,
        key: impl Into<String>,
        mesh: CpuMesh,
    ) -> CpuMeshHandle {
        let key = key.into();
        let h = self.register_mesh(mesh);
        self.imported_meshes.insert(key, h);
        h
    }

    /// Look up an imported mesh handle by key.
    pub fn imported_mesh(&self, key: &str) -> Option<CpuMeshHandle> {
        self.imported_meshes.get(key).copied()
    }

    pub fn cpu_mesh(&self, h: CpuMeshHandle) -> Option<&CpuMesh> {
        self.cpu_meshes.get(h.0 as usize)
    }

    pub fn cpu_mesh_count(&self) -> usize {
        self.cpu_meshes.len()
    }

    pub fn imported_mesh_count(&self) -> usize {
        self.imported_meshes.len()
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
