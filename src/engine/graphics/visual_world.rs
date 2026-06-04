use crate::engine::ecs::ComponentId;
use crate::engine::ecs::Transform;
use crate::engine::graphics::GpuRenderable;
use crate::engine::graphics::MsaaMode;
use crate::engine::graphics::post_processing::PostProcessingConfig;
use crate::engine::graphics::primitives::InstanceHandle;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::graphics::{Skin, SkinId};
use slotmap::{Key, SlotMap};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Copy)]
pub struct DrawBatch {
    pub material: crate::engine::graphics::MaterialHandle,
    pub mesh: crate::engine::graphics::primitives::MeshHandle,
    pub texture: Option<crate::engine::graphics::TextureHandle>,
    pub texture_filtering: TextureFiltering,
    pub quant_steps: f32,
    /// Effective stencil reference value for this batch.
    /// 0 = unclipped; >0 = draw only where stencil == this value.
    /// For clip-source instances this is `parent_ref + 1` (their visual draw is inside their own region).
    pub stencil_ref: u8,
    /// Start index into the phase's instance-index array (e.g. `overlay_stream_instances`).
    pub start: usize,
    pub count: usize,
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
    instances: Vec<VisualInstance>,
    clear_color: [f32; 4],
    renderer_msaa_mode: MsaaMode,
    preferred_window_size: Option<[u32; 2]>,
    post_processing: PostProcessingConfig,

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
    dirty_bones_palette: bool,

    // Shared skin definitions (glTF skins), keyed by (uri, skin_index).
    skins: SlotMap<SkinId, Skin>,
    skin_id_by_key: std::collections::HashMap<(String, usize), SkinId>,

    ambient_light: [f32; 3],

    point_lights: Vec<VisualPointLight>,
    point_light_index_by_component: std::collections::HashMap<ComponentId, usize>,
    dirty_lights: bool,

    // Target-scoped camera state. Window is typically mono; XR is stereo.
    visual_cameras: Vec<VisualCamera>,

    // Which CameraXRComponent (by ComponentId) is currently active for XR rig transforms.
    active_xr_camera: Option<ComponentId>,
    // Most recent render target size in pixels (width, height).
    viewport: [f32; 2],
    runtime_texture_handles: HashMap<String, crate::engine::graphics::TextureHandle>,
    stencil_clip_debug_requested: bool,
    // 2D camera view transform for translation/scale/rotation.
    // Stored as mat3 column vectors padded to vec4 columns (std140 friendly).
    camera_2d: [[f32; 4]; 3],
    dirty_camera: bool,

    next_handle: u32,
    handle_to_index: std::collections::HashMap<InstanceHandle, usize>,
    component_to_handle: std::collections::HashMap<ComponentId, InstanceHandle>,

    // Cached draw data (rebuilt when dirty)
    dirty_draw_cache: bool,
    /// True when per-instance data (e.g. model matrices) changed and any cached GPU instance
    /// buffer should be rebuilt/uploaded.
    dirty_instance_data: bool,

    // Background draw data (rebuilt when dirty)
    background_order: Vec<u32>, // indices into `instances`
    background_batches: Vec<DrawBatch>,
    background_occluded_lit_order: Vec<u32>,
    background_occluded_lit_batches: Vec<DrawBatch>,
    draw_order: Vec<u32>, // indices into `instances`
    draw_batches: Vec<DrawBatch>,

    // Emissive-only opaque draw data (rebuilt when dirty).
    emissive_draw_order: Vec<u32>,
    emissive_draw_batches: Vec<DrawBatch>,

    // Alpha-to-coverage cutout draw data (rebuilt when dirty).
    cutout_order: Vec<u32>,
    cutout_batches: Vec<DrawBatch>,

    // Emissive-only alpha-to-coverage draw data (rebuilt when dirty).
    emissive_cutout_order: Vec<u32>,
    emissive_cutout_batches: Vec<DrawBatch>,

    // DFS-ordered render stream for the cutout phase.
    cutout_stream: Vec<RenderOp>,
    cutout_stream_instances: Vec<u32>,

    // Overlay draw data (rebuilt when dirty).
    // Overlay is drawn on top of all other phases.
    overlay_order: Vec<u32>,
    overlay_batches: Vec<DrawBatch>,

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

    // Transparent draw data.
    // - Single-layer: cached (order does not depend on view), instanced.
    transparent_single_draw_order: Vec<u32>,
    transparent_single_draw_batches: Vec<DrawBatch>,
    transparent_single_stream: Vec<RenderOp>,
    transparent_single_stream_instances: Vec<u32>,
    // - Multi-layer: rebuilt per-eye (ordering depends on view), sorted + drawn one-by-one.
    transparent_multi_draw_order: Vec<u32>,
    transparent_multi_draw_batches: Vec<DrawBatch>,
}

#[derive(Debug, Clone, Copy)]
pub struct VisualInstance {
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

    /// Base index into `VisualWorld::bones_palette`.
    pub bones_base: u32,
    /// Number of bone matrices for this instance.
    pub bones_count: u32,

    /// Which clip region this instance is inside. 0 = unclipped.
    pub stencil_ref: u8,
    /// When true, this instance writes stencil before its descendant subtree draws.
    /// It also draws normally in the color pass (double duty: clip source + background quad).
    pub is_stencil_clip: bool,
}

fn sanitize_quant_steps(steps: f32) -> f32 {
    if !steps.is_finite() {
        3.0
    } else {
        steps.clamp(1.0, 64.0)
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
            instances: Vec::new(),
            clear_color: [0.0, 0.0, 0.0, 1.0],
            renderer_msaa_mode: MsaaMode::default(),
            preferred_window_size: None,
            post_processing: PostProcessingConfig::default(),

            window_frame_dt_sec: 0.0,
            xr_frame_dt_sec: None,

            bones_palette: vec![ident4],
            bones_free_ranges: Vec::new(),
            dirty_bones_palette: true,

            skins: SlotMap::with_key(),
            skin_id_by_key: std::collections::HashMap::new(),

            ambient_light: [0.0, 0.0, 0.0],

            point_lights: Vec::new(),
            point_light_index_by_component: std::collections::HashMap::new(),
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

            dirty_draw_cache: true,
            dirty_instance_data: true,
            background_order: Vec::new(),
            background_batches: Vec::new(),
            background_occluded_lit_order: Vec::new(),
            background_occluded_lit_batches: Vec::new(),
            draw_order: Vec::new(),
            draw_batches: Vec::new(),
            emissive_draw_order: Vec::new(),
            emissive_draw_batches: Vec::new(),

            cutout_order: Vec::new(),
            cutout_batches: Vec::new(),
            emissive_cutout_order: Vec::new(),
            emissive_cutout_batches: Vec::new(),
            cutout_stream: Vec::new(),
            cutout_stream_instances: Vec::new(),

            overlay_order: Vec::new(),
            overlay_batches: Vec::new(),

            stencil_clip_order: Vec::new(),

            overlay_stream: Vec::new(),
            overlay_stream_instances: Vec::new(),

            opaque_stream: Vec::new(),
            opaque_stream_instances: Vec::new(),

            transparent_single_draw_order: Vec::new(),
            transparent_single_draw_batches: Vec::new(),
            transparent_single_stream: Vec::new(),
            transparent_single_stream_instances: Vec::new(),
            transparent_multi_draw_order: Vec::new(),
            transparent_multi_draw_batches: Vec::new(),
        }
    }
}

impl VisualWorld {
    fn is_emissive_material(material: crate::engine::graphics::MaterialHandle) -> bool {
        matches!(
            material,
            crate::engine::graphics::MaterialHandle::EMISSIVE_TOON_MESH
                | crate::engine::graphics::MaterialHandle::SKINNED_EMISSIVE_TOON_MESH
        )
    }

    pub fn instance(&self, handle: InstanceHandle) -> Option<&VisualInstance> {
        let idx = *self.handle_to_index.get(&handle)?;
        self.instances.get(idx)
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
        self.dirty_bones_palette = true;
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
                self.dirty_instance_data = true;
            }
            return true;
        }

        let want_count = bones.len() as u32;
        let mut base = self.instances[idx].bones_base;
        let old_count = self.instances[idx].bones_count;

        if old_count == 0 {
            base = self.bones_alloc_range(want_count);
            self.instances[idx].bones_base = base;
            self.instances[idx].bones_count = want_count;
            self.dirty_instance_data = true;
        } else if old_count != want_count {
            // Reallocate with new size.
            self.bones_free_range(base, old_count);
            base = self.bones_alloc_range(want_count);
            self.instances[idx].bones_base = base;
            self.instances[idx].bones_count = want_count;
            self.dirty_instance_data = true;
        }

        // Write into the palette.
        let start = base as usize;
        let end = start + bones.len();
        if end > self.bones_palette.len() {
            self.bones_palette.resize(end, Self::bones_identity());
        }
        self.bones_palette[start..end].copy_from_slice(bones);
        self.dirty_bones_palette = true;

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

    /// Returns whether the bones palette changed since the last call, and clears the dirty flag.
    pub fn take_bones_palette_dirty(&mut self) -> bool {
        let dirty = self.dirty_bones_palette;
        self.dirty_bones_palette = false;
        dirty
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
            self.dirty_instance_data = true;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RenderOp, VisualWorld};
    use crate::engine::ecs::ComponentId;
    use crate::engine::graphics::primitives::{
        GpuRenderable, MaterialHandle, MeshHandle, Transform,
    };
    use slotmap::KeyData;

    fn cid(n: u64) -> ComponentId {
        KeyData::from_ffi(n).into()
    }

    fn dummy_renderable() -> GpuRenderable {
        GpuRenderable::new(MeshHandle::SQUARE, MaterialHandle::TOON_MESH)
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
}
#[derive(Debug, Clone, Copy, Default)]
pub struct VisualPointLight {
    /// Light type discriminator for GPU shading.
    ///
    /// Matches shader constants in `assets/shaders/toon-mesh.frag`:
    /// - 1 = point
    /// - 2 = directional
    pub light_type: u32,
    pub position_ws: [f32; 3],
    pub intensity: f32,
    pub distance: f32,
    pub color: [f32; 3],
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

    fn build_draw_batches_for_order(
        instances: &[VisualInstance],
        order: &[u32],
        out: &mut Vec<DrawBatch>,
    ) {
        out.clear();
        let mut cursor = 0usize;
        while cursor < order.len() {
            let idx0 = order[cursor] as usize;
            let inst0 = instances[idx0];
            let r0 = inst0.renderable;
            let material = r0.material;
            let mesh = r0.mesh;
            let texture = inst0.texture;
            let texture_filtering = inst0.texture_filtering;
            let quant_steps = sanitize_quant_steps(inst0.quant_steps);
            let stencil_ref = inst0.stencil_ref;

            let start = cursor;
            cursor += 1;

            while cursor < order.len() {
                let idx = order[cursor] as usize;
                let inst = instances[idx];
                let r = inst.renderable;
                if r.material == material
                    && r.mesh == mesh
                    && inst.texture == texture
                    && inst.texture_filtering == texture_filtering
                    && sanitize_quant_steps(inst.quant_steps).to_bits() == quant_steps.to_bits()
                    && inst.stencil_ref == stencil_ref
                {
                    cursor += 1;
                } else {
                    break;
                }
            }

            out.push(DrawBatch {
                material,
                mesh,
                texture,
                texture_filtering,
                quant_steps,
                stencil_ref,
                start,
                count: cursor - start,
            });
        }
    }

    /// Build the per-phase DFS render stream for the overlay pass.
    ///
    /// `overlay_order` must already be sorted by `(stencil_ref, material, tex, mesh, ...)`.
    ///
    /// Algorithm (one level at a time):
    /// - Non-clip instances at depth D → `DrawBatch` at stencil_ref D.
    /// - Clip sources at depth D → `EnterClip`, then their visual draw at stencil_ref D+1,
    ///   then all content at depth D+1 (recursed), then `ExitClip`.
    fn build_phase_render_stream(
        instances: &[VisualInstance],
        phase_order: &[u32],
        out_ops: &mut Vec<RenderOp>,
        out_instances: &mut Vec<u32>,
    ) {
        out_ops.clear();
        out_instances.clear();

        if phase_order.is_empty() {
            return;
        }

        let max_depth = phase_order
            .iter()
            .map(|&i| instances[i as usize].stencil_ref)
            .chain(
                instances
                    .iter()
                    .filter(|inst| inst.is_stencil_clip)
                    .map(|inst| inst.stencil_ref),
            )
            .max()
            .unwrap_or(0);

        // Split into per-depth groups preserving sort order within each group.
        // Group[d] = (non_clip indices, clip_source indices) at stencil_ref == d.
        let depth_count = max_depth as usize + 1;
        let mut non_clip_by_depth: Vec<Vec<u32>> = vec![Vec::new(); depth_count];
        let mut phase_clip_sources_by_depth: Vec<Vec<u32>> = vec![Vec::new(); depth_count];
        let mut all_clip_sources_by_depth: Vec<Vec<u32>> = vec![Vec::new(); depth_count];

        for (index, inst) in instances.iter().enumerate() {
            if inst.is_stencil_clip {
                all_clip_sources_by_depth[inst.stencil_ref as usize].push(index as u32);
            }
        }

        for &idx in phase_order {
            let inst = &instances[idx as usize];
            let d = inst.stencil_ref as usize;
            if inst.is_stencil_clip {
                phase_clip_sources_by_depth[d].push(idx);
            } else {
                non_clip_by_depth[d].push(idx);
            }
        }

        Self::build_overlay_level(
            0,
            max_depth,
            instances,
            &non_clip_by_depth,
            &all_clip_sources_by_depth,
            &phase_clip_sources_by_depth,
            out_ops,
            out_instances,
        );
    }

    fn build_overlay_level(
        depth: usize,
        max_depth: u8,
        instances: &[VisualInstance],
        non_clip_by_depth: &[Vec<u32>],
        all_clip_sources_by_depth: &[Vec<u32>],
        phase_clip_sources_by_depth: &[Vec<u32>],
        out_ops: &mut Vec<RenderOp>,
        out_instances: &mut Vec<u32>,
    ) {
        if depth >= non_clip_by_depth.len() {
            return;
        }

        // Draw non-clip instances at this depth.
        let non_clip = &non_clip_by_depth[depth];
        if !non_clip.is_empty() {
            Self::append_stream_batches(instances, non_clip, depth as u8, out_ops, out_instances);
        }

        let all_clip_sources = &all_clip_sources_by_depth[depth];
        if !all_clip_sources.is_empty() {
            let parent_ref = depth as u8;
            let new_ref = parent_ref.saturating_add(1);
            let phase_clip_sources = &phase_clip_sources_by_depth[depth];

            // Emit EnterClip for every clip source at this depth.
            for &src_idx in all_clip_sources {
                if !phase_clip_sources.contains(&src_idx) {
                    Self::append_clip_stream_instance(out_instances, src_idx);
                }
                out_ops.push(RenderOp::EnterClip {
                    instance_index: src_idx,
                    parent_ref,
                    new_ref,
                });
            }

            // Clip sources also draw as normal color instances, but at new_ref
            // (they are inside their own clip region after the INCR).
            if !phase_clip_sources.is_empty() {
                Self::append_stream_batches(
                    instances,
                    phase_clip_sources,
                    new_ref,
                    out_ops,
                    out_instances,
                );
            }

            // Recurse: all content at the next depth is inside the clip region.
            if depth + 1 <= max_depth as usize {
                Self::build_overlay_level(
                    depth + 1,
                    max_depth,
                    instances,
                    non_clip_by_depth,
                    all_clip_sources_by_depth,
                    phase_clip_sources_by_depth,
                    out_ops,
                    out_instances,
                );
            }

            // Emit ExitClip in reverse order (innermost-first, which here means
            // the last clip source entered is the first exited).
            for &src_idx in all_clip_sources.iter().rev() {
                out_ops.push(RenderOp::ExitClip {
                    instance_index: src_idx,
                    ref_value: new_ref,
                });
            }
        } else if depth + 1 <= max_depth as usize {
            // No clips here but deeper levels exist; keep recursing.
            Self::build_overlay_level(
                depth + 1,
                max_depth,
                instances,
                non_clip_by_depth,
                all_clip_sources_by_depth,
                phase_clip_sources_by_depth,
                out_ops,
                out_instances,
            );
        }
    }

    fn append_clip_stream_instance(out_instances: &mut Vec<u32>, instance_index: u32) {
        if !out_instances.contains(&instance_index) {
            out_instances.push(instance_index);
        }
    }

    /// Append `DrawBatch` ops for a pre-sorted slice of instance indices.
    ///
    /// `effective_ref` overrides the per-instance `stencil_ref` — necessary for clip
    /// sources that are visually drawn inside their own region (`new_ref`).
    fn append_stream_batches(
        instances: &[VisualInstance],
        indices: &[u32],
        effective_ref: u8,
        out_ops: &mut Vec<RenderOp>,
        out_instances: &mut Vec<u32>,
    ) {
        let mut cursor = 0usize;
        while cursor < indices.len() {
            let idx0 = indices[cursor] as usize;
            let inst0 = &instances[idx0];
            let r0 = inst0.renderable;
            let material = r0.material;
            let mesh = r0.mesh;
            let texture = inst0.texture;
            let texture_filtering = inst0.texture_filtering;
            let quant_steps = sanitize_quant_steps(inst0.quant_steps);

            let start = out_instances.len();
            out_instances.push(indices[cursor]);
            cursor += 1;

            while cursor < indices.len() {
                let idx = indices[cursor] as usize;
                let inst = &instances[idx];
                let r = inst.renderable;
                if r.material == material
                    && r.mesh == mesh
                    && inst.texture == texture
                    && inst.texture_filtering == texture_filtering
                    && sanitize_quant_steps(inst.quant_steps).to_bits() == quant_steps.to_bits()
                {
                    out_instances.push(indices[cursor]);
                    cursor += 1;
                } else {
                    break;
                }
            }

            out_ops.push(RenderOp::DrawBatch(DrawBatch {
                material,
                mesh,
                texture,
                texture_filtering,
                quant_steps,
                stencil_ref: effective_ref,
                start,
                count: out_instances.len() - start,
            }));
        }
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
        self.instances.clear();
        self.handle_to_index.clear();
        self.component_to_handle.clear();
        self.next_handle = 0;

        self.point_lights.clear();
        self.point_light_index_by_component.clear();
        self.dirty_lights = true;

        self.ambient_light = [0.0, 0.0, 0.0];

        self.dirty_draw_cache = true;
        self.dirty_instance_data = true;
        self.dirty_camera = true;
        self.background_order.clear();
        self.background_batches.clear();
        self.background_occluded_lit_order.clear();
        self.background_occluded_lit_batches.clear();
        self.draw_order.clear();
        self.draw_batches.clear();
        self.cutout_order.clear();
        self.cutout_batches.clear();
        self.cutout_stream.clear();
        self.cutout_stream_instances.clear();

        self.transparent_single_draw_order.clear();
        self.transparent_single_draw_batches.clear();
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

    pub fn point_lights(&self) -> &[VisualPointLight] {
        &self.point_lights
    }

    pub fn upsert_point_light(&mut self, cid: ComponentId, light: VisualPointLight) {
        if let Some(&idx) = self.point_light_index_by_component.get(&cid) {
            self.point_lights[idx] = light;
        } else {
            let idx = self.point_lights.len();
            self.point_lights.push(light);
            self.point_light_index_by_component.insert(cid, idx);
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
        self.set_camera_for_target(
            target,
            vec![CameraData {
                view,
                proj,
                transform: Transform::default(),
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

    /// Indices into `instances()` in the order they should be drawn (opaque batching).
    pub fn draw_order(&self) -> &[u32] {
        &self.draw_order
    }

    pub fn draw_batches(&self) -> &[DrawBatch] {
        &self.draw_batches
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

    pub fn cutout_batches(&self) -> &[DrawBatch] {
        &self.cutout_batches
    }

    /// DFS-ordered render stream for the alpha-to-coverage cutout phase.
    pub fn cutout_stream(&self) -> (&[RenderOp], &[u32]) {
        (&self.cutout_stream, &self.cutout_stream_instances)
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

    pub fn overlay_batches(&self) -> &[DrawBatch] {
        &self.overlay_batches
    }

    /// DFS-ordered render stream for the single-layer transparent phase.
    pub fn transparent_single_stream(&self) -> (&[RenderOp], &[u32]) {
        (
            &self.transparent_single_stream,
            &self.transparent_single_stream_instances,
        )
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

    /// DFS-ordered render stream for the opaque phase.
    ///
    /// Returns `(ops, instance_indices)`.
    pub fn opaque_stream(&self) -> (&[RenderOp], &[u32]) {
        (&self.opaque_stream, &self.opaque_stream_instances)
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
            self.dirty_draw_cache = true;
            true
        } else {
            false
        }
    }

    /// Remove stencil clip status from an instance.
    pub fn unregister_stencil_clip(&mut self, handle: InstanceHandle) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].is_stencil_clip = false;
            self.dirty_draw_cache = true;
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
            self.dirty_draw_cache = true;
            true
        } else {
            false
        }
    }

    /// Indices into `instances()` in the order they should be drawn (single-layer transparent pass).
    pub fn transparent_single_draw_order(&self) -> &[u32] {
        &self.transparent_single_draw_order
    }

    pub fn transparent_single_draw_batches(&self) -> &[DrawBatch] {
        &self.transparent_single_draw_batches
    }

    /// Indices into `instances()` in the order they should be drawn (multi-layer transparent pass).
    pub fn transparent_multi_draw_order(&self) -> &[u32] {
        &self.transparent_multi_draw_order
    }

    pub fn transparent_multi_draw_batches(&self) -> &[DrawBatch] {
        &self.transparent_multi_draw_batches
    }

    /// Call once per frame before rendering. Cheap if nothing changed.
    ///
    /// Returns `true` if the cached draw order/batches were rebuilt this call.
    pub fn prepare_draw_cache(&mut self) -> bool {
        if !self.dirty_draw_cache {
            return false;
        }

        self.background_order.clear();
        self.background_occluded_lit_order.clear();
        self.draw_order.clear();
        self.emissive_draw_order.clear();
        self.cutout_order.clear();
        self.emissive_cutout_order.clear();
        self.transparent_single_draw_order.clear();
        self.overlay_order.clear();
        self.stencil_clip_order.clear();
        // Opaque pass: exclude anything that is transparent.
        for i in 0..self.instances.len() {
            let inst = &self.instances[i];
            if inst.is_stencil_clip {
                self.stencil_clip_order.push(i as u32);
            }
            if inst.overlay {
                self.overlay_order.push(i as u32);
            } else if inst.background {
                if inst.background_occluded_lit {
                    self.background_occluded_lit_order.push(i as u32);
                } else {
                    self.background_order.push(i as u32);
                }
            } else if inst.transparent_cutout {
                self.cutout_order.push(i as u32);
            } else if !Self::is_transparent(inst) {
                self.draw_order.push(i as u32);
            } else if !inst.multiple_layers {
                self.transparent_single_draw_order.push(i as u32);
            }
        }
        self.stencil_clip_order
            .sort_by_key(|&i| self.instances[i as usize].stencil_ref);

        // Background pass: batch aggressively (order does not depend on view).
        // NOTE: Background instances are excluded from the normal opaque/transparent lists.
        self.background_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
                sanitize_quant_steps(inst.quant_steps).to_bits(),
            )
        });
        let instances = &self.instances;
        let background_draw_order = &self.background_order;
        Self::build_draw_batches_for_order(
            instances,
            background_draw_order,
            &mut self.background_batches,
        );

        // Background (occluded+lit) pass: same batching strategy.
        self.background_occluded_lit_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
                sanitize_quant_steps(inst.quant_steps).to_bits(),
            )
        });
        let background_occluded_lit_draw_order = &self.background_occluded_lit_order;
        Self::build_draw_batches_for_order(
            instances,
            background_occluded_lit_draw_order,
            &mut self.background_occluded_lit_batches,
        );

        // Sort by (stencil_ref, material, tex, mesh, filtering) so stencil-clipped
        // instances group with their clip region. tex before mesh matches overlay convention
        // (untextured quads draw before textured glyphs at same depth).
        self.draw_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map_or(0, |t| t.0.wrapping_add(1));
            (
                inst.stencil_ref,
                r.material.0,
                tex,
                r.mesh.0,
                inst.texture_filtering as u8,
                sanitize_quant_steps(inst.quant_steps).to_bits(),
            )
        });

        let draw_order = &self.draw_order;
        Self::build_draw_batches_for_order(instances, draw_order, &mut self.draw_batches);

        // Build the DFS render stream for the opaque phase.
        Self::build_phase_render_stream(
            instances,
            &self.draw_order,
            &mut self.opaque_stream,
            &mut self.opaque_stream_instances,
        );

        self.emissive_draw_order
            .extend(self.draw_order.iter().copied().filter(|&i| {
                Self::is_emissive_material(self.instances[i as usize].renderable.material)
            }));
        let emissive_draw_order = &self.emissive_draw_order;
        Self::build_draw_batches_for_order(
            instances,
            emissive_draw_order,
            &mut self.emissive_draw_batches,
        );

        // Cutout pass: batch aggressively (order does not depend on view).
        self.cutout_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                inst.stencil_ref,
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
                sanitize_quant_steps(inst.quant_steps).to_bits(),
            )
        });
        let cutout_order = &self.cutout_order;
        Self::build_draw_batches_for_order(instances, cutout_order, &mut self.cutout_batches);

        Self::build_phase_render_stream(
            instances,
            &self.cutout_order,
            &mut self.cutout_stream,
            &mut self.cutout_stream_instances,
        );

        self.emissive_cutout_order
            .extend(self.cutout_order.iter().copied().filter(|&i| {
                Self::is_emissive_material(self.instances[i as usize].renderable.material)
            }));
        let emissive_cutout_order = &self.emissive_cutout_order;
        Self::build_draw_batches_for_order(
            instances,
            emissive_cutout_order,
            &mut self.emissive_cutout_batches,
        );

        // Single-layer transparent pass: batch aggressively (order does not depend on view).
        self.transparent_single_draw_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (
                inst.stencil_ref,
                r.material.0,
                r.mesh.0,
                tex,
                inst.texture_filtering as u8,
                sanitize_quant_steps(inst.quant_steps).to_bits(),
            )
        });
        let transparent_single_draw_order = &self.transparent_single_draw_order;
        Self::build_draw_batches_for_order(
            instances,
            transparent_single_draw_order,
            &mut self.transparent_single_draw_batches,
        );

        Self::build_phase_render_stream(
            instances,
            &self.transparent_single_draw_order,
            &mut self.transparent_single_stream,
            &mut self.transparent_single_stream_instances,
        );

        // Overlay pass: no-texture instances (e.g. text backgrounds) must draw before
        // textured glyphs so that depth-write from glyph edge pixels does not block
        // the background quad.  tex=0 for untextured, tex=handle+1 for textured, so
        // placing tex BEFORE mesh ensures untextured always sorts first regardless of
        // which MeshHandle was allocated earlier.
        self.overlay_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map_or(0, |t| t.0.wrapping_add(1));
            (
                inst.stencil_ref,
                r.material.0,
                tex,
                r.mesh.0,
                inst.texture_filtering as u8,
                sanitize_quant_steps(inst.quant_steps).to_bits(),
            )
        });
        let overlay_order = &self.overlay_order;
        Self::build_draw_batches_for_order(instances, overlay_order, &mut self.overlay_batches);

        // Build the DFS render stream for the overlay phase.
        Self::build_phase_render_stream(
            instances,
            &self.overlay_order,
            &mut self.overlay_stream,
            &mut self.overlay_stream_instances,
        );

        self.dirty_draw_cache = false;
        true
    }

    /// Rebuild multi-layer transparent draw order/batches for a specific camera eye.
    ///
    /// Intended to be called by the renderer (ordering depends on view).
    pub fn prepare_transparent_multi_draw_cache_for_eye(
        &mut self,
        target: CameraTarget,
        eye: usize,
    ) {
        self.transparent_multi_draw_order.clear();

        for i in 0..self.instances.len() {
            let inst = &self.instances[i];
            if inst.overlay {
                continue;
            }
            if inst.background {
                continue;
            }
            if inst.transparent_cutout {
                continue;
            }
            if inst.multiple_layers && Self::is_transparent(inst) {
                self.transparent_multi_draw_order.push(i as u32);
            }
        }

        if self.transparent_multi_draw_order.is_empty() {
            self.transparent_multi_draw_batches.clear();
            return;
        }

        let view = self.camera_view_for_eye(target, eye);

        // Back-to-front for blending.
        self.transparent_multi_draw_order.sort_by(|&a, &b| {
            let ia = self.instances[a as usize];
            let ib = self.instances[b as usize];
            let za = Self::view_space_z(view, ia.transform.model);
            let zb = Self::view_space_z(view, ib.transform.model);
            za.partial_cmp(&zb)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(&b))
        });

        let instances = &self.instances;
        let transparent_draw_order = &self.transparent_multi_draw_order;
        Self::build_draw_batches_for_order(
            instances,
            transparent_draw_order,
            &mut self.transparent_multi_draw_batches,
        );
    }

    pub fn register(
        &mut self,
        cid: ComponentId,
        renderable: GpuRenderable,
        transform: Transform,
        color: [f32; 4],
        opacity: f32,
        multiple_layers: bool,
        transparent_cutout: bool,
        background: bool,
        background_occluded_lit: bool,
        overlay: bool,
        emissive: f32,
        texture: Option<crate::engine::graphics::TextureHandle>,
        quant_steps: f32,
    ) -> InstanceHandle {
        let handle = InstanceHandle(self.next_handle);
        self.next_handle = self.next_handle.wrapping_add(1);

        let idx = self.instances.len();
        self.instances.push(VisualInstance {
            renderable,
            transform,
            color,
            opacity: if opacity.is_finite() {
                opacity.clamp(0.0, 1.0)
            } else {
                1.0
            },
            multiple_layers,
            transparent_cutout,
            background,
            background_occluded_lit,
            overlay,
            emissive: if emissive.is_finite() {
                emissive.max(0.0)
            } else {
                0.0
            },
            texture,
            texture_filtering: TextureFiltering::default(),
            quant_steps: sanitize_quant_steps(quant_steps),

            bones_base: 0,
            bones_count: 0,

            stencil_ref: 0,
            is_stencil_clip: false,
        });
        self.handle_to_index.insert(handle, idx);
        self.component_to_handle.insert(cid, handle);

        self.dirty_draw_cache = true;
        self.dirty_instance_data = true;
        handle
    }

    pub fn update_quant_steps(&mut self, handle: InstanceHandle, quant_steps: f32) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            let q = sanitize_quant_steps(quant_steps);
            if self.instances[idx].quant_steps.to_bits() == q.to_bits() {
                return true;
            }
            self.instances[idx].quant_steps = q;
            // Quantization affects material UBO selection => batching.
            self.dirty_draw_cache = true;
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, handle: InstanceHandle) -> bool {
        if let Some(idx) = self.handle_to_index.remove(&handle) {
            // Free any skin allocation before removing the instance.
            let old_base = self.instances[idx].bones_base;
            let old_count = self.instances[idx].bones_count;
            if old_count != 0 {
                self.bones_free_range(old_base, old_count);
            }

            self.instances.swap_remove(idx);

            if idx < self.instances.len() {
                // NOTE: This is O(n). Consider storing index->handle too if it becomes hot.
                if let Some((moved_handle, _)) = self
                    .handle_to_index
                    .iter()
                    .find(|(_, i)| **i == self.instances.len())
                {
                    self.handle_to_index.insert(*moved_handle, idx);
                }
            }

            self.component_to_handle.retain(|_, &mut h| h != handle);

            self.dirty_draw_cache = true;
            self.dirty_instance_data = true;
            true
        } else {
            false
        }
    }

    pub fn update_transform(&mut self, handle: InstanceHandle, transform: Transform) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].transform = transform;
            self.dirty_instance_data = true;
            // transform-only doesn’t affect batching by (material, mesh)
            true
        } else {
            false
        }
    }

    pub fn update_model(&mut self, handle: InstanceHandle, model: TransformMatrix) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].transform.model = model;
            self.instances[idx].transform.matrix_world = model;
            self.dirty_instance_data = true;
            // model-only doesn’t affect batching by (material, mesh)
            true
        } else {
            false
        }
    }

    pub fn update_color(&mut self, handle: InstanceHandle, color: [f32; 4]) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].color = color;
            self.dirty_instance_data = true;
            // Color alpha can change transparent/opaque classification.
            self.dirty_draw_cache = true;
            true
        } else {
            false
        }
    }

    pub fn update_opacity(&mut self, handle: InstanceHandle, opacity: f32) -> bool {
        self.update_opacity_state(handle, opacity, None)
    }

    pub fn update_opacity_state(
        &mut self,
        handle: InstanceHandle,
        opacity: f32,
        multiple_layers: impl Into<Option<bool>>,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            let o = if opacity.is_finite() {
                opacity.clamp(0.0, 1.0)
            } else {
                1.0
            };
            let mut changed = false;

            if (self.instances[idx].opacity - o).abs() >= f32::EPSILON {
                self.instances[idx].opacity = o;
                changed = true;
            }

            if let Some(ml) = multiple_layers.into() {
                if self.instances[idx].multiple_layers != ml {
                    self.instances[idx].multiple_layers = ml;
                    changed = true;
                }
            }

            if !changed {
                return true;
            }
            self.dirty_instance_data = true;
            // Opacity changes can change transparent/opaque classification.
            self.dirty_draw_cache = true;
            true
        } else {
            false
        }
    }

    pub fn update_transparent_cutout(&mut self, handle: InstanceHandle, enabled: bool) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            if self.instances[idx].transparent_cutout == enabled {
                return true;
            }
            self.instances[idx].transparent_cutout = enabled;
            // Cutout changes affect pass classification.
            self.dirty_draw_cache = true;
            true
        } else {
            false
        }
    }

    pub fn update_emissive(&mut self, handle: InstanceHandle, emissive: f32) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].emissive = if emissive.is_finite() {
                emissive.max(0.0)
            } else {
                0.0
            };
            self.dirty_instance_data = true;
            true
        } else {
            false
        }
    }

    pub fn update_material(
        &mut self,
        handle: InstanceHandle,
        material: crate::engine::graphics::MaterialHandle,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            if self.instances[idx].renderable.material == material {
                return true;
            }
            self.instances[idx].renderable.material = material;
            self.dirty_draw_cache = true;
            true
        } else {
            false
        }
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
        self.runtime_texture_handles.insert(key.into(), handle);
    }

    pub fn update_texture(
        &mut self,
        handle: InstanceHandle,
        texture: Option<crate::engine::graphics::TextureHandle>,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].texture = texture;
            // Texture affects batching (descriptor binding), but not instance vertex data.
            self.dirty_draw_cache = true;
            true
        } else {
            false
        }
    }

    pub fn update_texture_filtering(
        &mut self,
        handle: InstanceHandle,
        filtering: TextureFiltering,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].texture_filtering = filtering;
            // Filtering affects batching (sampler binding), but not instance vertex data.
            self.dirty_draw_cache = true;
            true
        } else {
            false
        }
    }

    pub fn update(
        &mut self,
        handle: InstanceHandle,
        renderable: GpuRenderable,
        transform: Transform,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            // Preserve per-instance color when updating renderable/transform.
            let color = self.instances[idx].color;
            let opacity = self.instances[idx].opacity;
            let multiple_layers = self.instances[idx].multiple_layers;
            let transparent_cutout = self.instances[idx].transparent_cutout;
            let background = self.instances[idx].background;
            let background_occluded_lit = self.instances[idx].background_occluded_lit;
            let overlay = self.instances[idx].overlay;
            let emissive = self.instances[idx].emissive;
            let texture = self.instances[idx].texture;
            let texture_filtering = self.instances[idx].texture_filtering;
            let quant_steps = self.instances[idx].quant_steps;
            let bones_base = self.instances[idx].bones_base;
            let bones_count = self.instances[idx].bones_count;
            let stencil_ref = self.instances[idx].stencil_ref;
            let is_stencil_clip = self.instances[idx].is_stencil_clip;
            self.instances[idx] = VisualInstance {
                renderable,
                transform,
                color,
                opacity,
                multiple_layers,
                transparent_cutout,
                background,
                background_occluded_lit,
                overlay,
                emissive,
                texture,
                texture_filtering,
                quant_steps,
                bones_base,
                bones_count,
                stencil_ref,
                is_stencil_clip,
            };
            self.dirty_draw_cache = true; // renderable changes likely affect sort/batch
            self.dirty_instance_data = true;
            true
        } else {
            false
        }
    }
}
