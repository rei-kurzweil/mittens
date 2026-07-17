use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{SelectionComponent, SelectionEntry, SelectionMode};
use crate::engine::ecs::system::data_renderer_system::DataRendererSystem;
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::grid_panel::rerender_grid_panel_from_context;
use crate::engine::ecs::system::editor::inspector_panel::InspectorPanelModel;
use crate::engine::ecs::system::editor::settings_panel::sync_editor_settings_panel_selection;
use crate::engine::ecs::system::editor::world_panel::{
    PANEL_CONTENT_SLOT_SELECTOR, WORLD_PANEL_ROOT_SELECTOR, WORLD_PANEL_SELECTION_SELECTOR,
    WorldPanelModel, mark_nearest_layout_dirty, rerender_world_panel_content,
};
use crate::engine::ecs::system::panel_system::{
    PANEL_LAYOUT_MOUNT_NAME, ensure_panel_layout_selection, panel_layout_selection_id,
    spawn_editor_panel_layout_tree,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::RenderAssets;

pub(crate) fn reconcile_editor_panel_layout(
    world: &mut World,
    render_assets: &mut RenderAssets,
    emit: &mut dyn SignalEmitter,
    panel_layout_spawned: &mut bool,
    panel_query_root: ComponentId,
    editor_root: ComponentId,
    world_panel_pos: (f32, f32, f32),
    model: &WorldPanelModel,
    inspector_models: &[InspectorPanelModel],
    rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
    working_file_path: &Path,
    asset_system: &crate::engine::ecs::system::AssetSystem,
    data_renderer: &mut DataRendererSystem,
) {
    let existing_world_panel = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR);
    let existing_panel_mount = world.all_components().find(|&component_id| {
        world
            .component_label(component_id)
            .is_some_and(|label| label == PANEL_LAYOUT_MOUNT_NAME)
    });

    if *panel_layout_spawned {
        if existing_world_panel.is_none() && existing_panel_mount.is_none() {
            *panel_layout_spawned = false;
        } else {
            return;
        }
    }

    if existing_world_panel.is_some() || existing_panel_mount.is_some() {
        *panel_layout_spawned = true;
        return;
    }

    *panel_layout_spawned = true;

    let Some((panel_mount_root, layout_root_id)) =
        spawn_editor_panel_layout_tree(world, emit, model, working_file_path, world_panel_pos)
    else {
        return;
    };

    let selection = ensure_panel_layout_selection(world, layout_root_id);
    world.init_component_tree(selection, emit);

    if let Some(inspector_panel_selection) =
        world.find_component(panel_mount_root, "#inspector_panel_selection")
        && let Some(selection) =
            world.get_component_by_id_as_mut::<SelectionComponent>(inspector_panel_selection)
    {
        selection.mode = SelectionMode::Single;
        selection.clear();
    }

    populate_asset_panel(world, render_assets, emit, panel_mount_root, asset_system);

    if let Some(panel_layout_selection) = panel_layout_selection_id(world, panel_mount_root)
        && let Some(world_panel_root) =
            world.find_component(panel_mount_root, WORLD_PANEL_ROOT_SELECTOR)
    {
        emit.push_intent_now(
            panel_layout_selection,
            IntentValue::SelectionSet {
                component_ids: vec![panel_layout_selection],
                entries: vec![SelectionEntry {
                    index: Some(0),
                    component: world_panel_root,
                }],
                primary: Some(world_panel_root),
            },
        );
    }

    attach_panel_mount(emit, panel_query_root, panel_mount_root);

    if let Some(world_panel_root) =
        world.find_component(panel_mount_root, WORLD_PANEL_ROOT_SELECTOR)
        && let Some(content_slot) =
            world.find_component(world_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
        && let Some(selection_root) =
            world.find_component(world_panel_root, WORLD_PANEL_SELECTION_SELECTOR)
    {
        rerender_world_panel_content(
            world,
            emit,
            content_slot,
            selection_root,
            &model.rows,
            model.selected_index,
            data_renderer,
        );
    }

    // Inspector instances are projected by the post-bootstrap refresh after the mount is cached.
    let _ = inspector_models;
    let _ = rendered_inspector_models;

    let grid_context = EditorContextState {
        active_editor: Some(editor_root),
        ..EditorContextState::default()
    };
    rerender_grid_panel_from_context(world, emit, panel_mount_root, &grid_context, data_renderer);
    sync_editor_settings_panel_selection(world, emit, panel_mount_root, &grid_context);

    // Initial projections enqueue additional intents under the new subtree. Reassert the mount
    // attachment last so command coalescing cannot leave the completed layout detached.
    attach_panel_mount(emit, panel_query_root, panel_mount_root);
}

fn populate_asset_panel(
    world: &mut World,
    render_assets: &mut RenderAssets,
    emit: &mut dyn SignalEmitter,
    panel_mount_root: ComponentId,
    asset_system: &crate::engine::ecs::system::AssetSystem,
) {
    let Some(asset_panel_root) = world.find_component(panel_mount_root, "#assets_root") else {
        return;
    };
    let Some(selection_root) = world.find_component(asset_panel_root, "#assets_content_area")
    else {
        return;
    };
    if world.children_of(selection_root).len() > 2 {
        return;
    }

    let mut last_module_id = None;
    for (index, item) in asset_system.items.iter().enumerate() {
        if last_module_id != Some(item.module_id) {
            last_module_id = Some(item.module_id);
            if let Some(module_name) = asset_system.get_module_name(item.module_id) {
                match asset_system.build_asset_module_header(world, emit, &module_name) {
                    Ok(header_root) => {
                        world.init_component_tree(header_root, emit);
                        emit.push_intent_now(
                            header_root,
                            IntentValue::Attach {
                                parents: vec![selection_root],
                                child: header_root,
                            },
                        );
                    }
                    Err(error) => eprintln!(
                        "[InspectorSystem][error] failed to build asset header for {module_name}: {error}"
                    ),
                }
            }
        }

        match asset_system.build_asset_item_shell(world, render_assets, emit, item, index) {
            Ok(item_root) => {
                world.init_component_tree(item_root, emit);
                emit.push_intent_now(
                    item_root,
                    IntentValue::Attach {
                        parents: vec![selection_root],
                        child: item_root,
                    },
                );
            }
            Err(error) => eprintln!(
                "[InspectorSystem][error] failed to build asset item {}: {error}",
                item.export_name
            ),
        }
    }
    mark_nearest_layout_dirty(world, selection_root);
}

fn attach_panel_mount(
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    panel_mount_root: ComponentId,
) {
    emit.push_intent_now(
        panel_mount_root,
        IntentValue::Attach {
            parents: vec![panel_query_root],
            child: panel_mount_root,
        },
    );
}
