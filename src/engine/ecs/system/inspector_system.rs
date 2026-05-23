use crate::engine::ecs::IntentValue;
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::meow_meow::component_registry::spawn_tree_uninitialized;
use crate::meow_meow::evaluator::eval_mms_fn;
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
use crate::meow_meow::runner::MeowMeowRunner;

const WORLD_PANEL_MOUNT_NAME: &str = "editor_world_panel_mount";

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorldPanelModel {
    title: String,
    items: Vec<String>,
}

#[derive(Debug, Default)]
pub struct InspectorSystem;

impl InspectorSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn setup_panels_for_editor(
        &mut self,
        _rx: &mut RxWorld,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        _inspector_panel_pos: (f32, f32, f32),
    ) {
        if world.find_component(editor_root, "#world_panel_root").is_some() {
            return;
        }

        let model = self.build_world_panel_model(world, editor_root);
        self.spawn_world_panel(world, emit, editor_root, world_panel_pos, &model);
    }

    fn build_world_panel_model(&self, world: &World, editor_root: ComponentId) -> WorldPanelModel {
        let mut items: Vec<String> = world
            .all_components()
            .filter(|&component_id| {
                world.parent_of(component_id).is_none() && component_id != editor_root
            })
            .map(|component_id| world_panel_item_label(world, component_id))
            .collect();

        if items.is_empty() {
            items.push("<empty>".to_string());
        }

        WorldPanelModel {
            title: "World".to_string(),
            items,
        }
    }

    fn spawn_world_panel(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
    ) {
        let module = match MeowMeowRunner::load_module_file(world_panel_asset_path()) {
            Ok(module) => module,
            Err(error) => {
                eprintln!("[InspectorSystem] world panel module load error: {error}");
                return;
            }
        };

        let Some(world_panel_fn) = module.named_export("world_panel") else {
            eprintln!("[InspectorSystem] world panel export 'world_panel' was not found");
            return;
        };

        let args = vec![
            Value::String(model.title.clone()),
            Value::Array(model.items.iter().cloned().map(Value::String).collect()),
        ];
        let panel_value = match eval_mms_fn(world_panel_fn, args, None, Some(world), Some(emit)) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[InspectorSystem] world panel export call error: {error}");
                return;
            }
        };

        let Value::ComponentExpr(panel_root) = panel_value else {
            eprintln!("[InspectorSystem] world panel export did not return a component tree");
            return;
        };

        let mount_ce = MaterializedCE {
            component_type: "T".to_string(),
            ctor_method: Some("position".to_string()),
            ctor_args: vec![
                Value::Number(world_panel_pos.0 as f64),
                Value::Number(world_panel_pos.1 as f64),
                Value::Number(world_panel_pos.2 as f64),
            ],
            calls: Vec::new(),
            named: vec![(
                "name".to_string(),
                Value::String(WORLD_PANEL_MOUNT_NAME.to_string()),
            )],
            positionals: Vec::new(),
            children: vec![CeChild::Spawn(*panel_root)],
        };

        let panel_mount_root = match spawn_tree_uninitialized(&mount_ce, world, emit) {
            Ok(component_id) => component_id,
            Err(error) => {
                eprintln!("[InspectorSystem] world panel spawn error: {error}");
                return;
            }
        };

        emit.push_intent_now(
            panel_mount_root,
            IntentValue::Attach {
                parents: vec![editor_root],
                child: panel_mount_root,
            },
        );
    }
}

fn world_panel_item_label(world: &World, component_id: ComponentId) -> String {
    if let Some(label) = world.component_label(component_id) {
        if !label.is_empty() {
            return label.to_string();
        }
    }

    world
        .component_name(component_id)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("component_{:?}", component_id))
}

fn world_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/world_panel.mms")
}

#[cfg(test)]
mod tests {
    use super::InspectorSystem;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{EditorComponent, TransformComponent};
    use crate::engine::ecs::{SystemWorld, World};
    use crate::engine::graphics::VisualWorld;

    #[test]
    fn setup_panels_for_editor_spawns_world_panel_under_editor_root() {
        let mut world = World::default();
        let mut rx = crate::engine::ecs::RxWorld::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let mut inspector = InspectorSystem::new();

        let editor_root = world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let camera_root = world.add_component_boxed_named("camera_root", Box::new(TransformComponent::new()));

        assert!(world.parent_of(scene_root).is_none());
        assert!(world.parent_of(camera_root).is_none());

        inspector.setup_panels_for_editor(
            &mut rx,
            &mut world,
            &mut emit,
            editor_root,
            (-0.7, 1.6, -1.2),
            (-0.7, 1.6, -1.2),
        );

        systems.process_commands(&mut world, &mut visuals, &mut emit);

        let panel_mount = world
            .find_component(editor_root, "#editor_world_panel_mount")
            .expect("expected world panel mount under editor root");
        let panel_root = world
            .find_component(editor_root, "#world_panel_root")
            .expect("expected world panel root under editor root");
        assert_eq!(world.parent_of(panel_mount), Some(editor_root));
        assert_eq!(world.parent_of(panel_root), Some(panel_mount));
        assert!(world.find_component(editor_root, "#panel_status_value").is_some());
        assert!(world.find_component(editor_root, "#rows_mount").is_some());
        assert!(world.find_component(editor_root, "#item_0").is_some());
    }
}
