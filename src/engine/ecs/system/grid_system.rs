use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{
    ColorComponent, EditorComponent, GridComponent, OpacityComponent, RaycastableComponent,
    RenderableComponent, SelectableComponent, SerializeComponent, TransformComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, SignalEmitter, SignalKind, World,
};
use crate::utils::math::{
    mat_to_quat, mat4_inverse, mat4_mul, quat_from_axis_angle, quat_mul, vec3_normalize,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridEntry {
    pub grid_component: ComponentId,
    pub owner_transform: ComponentId,
    pub editor_root: ComponentId,
    pub enabled: bool,
    pub hidden: bool,
    pub selectable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveGrid {
    pub component: ComponentId,
    pub spacing: f32,
    pub origin_world: [f32; 3],
    pub normal_world: [f32; 3],
    pub matrix_world: [[f32; 4]; 4],
    pub inverse_world: [[f32; 4]; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridStep {
    pub cell: [i32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSnapResult {
    pub point_world: [f32; 3],
    pub normal_world: [f32; 3],
    pub step: GridStep,
}

#[derive(Debug, Default)]
struct GridRegistry {
    dirty: bool,
    handlers_installed: bool,
    by_grid: HashMap<ComponentId, GridEntry>,
    by_editor: HashMap<ComponentId, Vec<ComponentId>>,
    cached_component_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct GridSystem {
    registry: Arc<Mutex<GridRegistry>>,
}

const GRID_LIVE_ROOT_NAME: &str = "grid_live_root";
const GRID_LIVE_SELECTABLE_NAME: &str = "grid_live_selectable";
const GRID_LIVE_RAYCASTABLE_NAME: &str = "grid_live_raycastable";
const GRID_LIVE_SERIALIZE_NAME: &str = "grid_live_serialize";
const GRID_LIVE_SHAPE_NAME: &str = "grid_live_shape";
const GRID_LIVE_RENDERABLE_NAME: &str = "grid_live_renderable";
const GRID_LIVE_COLOR_NAME: &str = "grid_live_color";
const GRID_LIVE_OPACITY_NAME: &str = "grid_live_opacity";

impl GridSystem {
    pub const DEFAULT_EDITOR_GRID_SIZE_X: u32 = 8192;
    pub const DEFAULT_EDITOR_GRID_SIZE_Z: u32 = 8192;
    pub const DEFAULT_EDITOR_GRID_SPACING: f32 = 1.0;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_handlers(&self, rx: &mut RxWorld) {
        let mut registry = self.registry.lock().expect("grid registry mutex poisoned");
        if registry.handlers_installed {
            return;
        }
        registry.handlers_installed = true;
        drop(registry);

        let registry = Arc::clone(&self.registry);
        rx.add_global_handler_closure_named(
            SignalKind::ParentChanged,
            Some("grid_system_live_registry".to_string()),
            move |_world, _emit, signal| {
                if matches!(
                    signal.event.as_ref(),
                    Some(EventSignal::ParentChanged { .. })
                ) {
                    mark_registry_dirty(&registry);
                }
            },
        );
    }

    pub fn mark_dirty(&self) {
        mark_registry_dirty(&self.registry);
    }

    pub fn grid_owner_transform(world: &World, grid_component: ComponentId) -> Option<ComponentId> {
        let mut current = world.parent_of(grid_component);
        while let Some(component_id) = current {
            if world
                .get_component_by_id_as::<TransformComponent>(component_id)
                .is_some()
            {
                return Some(component_id);
            }
            current = world.parent_of(component_id);
        }
        None
    }

    pub fn active_grid_for_editor(
        &self,
        world: &World,
        editor_root: ComponentId,
    ) -> Option<ActiveGrid> {
        let selected = self
            .selected_grid_for_editor(world, editor_root)
            .or_else(|| {
                self.enumerate_grids_for_editor(world, editor_root)
                    .into_iter()
                    .next()
            })?;
        if !selected.selectable || !selected.enabled || selected.hidden {
            return None;
        }
        self.active_grid_from_entry(world, selected)
    }

    pub fn selected_grid_for_editor(
        &self,
        world: &World,
        editor_root: ComponentId,
    ) -> Option<GridEntry> {
        let editor = world.get_component_by_id_as::<EditorComponent>(editor_root)?;
        let selected = editor.selected?;
        if let Some(entry) = self.grid_entry(world, selected)
            && entry.editor_root == editor_root
        {
            return Some(entry);
        }
        self.grid_owned_by_transform(world, selected)
            .filter(|entry| entry.editor_root == editor_root)
    }

    pub fn enumerate_grids_for_editor(
        &self,
        world: &World,
        editor_root: ComponentId,
    ) -> Vec<GridEntry> {
        self.ensure_registry_current(world);
        let registry = self.registry.lock().expect("grid registry mutex poisoned");
        registry
            .by_editor
            .get(&editor_root)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|grid_id| {
                registry
                    .by_grid
                    .get(grid_id)
                    .copied()
                    .map(|entry| refresh_grid_entry(world, entry))
            })
            .collect()
    }

    pub fn grid_owned_by_transform(
        &self,
        world: &World,
        transform: ComponentId,
    ) -> Option<GridEntry> {
        self.ensure_registry_current(world);
        world
            .children_of(transform)
            .iter()
            .copied()
            .find_map(|child| self.grid_entry(world, child))
    }

    pub fn active_grid_for_owner_transform(
        &self,
        world: &World,
        owner_transform: ComponentId,
    ) -> Option<ActiveGrid> {
        let entry = self.grid_owned_by_transform(world, owner_transform)?;
        if !entry.enabled || entry.hidden || !entry.selectable {
            return None;
        }
        self.active_grid_from_entry(world, entry)
    }

    pub fn grid_entry(&self, world: &World, grid_component: ComponentId) -> Option<GridEntry> {
        self.ensure_registry_current(world);
        let registry = self.registry.lock().expect("grid registry mutex poisoned");
        registry
            .by_grid
            .get(&grid_component)
            .copied()
            .map(|entry| refresh_grid_entry(world, entry))
    }

    pub fn grid_hit_context_for_renderable(
        &self,
        world: &World,
        renderable: ComponentId,
    ) -> Option<ActiveGrid> {
        let grid_component = self.grid_component_for_renderable(world, renderable)?;
        let entry = self.grid_entry(world, grid_component)?;
        if !entry.enabled || entry.hidden || !entry.selectable {
            return None;
        }
        self.active_grid_from_entry(world, entry)
    }

    pub fn grid_component_for_renderable(
        &self,
        world: &World,
        renderable: ComponentId,
    ) -> Option<ComponentId> {
        let owner_transform = self.grid_owner_from_renderable(world, renderable)?;
        self.grid_owned_by_transform(world, owner_transform)
            .map(|entry| entry.grid_component)
    }

    pub fn grid_owner_from_renderable(
        &self,
        world: &World,
        renderable: ComponentId,
    ) -> Option<ComponentId> {
        let mut current = Some(renderable);
        while let Some(component_id) = current {
            if world.component_label(component_id) == Some(GRID_LIVE_ROOT_NAME) {
                return Self::grid_owner_transform(world, component_id);
            }
            current = world.parent_of(component_id);
        }
        None
    }

    pub fn snap_hit(active: &ActiveGrid, hit_point_world: [f32; 3]) -> GridSnapResult {
        let local = transform_point(active.inverse_world, hit_point_world);
        let cell_x = round_to_i32(local[0] / active.spacing);
        let cell_y = round_to_i32(local[2] / active.spacing);
        let snapped_local = [
            cell_x as f32 * active.spacing,
            0.0,
            cell_y as f32 * active.spacing,
        ];
        let snapped_world = transform_point(active.matrix_world, snapped_local);

        GridSnapResult {
            point_world: snapped_world,
            normal_world: active.normal_world,
            step: GridStep {
                cell: [cell_x, cell_y],
            },
        }
    }

    pub fn snap_point_preserving_plane_offset(
        active: &ActiveGrid,
        point_world: [f32; 3],
    ) -> GridSnapResult {
        let local = transform_point(active.inverse_world, point_world);
        let cell_x = round_to_i32(local[0] / active.spacing);
        let cell_y = round_to_i32(local[2] / active.spacing);
        let snapped_local = [
            cell_x as f32 * active.spacing,
            local[1],
            cell_y as f32 * active.spacing,
        ];
        let snapped_world = transform_point(active.matrix_world, snapped_local);

        GridSnapResult {
            point_world: snapped_world,
            normal_world: active.normal_world,
            step: GridStep {
                cell: [cell_x, cell_y],
            },
        }
    }

    pub fn same_step(a: Option<GridStep>, b: GridStep) -> bool {
        a == Some(b)
    }

    pub fn ensure_default_grid(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
    ) -> ComponentId {
        if let Some(existing) = self
            .enumerate_grids_for_editor(world, editor_root)
            .into_iter()
            .next()
        {
            return existing.owner_transform;
        }

        self.spawn_grid_for_editor(
            world,
            emit,
            editor_root,
            GridSpawnSpec::default_hidden_editor_grid(),
        )
    }

    pub fn spawn_grid_for_editor(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        spec: GridSpawnSpec,
    ) -> ComponentId {
        let index = self.enumerate_grids_for_editor(world, editor_root).len() + 1;
        let (translation, rotation) = if spec.world_space_pose {
            editor_local_pose_from_world(world, editor_root, spec.translation, spec.rotation)
                .unwrap_or((spec.translation, spec.rotation))
        } else {
            (spec.translation, spec.rotation)
        };
        let grid_component = GridComponent::new(spec.spacing)
            .with_size_x(spec.size_x)
            .with_size_z(spec.size_z)
            .with_enabled(spec.enabled)
            .with_hidden(spec.hidden)
            .with_selectable(true);

        let owner_transform = world.add_component_boxed_named(
            &format!("grid_{index}"),
            Box::new(
                TransformComponent::new()
                    .with_position(translation[0], translation[1], translation[2])
                    .with_rotation_quat(rotation),
            ),
        );
        let grid = world.add_component_boxed_named(
            &format!("grid_{index}_component"),
            Box::new(grid_component),
        );

        let _ = world.add_child(editor_root, owner_transform);
        let _ = world.add_child(owner_transform, grid);
        world.init_component_tree(owner_transform, emit);
        if spec.preview_mode || spec.enabled {
            self.ensure_live_runtime(world, emit, owner_transform, spec.preview_mode);
        }
        self.mark_dirty();
        owner_transform
    }

    pub fn set_grid_hidden(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        owner_transform: ComponentId,
        hidden: bool,
    ) -> bool {
        let Some(entry) = self.grid_owned_by_transform(world, owner_transform) else {
            return false;
        };
        let Some(grid) = world.get_component_by_id_as_mut::<GridComponent>(entry.grid_component)
        else {
            return false;
        };
        if grid.hidden == hidden {
            return false;
        }
        grid.hidden = hidden;
        if grid.enabled {
            self.ensure_live_runtime(world, emit, owner_transform, false);
            self.sync_live_runtime_visibility(world, emit, owner_transform, false);
        }
        self.mark_dirty();
        true
    }

    pub fn set_grid_enabled(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        owner_transform: ComponentId,
        enabled: bool,
    ) -> bool {
        let Some(entry) = self.grid_owned_by_transform(world, owner_transform) else {
            return false;
        };
        let Some(grid) = world.get_component_by_id_as_mut::<GridComponent>(entry.grid_component)
        else {
            return false;
        };
        if grid.enabled == enabled {
            return false;
        }
        grid.enabled = enabled;
        if enabled {
            self.ensure_live_runtime(world, emit, owner_transform, false);
            self.sync_live_runtime_visibility(world, emit, owner_transform, false);
        } else {
            self.remove_live_runtime(world, owner_transform);
        }
        self.mark_dirty();
        true
    }

    pub fn toggle_grid_hidden(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        owner_transform: ComponentId,
    ) -> bool {
        let hidden = self
            .grid_owned_by_transform(world, owner_transform)
            .and_then(|entry| world.get_component_by_id_as::<GridComponent>(entry.grid_component))
            .map(|grid| !grid.hidden);
        hidden.is_some_and(|next| self.set_grid_hidden(world, emit, owner_transform, next))
    }

    pub fn toggle_grid_enabled(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        owner_transform: ComponentId,
    ) -> bool {
        let enabled = self
            .grid_owned_by_transform(world, owner_transform)
            .and_then(|entry| world.get_component_by_id_as::<GridComponent>(entry.grid_component))
            .map(|grid| !grid.enabled);
        enabled.is_some_and(|next| self.set_grid_enabled(world, emit, owner_transform, next))
    }

    pub fn delete_grid(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        owner_transform: ComponentId,
    ) -> bool {
        if world.get_component_record(owner_transform).is_none() {
            return false;
        }

        // The cleanup-aware subtree executor unregisters renderables, BVH/raycast
        // entries, transforms, and other runtime state before removing ECS records.
        // Removing the World subtree here first leaves those later cleanup intents
        // without the component handles they need, producing orphaned visuals.
        emit.push_intent_now(
            owner_transform,
            IntentValue::RemoveSubtree {
                component_ids: vec![owner_transform],
            },
        );
        self.mark_dirty();
        true
    }

    fn ensure_registry_current(&self, world: &World) {
        let component_count = world.all_components().count();
        let needs_sync = {
            let registry = self.registry.lock().expect("grid registry mutex poisoned");
            registry.dirty || registry.cached_component_count != component_count
        };
        if !needs_sync {
            return;
        }

        let mut by_grid = HashMap::new();
        let mut by_editor: HashMap<ComponentId, Vec<ComponentId>> = HashMap::new();
        for component_id in world.all_components() {
            let Some(grid) = world.get_component_by_id_as::<GridComponent>(component_id) else {
                continue;
            };
            let Some(owner_transform) = Self::grid_owner_transform(world, component_id) else {
                continue;
            };
            let Some(editor_root) = nearest_editor_ancestor(world, owner_transform) else {
                continue;
            };
            let entry = GridEntry {
                grid_component: component_id,
                owner_transform,
                editor_root,
                enabled: grid.enabled,
                hidden: grid.hidden,
                selectable: grid.selectable,
            };
            by_grid.insert(component_id, entry);
            by_editor.entry(editor_root).or_default().push(component_id);
        }

        let mut registry = self.registry.lock().expect("grid registry mutex poisoned");
        registry.by_grid = by_grid;
        registry.by_editor = by_editor;
        registry.cached_component_count = component_count;
        registry.dirty = false;
    }

    fn active_grid_from_entry(&self, world: &World, entry: GridEntry) -> Option<ActiveGrid> {
        let matrix_world = TransformSystem::world_model(world, entry.owner_transform)?;
        let inverse_world = mat4_inverse(matrix_world)?;
        let origin_world = [matrix_world[3][0], matrix_world[3][1], matrix_world[3][2]];
        let normal_world = vec3_normalize(transform_direction(matrix_world, [0.0, 1.0, 0.0]));
        if normal_world == [0.0, 0.0, 0.0] {
            return None;
        }

        Some(ActiveGrid {
            component: entry.grid_component,
            spacing: world
                .get_component_by_id_as::<GridComponent>(entry.grid_component)?
                .spacing
                .max(1e-4),
            origin_world,
            normal_world,
            matrix_world,
            inverse_world,
        })
    }

    fn ensure_live_runtime(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        owner_transform: ComponentId,
        preview_mode: bool,
    ) {
        if self.live_runtime_root(world, owner_transform).is_some() {
            self.sync_live_runtime_visibility(world, emit, owner_transform, preview_mode);
            return;
        }
        let Some(entry) = self.grid_owned_by_transform(world, owner_transform) else {
            return;
        };
        let Some(grid) = world
            .get_component_by_id_as::<GridComponent>(entry.grid_component)
            .copied()
        else {
            return;
        };

        let visual_scale_x = grid.size_x as f32 * grid.spacing;
        let visual_scale_z = grid.size_z as f32 * grid.spacing;
        let live_root = world
            .add_component_boxed_named(GRID_LIVE_ROOT_NAME, Box::new(TransformComponent::new()));
        let live_selectable = world.add_component_boxed_named(
            GRID_LIVE_SELECTABLE_NAME,
            Box::new(SelectableComponent::off()),
        );
        let live_raycastable = world.add_component_boxed_named(
            GRID_LIVE_RAYCASTABLE_NAME,
            Box::new(RaycastableComponent::disabled()),
        );
        let live_serialize = world.add_component_boxed_named(
            GRID_LIVE_SERIALIZE_NAME,
            Box::new(SerializeComponent::off()),
        );
        let live_shape = world.add_component_boxed_named(
            GRID_LIVE_SHAPE_NAME,
            Box::new(
                TransformComponent::new()
                    .with_position(0.0, 0.005, 0.0)
                    .with_scale(visual_scale_x, 0.0025, visual_scale_z),
            ),
        );
        let live_renderable = world.add_component_boxed_named(
            GRID_LIVE_RENDERABLE_NAME,
            Box::new(RenderableComponent::from_cpu_mesh_handle(
                crate::engine::graphics::primitives::CpuMeshHandle::CUBE,
                crate::engine::graphics::primitives::MaterialHandle::GRID_MESH,
            )),
        );
        let live_color = world.add_component_boxed_named(
            GRID_LIVE_COLOR_NAME,
            Box::new(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0)),
        );
        let live_opacity = world.add_component_boxed_named(
            GRID_LIVE_OPACITY_NAME,
            Box::new(OpacityComponent::new().with_opacity(grid_opacity(grid.hidden, preview_mode))),
        );

        let _ = world.add_child(owner_transform, live_root);
        let _ = world.add_child(live_root, live_selectable);
        let _ = world.add_child(live_root, live_serialize);
        let _ = world.add_child(live_root, live_shape);
        let _ = world.add_child(live_shape, live_renderable);
        let _ = world.add_child(live_renderable, live_color);
        let _ = world.add_child(live_renderable, live_opacity);
        let _ = world.add_child(live_renderable, live_raycastable);
        world.init_component_tree(live_root, emit);
        emit.push_intent_now(
            live_root,
            IntentValue::RegisterTransform {
                component_ids: vec![live_root, live_shape],
            },
        );
        emit.push_intent_now(
            live_renderable,
            IntentValue::RegisterRenderable {
                component_ids: vec![live_renderable],
            },
        );
    }

    fn sync_live_runtime_visibility(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        owner_transform: ComponentId,
        preview_mode: bool,
    ) {
        let Some(entry) = self.grid_owned_by_transform(world, owner_transform) else {
            return;
        };
        let Some(grid) = world
            .get_component_by_id_as::<GridComponent>(entry.grid_component)
            .copied()
        else {
            return;
        };
        if let Some(opacity_id) = world.find_component(owner_transform, "#grid_live_opacity")
            && let Some(opacity) = world.get_component_by_id_as_mut::<OpacityComponent>(opacity_id)
        {
            opacity.opacity = grid_opacity(grid.hidden, preview_mode);
            emit.push_intent_now(
                owner_transform,
                IntentValue::RegisterOpacity {
                    component_ids: vec![opacity_id],
                },
            );
        }
        if let Some(selectable_id) = world.find_component(owner_transform, "#grid_live_selectable")
            && let Some(selectable) =
                world.get_component_by_id_as_mut::<SelectableComponent>(selectable_id)
        {
            selectable.enabled = false;
        }
        if let Some(raycastable_id) =
            world.find_component(owner_transform, "#grid_live_raycastable")
            && let Some(raycastable) =
                world.get_component_by_id_as_mut::<RaycastableComponent>(raycastable_id)
        {
            raycastable.enable = false;
        }
    }

    fn live_runtime_root(
        &self,
        world: &World,
        owner_transform: ComponentId,
    ) -> Option<ComponentId> {
        world.find_component(owner_transform, "#grid_live_root")
    }

    fn remove_live_runtime(&self, world: &mut World, owner_transform: ComponentId) {
        if let Some(live_root) = self.live_runtime_root(world, owner_transform) {
            let _ = world.remove_component_subtree(live_root);
        }
    }
}

fn mark_registry_dirty(registry: &Arc<Mutex<GridRegistry>>) {
    registry.lock().expect("grid registry mutex poisoned").dirty = true;
}

fn refresh_grid_entry(world: &World, mut entry: GridEntry) -> GridEntry {
    if let Some(grid) = world.get_component_by_id_as::<GridComponent>(entry.grid_component) {
        entry.enabled = grid.enabled;
        entry.hidden = grid.hidden;
        entry.selectable = grid.selectable;
    }
    entry
}

fn grid_opacity(hidden: bool, preview_mode: bool) -> f32 {
    if preview_mode {
        0.45
    } else if hidden {
        0.0
    } else {
        1.0
    }
}

fn nearest_editor_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(component_id) = current {
        if world
            .get_component_by_id_as::<EditorComponent>(component_id)
            .is_some()
        {
            return Some(component_id);
        }
        current = world.parent_of(component_id);
    }
    None
}

fn round_to_i32(value: f32) -> i32 {
    value.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

fn transform_point(m: [[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

fn transform_direction(m: [[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

fn editor_local_pose_from_world(
    world: &World,
    editor_root: ComponentId,
    world_translation: [f32; 3],
    world_rotation: [f32; 4],
) -> Option<([f32; 3], [f32; 4])> {
    let editor_world = TransformSystem::world_model(world, editor_root)?;
    let editor_inverse = mat4_inverse(editor_world)?;
    let world_model = TransformComponent::new()
        .with_position(
            world_translation[0],
            world_translation[1],
            world_translation[2],
        )
        .with_rotation_quat(world_rotation)
        .transform
        .model;
    let local_model = mat4_mul(editor_inverse, world_model);
    Some((
        [local_model[3][0], local_model[3][1], local_model[3][2]],
        mat_to_quat(local_model),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSpawnSpec {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub world_space_pose: bool,
    pub spacing: f32,
    pub size_x: u32,
    pub size_z: u32,
    pub enabled: bool,
    pub hidden: bool,
    pub preview_mode: bool,
}

impl GridSpawnSpec {
    pub fn default_hidden_editor_grid() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            world_space_pose: false,
            spacing: GridSystem::DEFAULT_EDITOR_GRID_SPACING,
            size_x: GridSystem::DEFAULT_EDITOR_GRID_SIZE_X,
            size_z: GridSystem::DEFAULT_EDITOR_GRID_SIZE_Z,
            enabled: true,
            hidden: true,
            preview_mode: false,
        }
    }

    pub fn from_cursor_pose(
        translation: Option<[f32; 3]>,
        rotation: Option<[f32; 4]>,
        preview_mode: bool,
    ) -> Self {
        Self {
            translation: translation.unwrap_or([0.0, 0.0, 0.0]),
            rotation: rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]),
            world_space_pose: true,
            spacing: GridSystem::DEFAULT_EDITOR_GRID_SPACING,
            size_x: GridComponent::DEFAULT_SIZE_X,
            size_z: GridComponent::DEFAULT_SIZE_Z,
            enabled: true,
            hidden: preview_mode,
            preview_mode,
        }
    }
}

pub fn remap_grid_rotation_to_surface_up(surface_aligned_rotation: [f32; 4]) -> [f32; 4] {
    let z_to_y = quat_from_axis_angle([1.0, 0.0, 0.0], std::f32::consts::FRAC_PI_2);
    quat_mul(surface_aligned_rotation, z_to_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;

    #[test]
    fn snap_hit_rounds_to_grid_cell_on_xz_plane() {
        let mut world = World::default();
        let active = ActiveGrid {
            component: world.add_component(TransformComponent::new()),
            spacing: 0.5,
            origin_world: [0.0, 0.0, 0.0],
            normal_world: [0.0, 1.0, 0.0],
            matrix_world: TransformComponent::new().transform.matrix_world,
            inverse_world: TransformComponent::new().transform.matrix_world,
        };

        let snapped = GridSystem::snap_hit(&active, [0.24, 0.76, 0.18]);
        assert_eq!(snapped.step.cell, [0, 0]);
        assert!((snapped.point_world[0] - 0.0).abs() < 1e-5);
        assert!((snapped.point_world[1] - 0.0).abs() < 1e-5);
        assert!((snapped.point_world[2] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn snap_point_preserving_plane_offset_keeps_local_height() {
        let mut world = World::default();
        let active = ActiveGrid {
            component: world.add_component(TransformComponent::new()),
            spacing: 0.5,
            origin_world: [0.0, 0.0, 0.0],
            normal_world: [0.0, 1.0, 0.0],
            matrix_world: TransformComponent::new().transform.matrix_world,
            inverse_world: TransformComponent::new().transform.matrix_world,
        };

        let snapped = GridSystem::snap_point_preserving_plane_offset(&active, [0.24, 0.76, 0.18]);
        assert_eq!(snapped.step.cell, [0, 0]);
        assert!((snapped.point_world[0] - 0.0).abs() < 1e-5);
        assert!((snapped.point_world[1] - 0.76).abs() < 1e-5);
        assert!((snapped.point_world[2] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn active_grid_uses_editor_selected_grid_component() {
        let mut world = World::default();
        let grids = GridSystem::new();
        let editor = world.add_component(EditorComponent::new());
        let grid_transform = world.add_component(TransformComponent::new());
        let grid = world.add_component(GridComponent::new(0.25));
        let _ = world.add_child(editor, grid_transform);
        let _ = world.add_child(grid_transform, grid);

        world
            .get_component_by_id_as_mut::<EditorComponent>(editor)
            .expect("editor")
            .selected = Some(grid);

        let active = grids
            .active_grid_for_editor(&world, editor)
            .expect("active grid");
        assert_eq!(active.component, grid);
        assert!((active.spacing - 0.25).abs() < 1e-5);
    }

    #[test]
    fn active_grid_uses_grid_owned_by_selected_transform() {
        let mut world = World::default();
        let grids = GridSystem::new();
        let editor = world.add_component(EditorComponent::new());
        let grid_transform = world.add_component(TransformComponent::new());
        let grid = world.add_component(GridComponent::new(0.75));
        let _ = world.add_child(editor, grid_transform);
        let _ = world.add_child(grid_transform, grid);

        world
            .get_component_by_id_as_mut::<EditorComponent>(editor)
            .expect("editor")
            .selected = Some(grid_transform);

        let active = grids
            .active_grid_for_editor(&world, editor)
            .expect("active grid");
        assert_eq!(active.component, grid);
        assert!((active.spacing - 0.75).abs() < 1e-5);
    }

    #[test]
    fn enumerate_grids_for_editor_returns_owner_transforms() {
        let mut world = World::default();
        let grids = GridSystem::new();
        let editor = world.add_component(EditorComponent::new());
        let a_transform = world.add_component(TransformComponent::new());
        let a_grid = world.add_component(GridComponent::new(0.25));
        let b_transform = world.add_component(TransformComponent::new());
        let b_grid = world.add_component(GridComponent::new(0.5).with_enabled(false));
        let _ = world.add_child(editor, a_transform);
        let _ = world.add_child(a_transform, a_grid);
        let _ = world.add_child(editor, b_transform);
        let _ = world.add_child(b_transform, b_grid);

        let entries = grids.enumerate_grids_for_editor(&world, editor);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| {
            entry.grid_component == a_grid
                && entry.owner_transform == a_transform
                && entry.editor_root == editor
                && entry.enabled
                && !entry.hidden
                && entry.selectable
        }));
        assert!(entries.iter().any(|entry| {
            entry.grid_component == b_grid
                && entry.owner_transform == b_transform
                && entry.editor_root == editor
                && !entry.enabled
        }));
    }

    #[test]
    fn grid_entry_reads_live_enabled_state_after_component_edit() {
        let mut world = World::default();
        let grids = GridSystem::new();
        let editor = world.add_component(EditorComponent::new());
        let transform = world.add_component(TransformComponent::new());
        let grid = world.add_component(GridComponent::new(1.0));
        let _ = world.add_child(editor, transform);
        let _ = world.add_child(transform, grid);

        assert!(
            grids
                .grid_entry(&world, grid)
                .expect("grid entry before toggle")
                .enabled
        );

        world
            .get_component_by_id_as_mut::<GridComponent>(grid)
            .expect("grid component")
            .enabled = false;

        assert!(
            !grids
                .grid_entry(&world, grid)
                .expect("grid entry after toggle")
                .enabled
        );
    }

    #[test]
    fn active_grid_ignores_hidden_grid() {
        let mut world = World::default();
        let grids = GridSystem::new();
        let editor = world.add_component(EditorComponent::new());
        let grid_transform = world.add_component(TransformComponent::new());
        let grid = world.add_component(GridComponent::new(1.0).with_hidden(true));
        let _ = world.add_child(editor, grid_transform);
        let _ = world.add_child(grid_transform, grid);

        world
            .get_component_by_id_as_mut::<EditorComponent>(editor)
            .expect("editor")
            .selected = Some(grid_transform);

        assert!(grids.active_grid_for_editor(&world, editor).is_none());
    }

    #[test]
    fn set_grid_hidden_re_registers_live_opacity() {
        let mut world = World::default();
        let grids = GridSystem::new();
        let editor = world.add_component(EditorComponent::new());
        let mut emit = CommandQueue::new();

        let owner_transform = grids.spawn_grid_for_editor(
            &mut world,
            &mut emit,
            editor,
            GridSpawnSpec::default_hidden_editor_grid(),
        );

        let mut rx = RxWorld::default();
        emit.drain_into_rx(&mut rx);
        let _ = rx.drain_ready_intents();

        assert!(grids.set_grid_hidden(&mut world, &mut emit, owner_transform, false));

        emit.drain_into_rx(&mut rx);
        let intents = rx.drain_ready_intents();
        assert!(intents.iter().any(|signal| {
            matches!(
                signal.intent.as_ref().map(|intent| &intent.value),
                Some(IntentValue::RegisterOpacity { .. })
            )
        }));
    }

    #[test]
    fn spawn_grid_from_cursor_pose_converts_world_pose_into_editor_local_space() {
        let mut world = World::default();
        let grids = GridSystem::new();
        let mut emit = CommandQueue::new();
        let editor_mount = world.add_component(
            TransformComponent::new()
                .with_position(5.0, 0.0, -2.0)
                .with_rotation_quat(quat_from_axis_angle(
                    [0.0, 1.0, 0.0],
                    std::f32::consts::FRAC_PI_2,
                )),
        );
        let editor = world.add_component(EditorComponent::new());
        let _ = world.add_child(editor_mount, editor);

        let desired_world_translation = [8.0, 1.5, 4.0];
        let desired_world_rotation =
            quat_from_axis_angle([1.0, 0.0, 0.0], std::f32::consts::FRAC_PI_2);

        let owner_transform = grids.spawn_grid_for_editor(
            &mut world,
            &mut emit,
            editor,
            GridSpawnSpec::from_cursor_pose(
                Some(desired_world_translation),
                Some(desired_world_rotation),
                false,
            ),
        );

        let editor_world = TransformSystem::world_model(&world, editor).expect("editor world");
        let owner_local = world
            .get_component_by_id_as::<TransformComponent>(owner_transform)
            .expect("grid local")
            .transform
            .model;
        let owner_world = mat4_mul(editor_world, owner_local);
        let actual_world_translation = [owner_world[3][0], owner_world[3][1], owner_world[3][2]];
        let actual_world_rotation = mat_to_quat(owner_world);

        for axis in 0..3 {
            assert!(
                (actual_world_translation[axis] - desired_world_translation[axis]).abs() < 1e-4
            );
        }
        let rotation_dot = actual_world_rotation[0] * desired_world_rotation[0]
            + actual_world_rotation[1] * desired_world_rotation[1]
            + actual_world_rotation[2] * desired_world_rotation[2]
            + actual_world_rotation[3] * desired_world_rotation[3];
        assert!(rotation_dot.abs() > 0.9999);
    }
}
