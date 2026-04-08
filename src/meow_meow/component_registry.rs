/// Component registry: maps MMS type names to engine component constructors.
///
/// This is the bridge between a `MaterializedCE` (fully-evaluated on the MMS thread)
/// and live engine components (created on the main thread).
///
/// `spawn_tree` is the only public entry point. It creates the component from the
/// ctor info, applies builder calls, named assignments, and positionals, then
/// recurses into children.
use crate::engine::ecs::component::{
    ActionComponent, AmbientLightComponent, AnimationComponent, AnimationState, BloomComponent,
    BlurPassComponent, AvatarBodyYawComponent, AvatarControlComponent, BackgroundColorComponent,
    BackgroundComponent, Camera3DComponent, CameraXRComponent, ClockComponent, ColorComponent,
    ControllerHand, ControllerPoseKind, ControllerXRComponent, DirectionalLightComponent,
    EditorComponent, EmissiveComponent, EmissivePassComponent, GLTFComponent, InputComponent,
    InputTransformModeComponent, InputXRComponent,
    InspectorPanelComponent, KeyframeComponent, NormalVisualisationComponent, OpenXRComponent,
    OverlayComponent, PointLightComponent, PointerComponent, RenderGraphComponent, ScrollingComponent,
    SelectableComponent, TextBackgroundComponent, TextureComponent, UVComponent, WorldPanelComponent,
    TransitionComponent, TransitionEasing, TransitionReplacePolicy,
    QuatTemporalFilterComponent, RaycastableComponent, RenderableComponent,
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
use crate::meow_meow::object::{MaterializedCE, Value};
use crate::meow_meow::token::expand_component_shortform;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Recursively spawn the component tree described by `ce`, attach it to `parent` (if any),
/// and initialise it. Returns the root `ComponentId`.
pub fn spawn_tree(
    ce: &MaterializedCE,
    parent: Option<ComponentId>,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
) -> Result<ComponentId, String> {
    let type_name = resolve_type_name(&ce.component_type);
    let id = create_component(world, type_name, ce.ctor_method.as_deref(), &ce.ctor_args)?;

    // Extra ctor calls + body builder calls (already evaluated).
    for (method, args) in &ce.calls {
        apply_call(world, id, method, args)?;
    }

    // Named property assignments — intercept node-level fields first.
    for (prop, val) in &ce.named {
        match prop.as_str() {
            "name" => {
                if let Some(node) = world.get_component_record_mut(id) {
                    node.name = val_as_str(val).unwrap_or("").to_string();
                }
            }
            "class" => {
                if let Some(node) = world.get_component_record_mut(id) {
                    match val {
                        Value::String(s) => {
                            node.classes = s.split_whitespace().map(str::to_string).collect();
                        }
                        Value::Array(arr) => {
                            node.classes = arr.iter()
                                .filter_map(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                                .collect();
                        }
                        _ => {}
                    }
                }
            }
            _ => apply_named_assignment(world, id, prop, val)?,
        }
    }

    // Positional content (strings etc).
    for val in &ce.positionals {
        apply_positional(world, id, val)?;
    }

    // Attach to parent before recursing into children.
    if let Some(p) = parent {
        if let Err(e) = world.add_child(p, id) {
            return Err(format!("attach failed: {e}"));
        }
    }

    // Recurse into children.
    for child in &ce.children {
        spawn_tree(child, Some(id), world, emit)?;
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
// Value conversion helpers
// ---------------------------------------------------------------------------

fn val_as_f32(v: &Value) -> Result<f32, String> {
    match v {
        Value::Number(n) => Ok(*n as f32),
        other => Err(format!("expected number, got {other:?}")),
    }
}

fn val_as_bool(v: &Value) -> Result<bool, String> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(format!("expected bool, got {other:?}")),
    }
}

fn val_as_str(v: &Value) -> Result<&str, String> {
    match v {
        Value::String(s) => Ok(s.as_str()),
        Value::Identifier(s) => Ok(s.as_str()),
        other => Err(format!("expected string/ident, got {other:?}")),
    }
}

fn val_as_f32_array<const N: usize>(v: &Value) -> Result<[f32; N], String> {
    match v {
        Value::Array(items) => {
            if items.len() != N {
                return Err(format!("expected array of {N}, got {}", items.len()));
            }
            let mut out = [0.0f32; N];
            for (i, item) in items.iter().enumerate() {
                out[i] = val_as_f32(item)?;
            }
            Ok(out)
        }
        other => Err(format!("expected array, got {other:?}")),
    }
}

// ---------------------------------------------------------------------------
// Argument helpers
// ---------------------------------------------------------------------------

fn arg(args: &[Value], i: usize) -> Result<&Value, String> {
    args.get(i).ok_or_else(|| format!("expected at least {} arg(s), got {}", i + 1, args.len()))
}

fn arg_f32(args: &[Value], i: usize) -> Result<f32, String> { val_as_f32(arg(args, i)?) }
fn arg_bool(args: &[Value], i: usize) -> Result<bool, String> { val_as_bool(arg(args, i)?) }
fn arg_str(args: &[Value], i: usize) -> Result<&str, String> { val_as_str(arg(args, i)?) }
fn arg_f32_arr<const N: usize>(args: &[Value], i: usize) -> Result<[f32; N], String> { val_as_f32_array(arg(args, i)?) }

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
                arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?, arg_f32(args, 3)?
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
        "Overlay" => add!(OverlayComponent::new()),
        "BackgroundColor" => add!(BackgroundColorComponent::new()),
        "AmbientLight" => match ctor {
            Some("rgb") => add!(AmbientLightComponent::rgb(
                arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?
            )),
            _ => add!(AmbientLightComponent::new()),
        },
        "RenderGraph" => match ctor {
            Some("off") => add!(RenderGraphComponent::off()),
            Some("on") => add!(RenderGraphComponent::on()),
            _ => add!(RenderGraphComponent::new()),
        },
        "EmissivePass" => add!(EmissivePassComponent::new()),
        "BlurPass" => add!(BlurPassComponent::new()),
        "Bloom" => add!(BloomComponent::new()),
        "DirectionalLight" => add!(DirectionalLightComponent::new()),
        "PointLight" => add!(PointLightComponent::new()),
        "Emissive" => match ctor {
            Some("on") => add!(EmissiveComponent::on()),
            Some("off") => add!(EmissiveComponent::off()),
            _ => add!(EmissiveComponent::on()),
        },
        "Input" => {
            let mut c = InputComponent::new();
            if let Some("speed") = ctor {
                c = c.with_speed(arg_f32(args, 0)?);
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
        "Pointer" => add!(PointerComponent::new()),
        "OpenXR" => match ctor {
            Some("on") => add!(OpenXRComponent::on()),
            _ => add!(OpenXRComponent::on()),
        },
        "ControllerXR" => match ctor {
            Some("new") => {
                let _enabled = arg_bool(args, 0)?;
                let hand = match arg_str(args, 1)? {
                    "Left" => ControllerHand::Left,
                    "Right" => ControllerHand::Right,
                    s => return Err(format!("unknown ControllerHand: {s}")),
                };
                let pose = match arg_str(args, 2)? {
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
                c = c.with_skip(arg_f32(args, 0)? as usize);
            }
            add!(c)
        }
        "QuatTemporalFilter" => {
            let mut c = QuatTemporalFilterComponent::new();
            if let Some("smoothing_factor") = ctor {
                c = c.with_smoothing_factor(arg_f32(args, 0)?);
            }
            add!(c)
        }
        "GLTF" => match ctor {
            Some("new") => add!(GLTFComponent::new(arg_str(args, 0)?)),
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
        "Scrolling" => match ctor {
            Some("new") => add!(ScrollingComponent::new(
                arg_f32(args, 0)?,
                arg_f32(args, 1)? as usize,
            )),
            _ => add!(ScrollingComponent::new(0.1, 10)),
        },
        "WorldPanel" => add!(WorldPanelComponent::new()),
        "Raycastable" => match ctor {
            Some("enabled") => add!(RaycastableComponent::enabled()),
            _ => add!(RaycastableComponent::enabled()),
        },
        "TextureFiltering" => match ctor {
            Some("linear") => add!(TextureFilteringComponent::linear()),
            Some("nearest_magnification") => add!(TextureFilteringComponent::nearest_magnification()),
            Some("nearest") => add!(TextureFilteringComponent::nearest()),
            _ => add!(TextureFilteringComponent::linear()),
        },
        "Texture" => match ctor {
            Some("render_image") => add!(TextureComponent::render_image(arg_str(args, 0)?)),
            Some("with_uri") | Some("uri") => add!(TextureComponent::with_uri(arg_str(args, 0)?)),
            Some("from_png") => add!(TextureComponent::from_png(arg_str(args, 0)?)),
            Some("from_dds") => add!(TextureComponent::from_dds(arg_str(args, 0)?)),
            _ => add!(TextureComponent::unresolved()),
        },
        "Transition" => add!(TransitionComponent::new()),
        "UV" => add!(UVComponent::new()),
        "Clock" => {
            let mut c = ClockComponent::new();
            if let Some("bpm") = ctor {
                c = c.with_bpm(arg_f32(args, 0)? as f64);
            }
            add!(c)
        }
        "Animation" => {
            let state = match ctor {
                Some("playing") => AnimationState::Playing,
                Some("paused")  => AnimationState::Paused,
                _               => AnimationState::Looping,
            };
            add!(AnimationComponent::new().with_state(state))
        }
        "Keyframe" => match ctor {
            Some("at") => add!(KeyframeComponent::new(arg_f32(args, 0)? as f64)),
            _ => Err("Keyframe requires .at(beat)".into()),
        },
        "Action" => match ctor {
            Some("print") => add!(ActionComponent::print(arg_str(args, 0)?)),
            _ => add!(ActionComponent::default()),
        },
        "NormalVis" => {
            let mut c = NormalVisualisationComponent::new();
            if let Some("thickness") = ctor {
                c = c.with_thickness(arg_f32(args, 0)?);
            }
            add!(c)
        }
        other => Err(format!("unknown component type: '{other}'")),
    }
}

fn apply_named_assignment(
    world: &mut World,
    id: ComponentId,
    name: &str,
    val: &Value,
) -> Result<(), String> {
    if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(id) {
        if name == "rotation" {
            let arr = val_as_f32_array::<3>(val)?;
            *t = t.clone().with_rotation_euler(arr[0], arr[1], arr[2]);
        }
        return Ok(());
    }
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
            "position" => *t = t.clone().with_position(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?),
            "scale"    => *t = t.clone().with_scale(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?),
            "rotation" | "rotation_euler" => *t = t.clone().with_rotation_euler(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(dl) = world.get_component_by_id_as_mut::<DirectionalLightComponent>(id) {
        match method {
            "intensity" => *dl = dl.clone().with_intensity(arg_f32(args, 0)?),
            "color"     => *dl = dl.clone().with_color(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(pl) = world.get_component_by_id_as_mut::<PointLightComponent>(id) {
        match method {
            "intensity" => *pl = pl.clone().with_intensity(arg_f32(args, 0)?),
            "distance" => *pl = pl.clone().with_distance(arg_f32(args, 0)?),
            "color" => *pl = pl.clone().with_color(
                arg_f32(args, 0)?,
                arg_f32(args, 1)?,
                arg_f32(args, 2)?,
            ),
            _ => {}
        }
        return Ok(());
    }
    if let Some(render_graph) = world.get_component_by_id_as_mut::<RenderGraphComponent>(id) {
        match method {
            "on" => *render_graph = render_graph.clone().with_enabled(true),
            "off" => *render_graph = render_graph.clone().with_enabled(false),
            "enabled" => {
                *render_graph = render_graph.clone().with_enabled(arg_bool(args, 0)?)
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(blur_pass) = world.get_component_by_id_as_mut::<BlurPassComponent>(id) {
        match method {
            "on" => *blur_pass = blur_pass.clone().with_enabled(true),
            "off" => *blur_pass = blur_pass.clone().with_enabled(false),
            "enabled" => *blur_pass = blur_pass.clone().with_enabled(arg_bool(args, 0)?),
            "radius_ndc" => *blur_pass = blur_pass.clone().with_radius_ndc(arg_f32(args, 0)?),
            "half_res" => *blur_pass = blur_pass.clone().with_half_res(arg_bool(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(bloom) = world.get_component_by_id_as_mut::<BloomComponent>(id) {
        match method {
            "on" => *bloom = bloom.clone().with_enabled(true),
            "off" => *bloom = bloom.clone().with_enabled(false),
            "enabled" => *bloom = bloom.clone().with_enabled(arg_bool(args, 0)?),
            "intensity" => *bloom = bloom.clone().with_intensity(arg_f32(args, 0)?),
            "radius_ndc" => {
                *bloom = bloom.clone().with_radius_ndc(arg_f32(args, 0)?)
            }
            "emissive_scale" => {
                *bloom = bloom.clone().with_emissive_scale(arg_f32(args, 0)?)
            }
            "half_res" => *bloom = bloom.clone().with_half_res(arg_bool(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(inp) = world.get_component_by_id_as_mut::<InputComponent>(id) {
        if method == "speed" {
            inp.speed = arg_f32(args, 0)?;
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
            *s = s.clone().with_window_size(arg_f32(args, 0)? as u32, arg_f32(args, 1)? as u32);
        }
        return Ok(());
    }
    if let Some(ts) = world.get_component_by_id_as_mut::<TextShadowComponent>(id) {
        match method {
            "offset_xy" => {
                let arr = arg_f32_arr::<2>(args, 0)?;
                *ts = ts.clone().with_offset_xy(arr);
            }
            "z_offset" => *ts = ts.clone().with_z_offset(arg_f32(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(rs) = world.get_component_by_id_as_mut::<RendererStatsComponent>(id) {
        if method == "camera_target" {
            let target = match arg_str(args, 0)? {
                "Xr" => CameraTarget::Xr,
                _ => CameraTarget::Window,
            };
            *rs = rs.clone().with_camera_target(target);
        }
        return Ok(());
    }
    if let Some(qtf) = world.get_component_by_id_as_mut::<QuatTemporalFilterComponent>(id) {
        if method == "smoothing_factor" {
            *qtf = qtf.clone().with_smoothing_factor(arg_f32(args, 0)?);
        }
        return Ok(());
    }
    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        match method {
            "head_bone"               => *avc = avc.clone().with_head_bone(arg_str(args, 0)?),
            "left_hand_bone"          => *avc = avc.clone().with_left_hand_bone(arg_str(args, 0)?),
            "right_hand_bone"         => *avc = avc.clone().with_right_hand_bone(arg_str(args, 0)?),
            "initial_yaw"             => *avc = avc.clone().with_initial_yaw(arg_f32(args, 0)?),
            "forward_plus_z"          => *avc = avc.clone().with_forward_plus_z(),
            "body_yaw_threshold"      => *avc = avc.clone().with_body_yaw_threshold(arg_f32(args, 0)?),
            "body_yaw_rate"           => *avc = avc.clone().with_body_yaw_rate(arg_f32(args, 0)?),
            "hand_rotation_smoothing" => *avc = avc.clone().with_hand_rotation_smoothing(arg_f32(args, 0)?),
            "camera_bone"             => *avc = avc.clone().with_camera_bone(arg_str(args, 0)?),
            "avatar_height"           => *avc = avc.clone().with_avatar_height(arg_f32(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(tb) = world.get_component_by_id_as_mut::<TextBackgroundComponent>(id) {
        match method {
            "padding"        => *tb = tb.clone().with_padding(arg_f32(args, 0)?),
            "padding_top"    => *tb = tb.clone().with_padding_top(arg_f32(args, 0)?),
            "padding_right"  => *tb = tb.clone().with_padding_right(arg_f32(args, 0)?),
            "padding_bottom" => *tb = tb.clone().with_padding_bottom(arg_f32(args, 0)?),
            "padding_left"   => *tb = tb.clone().with_padding_left(arg_f32(args, 0)?),
            "z_offset"       => *tb = tb.clone().with_z_offset(arg_f32(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(tex) = world.get_component_by_id_as_mut::<TextureComponent>(id) {
        match method {
            "render_image" => *tex = TextureComponent::render_image(arg_str(args, 0)?),
            "uri" | "with_uri" => *tex = TextureComponent::with_uri(arg_str(args, 0)?),
            "from_png" => *tex = TextureComponent::from_png(arg_str(args, 0)?),
            "from_dds" => *tex = TextureComponent::from_dds(arg_str(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(transition) = world.get_component_by_id_as_mut::<TransitionComponent>(id) {
        match method {
            "on" => *transition = transition.on(),
            "off" => *transition = transition.off(),
            "enabled" => *transition = transition.enabled(arg_bool(args, 0)?),
            "duration_beats" => {
                *transition = transition.with_duration_beats(arg_f32(args, 0)? as f64)
            }
            "capture_from_current" => {
                *transition = transition.with_capture_from_current(arg_bool(args, 0)?)
            }
            "step" => *transition = transition.with_easing(TransitionEasing::Step),
            "linear" => *transition = transition.with_easing(TransitionEasing::Linear),
            "ease_in_quad" => {
                *transition = transition.with_easing(TransitionEasing::EaseInQuad)
            }
            "ease_out_quad" => {
                *transition = transition.with_easing(TransitionEasing::EaseOutQuad)
            }
            "ease_in_out_quad" => {
                *transition = transition.with_easing(TransitionEasing::EaseInOutQuad)
            }
            "ease_in_cubic" => {
                *transition = transition.with_easing(TransitionEasing::EaseInCubic)
            }
            "ease_out_cubic" => {
                *transition = transition.with_easing(TransitionEasing::EaseOutCubic)
            }
            "ease_in_out_cubic" => {
                *transition = transition.with_easing(TransitionEasing::EaseInOutCubic)
            }
            "ease_in_out_sine" => {
                *transition = transition.with_easing(TransitionEasing::EaseInOutSine)
            }
            "replace_same_target" => {
                *transition = transition.with_replace(TransitionReplacePolicy::ReplaceSameTarget)
            }
            "allow_parallel" => {
                *transition = transition.with_replace(TransitionReplacePolicy::AllowParallel)
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(uv) = world.get_component_by_id_as_mut::<UVComponent>(id) {
        if method == "uv" {
            *uv = uv.clone().with_uv(arg_f32(args, 0)?, arg_f32(args, 1)?);
        }
        return Ok(());
    }
    if let Some(ck) = world.get_component_by_id_as_mut::<ClockComponent>(id) {
        if method == "bpm" {
            *ck = ck.clone().with_bpm(arg_f32(args, 0)? as f64);
        }
        return Ok(());
    }
    if let Some(anim) = world.get_component_by_id_as_mut::<AnimationComponent>(id) {
        match method {
            "playing" => *anim = anim.clone().with_state(AnimationState::Playing),
            "looping" => *anim = anim.clone().with_state(AnimationState::Looping),
            "paused"  => *anim = anim.clone().with_state(AnimationState::Paused),
            _ => {}
        }
        return Ok(());
    }
    println!("[registry] unhandled call '{method}' on component {id:?}");
    Ok(())
}

fn apply_positional(world: &mut World, id: ComponentId, val: &Value) -> Result<(), String> {
    if let Value::String(s) = val {
        if let Some(t) = world.get_component_by_id_as_mut::<TextComponent>(id) {
            *t = TextComponent::new(s.as_str());
            return Ok(());
        }
    }
    println!("[registry] unhandled positional on component {id:?}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Transform builder helper
// ---------------------------------------------------------------------------

fn apply_transform_builder(
    c: TransformComponent,
    method: &str,
    args: &[Value],
) -> Result<TransformComponent, String> {
    match method {
        "position"       => Ok(c.with_position(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?)),
        "scale"          => Ok(c.with_scale(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?)),
        "rotation" | "rotation_euler" => Ok(c.with_rotation_euler(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?)),
        other => {
            println!("[registry] unknown Transform builder: '{other}'");
            Ok(c)
        }
    }
}
