/// Component registry: maps MMS type names to engine component constructors.
///
/// This is the bridge between a parsed `ComponentExpression` and live engine components.
/// Called from the `SpawnComponentTree` intent executor on the main thread.
///
/// For each component type this module knows how to:
///   1. Create the component from an optional `ConstructorCall` (args already as `Value`s).
///   2. Apply `NamedAssignment` and `Call` body items as builder/setter calls.
///
/// Unknown type names or unrecognised methods produce an error string; the executor logs
/// them and continues rather than panicking.
use crate::engine::ecs::component::{
    AmbientLightComponent, AvatarBodyYawComponent, AvatarControlComponent, BackgroundColorComponent, BackgroundComponent, Camera3DComponent,
    CameraXRComponent, ColorComponent, ControllerHand, ControllerPoseKind,
    ControllerXRComponent, DirectionalLightComponent, EditorComponent, EmissiveComponent,
    GLTFComponent, InputComponent, InputTransformModeComponent, InputXRComponent,
    InspectorPanelComponent, OpenXRComponent, SelectableComponent, TextBackgroundComponent,
    WorldPanelComponent,
    QuatTemporalFilterComponent, RayCastComponent, RayCastMode, RenderableComponent,
    RendererSettingsComponent, RendererStatsComponent, TextComponent, TextShadowComponent,
    TextureFilteringComponent, TransformComponent, TransformDropComponent,
    TransformForkTRSComponent, TransformMapRotationComponent, TransformMapScaleComponent,
    TransformMapTranslationComponent, TransformMergeTRSComponent,
    TransformPipelineComponent, TransformPipelineOutputComponent,
    TransformSampleAncestorComponent,
};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::ecs::SignalEmitter;
use crate::engine::graphics::CameraTarget;
use crate::meow_meow::ast::expression::{ComponentBodyItem, ComponentExpression, Expression};
use crate::meow_meow::token::expand_component_shortform;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Recursively spawn the component tree described by `ce`, attach it to `parent` (if any),
/// and initialise it. Returns the root `ComponentId`.
pub fn spawn_tree(
    ce: &ComponentExpression,
    parent: Option<ComponentId>,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
) -> Result<ComponentId, String> {
    let type_name = resolve_type_name(&ce.component_type.0);

    // Evaluate constructor args (must be literals — no ObjectWorld on the main thread).
    let ctor_args: Vec<Value> = if let Some(ctor) = &ce.constructor {
        ctor.args.iter().map(eval_literal).collect::<Result<_, _>>()?
    } else {
        vec![]
    };
    let ctor_method = ce.constructor.as_ref().map(|c| c.method.0.as_str());

    // Create the component.
    let id = create_component(world, type_name, ctor_method, &ctor_args)?;

    // Apply non-child body items (named assignments, builder calls).
    apply_body_items(world, id, &ce.body)?;

    // Attach to parent before recursing into children (so init sees the right topology).
    if let Some(p) = parent {
        if let Err(e) = world.add_child(p, id) {
            return Err(format!("attach failed: {e}"));
        }
    }

    // Recurse into children.
    for item in &ce.body {
        if let ComponentBodyItem::Child(child_ce) = item {
            spawn_tree(child_ce, Some(id), world, emit)?;
        }
    }

    // Initialise tree (if parent is already initialised, or this is a new root).
    let parent_initialised = parent.map(|p| world.is_initialized(p)).unwrap_or(false);
    if parent.is_none() || parent_initialised {
        world.init_component_tree(id, emit);
    }

    Ok(id)
}

// ---------------------------------------------------------------------------
// Type name resolution
// ---------------------------------------------------------------------------

fn resolve_type_name(raw: &str) -> &str {
    expand_component_shortform(raw).unwrap_or(raw)
}

// ---------------------------------------------------------------------------
// Literal expression evaluator (main-thread only — no ObjectWorld)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    F32(f32),
    String(String),
    Identifier(String),
    Array(Vec<Value>),
}

fn eval_literal(expr: &Expression) -> Result<Value, String> {
    match expr {
        Expression::Null => Ok(Value::Null),
        Expression::Bool(b) => Ok(Value::Bool(*b)),
        Expression::Number(n) => Ok(Value::F32(*n as f32)),
        Expression::String(s) => Ok(Value::String(s.clone())),
        Expression::Identifier(id) => Ok(Value::Identifier(id.0.clone())),
        Expression::Array(items) => {
            let vals: Result<Vec<_>, _> = items.iter().map(eval_literal).collect();
            Ok(Value::Array(vals?))
        }
        Expression::Component(_) | Expression::Call(_) => {
            Err("complex expression in constructor args not supported in v1".into())
        }
    }
}

impl Value {
    fn as_f32(&self) -> Result<f32, String> {
        match self {
            Value::F32(v) => Ok(*v),
            other => Err(format!("expected f32, got {other:?}")),
        }
    }
    fn as_bool(&self) -> Result<bool, String> {
        match self {
            Value::Bool(v) => Ok(*v),
            other => Err(format!("expected bool, got {other:?}")),
        }
    }
    fn as_str(&self) -> Result<&str, String> {
        match self {
            Value::String(s) => Ok(s.as_str()),
            Value::Identifier(s) => Ok(s.as_str()),
            other => Err(format!("expected string/ident, got {other:?}")),
        }
    }
    fn as_f32_array<const N: usize>(&self) -> Result<[f32; N], String> {
        match self {
            Value::Array(items) => {
                if items.len() != N {
                    return Err(format!("expected array of {N}, got {}", items.len()));
                }
                let mut out = [0.0f32; N];
                for (i, v) in items.iter().enumerate() {
                    out[i] = v.as_f32()?;
                }
                Ok(out)
            }
            other => Err(format!("expected array, got {other:?}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Component creation
// ---------------------------------------------------------------------------

fn create_component(
    world: &mut World,
    type_name: &str,
    ctor: Option<&str>,
    args: &[Value],
) -> Result<ComponentId, String> {
    macro_rules! add {
        ($component:expr) => {
            Ok(world.add_component($component))
        };
    }

    match type_name {
        "Transform" => {
            let mut c = TransformComponent::new();
            if let Some(method) = ctor {
                c = apply_transform_builder(c, method, args)?;
            }
            add!(c)
        }
        "Color" => match ctor {
            Some("rgba") => add!(ColorComponent::rgba(
                args[0].as_f32()?, args[1].as_f32()?, args[2].as_f32()?, args[3].as_f32()?
            )),
            _ => add!(ColorComponent::new()),
        },
        "Renderable" => match ctor {
            Some("cube") => add!(RenderableComponent::cube()),
            Some("circle2d") => add!(RenderableComponent::circle2d()),
            Some("sphere") => add!(RenderableComponent::sphere()),
            Some("triangle") => add!(RenderableComponent::triangle()),
            Some("square") => add!(RenderableComponent::square()),
            Some("tetrahedron") => add!(RenderableComponent::tetrahedron()),
            _ => Err(format!("Renderable: unknown constructor '{}'", ctor.unwrap_or(""))),
        },
        "Background" => add!(BackgroundComponent::new()),
        "BackgroundColor" => match ctor {
            Some("rgba") => add!(BackgroundColorComponent::rgba(
                args[0].as_f32()?, args[1].as_f32()?, args[2].as_f32()?, args[3].as_f32()?
            )),
            _ => add!(BackgroundColorComponent::new()),
        },
        "AmbientLight" => match ctor {
            Some("rgb") => add!(AmbientLightComponent::rgb(
                args[0].as_f32()?, args[1].as_f32()?, args[2].as_f32()?
            )),
            _ => add!(AmbientLightComponent::new()),
        },
        "DirectionalLight" => add!(DirectionalLightComponent::new()),
        "Emissive" => match ctor {
            Some("on") => add!(EmissiveComponent::on()),
            Some("off") => add!(EmissiveComponent::off()),
            _ => add!(EmissiveComponent::on()),
        },
        "Input" => {
            let mut c = InputComponent::new();
            if let Some("speed") = ctor {
                c = c.with_speed(args[0].as_f32()?);
            }
            add!(c)
        }
        "InputXR" => match ctor {
            Some("on") => add!(InputXRComponent::on()),
            Some("off") => add!(InputXRComponent::off()),
            _ => add!(InputXRComponent::on()),
        },
        "InputTransformMode" => {
            let c = match ctor {
                Some("forward_z") => InputTransformModeComponent::forward_z(),
                _ => InputTransformModeComponent::forward_z(),
            };
            add!(c)
        }
        "Camera3D" => add!(Camera3DComponent::new()),
        "CameraXR" => match ctor {
            Some("on") => add!(CameraXRComponent::on()),
            _ => add!(CameraXRComponent::on()),
        },
        "OpenXR" => match ctor {
            Some("on") => add!(OpenXRComponent::on()),
            _ => add!(OpenXRComponent::on()),
        },
        "ControllerXR" => match ctor {
            Some("new") => {
                let _enabled = args[0].as_bool()?;
                let hand = match args[1].as_str()? {
                    "Left" => ControllerHand::Left,
                    "Right" => ControllerHand::Right,
                    s => return Err(format!("unknown ControllerHand: {s}")),
                };
                let pose = match args[2].as_str()? {
                    "Aim" => ControllerPoseKind::Aim,
                    "Grip" => ControllerPoseKind::Grip,
                    s => return Err(format!("unknown ControllerPoseKind: {s}")),
                };
                add!(ControllerXRComponent::new(true, hand, pose))
            }
            _ => Err("ControllerXR requires .new(enabled, hand, pose)".into()),
        },
        "TransformPipeline" => add!(TransformPipelineComponent::new()),
        "TransformForkTRS" => add!(TransformForkTRSComponent::new()),
        "TransformMapTranslation" => add!(TransformMapTranslationComponent::new()),
        "TransformMapRotation" => add!(TransformMapRotationComponent::new()),
        "TransformMapScale" => add!(TransformMapScaleComponent::new()),
        "TransformMergeTRS" => add!(TransformMergeTRSComponent::new()),
        "TransformPipelineOutput" => add!(TransformPipelineOutputComponent::new()),
        "TransformDrop" => add!(TransformDropComponent::new()),
        "TransformSampleAncestor" => {
            let mut c = TransformSampleAncestorComponent::new();
            if let Some("skip") = ctor {
                c = c.with_skip(args[0].as_f32()? as usize);
            }
            add!(c)
        }
        "QuatTemporalFilter" => {
            let mut c = QuatTemporalFilterComponent::new();
            if let Some("smoothing_factor") = ctor {
                c = c.with_smoothing_factor(args[0].as_f32()?);
            }
            add!(c)
        }
        "GLTF" => match ctor {
            Some("new") => add!(GLTFComponent::new(args[0].as_str()?)),
            _ => Err("GLTF requires .new(\"uri\")".into()),
        },
        "RendererSettings" => {
            let c = match ctor {
                Some("msaa_off") => RendererSettingsComponent::msaa_off(),
                _ => RendererSettingsComponent::new(),
            };
            add!(c)
        }
        "RendererStats" => add!(RendererStatsComponent::new()),
        "Text" => add!(TextComponent::new("")),
        "TextBackground" => add!(TextBackgroundComponent::new()),
        "TextShadow" => add!(TextShadowComponent::new()),
        "AvatarBodyYaw" => add!(AvatarBodyYawComponent::new()),
        "AvatarControl" => add!(AvatarControlComponent::new()),
        "Editor" => add!(EditorComponent::new()),
        "Selectable" => match ctor {
            Some("off") => add!(SelectableComponent::off()),
            _ => add!(SelectableComponent::on()),
        },
        "InspectorPanel" => add!(InspectorPanelComponent::new()),
        "WorldPanel" => add!(WorldPanelComponent::new()),
        "Raycastable" => match ctor {
            Some("enabled") => add!(RayCastComponent::new(RayCastMode::Continuous)),
            _ => add!(RayCastComponent::new(RayCastMode::Continuous)),
        },
        "TextureFiltering" => match ctor {
            Some("nearest_magnification") => add!(TextureFilteringComponent::nearest_magnification()),
            Some("nearest") => add!(TextureFilteringComponent::nearest()),
            _ => add!(TextureFilteringComponent::nearest()),
        },
        other => Err(format!("unknown component type: '{other}'")),
    }
}

// ---------------------------------------------------------------------------
// Body item application
// ---------------------------------------------------------------------------

fn apply_body_items(
    world: &mut World,
    id: ComponentId,
    items: &[ComponentBodyItem],
) -> Result<(), String> {
    for item in items {
        match item {
            ComponentBodyItem::Child(_) => {} // handled by caller
            ComponentBodyItem::NamedAssignment { name, value } => {
                apply_named_assignment(world, id, &name.0, value)?;
            }
            ComponentBodyItem::Call(call) => {
                let args: Vec<Value> = call.args.iter().map(eval_literal).collect::<Result<_, _>>()?;
                apply_call(world, id, &call.callee.0, &args)?;
            }
            ComponentBodyItem::Positional(expr) => {
                apply_positional(world, id, expr)?;
            }
        }
    }
    Ok(())
}

fn apply_named_assignment(
    world: &mut World,
    id: ComponentId,
    name: &str,
    value: &Expression,
) -> Result<(), String> {
    let val = eval_literal(value)?;
    // Route by inspecting the component type — try Transform first, then others.
    if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(id) {
        match name {
            "rotation" => {
                let arr = val.as_f32_array::<3>()?;
                *t = t.clone().with_rotation_euler(arr[0], arr[1], arr[2]);
            }
            _ => {}
        }
        return Ok(());
    }
    // Unknown named assignment — log and continue.
    println!("[registry] unhandled named assignment '{name}' on component {id:?}");
    Ok(())
}

fn apply_call(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    // Transform builders
    if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(id) {
        match method {
            "position" => *t = t.clone().with_position(args[0].as_f32()?, args[1].as_f32()?, args[2].as_f32()?),
            "scale"    => *t = t.clone().with_scale(args[0].as_f32()?, args[1].as_f32()?, args[2].as_f32()?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(dl) = world.get_component_by_id_as_mut::<DirectionalLightComponent>(id) {
        match method {
            "intensity" => *dl = dl.clone().with_intensity(args[0].as_f32()?),
            "color"     => *dl = dl.clone().with_color(args[0].as_f32()?, args[1].as_f32()?, args[2].as_f32()?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(itm) = world.get_component_by_id_as_mut::<InputTransformModeComponent>(id) {
        match method {
            "fps_rotation" => *itm = itm.clone().with_fps_rotation(),
            "roll_axis_y"  => *itm = itm.clone().with_roll_axis_y(),
            _ => {}
        }
        return Ok(());
    }
    if let Some(s) = world.get_component_by_id_as_mut::<RendererSettingsComponent>(id) {
        if method == "window_size" {
            *s = s.clone().with_window_size(args[0].as_f32()? as u32, args[1].as_f32()? as u32);
        }
        return Ok(());
    }
    if let Some(ts) = world.get_component_by_id_as_mut::<TextShadowComponent>(id) {
        match method {
            "offset_xy" => {
                let arr = args[0].as_f32_array::<2>()?;
                *ts = ts.clone().with_offset_xy(arr);
            }
            "z_offset" => *ts = ts.clone().with_z_offset(args[0].as_f32()?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(rs) = world.get_component_by_id_as_mut::<RendererStatsComponent>(id) {
        if method == "camera_target" {
            let target = match args[0].as_str()? {
                "Xr" => CameraTarget::Xr,
                _ => CameraTarget::Window,
            };
            *rs = rs.clone().with_camera_target(target);
        }
        return Ok(());
    }
    if let Some(qtf) = world.get_component_by_id_as_mut::<QuatTemporalFilterComponent>(id) {
        if method == "smoothing_factor" {
            *qtf = qtf.clone().with_smoothing_factor(args[0].as_f32()?);
        }
        return Ok(());
    }
    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        match method {
            "head_bone"               => *avc = avc.clone().with_head_bone(args[0].as_str()?),
            "left_hand_bone"          => *avc = avc.clone().with_left_hand_bone(args[0].as_str()?),
            "right_hand_bone"         => *avc = avc.clone().with_right_hand_bone(args[0].as_str()?),
            "initial_yaw"             => *avc = avc.clone().with_initial_yaw(args[0].as_f32()?),
            "forward_plus_z"          => *avc = avc.clone().with_forward_plus_z(),
            "body_yaw_threshold"      => *avc = avc.clone().with_body_yaw_threshold(args[0].as_f32()?),
            "body_yaw_rate"           => *avc = avc.clone().with_body_yaw_rate(args[0].as_f32()?),
            "hand_rotation_smoothing" => *avc = avc.clone().with_hand_rotation_smoothing(args[0].as_f32()?),
            "camera_bone"             => *avc = avc.clone().with_camera_bone(args[0].as_str()?),
            "avatar_height"           => *avc = avc.clone().with_avatar_height(args[0].as_f32()?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(tb) = world.get_component_by_id_as_mut::<TextBackgroundComponent>(id) {
        match method {
            "padding"        => *tb = tb.clone().with_padding(args[0].as_f32()?),
            "padding_top"    => *tb = tb.clone().with_padding_top(args[0].as_f32()?),
            "padding_right"  => *tb = tb.clone().with_padding_right(args[0].as_f32()?),
            "padding_bottom" => *tb = tb.clone().with_padding_bottom(args[0].as_f32()?),
            "padding_left"   => *tb = tb.clone().with_padding_left(args[0].as_f32()?),
            "z_offset"       => *tb = tb.clone().with_z_offset(args[0].as_f32()?),
            _ => {}
        }
        return Ok(());
    }
    println!("[registry] unhandled call '{method}' on component {id:?}");
    Ok(())
}

fn apply_positional(world: &mut World, id: ComponentId, expr: &Expression) -> Result<(), String> {
    // Text component: bare string literal sets the text content.
    if let Expression::String(s) = expr {
        if let Some(t) = world.get_component_by_id_as_mut::<TextComponent>(id) {
            *t = TextComponent::new(s.as_str());
            return Ok(());
        }
    }
    // Everything else: log and ignore for now.
    println!("[registry] unhandled positional on component {id:?}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Transform builder helper
// ---------------------------------------------------------------------------

fn apply_transform_builder(
    mut c: TransformComponent,
    method: &str,
    args: &[Value],
) -> Result<TransformComponent, String> {
    match method {
        "position"       => Ok(c.with_position(args[0].as_f32()?, args[1].as_f32()?, args[2].as_f32()?)),
        "scale"          => Ok(c.with_scale(args[0].as_f32()?, args[1].as_f32()?, args[2].as_f32()?)),
        "rotation_euler" => Ok(c.with_rotation_euler(args[0].as_f32()?, args[1].as_f32()?, args[2].as_f32()?)),
        other => {
            println!("[registry] unknown Transform builder: '{other}'");
            Ok(c)
        }
    }
}
