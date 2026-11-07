use super::material_binding_key::material_parameter;
use super::materials::{MaterialInstanceId, MaterialRegistry};
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::Transform;
use crate::engine::graphics::GpuRenderable;
use crate::engine::graphics::MsaaMode;
use crate::engine::graphics::deformation::{DeformationRange, DeformationRangeAllocator};
use crate::engine::graphics::post_processing::PostProcessingConfig;
use crate::engine::graphics::primitives::InstanceHandle;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::graphics::{Skin, SkinId};
use slotmap::{Key, SlotMap};
use std::collections::HashMap;

include!("visual_world/diagnostic_hooks.rs");
mod draw_cache;
mod render_streams;
mod instance_updates;
#[cfg(test)]
mod cache_profile;
#[cfg(test)]
use cache_profile::{DrawCacheProfile, VisualChange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextureFiltering {
    /// Default: linear filtering for both minification and magnification.
    #[default]
    Linear,
    /// Nearest-neighbor filtering for both minification and magnification.
    Nearest,
    /// Nearest for magnification, linear for minification.
    NearestMagnification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraTarget {
    Window,
    Xr,
}

#[derive(Debug, Clone, Copy)]
pub struct CameraData {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub transform: Transform,
}

#[derive(Debug, Clone)]
pub struct VisualCamera {
    pub target: CameraTarget,
    pub eyes: Vec<CameraData>,
}

pub use super::visual_mirror::{MirrorViewerFamily, MirrorCaptureRequest, VisualMirror};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawBatch {
    pub material: crate::engine::graphics::MaterialHandle,
    pub mesh: crate::engine::graphics::primitives::MeshHandle,
    pub texture: Option<crate::engine::graphics::TextureHandle>,
    /// Reflective surfaces bind a capture per view without mutating scene topology.
    pub view_texture_source: Option<ComponentId>,
    pub texture_filtering: TextureFiltering,
    pub quant_steps: f32,
    pub material_instance: Option<MaterialInstanceId>,
    /// Effective stencil reference value for this batch.
    /// 0 = unclipped; >0 = draw only where stencil == this value.
    /// For clip-source instances this is `parent_ref + 1` (their visual draw is inside their own region).
    pub stencil_ref: u8,
    /// Start index into the phase's instance-index array (e.g. `overlay_stream_instances`).
    pub start: usize,
    pub count: usize,
}

/// Packed, renderer-facing parameters for the albedo-derived anime material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimeShadingParams {
    /// RGB shade tint, followed by shade strength.
    pub shade_color_strength: [f32; 4],
    /// RGB rim tint; W is reserved.
    pub rim_color: [f32; 4],
    /// Shade threshold, lit threshold, rim strength, and rim power.
    pub controls: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToonOutlineParams {
    pub color: [f32; 4],
    pub width: f32,
}

impl AnimeShadingParams {
    /// Built-in definition schema. Slots are logical values, not GPU offsets.
    pub fn schema() -> super::materials::MaterialParameterSchema {
        use super::materials::{
            MaterialFieldType::{F32, Vec3},
            MaterialParameterSchema,
        };
        MaterialParameterSchema::new(
            [
                ("shade_color", Vec3),
                ("shade_strength", F32),
                ("rim_color", Vec3),
                ("shade_threshold", F32),
                ("lit_threshold", F32),
                ("rim_strength", F32),
                ("rim_power", F32),
            ]
            .into_iter()
            .map(|(name, ty)| (name.to_owned(), ty)),
        )
    }

    /// Compatibility packing for the existing built-in MaterialUBO interface.
    pub fn from_material(
        schema: &super::materials::MaterialParameterSchema,
        values: &[f32],
    ) -> Self {
        let mut packed = Self::default();
        let copy = |name: &str, target: &mut [f32]| {
            if let Some(value) = schema.read(values, name) {
                assert_eq!(value.len(), target.len());
                target.copy_from_slice(value);
            }
        };
        copy("shade_color", &mut packed.shade_color_strength[..3]);
        copy("shade_strength", &mut packed.shade_color_strength[3..]);
        copy("rim_color", &mut packed.rim_color[..3]);
        copy("shade_threshold", &mut packed.controls[..1]);
        copy("lit_threshold", &mut packed.controls[1..2]);
        copy("rim_strength", &mut packed.controls[2..3]);
        copy("rim_power", &mut packed.controls[3..]);
        packed
    }
    pub fn key_bits(self) -> [u32; 12] {
        let values = [self.shade_color_strength, self.rim_color, self.controls];
        let mut bits = [0; 12];
        for (index, value) in values.into_iter().flatten().enumerate() {
            bits[index] = value.to_bits();
        }
        bits
    }
}

impl From<AnimeShadingParams> for std::sync::Arc<[f32]> {
    fn from(params: AnimeShadingParams) -> Self {
        params
            .shade_color_strength
            .into_iter()
            .chain(params.rim_color.into_iter().take(3))
            .chain(params.controls)
            .collect::<Vec<_>>()
            .into()
    }
}

impl Default for AnimeShadingParams {
    fn default() -> Self {
        Self {
            shade_color_strength: [0.72, 0.50, 0.54, 0.30],
            rim_color: [1.0, 0.85, 0.92, 0.0],
            controls: [0.35, 0.55, 0.18, 4.0],
        }
    }
}

/// One entry in a per-phase DFS render stream.
///
/// The overlay stream is a sequence of these ops that the renderer executes in order,
/// binding different pipelines for stencil writes vs. clipped color draws.
#[derive(Debug, Clone, Copy)]
pub enum RenderOp {
    /// Write stencil INCR for a clip region entry.
    /// Render `instance_index`'s mesh with `pipeline_stencil_incr`, stencil reference = `parent_ref`.
    /// After this op, pixels under the mesh have stencil value `new_ref`.
    EnterClip {
        instance_index: u32,
        parent_ref: u8,
        new_ref: u8,
    },
    /// Draw a batch of instances.
    /// `batch.stencil_ref == 0` → use normal overlay pipeline.
    /// `batch.stencil_ref > 0` → use clipped overlay pipeline with that reference.
    /// Instance indices live in `[batch.start .. batch.start + batch.count)` of the stream's instance array.
    DrawBatch(DrawBatch),
    /// Write stencil DECR to close a clip region.
    /// Render `instance_index`'s mesh with `pipeline_stencil_decr`, stencil reference = `ref_value`.
    ExitClip { instance_index: u32, ref_value: u8 },
}

pub struct VisualWorld {
    pub materials: MaterialRegistry,
    material_consumers: slotmap::SecondaryMap<MaterialInstanceId, Vec<InstanceHandle>>,
    instances: Vec<VisualInstance>,
    clear_color: [f32; 4],
    renderer_msaa_mode: MsaaMode,
    preferred_window_size: Option<[u32; 2]>,
    transmission_depth_compare: bool,
    post_processing: PostProcessingConfig,
    bloom_runtime_enabled: bool,
    bloom_runtime_layers: u32,

    mirrors: Vec<VisualMirror>,
    mirror_captures_enabled: bool,
    mirror_quality_override: Option<i32>,

    // Frame timing stats captured from the main loop (window) and the XR render path.
    // These are best-effort diagnostics and are not used for simulation.
    window_frame_dt_sec: f32,
    xr_frame_dt_sec: Option<f32>,

    // Shared bones palette for all skinned instances.
    // Instances reference a subrange via (bones_base, bones_count).
    //
    // This is *persistent* across frames. We update only the subranges for rigs
    // that become dirty (via transform changes), and we keep offsets stable via a
    // tiny free-list allocator.
    bones_palette: Vec<TransformMatrix>,
    bones_free_ranges: Vec<(u32, u32)>,
    dirty_bones_ranges: Vec<DeformationRange>,
    deformation_ranges: DeformationRangeAllocator,

    // Shared skin definitions (glTF skins), keyed by (uri, skin_index).
    skins: SlotMap<SkinId, Skin>,
    skin_id_by_key: std::collections::HashMap<(String, usize), SkinId>,

    ambient_light: [f32; 3],

    lights: Vec<VisualLight>,
    light_index_by_component: std::collections::HashMap<ComponentId, usize>,
    dirty_lights: bool,

    // Target-scoped camera state. Window is typically mono; XR is stereo.
    visual_cameras: Vec<VisualCamera>,

    // Which CameraXRComponent (by ComponentId) is currently active for XR rig transforms.
    active_xr_camera: Option<ComponentId>,
    // Most recent render target size in pixels (width, height).
    viewport: [f32; 2],
    runtime_texture_handles: HashMap<String, crate::engine::graphics::TextureHandle>,
    retired_runtime_textures: Vec<crate::engine::graphics::TextureHandle>,
    stencil_clip_debug_requested: bool,
    // 2D camera view transform for translation/scale/rotation.
    // Stored as mat3 column vectors padded to vec4 columns (std140 friendly).
    camera_2d: [[f32; 4]; 3],
    dirty_camera: bool,

    next_handle: u32,
    handle_to_index: std::collections::HashMap<InstanceHandle, usize>,
    component_to_handle: std::collections::HashMap<ComponentId, InstanceHandle>,
    morph_inputs: Vec<Vec<(u32, f32)>>,

    // Cached draw data (rebuilt when dirty)
    dirty_draw_cache: bool,
    #[cfg(test)]
    draw_cache_profile: DrawCacheProfile,
    /// True when per-instance data (e.g. model matrices) changed and any cached GPU instance
    /// buffer should be rebuilt/uploaded.
    dirty_instance_data: bool,

    // Background draw data (rebuilt when dirty)
    background_order: Vec<u32>, // indices into `instances`
    background_batches: Vec<DrawBatch>,
    background_occluded_lit_order: Vec<u32>,
    background_occluded_lit_batches: Vec<DrawBatch>,
    background_occluded_lit_emissive_order: Vec<u32>,
    background_occluded_lit_emissive_batches: Vec<DrawBatch>,
    draw_order: Vec<u32>, // indices into `instances`

    // Emissive-only opaque draw data (rebuilt when dirty).
    emissive_draw_order: Vec<u32>,
    emissive_draw_batches: Vec<DrawBatch>,

    // Inverted-hull outlines, drawn after the foreground depth clear and before opaque content.
    outline_order: Vec<u32>,
    outline_stream: Vec<RenderOp>,
    outline_stream_instances: Vec<u32>,

    // Alpha-to-coverage cutout draw data (rebuilt when dirty).
    cutout_order: Vec<u32>,

    // Emissive-only alpha-to-coverage draw data (rebuilt when dirty).
    emissive_cutout_order: Vec<u32>,
    emissive_cutout_batches: Vec<DrawBatch>,

    // DFS-ordered render stream for the cutout phase.
    cutout_stream: Vec<RenderOp>,
    cutout_stream_instances: Vec<u32>,

    // Overlay draw data (rebuilt when dirty).
    // Overlay is drawn on top of all other phases.
    overlay_order: Vec<u32>,

    // Stencil clip sources: indices into `instances` where `is_stencil_clip=true`,
    // sorted ascending by stencil_ref (outer clips first). Rebuilt with draw cache.
    stencil_clip_order: Vec<u32>,

    // DFS-ordered render stream for the overlay phase.
    // overlay_stream_instances holds VisualInstance indices referenced by DrawBatch ops.
    // Rebuilt with draw cache whenever stencil clip state or overlay membership changes.
    overlay_stream: Vec<RenderOp>,
    overlay_stream_instances: Vec<u32>,

    // DFS-ordered render stream for the opaque phase (mirrors overlay_stream).
    opaque_stream: Vec<RenderOp>,
    opaque_stream_instances: Vec<u32>,

    // Dedicated scene-color transmission phase, drawn after opaque/cutout and before
    // ordinary transparency. Sharp and rough paths share the immutable scene snapshot.
    refraction_order: Vec<u32>,
    refraction_stream: Vec<RenderOp>,
    refraction_stream_instances: Vec<u32>,
    rough_transmission_order: Vec<u32>,
    rough_transmission_stream: Vec<RenderOp>,
    rough_transmission_stream_instances: Vec<u32>,

    // Transparent draw data.
    // - Single-layer: cached (order does not depend on view), instanced.
    transparent_single_draw_order: Vec<u32>,
    transparent_single_stream: Vec<RenderOp>,
    transparent_single_stream_instances: Vec<u32>,
    // - Multi-layer: rebuilt per-eye (ordering depends on view), sorted + drawn one-by-one.
    transparent_multi_draw_order: Vec<u32>,
    transparent_multi_draw_batches: Vec<DrawBatch>,
}

#[derive(Debug, Clone, Copy)]
pub struct VisualInstance {
    pub component: ComponentId,
    pub renderable: GpuRenderable,
    pub transform: Transform,
    pub color: [f32; 4],
    pub opacity: f32,
    pub multiple_layers: bool,
    pub transparent_cutout: bool,
    pub background: bool,
    pub background_occluded_lit: bool,
    pub overlay: bool,
    pub emissive: f32,
    pub texture: Option<crate::engine::graphics::TextureHandle>,
    pub texture_filtering: TextureFiltering,
    pub quant_steps: f32,
    pub material_instance: Option<MaterialInstanceId>,
    pub toon_outline: Option<ToonOutlineParams>,
    /// IOR, effective thickness, strength, and viewport-edge fade.
    pub transmission: [f32; 4],
    /// Rough-transmission filtering control. Non-rough material models ignore it.
    pub transmission_roughness: f32,

    /// Base index into `VisualWorld::bones_palette`.
    pub bones_base: u32,
    /// Number of bone matrices for this instance.
    pub bones_count: u32,
    /// Renderer-owned persistent deformation cache range.
    pub deformed_base: u32,
    pub deformed_count: u32,
    /// CPU-only scheduling state.
    pub deformation_dirty: bool,

    /// Which clip region this instance is inside. 0 = unclipped.
    pub stencil_ref: u8,
    /// When true, this instance writes stencil before its descendant subtree draws.
    /// It also draws normally in the color pass (double duty: clip source + background quad).
    pub is_stencil_clip: bool,
}

impl VisualInstance {
    /// Read shared retained values; draw records store only material identity.
    pub fn anime_shading(&self, materials: &MaterialRegistry) -> AnimeShadingParams {
        self.material_instance
            .and_then(|id| materials.instance(id))
            .and_then(|record| {
                materials.definition(record.definition).map(|definition| {
                    AnimeShadingParams::from_material(&definition.parameter_schema, &record.values)
                })
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod material_identity_tests {
    use super::*;
    use crate::engine::ecs::{World, component::ShadingComponent};
    use crate::engine::graphics::{MaterialHandle, MeshHandle};

    #[test]
    fn equal_valued_materials_keep_distinct_batches_and_shared_color_stays_per_renderable() {
        let mut world = World::default();
        let a = world.add_component(ShadingComponent::new());
        let b = world.add_component(ShadingComponent::new());
        let mut visuals = VisualWorld::new();
        let definition = visuals.materials.anime_definition;
        let id_a = visuals
            .materials
            .upsert_source(a, definition, AnimeShadingParams::default());
        let id_b = visuals
            .materials
            .upsert_source(b, definition, AnimeShadingParams::default());
        let mut handles = Vec::new();
        for (index, id) in [id_a, id_a, id_b].into_iter().enumerate() {
            let handle = visuals.register(
                a,
                GpuRenderable::new(MeshHandle(0), MaterialHandle::ANIME_MESH),
                Transform::default(),
                [index as f32 * 0.4, 1.0, 1.0, 1.0],
                1.0,
                false,
                false,
                false,
                false,
                false,
                index as f32,
                None,
                3.0,
            );
            assert!(visuals.bind_material_instance(handle, Some(id)));
            handles.push(handle);
        }
        visuals.prepare_draw_cache();
        let (stream, _) = visuals.opaque_stream();
        let batches: Vec<_> = stream
            .iter()
            .filter_map(|op| match op {
                RenderOp::DrawBatch(batch) => Some(batch),
                _ => None,
            })
            .collect();
        assert_eq!(batches.len(), 2);
        assert!(
            batches
                .iter()
                .any(|batch| batch.material_instance == Some(id_a) && batch.count == 2)
        );
        assert!(
            batches
                .iter()
                .any(|batch| batch.material_instance == Some(id_b) && batch.count == 1)
        );
        assert_ne!(
            visuals.instance(handles[0]).unwrap().color,
            visuals.instance(handles[1]).unwrap().color
        );
        assert_ne!(
            visuals.instance(handles[0]).unwrap().emissive,
            visuals.instance(handles[1]).unwrap().emissive
        );
        let mut edited = AnimeShadingParams::default();
        edited.controls[0] = 0.8;
        visuals.materials.upsert_source(a, definition, edited);
        assert!(!visuals.prepare_draw_cache());
        assert_eq!(visuals.anime_shading(handles[1]), Some(edited));
        assert_eq!(
            visuals.anime_shading(handles[2]),
            Some(AnimeShadingParams::default())
        );
        visuals.remove_material_source(a);
        assert!(!visuals.bind_material_instance(handles[2], Some(id_a)));
        assert_eq!(
            visuals.instance(handles[2]).unwrap().material_instance,
            Some(id_b)
        );
    }
}

fn sanitize_quant_steps(steps: f32) -> f32 {
    if !steps.is_finite() {
        3.0
    } else {
        // Grid materials use the sign as a visual-space selector and the
        // magnitude as authored spacing. Other materials continue to supply
        // positive quantization steps.
        steps.clamp(-1_000_000.0, 1_000_000.0)
    }
}

impl Default for VisualWorld {
    fn default() -> Self {
        let ident4 = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mut t = Transform::default();
        t.model = ident4;
        t.matrix_world = ident4;

        Self {
            materials: MaterialRegistry::default(),
            material_consumers: slotmap::SecondaryMap::new(),
            instances: Vec::new(),
            clear_color: [0.0, 0.0, 0.0, 1.0],
            renderer_msaa_mode: MsaaMode::default(),
            preferred_window_size: None,
            transmission_depth_compare: true,
            post_processing: PostProcessingConfig::default(),
            bloom_runtime_enabled: true,
            bloom_runtime_layers: 1,
            mirrors: Vec::new(),
            mirror_captures_enabled: true,
            mirror_quality_override: None,
            window_frame_dt_sec: 0.0,
            xr_frame_dt_sec: None,

            bones_palette: vec![ident4],
            bones_free_ranges: Vec::new(),
            dirty_bones_ranges: vec![DeformationRange {
                base: 0,
                vertex_count: 1,
            }],
            deformation_ranges: DeformationRangeAllocator::default(),

            skins: SlotMap::with_key(),
            skin_id_by_key: std::collections::HashMap::new(),

            ambient_light: [0.0, 0.0, 0.0],

            lights: Vec::new(),
            light_index_by_component: std::collections::HashMap::new(),
            dirty_lights: true,

            visual_cameras: vec![VisualCamera {
                target: CameraTarget::Window,
                eyes: vec![CameraData {
                    view: ident4,
                    proj: ident4,
                    transform: t,
                }],
            }],

            active_xr_camera: None,
            viewport: [1.0, 1.0],
            runtime_texture_handles: HashMap::new(),
            retired_runtime_textures: Vec::new(),
            stencil_clip_debug_requested: false,
            camera_2d: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
            dirty_camera: true,

            next_handle: 0,
            handle_to_index: std::collections::HashMap::new(),
            component_to_handle: std::collections::HashMap::new(),
            morph_inputs: Vec::new(),

            dirty_draw_cache: true,
            #[cfg(test)]
            draw_cache_profile: DrawCacheProfile::default(),
            dirty_instance_data: true,
            background_order: Vec::new(),
            background_batches: Vec::new(),
            background_occluded_lit_order: Vec::new(),
            background_occluded_lit_batches: Vec::new(),
            background_occluded_lit_emissive_order: Vec::new(),
            background_occluded_lit_emissive_batches: Vec::new(),
            draw_order: Vec::new(),
            emissive_draw_order: Vec::new(),
            emissive_draw_batches: Vec::new(),
            outline_order: Vec::new(),
            outline_stream: Vec::new(),
            outline_stream_instances: Vec::new(),

            cutout_order: Vec::new(),
            emissive_cutout_order: Vec::new(),
            emissive_cutout_batches: Vec::new(),
            cutout_stream: Vec::new(),
            cutout_stream_instances: Vec::new(),

            overlay_order: Vec::new(),

            stencil_clip_order: Vec::new(),

            overlay_stream: Vec::new(),
            overlay_stream_instances: Vec::new(),

            opaque_stream: Vec::new(),
            opaque_stream_instances: Vec::new(),

            refraction_order: Vec::new(),
            refraction_stream: Vec::new(),
            refraction_stream_instances: Vec::new(),
            rough_transmission_order: Vec::new(),
            rough_transmission_stream: Vec::new(),
            rough_transmission_stream_instances: Vec::new(),

            transparent_single_draw_order: Vec::new(),
            transparent_single_stream: Vec::new(),
            transparent_single_stream_instances: Vec::new(),
            transparent_multi_draw_order: Vec::new(),
            transparent_multi_draw_batches: Vec::new(),
        }
    }
}

impl VisualWorld {
    fn is_emissive_instance(instance: &VisualInstance, materials: &MaterialRegistry) -> bool {
        use super::materials::{MaterialEmission, MaterialStateProfile};
        let capabilities = materials
            .resolved_capabilities(instance.material_instance, instance.renderable.material);
        if capabilities.material.emission == MaterialEmission::None {
            return false;
        }
        match capabilities.material.state_profile {
            MaterialStateProfile::BuiltinMeshCompatibility => true,
            MaterialStateProfile::CustomSurface => {
                // Current custom extraction has an explicit coverage contract.
                instance.opacity >= 1.0
                    && !instance.transparent_cutout
                    && !instance.multiple_layers
                    && !instance.background
                    && !instance.background_occluded_lit
                    && !instance.overlay
                    && instance.stencil_ref == 0
                    && !instance.is_stencil_clip
                    && instance.bones_count == 0
            }
        }
    }

    /// Published geometry, including outputs owned by CombineMesh rather than a Renderable.
    pub(crate) fn instance_handle_for_component(
        &self,
        component: ComponentId,
    ) -> Option<InstanceHandle> {
        self.component_to_handle.get(&component).copied()
    }

    pub fn instance(&self, handle: InstanceHandle) -> Option<&VisualInstance> {
        let idx = *self.handle_to_index.get(&handle)?;
        self.instances.get(idx)
    }

    pub fn active_morphs(&self, instance_index: usize) -> &[(u32, f32)] {
        self.morph_inputs
            .get(instance_index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Updates one instance's sparse morph palette input and invalidates only
    /// that cached deformation when the effective values actually changed.
    pub fn set_active_morphs(&mut self, cid: ComponentId, active_morphs: Vec<(u32, f32)>) -> bool {
        let Some(&handle) = self.component_to_handle.get(&cid) else {
            return false;
        };
        let Some(&idx) = self.handle_to_index.get(&handle) else {
            return false;
        };
        if self
            .morph_inputs
            .get(idx)
            .is_some_and(|current| *current == active_morphs)
        {
            return true;
        }
        self.morph_inputs[idx] = active_morphs;
        self.instances[idx].deformation_dirty = true;
        true
    }

    pub fn skin(&self, id: SkinId) -> Option<&Skin> {
        self.skins.get(id)
    }

    pub fn skin_id_for(&self, uri: &str, skin_index: usize) -> Option<SkinId> {
        self.skin_id_by_key
            .get(&(uri.to_string(), skin_index))
            .copied()
    }

    pub fn upsert_skin(
        &mut self,
        uri: &str,
        skin_index: usize,
        joint_node_indices: Vec<usize>,
        inverse_bind_matrices: Vec<TransformMatrix>,
    ) -> SkinId {
        let key = (uri.to_string(), skin_index);
        if let Some(existing) = self.skin_id_by_key.get(&key).copied() {
            if let Some(skin) = self.skins.get_mut(existing) {
                skin.uri = uri.to_string();
                skin.skin_index = skin_index;
                skin.joint_node_indices = joint_node_indices;
                skin.inverse_bind_matrices = inverse_bind_matrices;
                return existing;
            }
        }

        let id = self.skins.insert(Skin {
            id: SkinId::null(),
            uri: uri.to_string(),
            skin_index,
            joint_node_indices,
            inverse_bind_matrices,
        });

        if let Some(s) = self.skins.get_mut(id) {
            s.id = id;
        }

        self.skin_id_by_key.insert(key, id);
        id
    }

    fn bones_identity() -> TransformMatrix {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    fn bones_free_coalesce(&mut self) {
        if self.bones_free_ranges.len() <= 1 {
            return;
        }

        self.bones_free_ranges.sort_by_key(|(b, _)| *b);
        let mut out: Vec<(u32, u32)> = Vec::with_capacity(self.bones_free_ranges.len());
        for (base, len) in self.bones_free_ranges.drain(..) {
            if let Some((prev_base, prev_len)) = out.last_mut() {
                let prev_end = *prev_base + *prev_len;
                if prev_end == base {
                    *prev_len += len;
                    continue;
                }
            }
            out.push((base, len));
        }
        self.bones_free_ranges = out;
    }

    fn bones_alloc_range(&mut self, len: u32) -> u32 {
        // Never allocate index 0 (reserved identity).
        debug_assert!(!self.bones_palette.is_empty());
        debug_assert_eq!(self.bones_palette[0], Self::bones_identity());

        if len == 0 {
            return 0;
        }

        // First-fit from free list.
        for i in 0..self.bones_free_ranges.len() {
            let (base, free_len) = self.bones_free_ranges[i];
            if free_len >= len {
                let alloc_base = base;
                if free_len == len {
                    self.bones_free_ranges.swap_remove(i);
                } else {
                    self.bones_free_ranges[i] = (base + len, free_len - len);
                }
                return alloc_base;
            }
        }

        // Otherwise grow the palette.
        let base = self.bones_palette.len() as u32;
        self.bones_palette.resize(
            self.bones_palette.len() + len as usize,
            Self::bones_identity(),
        );
        base
    }

    fn bones_free_range(&mut self, base: u32, len: u32) {
        if len == 0 {
            return;
        }

        // Never free the reserved identity element.
        if base == 0 {
            return;
        }

        // Fill freed region with identity so sampling a freed slot is benign.
        let start = base as usize;
        let end = (base + len) as usize;
        if end <= self.bones_palette.len() {
            for slot in &mut self.bones_palette[start..end] {
                *slot = Self::bones_identity();
            }
        }

        self.bones_free_ranges.push((base, len));
        self.bones_free_coalesce();
        self.mark_bones_dirty(base, len);
    }

    fn mark_bones_dirty(&mut self, base: u32, vertex_count: u32) {
        if vertex_count == 0 {
            return;
        }
        let mut start = base;
        let mut end = base.saturating_add(vertex_count);
        let mut index = 0;
        while index < self.dirty_bones_ranges.len() {
            let range = self.dirty_bones_ranges[index];
            let range_end = range.base.saturating_add(range.vertex_count);
            if range_end < start || end < range.base {
                index += 1;
                continue;
            }
            start = start.min(range.base);
            end = end.max(range_end);
            self.dirty_bones_ranges.swap_remove(index);
        }
        self.dirty_bones_ranges.push(DeformationRange {
            base: start,
            vertex_count: end - start,
        });
        self.dirty_bones_ranges.sort_by_key(|range| range.base);
    }

    /// Assigns the skin matrices for an instance into the shared palette.
    ///
    /// This keeps `bones_base` stable for an instance unless its bone count changes.
    pub fn set_skin_matrices(&mut self, handle: InstanceHandle, bones: &[TransformMatrix]) -> bool {
        let debug_skin_set = std::env::var("CAT_DEBUG_SKIN_SET")
            .ok()
            .map(|s| {
                let s = s.trim().to_ascii_lowercase();
                s == "1" || s == "true" || s == "on" || s == "yes"
            })
            .unwrap_or(false);

        let Some(&idx) = self.handle_to_index.get(&handle) else {
            if debug_skin_set {
                println!(
                    "[VisualWorld] set_skin_matrices: unknown handle={handle:?} bones_len={}",
                    bones.len()
                );
            }
            return false;
        };

        if bones.is_empty() {
            // Disable skinning for this instance and free its allocation.
            let old_base = self.instances[idx].bones_base;
            let old_count = self.instances[idx].bones_count;
            if old_count != 0 {
                self.bones_free_range(old_base, old_count);
            }
            if self.instances[idx].bones_base != 0 || self.instances[idx].bones_count != 0 {
                self.instances[idx].bones_base = 0;
                self.instances[idx].bones_count = 0;
                self.instances[idx].deformation_dirty = true;
                visual_data_dirty!(self, SetSkinMatrices);
            }
            return true;
        }

        let want_count = bones.len() as u32;
        let mut base = self.instances[idx].bones_base;
        let old_count = self.instances[idx].bones_count;

        if old_count == 0 {
            base = self.bones_alloc_range(want_count);
            self.mark_bones_dirty(base, want_count);
            self.instances[idx].bones_base = base;
            self.instances[idx].bones_count = want_count;
            self.instances[idx].deformation_dirty = true;
            visual_data_dirty!(self, SetSkinMatrices);
        } else if old_count != want_count {
            // Reallocate with new size.
            self.bones_free_range(base, old_count);
            base = self.bones_alloc_range(want_count);
            self.mark_bones_dirty(base, want_count);
            self.instances[idx].bones_base = base;
            self.instances[idx].bones_count = want_count;
            self.instances[idx].deformation_dirty = true;
            visual_data_dirty!(self, SetSkinMatrices);
        }

        // Write into the palette.
        let start = base as usize;
        let end = start + bones.len();
        if end > self.bones_palette.len() {
            self.bones_palette.resize(end, Self::bones_identity());
        }
        if self.bones_palette[start..end] != *bones {
            self.bones_palette[start..end].copy_from_slice(bones);
            self.mark_bones_dirty(base, want_count);
            self.instances[idx].deformation_dirty = true;
        }

        if debug_skin_set {
            println!(
                "[VisualWorld] set_skin_matrices: handle={handle:?} idx={idx} bones_base={} bones_count={} bones_len={}",
                self.instances[idx].bones_base,
                self.instances[idx].bones_count,
                bones.len(),
            );
        }
        true
    }

    pub fn bones_palette(&self) -> &[TransformMatrix] {
        &self.bones_palette
    }

    pub fn take_dirty_bones_ranges(&mut self) -> Vec<DeformationRange> {
        std::mem::take(&mut self.dirty_bones_ranges)
    }

    /// Compatibility helper for callers that do not consume interval information.
    pub fn take_bones_palette_dirty(&mut self) -> bool {
        !self.take_dirty_bones_ranges().is_empty()
    }

    /// Compatibility helper: updates the skin palette range for an instance.
    ///
    /// Prefer `set_skin_matrices()` for stable allocation.
    pub fn update_skin_range(
        &mut self,
        handle: InstanceHandle,
        bones_base: u32,
        bones_count: u32,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            if self.instances[idx].bones_base == bones_base
                && self.instances[idx].bones_count == bones_count
            {
                return true;
            }
            // If the caller is forcing a range, free the old allocation (best-effort).
            let old_base = self.instances[idx].bones_base;
            let old_count = self.instances[idx].bones_count;
            if old_count != 0 && (old_base != bones_base || old_count != bones_count) {
                self.bones_free_range(old_base, old_count);
            }
            self.instances[idx].bones_base = bones_base;
            self.instances[idx].bones_count = bones_count;
            self.instances[idx].deformation_dirty = true;
            visual_data_dirty!(self, UpdateSkinRange);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RenderOp, ToonOutlineParams, VisualWorld};
    use crate::engine::ecs::ComponentId;
    use crate::engine::graphics::primitives::{
        GpuRenderable, MaterialHandle, MeshHandle, Transform,
    };
    use slotmap::KeyData;
    use std::collections::HashMap;

    fn cid(n: u64) -> ComponentId {
        KeyData::from_ffi(n).into()
    }

    fn dummy_renderable() -> GpuRenderable {
        GpuRenderable::new(MeshHandle::SQUARE, MaterialHandle::TOON_MESH)
    }

    fn dummy_emissive_renderable() -> GpuRenderable {
        GpuRenderable::new(MeshHandle::SQUARE, MaterialHandle::EMISSIVE_TOON_MESH)
    }

    fn identity() -> [[f32; 4]; 4] {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    #[test]
    fn identical_palette_recomputation_does_not_dirty_deformation() {
        let mut visuals = VisualWorld::default();
        let handle = visuals.register(
            cid(90),
            dummy_renderable(),
            Transform::default(),
            [1.0; 4],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        visuals.take_dirty_bones_ranges();
        assert!(visuals.set_skin_matrices(handle, &[identity()]));
        assert!(!visuals.take_dirty_bones_ranges().is_empty());
        visuals.mark_deformations_clean();
        assert!(visuals.set_skin_matrices(handle, &[identity()]));
        assert!(visuals.take_dirty_bones_ranges().is_empty());
        assert!(!visuals.instance(handle).unwrap().deformation_dirty);
    }

    #[test]
    fn deformation_ranges_are_stable_reused_and_reallocated_on_size_change() {
        let mut visuals = VisualWorld::default();
        let a = visuals.register(
            cid(91),
            dummy_renderable(),
            Transform::default(),
            [1.0; 4],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let b = visuals.register(
            cid(92),
            dummy_renderable(),
            Transform::default(),
            [1.0; 4],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        visuals.set_skin_matrices(a, &[identity()]);
        visuals.set_skin_matrices(b, &[identity()]);
        let mut counts = HashMap::from([(MeshHandle::SQUARE, 12)]);
        visuals.sync_deformation_ranges(&counts);
        let a_range = (
            visuals.instance(a).unwrap().deformed_base,
            visuals.instance(a).unwrap().deformed_count,
        );
        visuals.sync_deformation_ranges(&counts);
        assert_eq!(
            (
                visuals.instance(a).unwrap().deformed_base,
                visuals.instance(a).unwrap().deformed_count,
            ),
            a_range
        );
        let b_base = visuals.instance(b).unwrap().deformed_base;
        visuals.remove(b);
        let c = visuals.register(
            cid(93),
            dummy_renderable(),
            Transform::default(),
            [1.0; 4],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        visuals.set_skin_matrices(c, &[identity()]);
        visuals.sync_deformation_ranges(&counts);
        assert_eq!(visuals.instance(c).unwrap().deformed_base, b_base);
        counts.insert(MeshHandle::SQUARE, 17);
        visuals.sync_deformation_ranges(&counts);
        assert_eq!(visuals.instance(a).unwrap().deformed_count, 17);
        assert!(visuals.instance(a).unwrap().deformation_dirty);
    }

    #[test]
    fn background_occluded_lit_emissive_subset_includes_only_eligible_instances() {
        let mut visuals = VisualWorld::default();

        let _ = visuals.register(
            cid(100),
            dummy_emissive_renderable(),
            Transform::default(),
            [1.0, 0.8, 0.4, 1.0],
            1.0,
            false,
            false,
            true,
            true,
            false,
            1.0,
            None,
            3.0,
        );
        let _ = visuals.register(
            cid(101),
            dummy_emissive_renderable(),
            Transform::default(),
            [1.0, 0.8, 0.4, 1.0],
            1.0,
            false,
            false,
            true,
            false,
            false,
            1.0,
            None,
            3.0,
        );
        let _ = visuals.register(
            cid(102),
            dummy_renderable(),
            Transform::default(),
            [0.6, 0.6, 0.6, 1.0],
            1.0,
            false,
            false,
            true,
            true,
            false,
            0.0,
            None,
            3.0,
        );

        visuals.prepare_draw_cache();

        assert!(visuals.has_background_occluded_lit_emissive());
        assert_eq!(visuals.background_occluded_lit_order().len(), 2);
        assert_eq!(visuals.background_occluded_lit_emissive_order().len(), 1);
        assert_eq!(visuals.background_occluded_lit_emissive_batches().len(), 1);
    }

    #[test]
    fn background_occluded_lit_non_emissive_keeps_subset_empty() {
        let mut visuals = VisualWorld::default();

        let _ = visuals.register(
            cid(110),
            dummy_renderable(),
            Transform::default(),
            [0.6, 0.6, 0.6, 1.0],
            1.0,
            false,
            false,
            true,
            true,
            false,
            0.0,
            None,
            3.0,
        );

        visuals.prepare_draw_cache();

        assert_eq!(visuals.background_occluded_lit_order().len(), 1);
        assert!(!visuals.has_background_occluded_lit_emissive());
        assert!(visuals.background_occluded_lit_emissive_order().is_empty());
        assert!(
            visuals
                .background_occluded_lit_emissive_batches()
                .is_empty()
        );
    }

    #[test]
    fn no_clip_streams_are_draw_batches_covering_each_phase_order_once() {
        let mut visuals = VisualWorld::default();

        for (component, alpha, cutout, overlay) in [
            (120, 1.0, false, false),
            (121, 1.0, true, false),
            (122, 0.5, false, false),
            (123, 1.0, false, true),
        ] {
            let _ = visuals.register(
                cid(component),
                dummy_renderable(),
                Transform::default(),
                [1.0, 1.0, 1.0, alpha],
                1.0,
                false,
                cutout,
                false,
                false,
                overlay,
                0.0,
                None,
                3.0,
            );
        }

        visuals.prepare_draw_cache();

        for (order, (ops, stream_instances)) in [
            (visuals.draw_order(), visuals.opaque_stream()),
            (visuals.cutout_order(), visuals.cutout_stream()),
            (
                visuals.transparent_single_draw_order(),
                visuals.transparent_single_stream(),
            ),
            (visuals.overlay_order(), visuals.overlay_stream()),
        ] {
            assert_eq!(stream_instances, order);
            assert!(ops.iter().all(|op| matches!(
                op,
                RenderOp::DrawBatch(batch) if batch.stencil_ref == 0
            )));
            let drawn_instances = ops
                .iter()
                .map(|op| match op {
                    RenderOp::DrawBatch(batch) => batch.count,
                    _ => 0,
                })
                .sum::<usize>();
            assert_eq!(drawn_instances, order.len());
        }
    }

    #[test]
    fn outline_stream_reuses_visual_instances_and_separates_static_from_skinned_batches() {
        let mut visuals = VisualWorld::default();
        let static_handle = visuals.register(
            cid(130),
            dummy_renderable(),
            Transform::default(),
            [1.0; 4],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let skinned_handle = visuals.register(
            cid(131),
            GpuRenderable::new(MeshHandle::SQUARE, MaterialHandle::SKINNED_TOON_MESH),
            Transform::default(),
            [1.0; 4],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let outline = ToonOutlineParams {
            color: [0.03, 0.02, 0.05, 1.0],
            width: 0.015,
        };
        assert!(visuals.update_toon_outline(static_handle, Some(outline)));
        assert!(visuals.update_toon_outline(skinned_handle, Some(outline)));

        visuals.prepare_draw_cache();

        let (ops, outline_instances) = visuals.outline_stream();
        assert_eq!(outline_instances.len(), 2);
        assert_eq!(visuals.outline_order(), outline_instances);
        assert!(
            outline_instances
                .iter()
                .all(|index| visuals.draw_order().contains(index))
        );
        assert_eq!(ops.len(), 2);
        assert!(matches!(
            ops[0],
            RenderOp::DrawBatch(batch)
                if batch.material == MaterialHandle::TOON_MESH
                    && batch.mesh == MeshHandle::SQUARE
                    && batch.count == 1
        ));
        assert!(matches!(
            ops[1],
            RenderOp::DrawBatch(batch)
                if batch.material == MaterialHandle::SKINNED_TOON_MESH
                    && batch.mesh == MeshHandle::SQUARE
                    && batch.count == 1
        ));
        assert_eq!(visuals.instances().len(), 2);
    }

    #[test]
    fn draw_cache_switches_between_no_clip_and_clip_stream_construction() {
        let mut visuals = VisualWorld::default();
        let clip_handle = visuals.register(
            cid(124),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let content_handle = visuals.register(
            cid(125),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );

        visuals.prepare_draw_cache();
        assert!(
            visuals
                .opaque_stream()
                .0
                .iter()
                .all(|op| matches!(op, RenderOp::DrawBatch(_)))
        );

        assert!(visuals.register_stencil_clip(clip_handle, 0));
        assert!(visuals.update_stencil_ref(content_handle, 1));
        visuals.prepare_draw_cache();
        assert!(
            visuals
                .opaque_stream()
                .0
                .iter()
                .any(|op| matches!(op, RenderOp::EnterClip { .. }))
        );

        assert!(visuals.unregister_stencil_clip(clip_handle));
        visuals.prepare_draw_cache();
        assert!(visuals.opaque_stream().0.iter().all(|op| matches!(
            op,
            RenderOp::DrawBatch(batch) if batch.stencil_ref == 0
        )));
    }

    #[test]
    fn opaque_stream_enters_and_exits_single_root_clip() {
        let mut visuals = VisualWorld::default();

        let clip_handle = visuals.register(
            cid(1),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let content_handle = visuals.register(
            cid(2),
            dummy_renderable(),
            Transform::default(),
            [0.8, 0.8, 0.8, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );

        let _ = visuals.register_stencil_clip(clip_handle, 0);
        let _ = visuals.update_stencil_ref(content_handle, 1);
        visuals.prepare_draw_cache();

        let (ops, instance_indices) = visuals.opaque_stream();
        assert_eq!(ops.len(), 4);
        assert_eq!(instance_indices.len(), 2);

        match ops[0] {
            RenderOp::EnterClip {
                parent_ref,
                new_ref,
                ..
            } => {
                assert_eq!(parent_ref, 0);
                assert_eq!(new_ref, 1);
            }
            other => panic!("expected EnterClip, got {other:?}"),
        }

        match ops[1] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 0);
            }
            other => panic!("expected clip-source DrawBatch, got {other:?}"),
        }

        match ops[2] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 1);
            }
            other => panic!("expected content DrawBatch, got {other:?}"),
        }

        match ops[3] {
            RenderOp::ExitClip { ref_value, .. } => assert_eq!(ref_value, 1),
            other => panic!("expected ExitClip, got {other:?}"),
        }
    }

    #[test]
    fn overlay_stream_nests_clip_regions_in_dfs_order() {
        let mut visuals = VisualWorld::default();

        let outer_clip = visuals.register(
            cid(10),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            true,
            0.0,
            None,
            3.0,
        );
        let outer_content = visuals.register(
            cid(11),
            dummy_renderable(),
            Transform::default(),
            [0.8, 0.8, 0.8, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            true,
            0.0,
            None,
            3.0,
        );
        let inner_clip = visuals.register(
            cid(12),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            true,
            0.0,
            None,
            3.0,
        );
        let inner_content = visuals.register(
            cid(13),
            dummy_renderable(),
            Transform::default(),
            [0.6, 0.6, 0.6, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            true,
            0.0,
            None,
            3.0,
        );

        let _ = visuals.register_stencil_clip(outer_clip, 0);
        let _ = visuals.update_stencil_ref(outer_content, 1);
        let _ = visuals.register_stencil_clip(inner_clip, 1);
        let _ = visuals.update_stencil_ref(inner_content, 2);
        visuals.prepare_draw_cache();

        let (ops, instance_indices) = visuals.overlay_stream();
        assert_eq!(ops.len(), 8);
        assert_eq!(instance_indices.len(), 4);

        match ops[0] {
            RenderOp::EnterClip {
                parent_ref,
                new_ref,
                ..
            } => {
                assert_eq!(parent_ref, 0);
                assert_eq!(new_ref, 1);
            }
            other => panic!("expected outer EnterClip, got {other:?}"),
        }
        match ops[1] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 0);
            }
            other => panic!("expected outer clip-source DrawBatch, got {other:?}"),
        }
        match ops[2] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 1);
            }
            other => panic!("expected outer content DrawBatch, got {other:?}"),
        }
        match ops[3] {
            RenderOp::EnterClip {
                parent_ref,
                new_ref,
                ..
            } => {
                assert_eq!(parent_ref, 1);
                assert_eq!(new_ref, 2);
            }
            other => panic!("expected inner EnterClip, got {other:?}"),
        }
        match ops[4] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 2);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 2);
            }
            other => panic!("expected inner clip-source DrawBatch, got {other:?}"),
        }
        match ops[5] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 2);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 3);
            }
            other => panic!("expected inner content DrawBatch, got {other:?}"),
        }
        match ops[6] {
            RenderOp::ExitClip { ref_value, .. } => assert_eq!(ref_value, 2),
            other => panic!("expected inner ExitClip, got {other:?}"),
        }
        match ops[7] {
            RenderOp::ExitClip { ref_value, .. } => assert_eq!(ref_value, 1),
            other => panic!("expected outer ExitClip, got {other:?}"),
        }
    }

    #[test]
    fn cutout_stream_enters_and_exits_single_root_clip() {
        let mut visuals = VisualWorld::default();

        let clip_handle = visuals.register(
            cid(20),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            true,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let content_handle = visuals.register(
            cid(21),
            dummy_renderable(),
            Transform::default(),
            [0.8, 0.8, 0.8, 1.0],
            1.0,
            false,
            true,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );

        let _ = visuals.register_stencil_clip(clip_handle, 0);
        let _ = visuals.update_stencil_ref(content_handle, 1);
        visuals.prepare_draw_cache();

        let (ops, instance_indices) = visuals.cutout_stream();
        assert_eq!(ops.len(), 4);
        assert_eq!(instance_indices.len(), 2);

        match ops[0] {
            RenderOp::EnterClip {
                parent_ref,
                new_ref,
                ..
            } => {
                assert_eq!(parent_ref, 0);
                assert_eq!(new_ref, 1);
            }
            other => panic!("expected EnterClip, got {other:?}"),
        }
        match ops[1] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 0);
            }
            other => panic!("expected clip-source DrawBatch, got {other:?}"),
        }
        match ops[2] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 1);
            }
            other => panic!("expected content DrawBatch, got {other:?}"),
        }
        match ops[3] {
            RenderOp::ExitClip { ref_value, .. } => assert_eq!(ref_value, 1),
            other => panic!("expected ExitClip, got {other:?}"),
        }
    }

    #[test]
    fn transparent_single_stream_enters_and_exits_single_root_clip() {
        let mut visuals = VisualWorld::default();

        let clip_handle = visuals.register(
            cid(30),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 0.5],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let content_handle = visuals.register(
            cid(31),
            dummy_renderable(),
            Transform::default(),
            [0.8, 0.8, 0.8, 0.5],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );

        let _ = visuals.register_stencil_clip(clip_handle, 0);
        let _ = visuals.update_stencil_ref(content_handle, 1);
        visuals.prepare_draw_cache();

        let (ops, instance_indices) = visuals.transparent_single_stream();
        assert_eq!(ops.len(), 4);
        assert_eq!(instance_indices.len(), 2);

        match ops[0] {
            RenderOp::EnterClip {
                parent_ref,
                new_ref,
                ..
            } => {
                assert_eq!(parent_ref, 0);
                assert_eq!(new_ref, 1);
            }
            other => panic!("expected EnterClip, got {other:?}"),
        }
        match ops[1] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 0);
            }
            other => panic!("expected clip-source DrawBatch, got {other:?}"),
        }
        match ops[2] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 1);
            }
            other => panic!("expected content DrawBatch, got {other:?}"),
        }
        match ops[3] {
            RenderOp::ExitClip { ref_value, .. } => assert_eq!(ref_value, 1),
            other => panic!("expected ExitClip, got {other:?}"),
        }
    }

    #[test]
    fn opaque_stream_uses_transparent_clip_source_for_stencil_ops() {
        let mut visuals = VisualWorld::default();

        let clip_handle = visuals.register(
            cid(40),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 0.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let content_handle = visuals.register(
            cid(41),
            dummy_renderable(),
            Transform::default(),
            [0.8, 0.8, 0.8, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );

        let _ = visuals.register_stencil_clip(clip_handle, 0);
        let _ = visuals.update_stencil_ref(content_handle, 1);
        visuals.prepare_draw_cache();

        let (ops, instance_indices) = visuals.opaque_stream();
        assert_eq!(ops.len(), 3);
        assert_eq!(instance_indices, &[0, 1]);

        match ops[0] {
            RenderOp::EnterClip {
                parent_ref,
                new_ref,
                ..
            } => {
                assert_eq!(parent_ref, 0);
                assert_eq!(new_ref, 1);
            }
            other => panic!("expected EnterClip, got {other:?}"),
        }
        match ops[1] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 1);
            }
            other => panic!("expected content DrawBatch, got {other:?}"),
        }
        match ops[2] {
            RenderOp::ExitClip { ref_value, .. } => assert_eq!(ref_value, 1),
            other => panic!("expected ExitClip, got {other:?}"),
        }
    }

    #[test]
    fn cutout_stream_uses_opaque_clip_source_for_stencil_ops() {
        let mut visuals = VisualWorld::default();

        let clip_handle = visuals.register(
            cid(50),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let content_handle = visuals.register(
            cid(51),
            dummy_renderable(),
            Transform::default(),
            [0.8, 0.8, 0.8, 1.0],
            1.0,
            false,
            true,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );

        let _ = visuals.register_stencil_clip(clip_handle, 0);
        let _ = visuals.update_stencil_ref(content_handle, 1);
        visuals.prepare_draw_cache();

        let (ops, instance_indices) = visuals.cutout_stream();
        assert_eq!(ops.len(), 3);
        assert_eq!(instance_indices, &[0, 1]);

        match ops[0] {
            RenderOp::EnterClip {
                parent_ref,
                new_ref,
                ..
            } => {
                assert_eq!(parent_ref, 0);
                assert_eq!(new_ref, 1);
            }
            other => panic!("expected EnterClip, got {other:?}"),
        }
        match ops[1] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(batch.count, 1);
                assert_eq!(instance_indices[batch.start], 1);
            }
            other => panic!("expected content DrawBatch, got {other:?}"),
        }
        match ops[2] {
            RenderOp::ExitClip { ref_value, .. } => assert_eq!(ref_value, 1),
            other => panic!("expected ExitClip, got {other:?}"),
        }
    }

    #[test]
    fn opaque_stream_excluding_source_instance_skips_clip_ops_for_that_source() {
        let mut visuals = VisualWorld::default();

        let clip_handle = visuals.register(
            cid(60),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 0.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let content_handle = visuals.register(
            cid(61),
            dummy_renderable(),
            Transform::default(),
            [0.8, 0.8, 0.8, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );

        let _ = visuals.register_stencil_clip(clip_handle, 0);
        let _ = visuals.update_stencil_ref(content_handle, 1);
        visuals.prepare_draw_cache();

        let (ops, instance_indices) = visuals.opaque_stream_excluding(Some(clip_handle));
        assert_eq!(instance_indices, &[1]);
        assert_eq!(ops.len(), 1);
        match ops[0] {
            RenderOp::DrawBatch(batch) => {
                assert_eq!(batch.count, 1);
                assert_eq!(batch.stencil_ref, 1);
                assert_eq!(instance_indices[batch.start], 1);
            }
            other => panic!("expected DrawBatch, got {other:?}"),
        }
    }

    #[test]
    fn phase_exclusion_helpers_drop_source_from_overlay_cutout_and_transparent_passes() {
        let mut visuals = VisualWorld::default();

        let overlay_handle = visuals.register(
            cid(70),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            true,
            0.0,
            None,
            3.0,
        );
        let cutout_handle = visuals.register(
            cid(71),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            false,
            true,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let transparent_single_handle = visuals.register(
            cid(72),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 0.5],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        let transparent_multi_handle = visuals.register(
            cid(73),
            dummy_renderable(),
            Transform::default(),
            [1.0, 1.0, 1.0, 0.5],
            1.0,
            true,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        visuals.prepare_draw_cache();
        visuals.prepare_transparent_multi_draw_cache_for_view(Transform::default().model);

        let (_, overlay_instances) = visuals.overlay_stream_excluding(Some(overlay_handle));
        let (_, cutout_instances) = visuals.cutout_stream_excluding(Some(cutout_handle));
        let (_, transparent_single_instances) =
            visuals.transparent_single_stream_excluding(Some(transparent_single_handle));
        let (transparent_multi_order, _) =
            visuals.transparent_multi_draw_excluding(Some(transparent_multi_handle));

        assert!(!overlay_instances.contains(&0));
        assert!(!cutout_instances.contains(&1));
        assert!(!transparent_single_instances.contains(&2));
        assert!(!transparent_multi_order.contains(&3));
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct VisualLight {
    /// Light type discriminator for GPU shading.
    ///
    /// Matches shader constants in `assets/shaders/toon-mesh.frag`:
    /// - 1 = point
    /// - 2 = directional
    /// - 3 = spot
    pub light_type: u32,
    pub position_ws: [f32; 3],
    pub intensity: f32,
    pub distance: f32,
    pub color: [f32; 3],
    /// Spot direction in world space (local +Z transformed to world space).
    pub direction_ws: [f32; 3],
    /// Spot outer half-angle in radians.
    pub angle: f32,
    /// Fractional softness of the spot cone edge.
    pub penumbra: f32,
}

impl VisualWorld {
    pub fn new() -> Self {
        Self::default()
    }

    fn is_transparent(inst: &VisualInstance) -> bool {
        // Conservative: any non-1 alpha/opacity is treated as transparent.
        // (Texture alpha is not considered here; that would require texture metadata.)
        inst.opacity < 0.999 || inst.color[3] < 0.999
    }

    fn view_space_z(view: TransformMatrix, model: TransformMatrix) -> f32 {
        // Matrices are stored as column vectors (shader uses mat4(i_model_c0..c3)).
        // Translation is column 3: (tx, ty, tz).
        let tx = model[3][0];
        let ty = model[3][1];
        let tz = model[3][2];

        // Multiply view * vec4(t,1) and return z.
        view[0][2] * tx + view[1][2] * ty + view[2][2] * tz + view[3][2]
    }

    pub fn clear_color(&self) -> [f32; 4] {
        self.clear_color
    }

    pub fn set_clear_color(&mut self, rgba: [f32; 4]) {
        self.clear_color = rgba;
    }

    pub fn renderer_msaa_mode(&self) -> MsaaMode {
        self.renderer_msaa_mode
    }

    pub fn set_renderer_msaa_mode(&mut self, mode: MsaaMode) {
        self.renderer_msaa_mode = mode;
    }

    pub fn preferred_window_size(&self) -> Option<[u32; 2]> {
        self.preferred_window_size
    }

    pub fn set_preferred_window_size(&mut self, size: Option<[u32; 2]>) {
        self.preferred_window_size = size.filter(|[w, h]| *w > 0 && *h > 0);
    }

    pub fn transmission_depth_compare(&self) -> bool {
        self.transmission_depth_compare
    }

    pub fn set_transmission_depth_compare(&mut self, enabled: bool) {
        self.transmission_depth_compare = enabled;
        self.dirty_camera = true;
    }

    pub fn take_preferred_window_size(&mut self) -> Option<[u32; 2]> {
        self.preferred_window_size.take()
    }

    pub fn window_frame_dt_sec(&self) -> f32 {
        self.window_frame_dt_sec
    }

    pub fn window_frame_fps(&self) -> f32 {
        if self.window_frame_dt_sec > 0.0 {
            1.0 / self.window_frame_dt_sec
        } else {
            0.0
        }
    }

    pub fn set_window_frame_dt_sec(&mut self, dt_sec: f32) {
        self.window_frame_dt_sec = dt_sec.max(0.0);
    }

    pub fn xr_frame_dt_sec(&self) -> Option<f32> {
        self.xr_frame_dt_sec
    }

    pub fn xr_frame_fps(&self) -> Option<f32> {
        let dt = self.xr_frame_dt_sec?;
        if dt > 0.0 { Some(1.0 / dt) } else { None }
    }

    pub fn set_xr_frame_dt_sec(&mut self, dt_sec: Option<f32>) {
        self.xr_frame_dt_sec = dt_sec.and_then(|dt| {
            if dt.is_finite() {
                Some(dt.max(0.0))
            } else {
                None
            }
        });
    }

    pub fn ambient_light(&self) -> [f32; 3] {
        self.ambient_light
    }

    pub fn set_ambient_light(&mut self, rgb: [f32; 3]) {
        self.ambient_light = rgb;
        // Stored in the global camera UBO for now.
        self.dirty_camera = true;
    }

    pub fn clear(&mut self) {
        self.materials.clear_instances();
        self.material_consumers.clear();
        self.instances.clear();
        self.handle_to_index.clear();
        self.component_to_handle.clear();
        self.next_handle = 0;

        self.lights.clear();
        self.light_index_by_component.clear();
        self.dirty_lights = true;

        self.ambient_light = [0.0, 0.0, 0.0];

        visual_topology_dirty!(self, Clear);
        visual_data_dirty!(self, Clear);
        self.dirty_camera = true;
        self.background_order.clear();
        self.background_batches.clear();
        self.background_occluded_lit_order.clear();
        self.background_occluded_lit_batches.clear();
        self.background_occluded_lit_emissive_order.clear();
        self.background_occluded_lit_emissive_batches.clear();
        self.draw_order.clear();
        self.emissive_draw_order.clear();
        self.emissive_draw_batches.clear();
        self.outline_order.clear();
        self.outline_stream.clear();
        self.outline_stream_instances.clear();
        self.cutout_order.clear();
        self.emissive_cutout_order.clear();
        self.emissive_cutout_batches.clear();
        self.cutout_stream.clear();
        self.cutout_stream_instances.clear();

        self.transparent_single_draw_order.clear();
        self.transparent_single_stream.clear();
        self.transparent_single_stream_instances.clear();
        self.transparent_multi_draw_order.clear();
        self.transparent_multi_draw_batches.clear();

        self.active_xr_camera = None;
    }

    pub fn active_xr_camera(&self) -> Option<ComponentId> {
        self.active_xr_camera
    }

    pub fn set_active_xr_camera(&mut self, component: Option<ComponentId>) {
        self.active_xr_camera = component;
    }

    pub fn lights_dirty(&self) -> bool {
        self.dirty_lights
    }

    pub fn take_lights_dirty(&mut self) -> bool {
        let v = self.dirty_lights;
        self.dirty_lights = false;
        v
    }

    pub fn lights(&self) -> &[VisualLight] {
        &self.lights
    }

    pub fn upsert_light(&mut self, cid: ComponentId, light: VisualLight) {
        if let Some(&idx) = self.light_index_by_component.get(&cid) {
            self.lights[idx] = light;
        } else {
            let idx = self.lights.len();
            self.lights.push(light);
            self.light_index_by_component.insert(cid, idx);
        }
        self.dirty_lights = true;
    }

    pub fn camera_dirty(&self) -> bool {
        self.dirty_camera
    }

    pub fn take_camera_dirty(&mut self) -> bool {
        let v = self.dirty_camera;
        self.dirty_camera = false;
        v
    }

    pub fn visual_cameras(&self) -> &[VisualCamera] {
        &self.visual_cameras
    }

    pub fn visual_camera(&self, target: CameraTarget) -> Option<&VisualCamera> {
        self.visual_cameras.iter().find(|c| c.target == target)
    }

    fn visual_camera_mut(&mut self, target: CameraTarget) -> &mut VisualCamera {
        if let Some(i) = self.visual_cameras.iter().position(|c| c.target == target) {
            return &mut self.visual_cameras[i];
        }

        self.visual_cameras.push(VisualCamera {
            target,
            eyes: Vec::new(),
        });
        self.visual_cameras.last_mut().unwrap()
    }

    /// Window-facing compatibility: returns the first eye's view matrix for the window target.
    pub fn camera_view(&self) -> [[f32; 4]; 4] {
        self.camera_view_for(CameraTarget::Window)
    }

    /// Window-facing compatibility: returns the first eye's projection matrix for the window target.
    pub fn camera_proj(&self) -> [[f32; 4]; 4] {
        self.camera_proj_for(CameraTarget::Window)
    }

    pub fn camera_view_for(&self, target: CameraTarget) -> [[f32; 4]; 4] {
        self.camera_view_for_eye(target, 0)
    }

    pub fn camera_view_for_eye(&self, target: CameraTarget, eye: usize) -> [[f32; 4]; 4] {
        self.visual_camera(target)
            .and_then(|c| c.eyes.get(eye))
            .map(|e| e.view)
            .unwrap_or([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
    }

    pub fn camera_proj_for(&self, target: CameraTarget) -> [[f32; 4]; 4] {
        self.camera_proj_for_eye(target, 0)
    }

    pub fn camera_proj_for_eye(&self, target: CameraTarget, eye: usize) -> [[f32; 4]; 4] {
        self.visual_camera(target)
            .and_then(|c| c.eyes.get(eye))
            .map(|e| e.proj)
            .unwrap_or([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
    }

    pub fn viewport(&self) -> [f32; 2] {
        self.viewport
    }

    pub fn set_viewport(&mut self, viewport: [f32; 2]) {
        self.viewport = viewport;
    }

    pub fn camera_2d(&self) -> [[f32; 4]; 3] {
        self.camera_2d
    }

    pub fn set_camera(&mut self, view: [[f32; 4]; 4], proj: [[f32; 4]; 4]) {
        self.set_camera_mono_for_target(CameraTarget::Window, view, proj);
        // When a 3D camera becomes active, the 2D camera transform should be neutral.
        self.camera_2d = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        self.dirty_camera = true;
    }

    /// Set all eyes for a target.
    ///
    /// - For `CameraTarget::Window`, pass a 1-element `eyes` vector.
    /// - For `CameraTarget::Xr`, pass 2 (or more) eyes.
    pub fn set_camera_for_target(&mut self, target: CameraTarget, eyes: Vec<CameraData>) {
        let c = self.visual_camera_mut(target);
        c.eyes = eyes;
        self.dirty_camera = true;
    }

    /// Convenience: set a single-eye camera for a target.
    pub fn set_camera_mono_for_target(
        &mut self,
        target: CameraTarget,
        view: [[f32; 4]; 4],
        proj: [[f32; 4]; 4],
    ) {
        self.set_camera_mono_for_target_with_transform(target, view, proj, Transform::default());
    }

    pub fn set_camera_mono_for_target_with_transform(
        &mut self,
        target: CameraTarget,
        view: [[f32; 4]; 4],
        proj: [[f32; 4]; 4],
        transform: Transform,
    ) {
        self.set_camera_for_target(
            target,
            vec![CameraData {
                view,
                proj,
                transform,
            }],
        );
    }

    /// Convenience: set XR eyes.
    pub fn set_xr_camera(&mut self, eyes: Vec<CameraData>) {
        self.set_camera_for_target(CameraTarget::Xr, eyes);
    }

    pub fn set_camera_2d(&mut self, m: [[f32; 4]; 3]) {
        if self.camera_2d == m {
            return;
        }
        self.camera_2d = m;
        self.dirty_camera = true;
    }

    pub fn register_mirror(&mut self, mirror: VisualMirror) {
        self.mirrors.push(mirror);
    }

    pub fn clear_mirrors(&mut self) {
        self.mirrors.clear();
    }

    pub fn mirrors(&self) -> &[VisualMirror] {
        &self.mirrors
    }

    pub fn set_mirror_captures_enabled(&mut self, enabled: bool) {
        self.mirror_captures_enabled = enabled;
    }

    pub fn mirror_captures_enabled(&self) -> bool {
        self.mirror_captures_enabled
    }

    pub fn set_mirror_quality_override(&mut self, quality: Option<i32>) {
        self.mirror_quality_override = quality;
    }

    pub fn mirror_quality_override(&self) -> Option<i32> {
        self.mirror_quality_override
    }

    pub fn mirror_texture_key_for_view(
        &self,
        mirror_component: ComponentId,
        family: MirrorViewerFamily,
        view_index: usize,
    ) -> Option<&str> {
        self.mirrors
            .iter()
            .find(|mirror| mirror.mirror_component == mirror_component)
            .and_then(|mirror| {
                mirror
                    .captures
                    .iter()
                    .find(|capture| capture.family == family && capture.view_index == view_index)
                    .map(|capture| capture.target_key.as_str())
            })
    }

    pub fn mirror_count(&self) -> usize {
        self.mirrors.len()
    }

    /// Returns whether any per-instance data has changed since the last time it was consumed.
    pub fn instance_data_dirty(&self) -> bool {
        self.dirty_instance_data
    }

    /// Consume the instance-data dirty flag.
    pub fn take_instance_data_dirty(&mut self) -> bool {
        let v = self.dirty_instance_data;
        self.dirty_instance_data = false;
        v
    }

    pub fn instances(&self) -> &[VisualInstance] {
        &self.instances
    }

    pub fn outline_order(&self) -> &[u32] {
        &self.outline_order
    }

    pub fn outline_stream(&self) -> (&[RenderOp], &[u32]) {
        (&self.outline_stream, &self.outline_stream_instances)
    }

    pub fn outline_stream_excluding(
        &self,
        excluded_instance: Option<InstanceHandle>,
    ) -> (Vec<RenderOp>, Vec<u32>) {
        let order = self.filtered_phase_order(&self.outline_order, excluded_instance);
        let mut ops = Vec::new();
        let mut instances = Vec::new();
        Self::build_outline_render_stream(
            &self.materials,
            &self.instances,
            &order,
            &mut ops,
            &mut instances,
        );
        (ops, instances)
    }

    /// Reconciles persistent output ranges with the current skinned instances.
    ///
    /// Existing ranges remain stable while their mesh vertex count is unchanged.
    pub fn sync_deformation_ranges(
        &mut self,
        mesh_vertex_counts: &HashMap<crate::engine::graphics::MeshHandle, u32>,
    ) -> bool {
        let mut grew = false;
        for index in 0..self.instances.len() {
            let desired = if self.instances[index].bones_count == 0 {
                0
            } else {
                mesh_vertex_counts
                    .get(&self.instances[index].renderable.mesh)
                    .copied()
                    .unwrap_or(0)
            };
            if self.instances[index].deformed_count == desired {
                continue;
            }
            let old = DeformationRange {
                base: self.instances[index].deformed_base,
                vertex_count: self.instances[index].deformed_count,
            };
            self.deformation_ranges.free(old);
            let (new_range, did_grow) = self.deformation_ranges.allocate_growing(desired);
            grew |= did_grow;
            self.instances[index].deformed_base = new_range.base;
            self.instances[index].deformed_count = new_range.vertex_count;
            self.instances[index].deformation_dirty = desired != 0;
            visual_data_dirty!(self, SyncDeformationRanges);
        }
        grew
    }

    pub fn deformation_cache_capacity(&self) -> u32 {
        self.deformation_ranges.capacity()
    }

    pub fn deformation_cache_live_vertices(&self) -> u32 {
        self.deformation_ranges.live_vertices()
    }

    pub fn mark_deformations_clean(&mut self) {
        for instance in &mut self.instances {
            instance.deformation_dirty = false;
        }
    }

    pub fn mark_all_deformations_dirty(&mut self) {
        for instance in &mut self.instances {
            if instance.deformed_count != 0 {
                instance.deformation_dirty = true;
            }
        }
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    /// Indices into `instances()` in the order they should be drawn (opaque batching).
    pub fn background_order(&self) -> &[u32] {
        &self.background_order
    }

    pub fn background_batches(&self) -> &[DrawBatch] {
        &self.background_batches
    }

    pub fn background_occluded_lit_order(&self) -> &[u32] {
        &self.background_occluded_lit_order
    }

    pub fn background_occluded_lit_batches(&self) -> &[DrawBatch] {
        &self.background_occluded_lit_batches
    }

    pub fn background_occluded_lit_emissive_order(&self) -> &[u32] {
        &self.background_occluded_lit_emissive_order
    }

    pub fn background_occluded_lit_emissive_batches(&self) -> &[DrawBatch] {
        &self.background_occluded_lit_emissive_batches
    }

    pub fn has_background_occluded_lit_emissive(&self) -> bool {
        !self.background_occluded_lit_emissive_order.is_empty()
    }

    /// Indices into `instances()` in the order they should be drawn (opaque batching).
    pub fn draw_order(&self) -> &[u32] {
        &self.draw_order
    }

    /// Indices into `instances()` in the order they should be drawn (emissive opaque batching).
    pub fn emissive_draw_order(&self) -> &[u32] {
        &self.emissive_draw_order
    }

    pub fn emissive_draw_batches(&self) -> &[DrawBatch] {
        &self.emissive_draw_batches
    }

    /// Indices into `instances()` in the order they should be drawn (alpha-to-coverage cutout pass).
    pub fn cutout_order(&self) -> &[u32] {
        &self.cutout_order
    }

    /// DFS-ordered render stream for the alpha-to-coverage cutout phase.
    pub fn cutout_stream(&self) -> (&[RenderOp], &[u32]) {
        (&self.cutout_stream, &self.cutout_stream_instances)
    }

    pub fn cutout_stream_excluding(
        &self,
        excluded_instance: Option<InstanceHandle>,
    ) -> (Vec<RenderOp>, Vec<u32>) {
        let mut ops = Vec::new();
        let mut instances = Vec::new();
        self.build_phase_render_stream_excluding(
            &self.cutout_order,
            excluded_instance,
            &mut ops,
            &mut instances,
        );
        (ops, instances)
    }

    pub fn refraction_stream(&self) -> (&[RenderOp], &[u32]) {
        (&self.refraction_stream, &self.refraction_stream_instances)
    }

    pub fn refraction_stream_excluding(
        &self,
        excluded_instance: Option<InstanceHandle>,
    ) -> (Vec<RenderOp>, Vec<u32>) {
        let mut ops = Vec::new();
        let mut instances = Vec::new();
        self.build_phase_render_stream_excluding(
            &self.refraction_order,
            excluded_instance,
            &mut ops,
            &mut instances,
        );
        (ops, instances)
    }

    pub fn rough_transmission_stream(&self) -> (&[RenderOp], &[u32]) {
        (
            &self.rough_transmission_stream,
            &self.rough_transmission_stream_instances,
        )
    }

    pub fn rough_transmission_stream_excluding(
        &self,
        excluded_instance: Option<InstanceHandle>,
    ) -> (Vec<RenderOp>, Vec<u32>) {
        let mut ops = Vec::new();
        let mut instances = Vec::new();
        self.build_phase_render_stream_excluding(
            &self.rough_transmission_order,
            excluded_instance,
            &mut ops,
            &mut instances,
        );
        (ops, instances)
    }

    pub fn has_transmissive_instances(&self) -> bool {
        self.instances.iter().any(|instance| {
            self.materials
                .resolved_capabilities(instance.material_instance, instance.renderable.material)
                .material
                .transmission
                != super::materials::MaterialTransmission::None
        })
    }

    pub fn has_rough_transmission_instances(&self) -> bool {
        self.instances.iter().any(|instance| {
            self.materials
                .resolved_capabilities(instance.material_instance, instance.renderable.material)
                .material
                .transmission
                == super::materials::MaterialTransmission::Rough
        })
    }

    /// Indices into `instances()` in the order they should be drawn (emissive cutout batching).
    pub fn emissive_cutout_order(&self) -> &[u32] {
        &self.emissive_cutout_order
    }

    pub fn emissive_cutout_batches(&self) -> &[DrawBatch] {
        &self.emissive_cutout_batches
    }

    /// Indices into `instances()` in the order they should be drawn (overlay pass).
    pub fn overlay_order(&self) -> &[u32] {
        &self.overlay_order
    }

    /// DFS-ordered render stream for the single-layer transparent phase.
    pub fn transparent_single_stream(&self) -> (&[RenderOp], &[u32]) {
        (
            &self.transparent_single_stream,
            &self.transparent_single_stream_instances,
        )
    }

    pub fn transparent_single_stream_excluding(
        &self,
        excluded_instance: Option<InstanceHandle>,
    ) -> (Vec<RenderOp>, Vec<u32>) {
        let mut ops = Vec::new();
        let mut instances = Vec::new();
        self.build_phase_render_stream_excluding(
            &self.transparent_single_draw_order,
            excluded_instance,
            &mut ops,
            &mut instances,
        );
        (ops, instances)
    }

    /// DFS-ordered render stream for the overlay phase.
    ///
    /// Returns `(ops, instance_indices)`.
    /// - `ops` is the sequence of `EnterClip`, `DrawBatch`, and `ExitClip` commands.
    /// - `instance_indices[batch.start .. batch.start + batch.count]` gives the
    ///   `VisualInstance` indices for each `DrawBatch` op.
    pub fn overlay_stream(&self) -> (&[RenderOp], &[u32]) {
        (&self.overlay_stream, &self.overlay_stream_instances)
    }

    pub fn overlay_stream_excluding(
        &self,
        excluded_instance: Option<InstanceHandle>,
    ) -> (Vec<RenderOp>, Vec<u32>) {
        let mut ops = Vec::new();
        let mut instances = Vec::new();
        self.build_phase_render_stream_excluding(
            &self.overlay_order,
            excluded_instance,
            &mut ops,
            &mut instances,
        );
        (ops, instances)
    }

    /// DFS-ordered render stream for the opaque phase.
    ///
    /// Returns `(ops, instance_indices)`.
    pub fn opaque_stream(&self) -> (&[RenderOp], &[u32]) {
        (&self.opaque_stream, &self.opaque_stream_instances)
    }

    pub fn opaque_stream_excluding(
        &self,
        excluded_instance: Option<InstanceHandle>,
    ) -> (Vec<RenderOp>, Vec<u32>) {
        let mut ops = Vec::new();
        let mut instances = Vec::new();
        self.build_phase_render_stream_excluding(
            &self.draw_order,
            excluded_instance,
            &mut ops,
            &mut instances,
        );
        (ops, instances)
    }

    /// Indices into `instances()` where `is_stencil_clip=true`, sorted by stencil_ref ascending.
    /// Used by the renderer to inject stencil write/restore draws around clipped batch groups.
    pub fn stencil_clip_order(&self) -> &[u32] {
        &self.stencil_clip_order
    }

    /// Mark an instance as a stencil clip source with the given reference value.
    /// Triggers draw cache rebuild.
    pub fn register_stencil_clip(&mut self, handle: InstanceHandle, stencil_ref: u8) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].is_stencil_clip = true;
            self.instances[idx].stencil_ref = stencil_ref;
            visual_topology_dirty!(self, RegisterStencilClip);
            true
        } else {
            false
        }
    }

    /// Remove stencil clip status from an instance.
    pub fn unregister_stencil_clip(&mut self, handle: InstanceHandle) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].is_stencil_clip = false;
            visual_topology_dirty!(self, UnregisterStencilClip);
            true
        } else {
            false
        }
    }

    /// Update the stencil reference value for a clipped instance (not the clip source itself).
    pub fn update_stencil_ref(&mut self, handle: InstanceHandle, stencil_ref: u8) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            if self.instances[idx].stencil_ref == stencil_ref {
                return true;
            }
            self.instances[idx].stencil_ref = stencil_ref;
            visual_topology_dirty!(self, UpdateStencilRef);
            true
        } else {
            false
        }
    }

    /// Indices into `instances()` in the order they should be drawn (single-layer transparent pass).
    pub fn transparent_single_draw_order(&self) -> &[u32] {
        &self.transparent_single_draw_order
    }

    /// Indices into `instances()` in the order they should be drawn (multi-layer transparent pass).
    pub fn transparent_multi_draw_order(&self) -> &[u32] {
        &self.transparent_multi_draw_order
    }

    pub fn transparent_multi_draw_batches(&self) -> &[DrawBatch] {
        &self.transparent_multi_draw_batches
    }

    pub fn transparent_multi_draw_excluding(
        &self,
        excluded_instance: Option<InstanceHandle>,
    ) -> (Vec<u32>, Vec<DrawBatch>) {
        let order =
            self.filtered_phase_order(&self.transparent_multi_draw_order, excluded_instance);
        let mut batches = Vec::new();
        Self::build_draw_batches_for_order(&self.materials, &self.instances, &order, &mut batches);
        (order, batches)
    }

    /// Call once per frame before rendering. Cheap if nothing changed.
    ///
    /// Returns `true` if the cached draw order/batches were rebuilt this call.
    pub fn set_bloom_settings(&mut self, enabled: bool, layers: u32) {
        self.bloom_runtime_enabled = enabled;
        self.bloom_runtime_layers = layers.clamp(1, 6);
    }

    /// Runtime settings overlay the authored graph without discarding its values.
    pub fn effective_post_processing(&self) -> PostProcessingConfig {
        let mut config = self.post_processing.clone();
        if let Some(bloom) = &mut config.bloom {
            if !self.bloom_runtime_enabled {
                bloom.intensity = 0.0;
            }
            bloom.layers = self.bloom_runtime_layers;
        }
        config
    }

    pub fn post_processing(&self) -> &PostProcessingConfig {
        &self.post_processing
    }

    pub fn post_processing_mut(&mut self) -> &mut PostProcessingConfig {
        &mut self.post_processing
    }

    pub fn set_post_processing(&mut self, config: PostProcessingConfig) {
        self.post_processing = config;
    }

    pub fn runtime_texture_handle(
        &self,
        key: &str,
    ) -> Option<crate::engine::graphics::TextureHandle> {
        self.runtime_texture_handles.get(key).copied()
    }

    pub fn stencil_clip_debug_requested(&self) -> bool {
        self.stencil_clip_debug_requested
    }

    pub fn set_stencil_clip_debug_requested(&mut self, requested: bool) {
        self.stencil_clip_debug_requested = requested;
    }

    pub fn set_runtime_texture_handle(
        &mut self,
        key: impl Into<String>,
        handle: crate::engine::graphics::TextureHandle,
    ) {
        if let Some(previous) = self.runtime_texture_handles.insert(key.into(), handle) {
            if previous != handle
                && !self
                    .runtime_texture_handles
                    .values()
                    .any(|&live| live == previous)
            {
                self.retired_runtime_textures.push(previous);
            }
        }
    }

    pub(crate) fn runtime_texture_handles(
        &self,
    ) -> impl Iterator<Item = crate::engine::graphics::TextureHandle> + '_ {
        self.runtime_texture_handles.values().copied()
    }

    pub(crate) fn retain_runtime_textures(&mut self, mut live: impl FnMut(&str) -> bool) {
        let mut removed = Vec::new();
        self.runtime_texture_handles.retain(|key, handle| {
            if live(key) {
                true
            } else {
                removed.push(*handle);
                false
            }
        });
        let still_live: std::collections::HashSet<_> = self.runtime_texture_handles().collect();
        removed.retain(|handle| !still_live.contains(handle));
        if !removed.is_empty() {
            for instance in &mut self.instances {
                if instance
                    .texture
                    .is_some_and(|texture| removed.contains(&texture))
                {
                    instance.texture = None;
                }
            }
            visual_topology_dirty!(self, RetainRuntimeTextures);
            self.retired_runtime_textures.extend(removed);
        }
    }

    pub(crate) fn take_retired_runtime_textures(
        &mut self,
    ) -> Vec<crate::engine::graphics::TextureHandle> {
        std::mem::take(&mut self.retired_runtime_textures)
    }


}
