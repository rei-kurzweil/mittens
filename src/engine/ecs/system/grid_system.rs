use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{EditorComponent, GridComponent, TransformComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, SignalKind, World};
use crate::utils::math::{mat4_inverse, vec3_normalize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridEntry {
    pub grid_component: ComponentId,
    pub owner_transform: ComponentId,
    pub editor_root: ComponentId,
    pub enabled: bool,
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

impl GridSystem {
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
        let selected = self.selected_grid_for_editor(world, editor_root)?;
        if !selected.enabled || !selected.selectable {
            return None;
        }

        let matrix_world = TransformSystem::world_model(world, selected.owner_transform)?;
        let inverse_world = mat4_inverse(matrix_world)?;
        let origin_world = [matrix_world[3][0], matrix_world[3][1], matrix_world[3][2]];
        let normal_world = vec3_normalize(transform_direction(matrix_world, [0.0, 0.0, 1.0]));
        if normal_world == [0.0, 0.0, 0.0] {
            return None;
        }

        Some(ActiveGrid {
            component: selected.grid_component,
            spacing: world
                .get_component_by_id_as::<GridComponent>(selected.grid_component)?
                .spacing
                .max(1e-4),
            origin_world,
            normal_world,
            matrix_world,
            inverse_world,
        })
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

    pub fn grid_entry(&self, world: &World, grid_component: ComponentId) -> Option<GridEntry> {
        self.ensure_registry_current(world);
        let registry = self.registry.lock().expect("grid registry mutex poisoned");
        registry
            .by_grid
            .get(&grid_component)
            .copied()
            .map(|entry| refresh_grid_entry(world, entry))
    }

    pub fn snap_hit(active: &ActiveGrid, hit_point_world: [f32; 3]) -> GridSnapResult {
        let local = transform_point(active.inverse_world, hit_point_world);
        let cell_x = round_to_i32(local[0] / active.spacing);
        let cell_y = round_to_i32(local[1] / active.spacing);
        let snapped_local = [
            cell_x as f32 * active.spacing,
            cell_y as f32 * active.spacing,
            0.0,
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
}

fn mark_registry_dirty(registry: &Arc<Mutex<GridRegistry>>) {
    registry.lock().expect("grid registry mutex poisoned").dirty = true;
}

fn refresh_grid_entry(world: &World, mut entry: GridEntry) -> GridEntry {
    if let Some(grid) = world.get_component_by_id_as::<GridComponent>(entry.grid_component) {
        entry.enabled = grid.enabled;
        entry.selectable = grid.selectable;
    }
    entry
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_hit_rounds_to_grid_cell_on_xy_plane() {
        let mut world = World::default();
        let active = ActiveGrid {
            component: world.add_component(TransformComponent::new()),
            spacing: 0.5,
            origin_world: [0.0, 0.0, 0.0],
            normal_world: [0.0, 0.0, 1.0],
            matrix_world: TransformComponent::new().transform.matrix_world,
            inverse_world: TransformComponent::new().transform.matrix_world,
        };

        let snapped = GridSystem::snap_hit(&active, [0.24, 0.76, 0.18]);
        assert_eq!(snapped.step.cell, [0, 2]);
        assert!((snapped.point_world[0] - 0.0).abs() < 1e-5);
        assert!((snapped.point_world[1] - 1.0).abs() < 1e-5);
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
}
