//! Ancestor skinning driver with an independently referenced joint tree.
use super::{Component, ComponentRef, ce_helpers::*};
use crate::engine::ecs::World;

#[derive(Debug, Clone, Default)]
pub struct JointComponent;
impl Component for JointComponent {
    fn name(&self) -> &'static str {
        "joint"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_mms_ast(&self, _: &World) -> crate::scripting::ast::ComponentExpression {
        ce("Joint")
    }
}

#[derive(Debug, Clone)]
pub struct ArmatureComponent {
    pub joints: Option<ComponentRef>,
    pub automatic_weights: bool,
    /// Capture authored transform rest poses without creating skin weights.
    pub rest_pose_only: bool,
    pub request_revision: u64,
    /// Runtime diagnostics; generated weights/rest matrices are derived data.
    pub status: String,
}
impl Default for ArmatureComponent {
    fn default() -> Self {
        Self {
            joints: None,
            automatic_weights: false,
            rest_pose_only: false,
            request_revision: 0,
            status: "not requested".into(),
        }
    }
}
impl ArmatureComponent {
    pub fn new(joints: ComponentRef) -> Self {
        Self {
            joints: Some(joints),
            ..Self::default()
        }
    }
    pub fn give_automatic_weights(&mut self) {
        self.automatic_weights = true;
        self.rest_pose_only = false;
        self.request_revision = self.request_revision.wrapping_add(1);
        self.status = "pending".into();
    }
    pub fn capture_rest_pose(&mut self) {
        self.automatic_weights = false;
        self.rest_pose_only = true;
        self.request_revision = self.request_revision.wrapping_add(1);
        self.status = "pending rest capture".into();
    }
}
impl Component for ArmatureComponent {
    fn name(&self) -> &'static str {
        "armature"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_mms_ast(&self, _: &World) -> crate::scripting::ast::ComponentExpression {
        let mut result = ce("Armature");
        if let Some(reference) = &self.joints {
            let value = match reference {
                ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
                ComponentRef::Query(query) => s(query),
            };
            result = result.with_call("joints", vec![value]);
        }
        if self.automatic_weights {
            result = result.with_call("give_automatic_weights", vec![]);
        } else if self.rest_pose_only {
            result = result.with_call("capture_rest_pose", vec![]);
        }
        result
    }
}
