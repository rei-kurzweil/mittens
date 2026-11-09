//! Authored armatures and explicit, repeatable skin-binding requests.
use super::{Component, ComponentRef, ce_helpers::*};
use crate::engine::ecs::World;

#[derive(Debug, Clone, Default)]
pub struct ArmatureComponent;
#[derive(Debug, Clone, Default)]
pub struct JointComponent;

macro_rules! marker {
    ($ty:ty, $name:literal, $mms:literal) => {
        impl Component for $ty {
            fn name(&self) -> &'static str {
                $name
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn to_mms_ast(&self, _: &World) -> crate::scripting::ast::ComponentExpression {
                ce($mms)
            }
        }
    };
}
marker!(ArmatureComponent, "armature", "Armature");
marker!(JointComponent, "joint", "Joint");

#[derive(Debug, Clone)]
pub struct RiggingComponent {
    pub armature: ComponentRef,
    pub target: ComponentRef,
    pub automatic_weights: bool,
    pub request_revision: u64,
    /// Runtime diagnostics; generated weights/rest transforms are not authored data.
    pub status: String,
}
impl RiggingComponent {
    pub fn new(armature: ComponentRef, target: ComponentRef) -> Self {
        Self {
            armature,
            target,
            automatic_weights: false,
            request_revision: 0,
            status: "not requested".into(),
        }
    }
    pub fn give_automatic_weights(&mut self) {
        self.automatic_weights = true;
        self.request_revision = self.request_revision.wrapping_add(1);
        self.status = "pending".into();
    }
}
impl Component for RiggingComponent {
    fn name(&self) -> &'static str {
        "rigging"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_mms_ast(&self, _: &World) -> crate::scripting::ast::ComponentExpression {
        let reference = |r: &ComponentRef| match r {
            ComponentRef::Guid(guid) => s(&format!("@uuid:{guid}")),
            ComponentRef::Query(query) => s(query),
        };
        let mut result = ce_call(
            "Rigging",
            "new",
            vec![reference(&self.armature), reference(&self.target)],
        );
        if self.automatic_weights {
            result = result.with_call("give_automatic_weights", vec![]);
        }
        result
    }
}
