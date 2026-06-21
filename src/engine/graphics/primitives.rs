/// Mesh helpers / basic primitives placeholder.

/// Column-major 4x4 transform matrix.
pub type TransformMatrix = [[f32; 4]; 4];

/// Minimal transform (placeholder).
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4], // quat xyzw
    pub scale: [f32; 3],

    /// Cached model matrix (column-major). Keep this in sync with TRS.
    pub model: TransformMatrix,

    /// Cached world matrix (column-major).
    ///
    /// This is computed/maintained by `TransformSystem` by propagating parent transforms.
    /// It should be treated as derived runtime state.
    pub matrix_world: TransformMatrix,
}

impl Default for Transform {
    fn default() -> Self {
        let translation = [0.0; 3];
        let rotation = [0.0, 0.0, 0.0, 1.0];
        let scale = [1.0; 3];
        let model = [
            [scale[0], 0.0, 0.0, 0.0],
            [0.0, scale[1], 0.0, 0.0],
            [0.0, 0.0, scale[2], 0.0],
            [translation[0], translation[1], translation[2], 1.0],
        ];
        Self {
            translation,
            rotation,
            scale,
            model,
            matrix_world: model,
        }
    }
}

impl Transform {
    /// Recompute `self.model` from translation/rotation/scale.
    pub fn recompute_model(&mut self) {
        let [tx, ty, tz] = self.translation;
        let [sx, sy, sz] = self.scale;
        let [x, y, z, w] = self.rotation;

        // Normalize quat defensively.
        let len2 = x * x + y * y + z * z + w * w;
        let inv_len = if len2 > 0.0 { len2.sqrt().recip() } else { 1.0 };
        let (x, y, z, w) = (x * inv_len, y * inv_len, z * inv_len, w * inv_len);

        // Quaternion to rotation matrix (column-major).
        let xx = x * x;
        let yy = y * y;
        let zz = z * z;
        let xy = x * y;
        let xz = x * z;
        let yz = y * z;
        let wx = w * x;
        let wy = w * y;
        let wz = w * z;

        let r00 = 1.0 - 2.0 * (yy + zz);
        let r01 = 2.0 * (xy + wz);
        let r02 = 2.0 * (xz - wy);

        let r10 = 2.0 * (xy - wz);
        let r11 = 1.0 - 2.0 * (xx + zz);
        let r12 = 2.0 * (yz + wx);

        let r20 = 2.0 * (xz + wy);
        let r21 = 2.0 * (yz - wx);
        let r22 = 1.0 - 2.0 * (xx + yy);

        // Apply scale by scaling the rotation columns.
        let c0 = [r00 * sx, r01 * sx, r02 * sx, 0.0];
        let c1 = [r10 * sy, r11 * sy, r12 * sy, 0.0];
        let c2 = [r20 * sz, r21 * sz, r22 * sz, 0.0];
        let c3 = [tx, ty, tz, 1.0];

        self.model = [c0, c1, c2, c3];
    }
}

/// Renderable component: references renderer-managed resources.
/// Vulkan-minded: material -> pipeline/layout + descriptors.
///
/// The mesh here is a *CPU-side* asset handle. `RenderAssets` stores the actual `CpuMesh`
/// and uploads it to the renderer on demand.
#[derive(Debug, Clone)]
pub struct Renderable {
    /// The mesh actually used for rendering.
    pub mesh: CpuMeshHandle,

    /// The "base" mesh this renderable was derived from.
    ///
    /// For UV-baked variants (e.g. text glyphs), `mesh` is a dynamically-registered clone, while
    /// `base_mesh` stays as the original (typically `CpuMeshHandle::QUAD_2D`).
    ///
    /// For normal renderables, `base_mesh == mesh`.
    pub base_mesh: CpuMeshHandle,
    pub material: MaterialHandle,
}

impl Renderable {
    pub fn new(mesh: CpuMeshHandle, material: MaterialHandle) -> Self {
        Self {
            mesh,
            base_mesh: mesh,
            material,
        }
    }

    pub fn with_base_mesh(mut self, base_mesh: CpuMeshHandle) -> Self {
        self.base_mesh = base_mesh;
        self
    }
}

/// GPU-facing renderable record stored in `VisualWorld`.
///
/// This is intentionally a thin, renderer-ready version of `Renderable`.
/// It avoids pulling any ECS concepts into `VisualWorld`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuRenderable {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
}

impl GpuRenderable {
    pub fn new(mesh: MeshHandle, material: MaterialHandle) -> Self {
        Self { mesh, material }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferHandle(pub u32);

/// Vertex buffer layout description (API-agnostic placeholder).
#[derive(Debug, Clone)]
pub struct VertexLayout {
    pub stride: u32,
    pub attributes: &'static [VertexAttribute],
}

#[derive(Debug, Clone, Copy)]
pub struct VertexAttribute {
    pub location: u32,
    pub offset: u32,
    pub format: VertexFormat,
}

#[derive(Debug, Clone, Copy)]
pub enum VertexFormat {
    Float32x2,
    Float32x3,
    Float32x4,
    Uint32,
}

// CPU-side mesh handles
impl MeshHandle {
    pub const TRIANGLE: MeshHandle = MeshHandle(2);
    pub const SQUARE: MeshHandle = MeshHandle(3);

    pub const CUBE: MeshHandle = MeshHandle(0);
    pub const TETRAHEDRON: MeshHandle = MeshHandle(1);
}

/// Renderer-owned GPU mesh resource (looked up by `MeshHandle`).
#[derive(Debug, Clone, Copy)]
pub struct GpuMesh {
    pub vertex_buffer: BufferHandle,
    pub index_buffer: BufferHandle,
    pub index_count: u32,
    pub vertex_layout: &'static VertexLayout,
}

/// Renderer-owned resource handles (lightweight ids into renderer/asset tables).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle(pub u32);

/// CPU-side mesh identity (owned by `RenderAssets`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CpuMeshHandle(pub u32);

impl CpuMeshHandle {
    // These constants are relied on by scene serialization and built-in inference.
    // Keep in sync with `RenderAssets::register_builtin_meshes` order.
    pub const TRIANGLE_2D: CpuMeshHandle = CpuMeshHandle(0);
    pub const QUAD_2D: CpuMeshHandle = CpuMeshHandle(1);
    pub const CUBE: CpuMeshHandle = CpuMeshHandle(2);
    pub const TETRAHEDRON: CpuMeshHandle = CpuMeshHandle(3);
    pub const SPHERE: CpuMeshHandle = CpuMeshHandle(4);

    // Appended built-ins (keep stable and in sync with RenderAssets registration order).
    pub const CONE: CpuMeshHandle = CpuMeshHandle(5);
    pub const CIRCLE_2D: CpuMeshHandle = CpuMeshHandle(6);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceHandle(pub u32);

/// Renderer-owned material definition (API-agnostic placeholder).
/// For now we reference shaders by name/path; later this becomes pipeline state + descriptor layouts.
#[derive(Debug, Clone)]
pub struct Material {
    pub vertex_shader: &'static str,
    pub fragment_shader: &'static str,
    // Later:
    // pub pipeline_config: PipelineConfig,
    // pub uniforms: MaterialUniforms,
}

// Optional convenience: built-in material names/paths.
impl Material {
    /// Unlit material intended for normal mesh rendering (vertex/index buffers + transforms).
    pub const UNLIT_MESH: Material = Material {
        vertex_shader: "assets/shaders/unlit-mesh.vert",
        fragment_shader: "assets/shaders/unlit-mesh.frag",
    };

    /// Toon material used by the Vulkano renderer bring-up pipeline.
    pub const TOON_MESH: Material = Material {
        vertex_shader: "assets/shaders/toon-mesh.vert",
        fragment_shader: "assets/shaders/toon-mesh.frag",
    };

    /// Skinned toon material (uses a skinned vertex shader).
    pub const SKINNED_TOON_MESH: Material = Material {
        vertex_shader: "assets/shaders/skinned-toon-mesh.vert",
        fragment_shader: "assets/shaders/toon-mesh.frag",
    };

    /// Emissive toon material.
    pub const EMISSIVE_TOON_MESH: Material = Material {
        vertex_shader: "assets/shaders/toon-mesh.vert",
        fragment_shader: "assets/shaders/emissive-toon-mesh.frag",
    };

    /// Skinned emissive toon material.
    pub const SKINNED_EMISSIVE_TOON_MESH: Material = Material {
        vertex_shader: "assets/shaders/skinned-toon-mesh.vert",
        fragment_shader: "assets/shaders/emissive-toon-mesh.frag",
    };

    /// Procedural square grid material.
    pub const GRID_MESH: Material = Material {
        vertex_shader: "assets/shaders/grid.vert",
        fragment_shader: "assets/shaders/grid-square.frag",
    };

    /// Planar mirror material.
    pub const MIRROR: Material = Material {
        vertex_shader: "assets/shaders/mirror-mesh.vert",
        fragment_shader: "assets/shaders/mirror-mesh.frag",
    };
}

impl MaterialHandle {
    /// Unlit mesh material (see `Material::UNLIT_MESH`).
    pub const UNLIT_MESH: MaterialHandle = MaterialHandle(0);

    /// Toon mesh material (see `Material::TOON_MESH`).
    pub const TOON_MESH: MaterialHandle = MaterialHandle(1);

    /// Skinned toon mesh material (see `Material::SKINNED_TOON_MESH`).
    pub const SKINNED_TOON_MESH: MaterialHandle = MaterialHandle(2);

    /// Emissive toon mesh material (see `Material::EMISSIVE_TOON_MESH`).
    pub const EMISSIVE_TOON_MESH: MaterialHandle = MaterialHandle(3);

    /// Skinned emissive toon mesh material (see `Material::SKINNED_EMISSIVE_TOON_MESH`).
    pub const SKINNED_EMISSIVE_TOON_MESH: MaterialHandle = MaterialHandle(4);

    /// Procedural square grid material (see `Material::GRID_MESH`).
    pub const GRID_MESH: MaterialHandle = MaterialHandle(5);

    /// Mirror material for planar reflections.
    pub const MIRROR: MaterialHandle = MaterialHandle(6);
}
