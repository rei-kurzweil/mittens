use crate::engine::ecs::component::{EditorComponent, GridComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, World};
use crate::utils::math::{mat4_inverse, vec3_normalize};

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
pub struct GridSystem;

impl GridSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn active_grid_for_editor(world: &World, editor_root: ComponentId) -> Option<ActiveGrid> {
        let editor = world.get_component_by_id_as::<EditorComponent>(editor_root)?;
        let selected = editor.selected?;
        let grid = world.get_component_by_id_as::<GridComponent>(selected)?;
        if !grid.enabled || !grid.selectable {
            return None;
        }

        let matrix_world = TransformSystem::world_model(world, selected)?;
        let inverse_world = mat4_inverse(matrix_world)?;
        let origin_world = [matrix_world[3][0], matrix_world[3][1], matrix_world[3][2]];
        let normal_world = vec3_normalize(transform_direction(matrix_world, [0.0, 0.0, 1.0]));
        if normal_world == [0.0, 0.0, 0.0] {
            return None;
        }

        Some(ActiveGrid {
            component: selected,
            spacing: grid.spacing.max(1e-4),
            origin_world,
            normal_world,
            matrix_world,
            inverse_world,
        })
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
    use crate::engine::ecs::component::TransformComponent;

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
        let editor = world.add_component(EditorComponent::new());
        let grid_transform =
            world.add_component(crate::engine::ecs::component::TransformComponent::new());
        let grid = world.add_component(GridComponent::new(0.25));
        let _ = world.add_child(editor, grid_transform);
        let _ = world.add_child(grid_transform, grid);

        world
            .get_component_by_id_as_mut::<EditorComponent>(editor)
            .expect("editor")
            .selected = Some(grid);

        let active = GridSystem::active_grid_for_editor(&world, editor).expect("active grid");
        assert_eq!(active.component, grid);
        assert!((active.spacing - 0.25).abs() < 1e-5);
    }
}
