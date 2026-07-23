use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::EditorInteractionMode;
use crate::engine::ecs::component::{
    DataComponent, EditorComponent, GLTFComponent, SelectionEntry,
};
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::world_panel::effective_editor_roots;
use crate::engine::ecs::system::panel_system::{data_text, is_descendant_or_self};
use crate::engine::ecs::system::selection_system::apply_selection_set;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

pub(crate) const EDITOR_SETTINGS_PANEL_ROOT_SELECTOR: &str = "#editor_settings_panel_root";
pub(crate) const EDITOR_SETTINGS_SELECTION_NAME: &str = "editor_settings_selection";
pub(crate) const EDITOR_SETTINGS_SELECTION_SELECTOR: &str = "#editor_settings_selection";
pub(crate) const EDITOR_SETTINGS_PAYLOAD_NAME: &str = "editor_settings_payload";
pub(crate) const EDITOR_SETTINGS_SELECT_ROW_NAME: &str = "editor_settings_mode_select";
pub(crate) const EDITOR_SETTINGS_CURSOR_ROW_NAME: &str = "editor_settings_mode_cursor_3d";
pub(crate) const EDITOR_SETTINGS_SELECT_CURSOR_ROW_NAME: &str =
    "editor_settings_mode_select_cursor";
pub(crate) const EDITOR_SETTINGS_ARMATURE_ROW_NAME: &str = "editor_settings_armature_visibility";
pub(crate) const EDITOR_SETTINGS_ARMATURE_TOGGLE_SLOT_NAME: &str = "armature_toggle_slot";
pub(crate) const EDITOR_SETTINGS_BOUNDS_ROW_NAME: &str = "editor_settings_bounds_visibility";
pub(crate) const EDITOR_SETTINGS_BOUNDS_TOGGLE_SLOT_NAME: &str = "bounds_toggle_slot";
pub(crate) const EDITOR_SETTINGS_CAMERAS_ROW_NAME: &str = "editor_settings_cameras_visibility";
pub(crate) const EDITOR_SETTINGS_CAMERAS_TOGGLE_SLOT_NAME: &str = "cameras_toggle_slot";
pub(crate) const EDITOR_SETTINGS_COLLIDERS_ROW_NAME: &str = "editor_settings_colliders_visibility";
pub(crate) const EDITOR_SETTINGS_COLLIDERS_TOGGLE_SLOT_NAME: &str = "colliders_toggle_slot";
pub(crate) const EDITOR_SETTINGS_GLTF_COLLIDERS_ROW_NAME: &str =
    "editor_settings_gltf_colliders_visibility";
pub(crate) const EDITOR_SETTINGS_GLTF_COLLIDERS_TOGGLE_SLOT_NAME: &str =
    "gltf_colliders_toggle_slot";
pub(crate) const EDITOR_SETTINGS_SPRING_BONES_ROW_NAME: &str =
    "editor_settings_spring_bones_visibility";
pub(crate) const EDITOR_SETTINGS_SPRING_BONES_TOGGLE_SLOT_NAME: &str = "spring_bones_toggle_slot";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorSettingsOption {
    Select,
    Cursor3d,
    SelectAndCursor,
}

impl EditorSettingsOption {
    pub(crate) fn interaction_mode(self) -> EditorInteractionMode {
        match self {
            Self::Select => EditorInteractionMode::Select,
            Self::Cursor3d => EditorInteractionMode::Cursor3d,
            Self::SelectAndCursor => EditorInteractionMode::SelectAndCursor,
        }
    }

    pub(crate) fn row_name(self) -> &'static str {
        match self {
            Self::Select => EDITOR_SETTINGS_SELECT_ROW_NAME,
            Self::Cursor3d => EDITOR_SETTINGS_CURSOR_ROW_NAME,
            Self::SelectAndCursor => EDITOR_SETTINGS_SELECT_CURSOR_ROW_NAME,
        }
    }

    pub(crate) fn from_mode_value(mode_value: &str) -> Option<Self> {
        match mode_value {
            "select" => Some(Self::Select),
            "cursor_3d" => Some(Self::Cursor3d),
            "select_cursor" => Some(Self::SelectAndCursor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorSettingsPanelState {
    pub(crate) active_editor: Option<ComponentId>,
    pub(crate) interaction_mode: EditorInteractionMode,
    pub(crate) armature_visible: bool,
    pub(crate) bounds_visible: bool,
}

impl Default for EditorSettingsPanelState {
    fn default() -> Self {
        Self {
            active_editor: None,
            interaction_mode: EditorInteractionMode::Select,
            armature_visible: false,
            bounds_visible: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditorSettingsPanelEvent {
    ActiveEditorChanged {
        editor: Option<ComponentId>,
        interaction_mode: EditorInteractionMode,
    },
    InteractionModeChanged {
        editor: Option<ComponentId>,
        interaction_mode: EditorInteractionMode,
    },
    ArmatureVisibilityChanged {
        visible: bool,
    },
    BoundsVisibilityChanged {
        visible: bool,
    },
}

pub(crate) fn reduce_editor_settings_panel_state(
    old: &EditorSettingsPanelState,
    event: &EditorSettingsPanelEvent,
) -> EditorSettingsPanelState {
    let mut new = old.clone();
    match event {
        EditorSettingsPanelEvent::ActiveEditorChanged {
            editor,
            interaction_mode,
        }
        | EditorSettingsPanelEvent::InteractionModeChanged {
            editor,
            interaction_mode,
        } => {
            new.active_editor = *editor;
            new.interaction_mode = *interaction_mode;
        }
        EditorSettingsPanelEvent::ArmatureVisibilityChanged { visible } => {
            new.armature_visible = *visible;
        }
        EditorSettingsPanelEvent::BoundsVisibilityChanged { visible } => {
            new.bounds_visible = *visible;
        }
    }
    new
}

fn find_gltf_components_under(world: &World, root: ComponentId) -> Vec<ComponentId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(component_id) = stack.pop() {
        if world
            .get_component_by_id_as::<GLTFComponent>(component_id)
            .is_some()
        {
            out.push(component_id);
        }
        for &child in world.children_of(component_id) {
            stack.push(child);
        }
    }
    out
}

pub(crate) fn sync_editor_settings_armature_toggle(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context: &EditorContextState,
) {
    use crate::engine::ecs::component::{Display, SizeDimension, StyleComponent};

    let Some(settings_panel_root) =
        world.find_component(panel_query_root, EDITOR_SETTINGS_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(armature_row_root) = world.find_component(
        settings_panel_root,
        &format!("#{EDITOR_SETTINGS_ARMATURE_ROW_NAME}"),
    ) else {
        return;
    };
    let Some(toggle_slot) = world.find_component(
        armature_row_root,
        &format!("#{EDITOR_SETTINGS_ARMATURE_TOGGLE_SLOT_NAME}"),
    ) else {
        return;
    };
    if let Some(toggle) = world
        .children_of(armature_row_root)
        .iter()
        .copied()
        .find(|child| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::ToggleComponent>(*child)
                .is_some()
        })
    {
        emit.push_intent_now(
            toggle,
            IntentValue::ToggleSet {
                component_ids: vec![toggle],
                value: editor_context.armature_visible,
            },
        );
    }

    let existing_children = world.children_of(toggle_slot).to_vec();
    for child in existing_children {
        if world.get_component_record(child).is_some() {
            emit.push_intent_now(
                child,
                IntentValue::RemoveSubtree {
                    component_ids: vec![child],
                },
            );
        }
    }

    let on = editor_context.armature_visible;
    let bg = if on {
        [0.95, 0.73, 0.16, 1.0]
    } else {
        [0.12, 0.36, 0.72, 1.0]
    };

    let root = world.add_component_boxed_named(
        "armature_toggle",
        Box::new(crate::engine::ecs::component::TransformComponent::new()),
    );
    let style = world.add_component_boxed_named(
        "armature_toggle_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::InlineBlock);
            style.width = SizeDimension::GlyphUnits(3.5);
            style.height = SizeDimension::GlyphUnits(2.0);
            style.text_align = crate::engine::ecs::component::TextAlign::Center;
            style.vertical_align = crate::engine::ecs::component::style::VerticalAlign::Middle;
            style.background_color = Some(bg);
            style.color = Some([0.96, 0.98, 0.96, 1.0]);
            style
        }),
    );
    let _ = world.add_child(root, style);
    let _ = world.add_child(toggle_slot, root);
}

pub(crate) fn sync_editor_settings_bounds_toggle(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context: &EditorContextState,
) {
    sync_boolean_toggle(
        world,
        emit,
        panel_query_root,
        EDITOR_SETTINGS_BOUNDS_ROW_NAME,
        EDITOR_SETTINGS_BOUNDS_TOGGLE_SLOT_NAME,
        "bounds_toggle",
        editor_context.bounds_visible,
    );
}

fn sync_boolean_toggle(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    row_name: &str,
    slot_name: &str,
    toggle_name: &str,
    on: bool,
) {
    use crate::engine::ecs::component::{Display, SizeDimension, StyleComponent};
    let Some(panel) = world.find_component(panel_query_root, EDITOR_SETTINGS_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(row) = world.find_component(panel, &format!("#{row_name}")) else {
        return;
    };
    let Some(slot) = world.find_component(row, &format!("#{slot_name}")) else {
        return;
    };
    if let Some(toggle) = world.children_of(row).iter().copied().find(|child| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::ToggleComponent>(*child)
            .is_some()
    }) {
        emit.push_intent_now(
            toggle,
            IntentValue::ToggleSet {
                component_ids: vec![toggle],
                value: on,
            },
        );
    }
    for child in world.children_of(slot).to_vec() {
        emit.push_intent_now(
            child,
            IntentValue::RemoveSubtree {
                component_ids: vec![child],
            },
        );
    }
    let root = world.add_component_boxed_named(
        toggle_name,
        Box::new(crate::engine::ecs::component::TransformComponent::new()),
    );
    let style = world.add_component({
        let mut style = StyleComponent::default();
        style.display = Some(Display::InlineBlock);
        style.width = SizeDimension::GlyphUnits(3.5);
        style.height = SizeDimension::GlyphUnits(2.0);
        style.text_align = crate::engine::ecs::component::TextAlign::Center;
        style.vertical_align = crate::engine::ecs::component::style::VerticalAlign::Middle;
        style.background_color = Some(if on {
            [0.95, 0.73, 0.16, 1.0]
        } else {
            [0.12, 0.36, 0.72, 1.0]
        });
        style.color = Some([0.96, 0.98, 0.96, 1.0]);
        style
    });
    let _ = world.add_child(root, style);
    let _ = world.add_child(slot, root);
}

pub(crate) fn sync_editor_settings_panel_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context: &EditorContextState,
) {
    let Some(selection_root) =
        world.find_component(panel_query_root, EDITOR_SETTINGS_SELECTION_SELECTOR)
    else {
        return;
    };

    let desired_option = match editor_context.interaction_mode {
        EditorInteractionMode::Select => EditorSettingsOption::Select,
        EditorInteractionMode::Cursor3d => EditorSettingsOption::Cursor3d,
        EditorInteractionMode::SelectAndCursor => EditorSettingsOption::SelectAndCursor,
    };
    let Some(panel_root) =
        world.find_component(panel_query_root, EDITOR_SETTINGS_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(row_root) =
        world.find_component(panel_root, &format!("#{}", desired_option.row_name()))
    else {
        return;
    };
    apply_selection_set(
        world,
        emit,
        selection_root,
        vec![SelectionEntry {
            index: Some(match desired_option {
                EditorSettingsOption::Select => 0,
                EditorSettingsOption::Cursor3d => 1,
                EditorSettingsOption::SelectAndCursor => 2,
            }),
            component: row_root,
        }],
        Some(row_root),
    );

    sync_editor_settings_armature_toggle(world, emit, panel_query_root, editor_context);
    sync_editor_settings_bounds_toggle(world, emit, panel_query_root, editor_context);
    sync_boolean_toggle(
        world,
        emit,
        panel_query_root,
        EDITOR_SETTINGS_CAMERAS_ROW_NAME,
        EDITOR_SETTINGS_CAMERAS_TOGGLE_SLOT_NAME,
        "cameras_toggle",
        editor_context.cameras_visible,
    );
    sync_boolean_toggle(
        world,
        emit,
        panel_query_root,
        EDITOR_SETTINGS_COLLIDERS_ROW_NAME,
        EDITOR_SETTINGS_COLLIDERS_TOGGLE_SLOT_NAME,
        "colliders_toggle",
        editor_context.collider_visibility
            == Some(crate::engine::ecs::system::CollisionVisualizationMode::All),
    );
    sync_boolean_toggle(
        world,
        emit,
        panel_query_root,
        EDITOR_SETTINGS_GLTF_COLLIDERS_ROW_NAME,
        EDITOR_SETTINGS_GLTF_COLLIDERS_TOGGLE_SLOT_NAME,
        "gltf_colliders_toggle",
        editor_context.collider_visibility
            == Some(crate::engine::ecs::system::CollisionVisualizationMode::GltfOwned),
    );
    sync_boolean_toggle(
        world,
        emit,
        panel_query_root,
        EDITOR_SETTINGS_SPRING_BONES_ROW_NAME,
        EDITOR_SETTINGS_SPRING_BONES_TOGGLE_SLOT_NAME,
        "spring_bones_toggle",
        editor_context.spring_bones_visible,
    );
}

fn owning_editor_ui(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(id) = current {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::EditorUIComponent>(id)
            .is_some()
        {
            return Some(id);
        }
        current = world.parent_of(id);
    }
    world.all_components().find(|id| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::EditorUIComponent>(*id)
            .is_some()
    })
}

pub(crate) fn handle_editor_settings_panel_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    renderable: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    installed_editor_roots: &Arc<Mutex<Vec<ComponentId>>>,
) -> bool {
    let Some(settings_panel_root) =
        world.find_component(panel_query_root, EDITOR_SETTINGS_PANEL_ROOT_SELECTOR)
    else {
        return false;
    };
    if !is_descendant_or_self(world, settings_panel_root, renderable) {
        return false;
    }

    let mut current = Some(renderable);
    while let Some(component_id) = current {
        let Some(payload_id) = world
            .children_of(component_id)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some(EDITOR_SETTINGS_PAYLOAD_NAME))
        else {
            current = world.parent_of(component_id);
            continue;
        };

        let Some(payload) = world.get_component_by_id_as::<DataComponent>(payload_id) else {
            return true;
        };
        let row_kind = data_text(payload, "row_kind").unwrap_or_default();
        if row_kind == "EditorMode" {
            let Some(option) = data_text(payload, "mode_value")
                .as_deref()
                .and_then(EditorSettingsOption::from_mode_value)
            else {
                return true;
            };
            let editor_roots = effective_editor_roots(world, installed_editor_roots);
            let active_editor = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned")
                .active_editor
                .or_else(|| editor_roots.first().copied());
            if let Some(editor_root) = active_editor
                && let Some(editor) =
                    world.get_component_by_id_as_mut::<EditorComponent>(editor_root)
            {
                editor.interaction_mode = option.interaction_mode();
            }
            {
                let mut context = editor_context_state
                    .lock()
                    .expect("editor context state mutex poisoned");
                context.active_editor = active_editor;
                context.interaction_mode = option.interaction_mode();
            }
            if let Some(selection_root) =
                world.find_component(settings_panel_root, EDITOR_SETTINGS_SELECTION_SELECTOR)
            {
                apply_selection_set(
                    world,
                    emit,
                    selection_root,
                    vec![SelectionEntry {
                        index: Some(match option {
                            EditorSettingsOption::Select => 0,
                            EditorSettingsOption::Cursor3d => 1,
                            EditorSettingsOption::SelectAndCursor => 2,
                        }),
                        component: component_id,
                    }],
                    Some(component_id),
                );
            }
            return true;
        }
        if row_kind == "GLTFBoundsVisibility" {
            let visible = !editor_context_state
                .lock()
                .expect("editor context state mutex poisoned")
                .bounds_visible;
            editor_context_state
                .lock()
                .expect("editor context state mutex poisoned")
                .bounds_visible = visible;
            for editor_root in effective_editor_roots(world, installed_editor_roots) {
                for gltf_component in find_gltf_components_under(world, editor_root) {
                    if let Some(gltf) =
                        world.get_component_by_id_as_mut::<GLTFComponent>(gltf_component)
                    {
                        gltf.bounds_visible = visible;
                    }
                }
            }
            let editor_context = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned")
                .clone();
            sync_editor_settings_panel_selection(world, emit, panel_query_root, &editor_context);
            return true;
        }
        if row_kind == "AllCollidersVisibility" || row_kind == "GltfCollidersVisibility" {
            use crate::engine::ecs::system::CollisionVisualizationMode;
            let clicked = if row_kind == "AllCollidersVisibility" {
                CollisionVisualizationMode::All
            } else {
                CollisionVisualizationMode::GltfOwned
            };
            let mode = {
                let mut context = editor_context_state
                    .lock()
                    .expect("editor context state mutex poisoned");
                context.collider_visibility = if context.collider_visibility == Some(clicked) {
                    None
                } else {
                    Some(clicked)
                };
                context.collider_visibility
            };
            if let Some(owner) = owning_editor_ui(world, settings_panel_root) {
                emit.push_intent_now(
                    owner,
                    IntentValue::CollisionVisualizationSet {
                        component_ids: vec![owner],
                        scope_roots: effective_editor_roots(world, installed_editor_roots),
                        mode,
                    },
                );
            }
            let context = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned")
                .clone();
            sync_editor_settings_panel_selection(world, emit, panel_query_root, &context);
            return true;
        }
        if row_kind == "CameraVisibility" {
            let visible = {
                let mut context = editor_context_state
                    .lock()
                    .expect("editor context state mutex poisoned");
                context.cameras_visible = !context.cameras_visible;
                context.cameras_visible
            };
            if let Some(owner) = owning_editor_ui(world, settings_panel_root) {
                emit.push_intent_now(
                    owner,
                    IntentValue::CameraVisualizationSet {
                        component_ids: vec![owner],
                        scope_roots: effective_editor_roots(world, installed_editor_roots),
                        visible,
                    },
                );
            }
            let context = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned")
                .clone();
            sync_editor_settings_panel_selection(world, emit, panel_query_root, &context);
            return true;
        }
        if row_kind == "SpringBonesVisibility" {
            let visible = {
                let mut context = editor_context_state
                    .lock()
                    .expect("editor context state mutex poisoned");
                context.spring_bones_visible = !context.spring_bones_visible;
                context.spring_bones_visible
            };
            if let Some(owner) = owning_editor_ui(world, settings_panel_root) {
                emit.push_intent_now(
                    owner,
                    IntentValue::SpringBoneVisualizationSet {
                        component_ids: vec![owner],
                        scope_roots: effective_editor_roots(world, installed_editor_roots),
                        visible,
                    },
                );
            }
            let context = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned")
                .clone();
            sync_editor_settings_panel_selection(world, emit, panel_query_root, &context);
            return true;
        }
        if row_kind != "GLTFArmatureVisibility" {
            return false;
        }

        let visible = !editor_context_state
            .lock()
            .expect("editor context state mutex poisoned")
            .armature_visible;

        {
            let mut editor_context = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned");
            editor_context.armature_visible = visible;
        }

        let editor_roots = effective_editor_roots(world, installed_editor_roots);
        for editor_root in editor_roots {
            let gltf_components = find_gltf_components_under(world, editor_root);
            for gltf_component in gltf_components {
                emit.push_intent_now(
                    gltf_component,
                    IntentValue::GLTFArmatureVisible {
                        component_ids: vec![gltf_component],
                        visible,
                    },
                );
            }
        }

        let editor_context = editor_context_state
            .lock()
            .expect("editor context state mutex poisoned")
            .clone();
        sync_editor_settings_panel_selection(world, emit, panel_query_root, &editor_context);
        return true;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{DataValue, EditorComponent};
    use crate::engine::ecs::system::{CollisionVisualizationMode, SystemWorld};
    use crate::engine::graphics::{RenderAssets, VisualWorld};

    #[test]
    fn armature_settings_click_toggles_state_renders_toggle_and_fans_out_to_all_editors() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();

        let panel_query_root = world.add_component_boxed_named(
            "panel_root",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let settings_panel_root = world.add_component_boxed_named(
            "editor_settings_panel_root",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let armature_row = world.add_component_boxed_named(
            EDITOR_SETTINGS_ARMATURE_ROW_NAME,
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let toggle_slot = world.add_component_boxed_named(
            EDITOR_SETTINGS_ARMATURE_TOGGLE_SLOT_NAME,
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let payload = world.add_component_boxed_named(
            EDITOR_SETTINGS_PAYLOAD_NAME,
            Box::new(
                DataComponent::new()
                    .with_entry("row_kind", DataValue::Text("GLTFArmatureVisibility".into()))
                    .with_entry("visible", DataValue::Bool(false)),
            ),
        );
        let _ = world.add_child(panel_query_root, settings_panel_root);
        let _ = world.add_child(settings_panel_root, armature_row);
        let _ = world.add_child(armature_row, toggle_slot);
        let _ = world.add_child(armature_row, payload);

        let editor_a =
            world.add_component_boxed_named("editor_a", Box::new(EditorComponent::new()));
        let editor_b =
            world.add_component_boxed_named("editor_b", Box::new(EditorComponent::new()));
        let gltf_a = world.add_component(GLTFComponent::new("a.glb"));
        let gltf_b = world.add_component(GLTFComponent::new("b.glb"));
        let _ = world.add_child(editor_a, gltf_a);
        let _ = world.add_child(editor_b, gltf_b);

        let editor_context_state = Arc::new(Mutex::new(EditorContextState::default()));
        let installed_editor_roots = Arc::new(Mutex::new(vec![editor_a, editor_b]));

        assert!(handle_editor_settings_panel_click(
            &mut world,
            &mut emit,
            panel_query_root,
            armature_row,
            &editor_context_state,
            &installed_editor_roots,
        ));

        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);
        let editor_context = editor_context_state
            .lock()
            .expect("editor context state mutex poisoned")
            .clone();
        assert!(editor_context.armature_visible);
        assert!(
            world
                .get_component_by_id_as::<GLTFComponent>(gltf_a)
                .expect("gltf_a")
                .armature_visible
        );
        assert!(
            world
                .get_component_by_id_as::<GLTFComponent>(gltf_b)
                .expect("gltf_b")
                .armature_visible
        );

        sync_editor_settings_armature_toggle(
            &mut world,
            &mut emit,
            panel_query_root,
            &editor_context,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);
        assert!(
            !world.children_of(toggle_slot).is_empty(),
            "expected toggle button to be rendered when armature is visible"
        );

        assert!(handle_editor_settings_panel_click(
            &mut world,
            &mut emit,
            panel_query_root,
            armature_row,
            &editor_context_state,
            &installed_editor_roots,
        ));
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);
        let editor_context = editor_context_state
            .lock()
            .expect("editor context state mutex poisoned")
            .clone();
        assert!(!editor_context.armature_visible);
        sync_editor_settings_armature_toggle(
            &mut world,
            &mut emit,
            panel_query_root,
            &editor_context,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);
        assert!(
            !world.children_of(toggle_slot).is_empty(),
            "expected toggle button to still be rendered when armature is hidden"
        );
    }

    #[test]
    fn collider_settings_are_exclusive_and_clicking_active_mode_turns_them_off() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let editor_ui = world.add_component(
            crate::engine::ecs::component::EditorUIComponent::new()
                .with_panels([crate::engine::ecs::component::EditorPanel::Settings]),
        );
        let settings_panel = world.add_component_boxed_named(
            "editor_settings_panel_root",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        world.add_child(editor_ui, settings_panel).unwrap();

        let make_row = |world: &mut World, name: &str, kind: &str| {
            let row = world.add_component_boxed_named(
                name,
                Box::new(crate::engine::ecs::component::TransformComponent::new()),
            );
            let payload = world.add_component_boxed_named(
                EDITOR_SETTINGS_PAYLOAD_NAME,
                Box::new(
                    DataComponent::new().with_entry("row_kind", DataValue::Text(kind.to_string())),
                ),
            );
            world.add_child(row, payload).unwrap();
            world.add_child(settings_panel, row).unwrap();
            row
        };
        let all_row = make_row(
            &mut world,
            EDITOR_SETTINGS_COLLIDERS_ROW_NAME,
            "AllCollidersVisibility",
        );
        let gltf_row = make_row(
            &mut world,
            EDITOR_SETTINGS_GLTF_COLLIDERS_ROW_NAME,
            "GltfCollidersVisibility",
        );
        let editor_root = world.add_component(EditorComponent::new());
        let context = Arc::new(Mutex::new(EditorContextState::default()));
        let roots = Arc::new(Mutex::new(vec![editor_root]));

        for (clicked, expected) in [
            (all_row, Some(CollisionVisualizationMode::All)),
            (gltf_row, Some(CollisionVisualizationMode::GltfOwned)),
            (gltf_row, None),
        ] {
            assert!(handle_editor_settings_panel_click(
                &mut world, &mut emit, editor_ui, clicked, &context, &roots,
            ));
            systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);
            assert_eq!(context.lock().unwrap().collider_visibility, expected);
            assert_eq!(
                systems
                    .collision_visualization
                    .requests()
                    .get(&editor_ui)
                    .map(|request| request.mode),
                expected
            );
        }
    }

    #[test]
    fn spring_bone_toggle_is_independent_from_collision_visualization() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();
        let editor_ui =
            world.add_component(crate::engine::ecs::component::EditorUIComponent::new());
        let panel = world.add_component_boxed_named(
            "editor_settings_panel_root",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let row = world.add_component_boxed_named(
            EDITOR_SETTINGS_SPRING_BONES_ROW_NAME,
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let payload = world.add_component_boxed_named(
            EDITOR_SETTINGS_PAYLOAD_NAME,
            Box::new(
                DataComponent::new()
                    .with_entry("row_kind", DataValue::Text("SpringBonesVisibility".into())),
            ),
        );
        world.add_child(editor_ui, panel).unwrap();
        world.add_child(panel, row).unwrap();
        world.add_child(row, payload).unwrap();
        let editor_root = world.add_component(EditorComponent::new());
        let context = Arc::new(Mutex::new(EditorContextState::default()));
        let roots = Arc::new(Mutex::new(vec![editor_root]));

        assert!(handle_editor_settings_panel_click(
            &mut world,
            &mut emit,
            editor_ui,
            row,
            &context,
            &roots,
        ));
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);
        assert!(context.lock().unwrap().spring_bones_visible);
        assert!(systems
            .spring_bone_visualization
            .requests()
            .contains_key(&editor_ui));
        assert!(systems.collision_visualization.requests().is_empty());
    }
}
