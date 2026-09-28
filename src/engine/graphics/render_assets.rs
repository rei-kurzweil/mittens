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
    Annulus2D,
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
    wireframe_sphere_meshes: HashMap<(u32, u32, u32), CpuMeshHandle>,
    wireframe_icosahedron_meshes: HashMap<(u32, u32, u32), CpuMeshHandle>,
    capsule_y_meshes: HashMap<(u32, u32), CpuMeshHandle>,

    /// Authored polygons have globally unique keys within this asset registry.
    /// Keep keys namespaced and versioned (for example `ui/accordion/chevron/v1`).
    polygon_meshes: HashMap<String, (Vec<[f32; 2]>, CpuMeshHandle)>,
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
        let _ = self.ensure_builtin_mesh(BuiltinMeshType::Annulus2D);
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
            BuiltinMeshType::Annulus2D => MeshFactory::annulus_2d(0.45, 0.5, 64),
            BuiltinMeshType::Circle2D => MeshFactory::circle_2d(0.5, 64),
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

    /// Return the mesh registered under `mesh_key`, defining it from `points` on first use.
    ///
    /// The key is authoritative: after first registration, callers must use a new key to
    /// define different geometry. Existing keys return without inspecting `points`.
    pub fn polygon_mesh(
        &mut self,
        mesh_key: &str,
        points: &[[f32; 2]],
    ) -> Result<CpuMeshHandle, String> {
        if mesh_key.trim().is_empty() {
            return Err("polygon mesh_key must not be empty".to_string());
        }
        if let Some((_, handle)) = self.polygon_meshes.get(mesh_key) {
            return Ok(*handle);
        }
        let canonical = MeshFactory::canonical_polygon_2d(points)?;
        let mesh = MeshFactory::polygon_2d(&canonical)?;
        let handle = self.register_mesh(mesh);
        self.polygon_meshes
            .insert(mesh_key.to_string(), (canonical, handle));
        Ok(handle)
    }

    pub(crate) fn polygon_mesh_points(&self, mesh_key: &str) -> Option<&[[f32; 2]]> {
        self.polygon_meshes
            .get(mesh_key)
            .map(|(points, _)| points.as_slice())
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

    pub fn wireframe_sphere_mesh(
        &mut self,
        latitude_segments: u32,
        longitude_segments: u32,
        thickness: f32,
    ) -> CpuMeshHandle {
        let latitude_segments = latitude_segments.max(2);
        let longitude_segments = longitude_segments.max(3);
        let thickness = thickness.clamp(1.0e-4, 0.5);
        let key = (latitude_segments, longitude_segments, thickness.to_bits());
        if let Some(handle) = self.wireframe_sphere_meshes.get(&key).copied() {
            return handle;
        }
        let handle = self.register_mesh(MeshFactory::wireframe_sphere(
            latitude_segments,
            longitude_segments,
            thickness,
        ));
        self.wireframe_sphere_meshes.insert(key, handle);
        handle
    }

    pub fn wireframe_icosahedron_mesh(
        &mut self,
        tessellations: u32,
        sphericalness: f32,
        thickness: f32,
    ) -> CpuMeshHandle {
        let sphericalness = sphericalness.clamp(0.0, 1.0);
        let thickness = thickness.clamp(1.0e-4, 0.5);
        let key = (tessellations, sphericalness.to_bits(), thickness.to_bits());
        if let Some(handle) = self.wireframe_icosahedron_meshes.get(&key).copied() {
            return handle;
        }
        let handle = self.register_mesh(MeshFactory::wireframe_icosahedron(
            tessellations,
            sphericalness,
            thickness,
        ));
        self.wireframe_icosahedron_meshes.insert(key, handle);
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

#[cfg(test)]
mod tests {
    use super::{BuiltinMeshType, RenderAssets};
    use crate::engine::graphics::primitives::CpuMeshHandle;

    const CHEVRON: &[[f32; 2]] = &[
        [-0.5, 0.25],
        [0.0, -0.25],
        [0.5, 0.25],
        [0.35, 0.4],
        [0.0, 0.05],
        [-0.35, 0.4],
    ];

    #[test]
    fn renamed_annulus_keeps_its_handle_and_circle_appends_a_new_mesh() {
        let mut assets = RenderAssets::new();
        assert_eq!(
            assets.get_mesh(BuiltinMeshType::Annulus2D),
            CpuMeshHandle::ANNULUS_2D
        );
        assert_eq!(
            assets.get_mesh(BuiltinMeshType::Circle2D),
            CpuMeshHandle::CIRCLE_2D
        );
        assert_eq!(
            assets
                .cpu_mesh(CpuMeshHandle::ANNULUS_2D)
                .unwrap()
                .vertices
                .len(),
            128
        );
        assert_eq!(
            assets
                .cpu_mesh(CpuMeshHandle::CIRCLE_2D)
                .unwrap()
                .vertices
                .len(),
            65
        );
    }

    #[test]
    fn named_polygons_use_first_registration_as_authoritative_identity() {
        let mut assets = RenderAssets::new();
        let initial_count = assets.cpu_mesh_count();
        let first = assets.polygon_mesh("ui/chevron/v1", CHEVRON).unwrap();
        let reused = assets.polygon_mesh("ui/chevron/v1", &[]).unwrap();
        assert_eq!(first, reused);
        assert_eq!(assets.cpu_mesh_count(), initial_count + 1);

        let mut changed = CHEVRON.to_vec();
        changed[0][0] = -0.45;
        let still_reused = assets.polygon_mesh("ui/chevron/v1", &changed).unwrap();
        assert_eq!(first, still_reused);
        assert_eq!(assets.cpu_mesh_count(), initial_count + 1);

        let second = assets.polygon_mesh("ui/chevron/v2", CHEVRON).unwrap();
        assert_ne!(first, second);
        assert_eq!(assets.cpu_mesh_count(), initial_count + 2);
        assert_eq!(assets.polygon_mesh_points("ui/chevron/v1"), Some(CHEVRON));
    }

    #[test]
    fn named_polygon_rejects_empty_keys() {
        let error = RenderAssets::new().polygon_mesh("  ", CHEVRON).unwrap_err();
        assert!(error.contains("must not be empty"), "{error}");
    }
}
