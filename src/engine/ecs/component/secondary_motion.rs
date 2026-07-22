use super::{Component, ComponentRef, ce_helpers::*};
use crate::engine::ecs::ComponentId;
use crate::scripting::ast::Expression;

fn ref_expr(value: &ComponentRef) -> Expression {
    match value {
        ComponentRef::Guid(guid) => Expression::String(format!("@uuid:{guid}")),
        ComponentRef::Query(query) => Expression::String(query.clone()),
    }
}

fn ref_surface(value: &ComponentRef) -> String {
    match value {
        ComponentRef::Guid(guid) => format!("@uuid:{guid}"),
        ComponentRef::Query(query) => query.clone(),
    }
}

#[derive(Debug, Clone, Default)]
pub struct SecondaryMotionComponent {
    component: Option<ComponentId>,
}
impl SecondaryMotionComponent {
    pub fn new() -> Self {
        Self::default()
    }
}
impl Component for SecondaryMotionComponent {
    fn name(&self) -> &'static str {
        "secondary_motion"
    }
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
    }
    fn init(&mut self, emit: &mut dyn crate::engine::ecs::SignalEmitter, component: ComponentId) {
        emit.push_intent_now(
            component,
            crate::engine::ecs::IntentValue::RegisterSecondaryMotion {
                component_ids: vec![component],
            },
        );
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
    ) -> crate::scripting::ast::ComponentExpression {
        ce("SecondaryMotion")
    }
}

#[derive(Debug, Clone)]
pub struct SpringBoneComponent {
    pub stable_name: String,
    pub center: Option<ComponentRef>,
    pub enabled: bool,
    pub virtual_end_length_ratio: Option<f32>,
    component: Option<ComponentId>,
}
impl SpringBoneComponent {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            stable_name: name.into(),
            center: None,
            enabled: true,
            virtual_end_length_ratio: None,
            component: None,
        }
    }
    pub fn center(mut self, target: ComponentRef) -> Self {
        self.center = Some(target);
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn virtual_end_length_ratio(mut self, ratio: f32) -> Self {
        self.virtual_end_length_ratio = Some(ratio.max(0.0));
        self
    }
}
impl Component for SpringBoneComponent {
    fn name(&self) -> &'static str {
        "spring_bone"
    }
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
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
    ) -> crate::scripting::ast::ComponentExpression {
        let mut out = ce_call("SpringBone", "new", vec![s(&self.stable_name)]);
        if let Some(target) = &self.center {
            out = out.with_call("center", vec![ref_expr(target)]);
        }
        if !self.enabled {
            out = out.with_call("enabled", vec![b(false)]);
        }
        if let Some(r) = self.virtual_end_length_ratio {
            out = out.with_call("virtual_end_length_ratio", vec![num(r as f64)]);
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct SpringJointComponent {
    pub node: ComponentRef,
    pub stiffness: f32,
    pub drag_force: f32,
    pub gravity_power: f32,
    pub gravity_dir: [f32; 3],
    component: Option<ComponentId>,
}
impl SpringJointComponent {
    pub fn new(node: ComponentRef) -> Self {
        Self {
            node,
            stiffness: 1.0,
            drag_force: 0.4,
            gravity_power: 0.0,
            gravity_dir: [0.0, -1.0, 0.0],
            component: None,
        }
    }
    pub fn query(selector: impl Into<String>) -> Self {
        Self::new(ComponentRef::Query(selector.into()))
    }
    pub fn stiffness(mut self, value: f32) -> Self {
        self.stiffness = value.max(0.0);
        self
    }
    pub fn drag_force(mut self, value: f32) -> Self {
        self.drag_force = value.clamp(0.0, 1.0);
        self
    }
    pub fn gravity(mut self, power: f32, direction: [f32; 3]) -> Self {
        self.gravity_power = power;
        self.gravity_dir = direction;
        self
    }
}
impl Component for SpringJointComponent {
    fn name(&self) -> &'static str {
        "spring_joint"
    }
    fn set_id(&mut self, id: ComponentId) {
        self.component = Some(id);
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
    ) -> crate::scripting::ast::ComponentExpression {
        ce_call("SpringJoint", "new", vec![ref_expr(&self.node)])
            .with_call("stiffness", vec![num(self.stiffness as f64)])
            .with_call("drag_force", vec![num(self.drag_force as f64)])
            .with_call(
                "gravity",
                vec![
                    num(self.gravity_power as f64),
                    num(self.gravity_dir[0] as f64),
                    num(self.gravity_dir[1] as f64),
                    num(self.gravity_dir[2] as f64),
                ],
            )
    }
}

pub const GENERATED_SIDECAR_MARKER: &str = "// @generated by cat-engine secondary motion\n";

/// Export the authored metadata below one GLTF instance to `<asset>.mms`.
/// Existing hand-authored modules are deliberately never overwritten.
pub fn export_secondary_motion_sidecar(
    world: &crate::engine::ecs::World,
    gltf_id: ComponentId,
) -> Result<std::path::PathBuf, String> {
    let gltf = world
        .get_component_by_id_as::<super::GLTFComponent>(gltf_id)
        .ok_or("target is not a GLTF component")?;
    let metadata = world
        .children_of(gltf_id)
        .iter()
        .copied()
        .find(|id| {
            world
                .get_component_by_id_as::<SecondaryMotionComponent>(*id)
                .is_some()
        })
        .ok_or("GLTF has no SecondaryMotion child")?;
    let path = std::path::PathBuf::from(format!("{}.mms", gltf.uri));
    if let Ok(old) = std::fs::read_to_string(&path) {
        if !old.starts_with(GENERATED_SIDECAR_MARKER) {
            return Err(format!(
                "refusing to overwrite hand-authored module '{}'",
                path.display()
            ));
        }
    }
    fn q(value: &str) -> String {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
    let mut text = String::from(GENERATED_SIDECAR_MARKER);
    text.push_str("export fn secondary_motion() {\n    return SecondaryMotion {\n");
    for chain_id in world.children_of(metadata) {
        let Some(chain) = world.get_component_by_id_as::<SpringBoneComponent>(*chain_id) else {
            continue;
        };
        text.push_str(&format!(
            "        SpringBone.new({})",
            q(&chain.stable_name)
        ));
        if let Some(center) = &chain.center {
            text.push_str(&format!(".center({})", q(&ref_surface(center))));
        }
        if !chain.enabled {
            text.push_str(".enabled(false)");
        }
        if let Some(r) = chain.virtual_end_length_ratio {
            text.push_str(&format!(".virtual_end_length_ratio({r})"));
        }
        text.push_str(" {\n");
        for joint_id in world.children_of(*chain_id) {
            let Some(j) = world.get_component_by_id_as::<SpringJointComponent>(*joint_id) else {
                continue;
            };
            text.push_str(&format!("            SpringJoint.new({}).stiffness({}).drag_force({}).gravity({}, [{}, {}, {}])\n",q(&ref_surface(&j.node)),j.stiffness,j.drag_force,j.gravity_power,j.gravity_dir[0],j.gravity_dir[1],j.gravity_dir[2]));
        }
        text.push_str("        }\n");
    }
    text.push_str("    }\n}\n");
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = path.with_extension(format!(
        "{}tmp",
        path.extension().and_then(|s| s.to_str()).unwrap_or("mms.")
    ));
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(path)
}
