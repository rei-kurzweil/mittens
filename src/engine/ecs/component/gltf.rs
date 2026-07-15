use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;

/// Load and spawn content from a glTF asset.
///
/// Attach this component somewhere under a `TransformComponent` to use that transform as an anchor.
#[derive(Debug, Clone)]
pub struct GLTFComponent {
    /// Path/URI to a `.gltf` or `.glb` asset (currently treated as local filesystem path).
    pub uri: String,

    /// If true, GLTFSystem will give transform-only nodes a small debug renderable.
    ///
    /// This is useful for editor-style workflows where you want to see and grab node transforms
    /// even when the node has no mesh.
    pub with_visualized_transforms: bool,

    /// Runtime-only: used by GLTFSystem to avoid re-spawning the same asset repeatedly.
    pub spawned: bool,

    /// Runtime-only: whether armature visualization should be shown for this instance.
    pub armature_visible: bool,

    /// Runtime-only: whether imported renderable bounds should be visualized.
    pub bounds_visible: bool,

    /// Runtime-only: spawned transform ids for imported glTF nodes in this instance.
    pub spawned_node_transforms: Vec<ComponentId>,

    /// Runtime-only: subset of `spawned_node_transforms` that correspond to skin joints.
    pub armature_joint_transforms: Vec<ComponentId>,

    component: Option<ComponentId>,
}

impl GLTFComponent {
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            with_visualized_transforms: false,
            spawned: false,
            armature_visible: false,
            bounds_visible: false,
            spawned_node_transforms: Vec::new(),
            armature_joint_transforms: Vec::new(),
            component: None,
        }
    }

    pub fn with_visualized_transforms(mut self, with_visualized_transforms: bool) -> Self {
        self.with_visualized_transforms = with_visualized_transforms;
        self
    }
}

impl Component for GLTFComponent {
    fn name(&self) -> &'static str {
        "gltf"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
        let _ = self.component;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterGLTF {
                component_ids: vec![component],
            },
        );
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = ce_call("GLTF", "new", vec![s(&self.uri)]);
        if self.with_visualized_transforms {
            ce = ce.with_call("with_visualized_transforms", vec![b(true)]);
        }
        ce
    }
}
