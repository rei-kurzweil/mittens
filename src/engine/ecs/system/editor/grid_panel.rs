use crate::engine::ecs::component::EditorComponent;
use crate::engine::ecs::system::GridSystem;
use crate::engine::ecs::system::editor::world_panel::world_panel_item_label;
use crate::engine::ecs::{ComponentId, World};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GridPanelState {
    pub(crate) active_editor: Option<ComponentId>,
    pub(crate) selected_grid_transform: Option<ComponentId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GridPanelEvent {
    SelectionChanged {
        editor_root: Option<ComponentId>,
        selected_component: Option<ComponentId>,
    },
    GridDeleted {
        owner_transform: ComponentId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GridPanelEntry {
    pub(crate) grid_component: ComponentId,
    pub(crate) owner_transform: ComponentId,
    pub(crate) label: String,
    pub(crate) visible: bool,
    pub(crate) selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GridPanelModel {
    pub(crate) title: String,
    pub(crate) rows: Vec<GridPanelEntry>,
    pub(crate) active_editor: Option<ComponentId>,
}

pub(crate) fn reduce_grid_panel_state(
    old: &GridPanelState,
    event: &GridPanelEvent,
) -> GridPanelState {
    let mut new = old.clone();
    match event {
        GridPanelEvent::SelectionChanged {
            editor_root,
            selected_component,
        } => {
            new.active_editor = *editor_root;
            new.selected_grid_transform = *selected_component;
        }
        GridPanelEvent::GridDeleted { owner_transform } => {
            if new.selected_grid_transform == Some(*owner_transform) {
                new.selected_grid_transform = None;
            }
        }
    }
    new
}

pub(crate) fn build_grid_panel_model(
    world: &World,
    grids: &GridSystem,
    editor_root: ComponentId,
) -> GridPanelModel {
    let selected_component = world
        .get_component_by_id_as::<EditorComponent>(editor_root)
        .and_then(|editor| editor.selected);

    let rows = grids
        .enumerate_grids_for_editor(world, editor_root)
        .into_iter()
        .map(|entry| GridPanelEntry {
            grid_component: entry.grid_component,
            owner_transform: entry.owner_transform,
            label: world_panel_item_label(world, entry.owner_transform),
            visible: entry.enabled,
            selected: selected_component == Some(entry.owner_transform)
                || selected_component == Some(entry.grid_component),
        })
        .collect();

    GridPanelModel {
        title: "Grids".to_string(),
        rows,
        active_editor: Some(editor_root),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{GridComponent, TransformComponent};

    #[test]
    fn reduce_grid_panel_state_tracks_selection_and_delete() {
        let mut world = World::default();
        let a = world.add_component(TransformComponent::new());
        let b = world.add_component(TransformComponent::new());
        let state = reduce_grid_panel_state(
            &GridPanelState::default(),
            &GridPanelEvent::SelectionChanged {
                editor_root: Some(a),
                selected_component: Some(b),
            },
        );
        assert_eq!(state.active_editor, Some(a));
        assert_eq!(state.selected_grid_transform, Some(b));

        let cleared =
            reduce_grid_panel_state(&state, &GridPanelEvent::GridDeleted { owner_transform: b });
        assert_eq!(cleared.selected_grid_transform, None);
    }

    #[test]
    fn build_grid_panel_model_marks_selected_transform_grid() {
        let mut world = World::default();
        let grids = GridSystem::new();
        let editor = world.add_component(EditorComponent::new());
        let transform =
            world.add_component_boxed_named("grid_1", Box::new(TransformComponent::new()));
        let grid = world.add_component(GridComponent::new(0.5));
        let _ = world.add_child(editor, transform);
        let _ = world.add_child(transform, grid);
        world
            .get_component_by_id_as_mut::<EditorComponent>(editor)
            .expect("editor")
            .selected = Some(transform);

        let model = build_grid_panel_model(&world, &grids, editor);
        assert_eq!(model.title, "Grids");
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].label, "grid_1");
        assert!(model.rows[0].visible);
        assert!(model.rows[0].selected);
    }
}
