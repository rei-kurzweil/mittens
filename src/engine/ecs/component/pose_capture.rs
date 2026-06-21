use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoseCaptureTargetMode {
    WholeSubtree,
    SkinnedJointsOnly,
    NamedRoot { selector_or_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseCaptureComponent {
    pub label: Option<String>,
    pub target_mode: PoseCaptureTargetMode,
    pub include_scale: bool,
    pub store_rest_deltas: bool,
    #[serde(skip)]
    component: Option<ComponentId>,
}

impl PoseCaptureComponent {
    pub fn new() -> Self {
        Self {
            label: None,
            target_mode: PoseCaptureTargetMode::WholeSubtree,
            include_scale: true,
            store_rest_deltas: false,
            component: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Component for PoseCaptureComponent {
    fn name(&self) -> &'static str {
        "pose_capture"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = ce_call("PoseCapture", "new", vec![]);
        if let Some(label) = &self.label {
            ce = ce.with_call("with_label", vec![s(label)]);
        }
        ce
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoseTargetRef {
    Query(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseBoneEntry {
    pub path: String,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseCapturePoseComponent {
    pub name: String,
    pub target_root_ref: PoseTargetRef,
    pub entries: Vec<PoseBoneEntry>,
    #[serde(skip)]
    component: Option<ComponentId>,
}

impl PoseCapturePoseComponent {
    pub fn new(
        name: impl Into<String>,
        target_root_ref: PoseTargetRef,
        entries: Vec<PoseBoneEntry>,
    ) -> Self {
        Self {
            name: name.into(),
            target_root_ref,
            entries,
            component: None,
        }
    }
}

impl Component for PoseCapturePoseComponent {
    fn name(&self) -> &'static str {
        "pose_capture_pose"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("PoseCapturePose", "new", vec![s(&self.name)])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseCaptureLibraryComponent {
    pub target_root_ref: PoseTargetRef,
    #[serde(skip)]
    component: Option<ComponentId>,
}

impl PoseCaptureLibraryComponent {
    pub fn new(target_root_ref: PoseTargetRef) -> Self {
        Self {
            target_root_ref,
            component: None,
        }
    }
}

impl Component for PoseCaptureLibraryComponent {
    fn name(&self) -> &'static str {
        "pose_capture_library"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::meow_meow::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("PoseCaptureLibrary", "new", vec![])
    }
}
