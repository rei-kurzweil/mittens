use crate::engine::ecs::ComponentId;
use crate::engine::ecs::Transform;
use crate::engine::graphics::GpuRenderable;
use crate::engine::graphics::primitives::InstanceHandle;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::graphics::MsaaMode;
use crate::engine::graphics::{Skin, SkinId};
use slotmap::{Key, SlotMap};

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
    /// Range into `draw_order`
    pub start: usize,
    pub count: usize,
}

pub struct VisualWorld {
    instances: Vec<VisualInstance>,
    clear_color: [f32; 4],
    renderer_msaa_mode: MsaaMode,
    preferred_window_size: Option<[u32; 2]>,

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

    // Alpha-to-coverage cutout draw data (rebuilt when dirty).
    cutout_order: Vec<u32>,
    cutout_batches: Vec<DrawBatch>,

    // Overlay draw data (rebuilt when dirty).
    // Overlay is drawn on top of all other phases.
    overlay_order: Vec<u32>,
    overlay_batches: Vec<DrawBatch>,

    // Transparent draw data.
    // - Single-layer: cached (order does not depend on view), instanced.
    transparent_single_draw_order: Vec<u32>,
    transparent_single_draw_batches: Vec<DrawBatch>,
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
    pub emissive: u32,
    pub texture: Option<crate::engine::graphics::TextureHandle>,
    pub texture_filtering: TextureFiltering,
    pub quant_steps: f32,

    /// Base index into `VisualWorld::bones_palette`.
    pub bones_base: u32,
    /// Number of bone matrices for this instance.
    pub bones_count: u32,
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

            cutout_order: Vec::new(),
            cutout_batches: Vec::new(),

            overlay_order: Vec::new(),
            overlay_batches: Vec::new(),

            transparent_single_draw_order: Vec::new(),
            transparent_single_draw_batches: Vec::new(),
            transparent_multi_draw_order: Vec::new(),
            transparent_multi_draw_batches: Vec::new(),
        }
    }
}

impl VisualWorld {
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
                start,
                count: cursor - start,
            });
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
        if dt > 0.0 {
            Some(1.0 / dt)
        } else {
            None
        }
    }

    pub fn set_xr_frame_dt_sec(&mut self, dt_sec: Option<f32>) {
        self.xr_frame_dt_sec = dt_sec.and_then(|dt| if dt.is_finite() { Some(dt.max(0.0)) } else { None });
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

        self.transparent_single_draw_order.clear();
        self.transparent_single_draw_batches.clear();
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

    /// Indices into `instances()` in the order they should be drawn (alpha-to-coverage cutout pass).
    pub fn cutout_order(&self) -> &[u32] {
        &self.cutout_order
    }

    pub fn cutout_batches(&self) -> &[DrawBatch] {
        &self.cutout_batches
    }

    /// Indices into `instances()` in the order they should be drawn (overlay pass).
    pub fn overlay_order(&self) -> &[u32] {
        &self.overlay_order
    }

    pub fn overlay_batches(&self) -> &[DrawBatch] {
        &self.overlay_batches
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
        self.cutout_order.clear();
        self.transparent_single_draw_order.clear();
        self.overlay_order.clear();
        // Opaque pass: exclude anything that is transparent.
        for i in 0..self.instances.len() {
            let inst = &self.instances[i];
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

        // Sort by (material, mesh, texture, filtering). Stable sort keeps relative order for identical keys.
        self.draw_order.sort_by_key(|&i| {
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

        let draw_order = &self.draw_order;
        Self::build_draw_batches_for_order(instances, draw_order, &mut self.draw_batches);

        // Cutout pass: batch aggressively (order does not depend on view).
        self.cutout_order.sort_by_key(|&i| {
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
        let cutout_order = &self.cutout_order;
        Self::build_draw_batches_for_order(instances, cutout_order, &mut self.cutout_batches);

        // Single-layer transparent pass: batch aggressively (order does not depend on view).
        self.transparent_single_draw_order.sort_by_key(|&i| {
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
        let transparent_single_draw_order = &self.transparent_single_draw_order;
        Self::build_draw_batches_for_order(
            instances,
            transparent_single_draw_order,
            &mut self.transparent_single_draw_batches,
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
                r.material.0,
                tex,
                r.mesh.0,
                inst.texture_filtering as u8,
                sanitize_quant_steps(inst.quant_steps).to_bits(),
            )
        });
        let overlay_order = &self.overlay_order;
        Self::build_draw_batches_for_order(instances, overlay_order, &mut self.overlay_batches);

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
        emissive: u32,
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
            emissive,
            texture,
            texture_filtering: TextureFiltering::default(),
            quant_steps: sanitize_quant_steps(quant_steps),

            bones_base: 0,
            bones_count: 0,
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

    pub fn update_emissive(&mut self, handle: InstanceHandle, emissive: u32) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            self.instances[idx].emissive = emissive;
            self.dirty_instance_data = true;
            true
        } else {
            false
        }
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
            };
            self.dirty_draw_cache = true; // renderable changes likely affect sort/batch
            self.dirty_instance_data = true;
            true
        } else {
            false
        }
    }
}
