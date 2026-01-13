use crate::engine::ecs::ComponentId;
use crate::engine::ecs::Transform;
use crate::engine::graphics::GpuRenderable;
use crate::engine::graphics::primitives::InstanceHandle;
use crate::engine::graphics::primitives::TransformMatrix;

#[derive(Debug, Clone, Copy)]
pub struct DrawBatch {
    pub material: crate::engine::graphics::MaterialHandle,
    pub mesh: crate::engine::graphics::primitives::MeshHandle,
    pub texture: Option<crate::engine::graphics::TextureHandle>,
    /// Range into `draw_order`
    pub start: usize,
    pub count: usize,
}

pub struct VisualWorld {
    instances: Vec<VisualInstance>,

    point_lights: Vec<VisualPointLight>,
    point_light_index_by_component: std::collections::HashMap<ComponentId, usize>,
    dirty_lights: bool,

    // Active camera state (owned by CameraSystem, mirrored here for renderer snapshot).
    camera_view: [[f32; 4]; 4],
    camera_proj: [[f32; 4]; 4],
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
    draw_order: Vec<u32>, // indices into `instances`
    draw_batches: Vec<DrawBatch>,
}

#[derive(Debug, Clone, Copy)]
pub struct VisualInstance {
    pub renderable: GpuRenderable,
    pub transform: Transform,
    pub color: [f32; 4],
    pub emissive: u32,
    pub texture: Option<crate::engine::graphics::TextureHandle>,
}

impl Default for VisualWorld {
    fn default() -> Self {
        Self {
            instances: Vec::new(),

            point_lights: Vec::new(),
            point_light_index_by_component: std::collections::HashMap::new(),
            dirty_lights: true,

            camera_view: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            camera_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
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
            draw_order: Vec::new(),
            draw_batches: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VisualPointLight {
    pub position_ws: [f32; 3],
    pub intensity: f32,
    pub distance: f32,
    pub color: [f32; 3],
}

impl VisualWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.instances.clear();
        self.handle_to_index.clear();
        self.component_to_handle.clear();
        self.next_handle = 0;

        self.point_lights.clear();
        self.point_light_index_by_component.clear();
        self.dirty_lights = true;

        self.dirty_draw_cache = true;
        self.dirty_instance_data = true;
        self.dirty_camera = true;
        self.draw_order.clear();
        self.draw_batches.clear();
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

    pub fn camera_view(&self) -> [[f32; 4]; 4] {
        self.camera_view
    }

    pub fn camera_proj(&self) -> [[f32; 4]; 4] {
        self.camera_proj
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
        self.camera_view = view;
        self.camera_proj = proj;
        // When a 3D camera becomes active, the 2D camera transform should be neutral.
        self.camera_2d = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        self.dirty_camera = true;
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
    pub fn draw_order(&self) -> &[u32] {
        &self.draw_order
    }

    pub fn draw_batches(&self) -> &[DrawBatch] {
        &self.draw_batches
    }

    /// Call once per frame before rendering. Cheap if nothing changed.
    ///
    /// Returns `true` if the cached draw order/batches were rebuilt this call.
    pub fn prepare_draw_cache(&mut self) -> bool {
        if !self.dirty_draw_cache {
            return false;
        }

        self.draw_order.clear();
        self.draw_order.extend(0..self.instances.len() as u32);

        // Sort by (material, mesh). Stable sort keeps relative order for identical keys.
        self.draw_order.sort_by_key(|&i| {
            let inst = self.instances[i as usize];
            let r = inst.renderable;
            let tex = inst.texture.map(|t| t.0).unwrap_or(u32::MAX);
            (r.material.0, r.mesh.0, tex)
        });

        self.draw_batches.clear();
        let mut cursor = 0usize;
        while cursor < self.draw_order.len() {
            let idx0 = self.draw_order[cursor] as usize;
            let inst0 = self.instances[idx0];
            let r0 = inst0.renderable;
            let material = r0.material;
            let mesh = r0.mesh;
            let texture = inst0.texture;

            let start = cursor;
            cursor += 1;

            while cursor < self.draw_order.len() {
                let idx = self.draw_order[cursor] as usize;
                let inst = self.instances[idx];
                let r = inst.renderable;
                if r.material == material && r.mesh == mesh && inst.texture == texture {
                    cursor += 1;
                } else {
                    break;
                }
            }

            self.draw_batches.push(DrawBatch {
                material,
                mesh,
                texture,
                start,
                count: cursor - start,
            });
        }

        self.dirty_draw_cache = false;
        true
    }

    pub fn register(
        &mut self,
        cid: ComponentId,
        renderable: GpuRenderable,
        transform: Transform,
        color: [f32; 4],
        emissive: u32,
        texture: Option<crate::engine::graphics::TextureHandle>,
    ) -> InstanceHandle {
        let handle = InstanceHandle(self.next_handle);
        self.next_handle = self.next_handle.wrapping_add(1);

        let idx = self.instances.len();
        self.instances.push(VisualInstance {
            renderable,
            transform,
            color,
            emissive,
            texture,
        });
        self.handle_to_index.insert(handle, idx);
        self.component_to_handle.insert(cid, handle);

        self.dirty_draw_cache = true;
        self.dirty_instance_data = true;
        handle
    }

    pub fn remove(&mut self, handle: InstanceHandle) -> bool {
        if let Some(idx) = self.handle_to_index.remove(&handle) {
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

    pub fn update(
        &mut self,
        handle: InstanceHandle,
        renderable: GpuRenderable,
        transform: Transform,
    ) -> bool {
        if let Some(&idx) = self.handle_to_index.get(&handle) {
            // Preserve per-instance color when updating renderable/transform.
            let color = self.instances[idx].color;
            let emissive = self.instances[idx].emissive;
            let texture = self.instances[idx].texture;
            self.instances[idx] = VisualInstance {
                renderable,
                transform,
                color,
                emissive,
                texture,
            };
            self.dirty_draw_cache = true; // renderable changes likely affect sort/batch
            self.dirty_instance_data = true;
            true
        } else {
            false
        }
    }
}
