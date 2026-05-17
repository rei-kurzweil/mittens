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
    BackgroundComponent, Camera2DComponent, Camera3DComponent, CameraXRComponent, ClockComponent, ColorComponent,
    ControllerHand, ControllerPoseKind, ControllerXRComponent, DirectionalLightComponent,
    EditorComponent, EmissiveComponent, EmissivePassComponent, GLTFComponent, TransformGizmoCoordSpace,
    PointerEvents,
    HtmlElementComponent, ElementType,
    InputComponent, InputTransformModeComponent, InputXRComponent,
    InspectorPanelComponent, KeyframeComponent, LayoutComponent, NormalVisualisationComponent,
    LightQuantizationComponent, OpacityComponent, OpenXRComponent, OverlayComponent,
    PointLightComponent, PointerComponent, TransparentCutoutComponent,
    RouterComponent,
    RenderGraphComponent, ScrollingComponent, SelectableComponent,
    StyleComponent, AlignItems, BoxSizing, Display, EdgeInsets, FlexDirection, FlexWrap,
    JustifyContent, Overflow, Position, SizeDimension, TextAlign, WordWrapMode,
    TextureComponent, UVComponent, WorldPanelComponent,
    TransitionComponent, TransitionEasing, TransitionReplacePolicy,
    QuatTemporalFilterComponent, RaycastableComponent, RenderableComponent,
    RendererSettingsComponent, RendererStatsComponent, TextComponent, TextShadowComponent,
    StencilClipComponent, TextureFilteringComponent, TransformComponent, TransformDropComponent,
    TransformParentComponent,
    TransformForkTRSComponent, TransformMapRotationComponent, TransformMapScaleComponent,
    TransformMapTranslationComponent, TransformMergeTRSComponent,
    TransformSampleAncestorComponent,
    BoundsComponent, MeshComponent, GestureCoordTypeComponent, GestureCoordType,
    CollisionShapeComponent, CollisionShape, CollisionComponent, CollisionMode,
    GravityComponent, MusicNote, MusicNoteComponent,
    Vector3TemporalFilterComponent, QuatYawFollowComponent,
    SignalRouteUpwardComponent, SkinnedMeshComponent, RayCastComponent, RayCastMode,
    RaycastableShapeComponent, RaycastableShapeType, IKChainComponent, IKSolver,
    TransformGizmoComponent, TransformGizmoTranslateComponent, TransformGizmoRotateComponent,
    TransformGizmoScaleComponent, TransformGizmoAxis,
    KineticResponseComponent, KineticResponseMode,
};
use crate::engine::graphics::bounds::Aabb;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::ecs::SignalEmitter;
use crate::engine::graphics::CameraTarget;
use crate::meow_meow::ast::{
    BlockStatement, ComponentExpression, Expression, Ident, Statement, UnaryOpKind,
};
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
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
    let id = create_component(world, &type_name, ce.ctor_method.as_deref(), &ce.ctor_args)?;

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
            "guid" => apply_guid_named_prop(world, id, val)?,
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

    // Recurse into children. Spawn-children create fresh subtrees; Attach-children
    // splice an already-Registered (detached, uninitialised) subtree into place.
    for child in &ce.children {
        match child {
            CeChild::Spawn(child_ce) => {
                spawn_tree(child_ce, Some(id), world, emit)?;
            }
            CeChild::Attach(existing_id) => {
                if let Err(e) = world.add_child(id, *existing_id) {
                    return Err(format!("attach existing child {:?} failed: {e}", existing_id));
                }
                // No init here: the init walk below covers the whole subtree.
            }
        }
    }

    // Initialise tree (if parent is already initialised, or this is a new root).
    let parent_initialised = parent.map(|p| world.is_initialized(p)).unwrap_or(false);
    if parent.is_none() || parent_initialised {
        world.init_component_tree(id, emit);
    }

    Ok(id)
}

/// Like `spawn_tree`, but does **not** attach to a parent and does **not**
/// run `init_component_tree`. The resulting subtree exists in the `World`'s
/// component slotmap as a detached, uninitialised tree, addressable by the
/// returned `ComponentId`.
///
/// Used by `HostCallKind::Register` so that `let x = CE` can produce a live
/// `ComponentId` without committing the tree to the live world graph yet.
/// A later `Attach` HostCall (or splice as a `CeChild::Attach` inside another
/// CE body) places it and runs init.
pub fn spawn_tree_uninitialized(
    ce: &MaterializedCE,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
) -> Result<ComponentId, String> {
    let type_name = resolve_type_name(&ce.component_type);
    let id = create_component(world, &type_name, ce.ctor_method.as_deref(), &ce.ctor_args)?;

    for (method, args) in &ce.calls {
        apply_call(world, id, method, args)?;
    }

    for (prop, val) in &ce.named {
        match prop.as_str() {
            "name" => {
                if let Some(node) = world.get_component_record_mut(id) {
                    node.name = val_as_str(val).unwrap_or("").to_string();
                }
            }
            "guid" => apply_guid_named_prop(world, id, val)?,
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

    for val in &ce.positionals {
        apply_positional(world, id, val)?;
    }

    // Children: spawn-children become children of this uninitialised parent
    // (still uninitialised); attach-children splice in an already-Registered
    // subtree without init.
    for child in &ce.children {
        match child {
            CeChild::Spawn(child_ce) => {
                let child_id = spawn_tree_uninitialized(child_ce, world, emit)?;
                if let Err(e) = world.add_child(id, child_id) {
                    return Err(format!("attach uninit child failed: {e}"));
                }
            }
            CeChild::Attach(existing_id) => {
                if let Err(e) = world.add_child(id, *existing_id) {
                    return Err(format!("attach existing child {:?} failed: {e}", existing_id));
                }
            }
        }
    }

    Ok(id)
}

// ---------------------------------------------------------------------------
// Type name resolution
// ---------------------------------------------------------------------------

/// Resolve a raw type identifier to the canonical PascalCase name used by
/// `create_component`.
///
/// Accepts these forms:
/// - Shortform: `"T"` -> `"Transform"` (from `COMPONENT_SHORTFORMS`).
/// - PascalCase: `"Transform"` -> `"Transform"` (already canonical).
/// - snake_case: `"transform"` -> `"Transform"` (from `Component::name()`).
/// - Loose case-insensitive match against canonical names: handles acronyms
///   that don't survive a mechanical snake_to_pascal (`"camera3d"` ->
///   `"Camera3D"`, `"gltf"` -> `"GLTF"`).
fn resolve_type_name(raw: &str) -> String {
    if let Some(full) = expand_component_shortform(raw) {
        return full.to_string();
    }
    if raw.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        return raw.to_string();
    }
    let stripped: String = raw.chars().filter(|c| *c != '_').collect();
    let lowered = stripped.to_lowercase();
    for entry in crate::meow_meow::token::COMPONENT_SHORTFORMS {
        if entry.full.to_lowercase() == lowered {
            return entry.full.to_string();
        }
    }
    snake_to_pascal(raw)
}

// ---------------------------------------------------------------------------
// Subtree → AST encoding (the reverse direction of `spawn_tree`)
// ---------------------------------------------------------------------------

/// Walk a live component subtree and emit a `ComponentExpression` AST node
/// (root + nested children in the body). Each component contributes its own
/// `to_mms_ast()` for its header; children are appended as
/// `Statement::Expression(Expression::Component(...))` in the body.
///
/// Used by:
/// - `attach_clone` intent: subtree → CE → MaterializedCE → spawn_tree (clone).
/// - REPL `dump` and scene save: subtree → CE → unparser → text.
pub fn subtree_to_ce_ast(world: &World, root: ComponentId) -> Result<ComponentExpression, String> {
    // First pass: collect every GUID referenced by any ActionComponent in
    // the subtree via `ComponentRef::Guid`. These are the targets that
    // need their GUID preserved across save/load so the dumped
    // `@uuid:<g>` selector still resolves on reload.
    let mut referenced_guids: std::collections::HashSet<uuid::Uuid> =
        std::collections::HashSet::new();
    collect_referenced_guids(world, root, &mut referenced_guids);

    subtree_to_ce_ast_inner(world, root, &referenced_guids)
}

fn collect_referenced_guids(
    world: &World,
    node: ComponentId,
    out: &mut std::collections::HashSet<uuid::Uuid>,
) {
    use crate::engine::ecs::component::{
        ActionComponent, ComponentRef, IKChainComponent, TransformParentComponent,
    };
    if let Some(action) = world.get_component_by_id_as::<ActionComponent>(node) {
        for src in &action.target_sources {
            if let ComponentRef::Guid(u) = src {
                out.insert(*u);
            }
        }
    }
    if let Some(ik) = world.get_component_by_id_as::<IKChainComponent>(node) {
        for src in [&ik.target_source, &ik.end_effector_source].iter().copied().flatten() {
            if let ComponentRef::Guid(u) = src {
                out.insert(*u);
            }
        }
    }
    if let Some(tp) = world.get_component_by_id_as::<TransformParentComponent>(node) {
        for src in [&tp.target_source, &tp.root_source].iter().copied().flatten() {
            if let ComponentRef::Guid(u) = src {
                out.insert(*u);
            }
        }
    }
    let children: Vec<ComponentId> = world
        .get_component_record(node)
        .map(|n| n.children.clone())
        .unwrap_or_default();
    for child in children {
        collect_referenced_guids(world, child, out);
    }
}

fn subtree_to_ce_ast_inner(
    world: &World,
    root: ComponentId,
    referenced_guids: &std::collections::HashSet<uuid::Uuid>,
) -> Result<ComponentExpression, String> {
    let node = world
        .get_component_record(root)
        .ok_or_else(|| format!("subtree_to_ce_ast: missing component {root:?}"))?;
    let mut ce = node.component.to_mms_ast(world);

    // Preserve `name` if the author set one. `name` lets `#name`
    // selectors keep resolving across reload; it is independent of
    // `guid` (which serves `@uuid:` selectors). Both are emitted when
    // both apply.
    if !node.name.is_empty() {
        ce.body.statements.push(Statement::Reassign {
            name: crate::meow_meow::ast::Ident("name".to_string()),
            value: Expression::String(node.name.clone()),
        });
    }

    // Preserve `guid` whenever this component is referenced by some
    // Action via `ComponentRef::Guid`. The dumped `@uuid:<g>` selector
    // can only resolve on reload if `spawn_tree` restores the same GUID.
    // Whether `name` is also set is irrelevant — names don't help
    // `@uuid:` lookups.
    if referenced_guids.contains(&node.guid) {
        ce.body.statements.push(Statement::Reassign {
            name: crate::meow_meow::ast::Ident("guid".to_string()),
            value: Expression::String(node.guid.to_string()),
        });
    }

    let children: Vec<ComponentId> = node.children.clone();
    for child_id in children {
        let child_ce = subtree_to_ce_ast_inner(world, child_id, referenced_guids)?;
        ce.body
            .statements
            .push(Statement::Expression(Expression::Component(child_ce)));
    }
    Ok(ce)
}

/// Convert a *ground* `ComponentExpression` (literals only — no binops,
/// no function calls in args other than nested component expressions) into
/// a `MaterializedCE` that `spawn_tree` consumes. This is the synchronous
/// bridge between `to_mms_ast` output and the live spawn path; it does not
/// involve the MMS evaluator thread.
pub fn ce_ast_to_materialized(ce: &ComponentExpression) -> Result<MaterializedCE, String> {
    let mut ctor_method: Option<String> = None;
    let mut ctor_args: Vec<Value> = Vec::new();
    let mut calls: Vec<(String, Vec<Value>)> = Vec::new();

    let mut ctor_iter = ce.constructors.iter();
    if let Some(first) = ctor_iter.next() {
        ctor_method = Some(first.method.0.clone());
        ctor_args = first
            .args
            .iter()
            .map(expression_to_value)
            .collect::<Result<_, _>>()?;
    }
    for ctor in ctor_iter {
        let args: Vec<Value> = ctor
            .args
            .iter()
            .map(expression_to_value)
            .collect::<Result<_, _>>()?;
        calls.push((ctor.method.0.clone(), args));
    }

    let mut children: Vec<CeChild> = Vec::new();
    let mut named: Vec<(String, Value)> = Vec::new();
    for stmt in &ce.body.statements {
        match stmt {
            Statement::Expression(Expression::Component(child_ce)) => {
                children.push(CeChild::Spawn(ce_ast_to_materialized(child_ce)?));
            }
            Statement::Expression(Expression::Call(c)) => {
                // Body builder call, e.g. `scale(0.5, 0.5, 0.5)`.
                if let Expression::Identifier(Ident(name)) = &*c.callee {
                    let args: Vec<Value> = c
                        .args
                        .iter()
                        .map(expression_to_value)
                        .collect::<Result<_, _>>()?;
                    calls.push((name.clone(), args));
                }
            }
            Statement::Reassign { name, value } => {
                // Named-prop in a CE body, e.g. `name = "hero"`, `guid = "..."`.
                // The full evaluator handles this via builder.named.push in
                // evaluator.rs; replicate the same mapping here so the
                // ground-CE dump path preserves named props on round-trip.
                let val = expression_to_value(value)?;
                named.push((name.0.clone(), val));
            }
            _ => {
                // Skip other statement kinds (control flow, lets) —
                // `to_mms_ast` impls should not emit these. If one slips
                // through, we drop it rather than fail the clone.
            }
        }
    }

    Ok(MaterializedCE {
        component_type: ce.component_type.0.clone(),
        ctor_method,
        ctor_args,
        calls,
        named,
        positionals: Vec::new(),
        children,
    })
}

fn expression_to_value(e: &Expression) -> Result<Value, String> {
    match e {
        Expression::Number(n) => Ok(Value::Number(*n)),
        Expression::String(s) => Ok(Value::String(s.clone())),
        Expression::Bool(b) => Ok(Value::Bool(*b)),
        Expression::Null => Ok(Value::Null),
        Expression::Dimension(n, u) => Ok(Value::Dimension { value: *n, unit: *u }),
        Expression::Identifier(Ident(s)) => Ok(Value::Identifier(s.clone())),
        Expression::Array(items) => {
            let vals: Vec<Value> = items
                .iter()
                .map(expression_to_value)
                .collect::<Result<_, _>>()?;
            Ok(Value::Array(vals))
        }
        Expression::UnaryOp { op: UnaryOpKind::Neg, operand } => {
            match expression_to_value(operand)? {
                Value::Number(n) => Ok(Value::Number(-n)),
                Value::Dimension { value, unit } => {
                    Ok(Value::Dimension { value: -value, unit })
                }
                v => Err(format!("cannot negate value: {v:?}")),
            }
        }
        Expression::Component(child_ce) => {
            let m = ce_ast_to_materialized(child_ce)?;
            Ok(Value::ComponentExpr(Box::new(m)))
        }
        other => Err(format!("expression_to_value: unsupported expression {other:?}")),
    }
}

// Silence unused-import warnings if a builder cycle removes the only consumer.
#[allow(dead_code)]
fn _ce_ast_helpers_used(_: BlockStatement) {}

/// Convert `snake_case` (the form returned by `Component::name()`) to the
/// `PascalCase` form recognized by `create_component`.
///
/// `"transform"` -> `"Transform"`, `"audio_output"` -> `"AudioOutput"`,
/// `"camera_3d"` -> `"Camera3d"` (note: GLTF uses caps, see fallback below).
fn snake_to_pascal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            out.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            out.push(ch);
        }
    }
    out
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

fn val_as_str_vec(v: &Value) -> Result<Vec<String>, String> {
    match v {
        Value::Array(items) => items
            .iter()
            .map(|it| val_as_str(it).map(|s| s.to_string()))
            .collect(),
        Value::String(s) | Value::Identifier(s) => Ok(vec![s.clone()]),
        other => Err(format!("expected string/array, got {other:?}")),
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
fn arg_str_vec(args: &[Value], i: usize) -> Result<Vec<String>, String> { val_as_str_vec(arg(args, i)?) }

/// Produce an `ComponentRef` (authoring metadata) from the i-th arg.
///
/// Mapping:
/// - `Value::ComponentObject { id, .. }` → `Guid(world.guid_of(id))`. The
///   live handle case collapses to a guid so dump emits `@uuid:<g>` and
///   runtime resolution uses the O(1) guid index instead of a selector walk.
/// - String starting `@uuid:<hex>` → `Guid(parsed)`. Pre-parsing here means
///   runtime resolution skips selector parsing entirely for the common
///   guid-reference case.
/// - Any other string / identifier → `Query(s)` verbatim.
pub(crate) fn arg_component_ref(
    world: &World,
    args: &[Value],
    i: usize,
) -> Result<crate::engine::ecs::component::ComponentRef, String> {
    value_to_component_ref(world, arg(args, i)?)
}

/// Vec form of `arg_component_ref`. Mixed handle / string elements OK.
pub(crate) fn arg_component_ref_vec(
    world: &World,
    args: &[Value],
    i: usize,
) -> Result<Vec<crate::engine::ecs::component::ComponentRef>, String> {
    match arg(args, i)? {
        Value::Array(items) => items.iter().map(|v| value_to_component_ref(world, v)).collect(),
        other => value_to_component_ref(world, other).map(|t| vec![t]),
    }
}

/// Handle `guid = "8c4f3e72-..."` on a component CE. Replaces the freshly
/// minted GUID with the authored one so `@uuid:` selectors saved against
/// this component still resolve across save/load.
fn apply_guid_named_prop(world: &mut World, id: ComponentId, val: &Value) -> Result<(), String> {
    let s = val_as_str(val).map_err(|e| format!("guid prop: {e}"))?;
    let parsed = uuid::Uuid::parse_str(s)
        .map_err(|e| format!("guid prop: invalid uuid '{s}': {e}"))?;
    world.set_component_guid(id, parsed)
}

/// Best-effort resolution of an `ComponentRef` to a ComponentId at
/// registry-call time. Returns `None` when the referent doesn't exist
/// yet — caller is expected to leave the resolved id as a null sentinel
/// and have a later system pass fill it in (e.g. the AnimationSystem
/// resolution path for Action; AvatarControlSystem for IKChain).
pub(crate) fn resolve_component_ref(
    world: &World,
    src: &crate::engine::ecs::component::ComponentRef,
) -> Option<ComponentId> {
    use crate::engine::ecs::component::ComponentRef;
    match src {
        ComponentRef::Guid(uuid) => world.component_id_by_guid(*uuid),
        ComponentRef::Query(selector) => {
            let roots: Vec<ComponentId> = world
                .all_components()
                .filter(|&cid| world.parent_of(cid).is_none())
                .collect();
            roots
                .into_iter()
                .find_map(|root| world.find_component(root, selector))
        }
    }
}

fn value_to_component_ref(
    world: &World,
    v: &Value,
) -> Result<crate::engine::ecs::component::ComponentRef, String> {
    use crate::engine::ecs::component::ComponentRef;
    match v {
        Value::ComponentObject { id, .. } => {
            let guid = world
                .get_component_record(*id)
                .map(|n| n.guid)
                .ok_or_else(|| format!("component handle {id:?} not found in world"))?;
            Ok(ComponentRef::Guid(guid))
        }
        Value::String(s) | Value::Identifier(s) => {
            if let Some(hex) = s.strip_prefix("@uuid:") {
                let uuid = uuid::Uuid::parse_str(hex)
                    .map_err(|e| format!("invalid uuid in '@uuid:{hex}': {e}"))?;
                Ok(ComponentRef::Guid(uuid))
            } else {
                Ok(ComponentRef::Query(s.clone()))
            }
        }
        other => Err(format!(
            "expected component handle or selector string, got {other:?}"
        )),
    }
}

fn parse_gizmo_axis(ctor: Option<&str>) -> TransformGizmoAxis {
    match ctor {
        Some("x") | Some("X") => TransformGizmoAxis::X,
        Some("y") | Some("Y") => TransformGizmoAxis::Y,
        Some("z") | Some("Z") => TransformGizmoAxis::Z,
        _ => TransformGizmoAxis::X,
    }
}

/// Accept either a unit-literal (`50%`, `20gu`) or a bare number (interpreted
/// as glyph units) and produce a `SizeDimension`. Used by Style sizing setters.
fn arg_size_dimension(args: &[Value], i: usize) -> Result<SizeDimension, String> {
    use crate::meow_meow::token::Unit;
    match arg(args, i)? {
        Value::Number(n) => Ok(SizeDimension::GlyphUnits(*n as f32)),
        Value::Dimension { value, unit } => match unit {
            Unit::Percent => Ok(SizeDimension::Percent(*value as f32)),
            Unit::GlyphUnits => Ok(SizeDimension::GlyphUnits(*value as f32)),
            Unit::Degrees | Unit::Radians => {
                Err(format!("expected length unit (gu, %) for size, got {:?}", unit))
            }
        },
        v => Err(format!("expected number or dimension for size, got {:?}", v)),
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
            Some("plane") => add!(RenderableComponent::plane()),
            Some("tetrahedron") => add!(RenderableComponent::tetrahedron()),
            _ => Err(format!("Renderable: unknown constructor '{}'", ctor.unwrap_or(""))),
        },
        "StencilClip" => {
            let id = world.add_component(StencilClipComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Background" => {
            let id = world.add_component(BackgroundComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
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
        "BlurPass" => {
            let id = world.add_component(BlurPassComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Bloom" => {
            let id = world.add_component(BloomComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "DirectionalLight" => {
            let id = world.add_component(DirectionalLightComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "PointLight" => {
            let id = world.add_component(PointLightComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Emissive" => match ctor {
            Some("on") => add!(EmissiveComponent::on()),
            Some("off") => add!(EmissiveComponent::off()),
            _ => add!(EmissiveComponent::on()),
        },
        "Opacity" => {
            let id = world.add_component(OpacityComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
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
                Some("forward_y") => InputTransformModeComponent::forward_y(),
                Some("forward_z") => InputTransformModeComponent::forward_z(),
                _ => InputTransformModeComponent::forward_z(),
            };
            let id = world.add_component(c);
            // Remaining builder calls (e.g. roll_axis_y, fps_rotation) get applied
            // by spawn_tree's normal call-list pass via apply_call.
            Ok(id)
        }
        "Camera3D" => {
            let id = world.add_component(Camera3DComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Camera2D" => {
            let id = world.add_component(Camera2DComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "CameraXR" => match ctor {
            Some("off") => add!(CameraXRComponent::off()),
            Some("on") => add!(CameraXRComponent::on()),
            _ => add!(CameraXRComponent::on()),
        },
        "Pointer" => match ctor {
            Some("disabled") => add!(PointerComponent::disabled()),
            _ => add!(PointerComponent::new()),
        },
        "OpenXR" => match ctor {
            Some("off") => add!(OpenXRComponent::off()),
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
        "TransformParent" => match ctor {
            Some("target") => add!(
                TransformParentComponent::new().with_target_source(arg_component_ref(world, args, 0)?)
            ),
            _ => add!(TransformParentComponent::new()),
        },
        "TransformForkTRS" => add!(TransformForkTRSComponent::new()),
        "TransformMapTranslation" => add!(TransformMapTranslationComponent::new()),
        "TransformMapRotation" => add!(TransformMapRotationComponent::new()),
        "TransformMapScale" => add!(TransformMapScaleComponent::new()),
        "TransformMergeTRS" => add!(TransformMergeTRSComponent::new()),
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
            let id = world.add_component(c);
            // Builder calls (e.g. `.window_size(...)`) chained after the ctor go
            // through apply_call in spawn_tree's call-list pass. The non-`msaa_off`
            // path may pass through a builder name as the first ctor; route it.
            if let Some(method) = ctor {
                if method != "msaa_off" {
                    apply_call(world, id, method, args)?;
                }
            }
            Ok(id)
        }
        "RendererStats" => {
            let id = world.add_component(RendererStatsComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Text" => add!(TextComponent::new("")),
        "TextShadow" => {
            let id = world.add_component(TextShadowComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "AvatarControl" => {
            let id = world.add_component(AvatarControlComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Editor" => {
            let id = world.add_component(EditorComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Router" => {
            let id = world.add_component(RouterComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Selectable" => match ctor {
            Some("off") => add!(SelectableComponent::off()),
            _ => add!(SelectableComponent::on()),
        },
        "InspectorPanel" => add!(InspectorPanelComponent::new()),
        "Scrolling" => match ctor {
            Some("new") => add!(ScrollingComponent::new(arg_f32(args, 0)?, arg_f32(args, 1)?)),
            _ => add!(ScrollingComponent::new(0.1, 0.1)),
        },
        "WorldPanel" => add!(WorldPanelComponent::new()),
        "HtmlElement" => {
            let c = match ctor {
                Some("div")     => HtmlElementComponent::div(),
                Some("span")    => HtmlElementComponent::span(),
                Some("body")    => HtmlElementComponent::body(),
                Some("header")  => HtmlElementComponent::header(),
                Some("p")       => HtmlElementComponent::p(),
                Some("section") => HtmlElementComponent::new(ElementType::Section),
                Some("article") => HtmlElementComponent::new(ElementType::Article),
                Some("footer")  => HtmlElementComponent::new(ElementType::Footer),
                Some("nav")     => HtmlElementComponent::new(ElementType::Nav),
                Some("aside")   => HtmlElementComponent::new(ElementType::Aside),
                Some("main")    => HtmlElementComponent::new(ElementType::Main),
                Some("h1")      => HtmlElementComponent::new(ElementType::H1),
                Some("h2")      => HtmlElementComponent::new(ElementType::H2),
                Some("h3")      => HtmlElementComponent::new(ElementType::H3),
                Some("h4")      => HtmlElementComponent::new(ElementType::H4),
                Some("h5")      => HtmlElementComponent::new(ElementType::H5),
                Some("h6")      => HtmlElementComponent::new(ElementType::H6),
                _               => HtmlElementComponent::new(ElementType::Element),
            };
            add!(c)
        }
        "Style" => add!(StyleComponent::new()),
        "LayoutRoot" => {
            let w = if !args.is_empty() { arg_f32(args, 0)? } else { 80.0 };
            add!(LayoutComponent::new(w))
        }
        "Raycastable" => match ctor {
            Some("disabled") => add!(RaycastableComponent::disabled()),
            Some("drag_only") => add!(RaycastableComponent::drag_only()),
            Some("click_only") => add!(RaycastableComponent::click_only()),
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
        "Transition" => {
            let id = world.add_component(TransitionComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "UV" => {
            let id = world.add_component(UVComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
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
        "Action" => {
            use crate::engine::ecs::IntentValue as IV;
            use slotmap::Key;
            let null_ids = |n: usize| vec![ComponentId::null(); n];
            match ctor {
                Some("noop") => add!(ActionComponent::default()),
                Some("print") => add!(ActionComponent::print(arg_str(args, 0)?)),
                Some("set_color") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let rgba = arg_f32_arr::<4>(args, 1)?;
                    let signal = IV::SetColor { component_ids: null_ids(targets.len()), rgba };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("set_text") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let text = arg_str(args, 1)?.to_string();
                    let signal = IV::SetText { component_ids: null_ids(targets.len()), text };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("set_position") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let position = arg_f32_arr::<3>(args, 1)?;
                    let signal = IV::SetPosition { component_ids: null_ids(targets.len()), position };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("attach") => {
                    let parents = arg_component_ref_vec(world, args, 0)?;
                    let child = arg_component_ref(world, args, 1)?;
                    let mut sources = parents.clone();
                    sources.push(child);
                    let signal = IV::Attach {
                        parents: null_ids(parents.len()),
                        child: ComponentId::null(),
                    };
                    add!(ActionComponent::new_authored(signal, sources))
                }
                Some("attach_clone") => {
                    let parents = arg_component_ref_vec(world, args, 0)?;
                    let prefab = arg_component_ref(world, args, 1)?;
                    let mut sources = parents.clone();
                    sources.push(prefab);
                    let signal = IV::AttachClone {
                        parents: null_ids(parents.len()),
                        prefab_root: ComponentId::null(),
                    };
                    add!(ActionComponent::new_authored(signal, sources))
                }
                Some("detach") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let signal = IV::Detach { component_ids: null_ids(targets.len()) };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("remove_subtree") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let signal = IV::RemoveSubtree { component_ids: null_ids(targets.len()) };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("request_raycast") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let signal = IV::RequestRaycast { component_ids: null_ids(targets.len()) };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("update_transform") => {
                    let target = arg_component_ref(world, args, 0)?;
                    let translation = arg_f32_arr::<3>(args, 1)?;
                    let rotation_euler = arg_f32_arr::<3>(args, 2)?;
                    let scale = arg_f32_arr::<3>(args, 3)?;
                    let transform = TransformComponent::new()
                        .with_position(translation[0], translation[1], translation[2])
                        .with_rotation_euler(rotation_euler[0], rotation_euler[1], rotation_euler[2])
                        .with_scale(scale[0], scale[1], scale[2]);
                    let signal = IV::UpdateTransform {
                        component_ids: null_ids(1),
                        translation: transform.transform.translation,
                        rotation_quat_xyzw: transform.transform.rotation,
                        scale: transform.transform.scale,
                    };
                    add!(ActionComponent::new_authored(signal, vec![target]))
                }
                Some("update_transform_quat") => {
                    let target = arg_component_ref(world, args, 0)?;
                    let translation = arg_f32_arr::<3>(args, 1)?;
                    let rotation_quat = arg_f32_arr::<4>(args, 2)?;
                    let scale = arg_f32_arr::<3>(args, 3)?;
                    let signal = IV::UpdateTransform {
                        component_ids: null_ids(1),
                        translation,
                        rotation_quat_xyzw: rotation_quat,
                        scale,
                    };
                    add!(ActionComponent::new_authored(signal, vec![target]))
                }
                _ => add!(ActionComponent::default()),
            }
        },
        "NormalVis" => {
            let mut c = NormalVisualisationComponent::new();
            if let Some("thickness") = ctor {
                c = c.with_thickness(arg_f32(args, 0)?);
            }
            add!(c)
        }
        "TransparentCutout" => match ctor {
            Some("disabled") => add!(TransparentCutoutComponent::new().with_enabled(false)),
            _ => add!(TransparentCutoutComponent::new()),
        },
        "LightQuantization" => match ctor {
            Some("steps") => add!(LightQuantizationComponent::steps(arg_f32(args, 0)?)),
            _ => add!(LightQuantizationComponent::new()),
        },
        "Bounds" => {
            let (min, max) = match ctor {
                Some("aabb") => (arg_f32_arr::<3>(args, 0)?, arg_f32_arr::<3>(args, 1)?),
                _ => ([0.0; 3], [0.0; 3]),
            };
            add!(BoundsComponent::new(Aabb { min, max }))
        }
        "Mesh" => match ctor {
            Some("new") => add!(MeshComponent::new(arg_str(args, 0)?)),
            _ => add!(MeshComponent::new("")),
        },
        "GestureCoordType" => match ctor {
            Some("screen_space_1d_slider") => add!(GestureCoordTypeComponent::screen_space_1d_slider()),
            Some("world_plane") => add!(GestureCoordTypeComponent::world_plane()),
            _ => add!(GestureCoordTypeComponent::world_plane()),
        },
        "CollisionShape" => match ctor {
            Some("cube") => {
                let half_extents = arg_f32_arr::<3>(args, 0)?;
                add!(CollisionShapeComponent::new(
                    CollisionShape::cube_half_extents(half_extents)
                ))
            }
            Some("sphere") => add!(CollisionShapeComponent::new(
                CollisionShape::sphere_radius(arg_f32(args, 0)?)
            )),
            _ => add!(CollisionShapeComponent::cube()),
        },
        "RaycastableShape" => {
            let shape = match ctor {
                Some("aabb") => RaycastableShapeType::Aabb,
                Some("cone") => RaycastableShapeType::Cone,
                Some("ring_2d") => RaycastableShapeType::Ring2D,
                Some("quad_2d") => RaycastableShapeType::Quad2D,
                Some("triangle_2d") => RaycastableShapeType::Triangle2D,
                Some("tetrahedron") => RaycastableShapeType::Tetrahedron,
                Some("box") => RaycastableShapeType::Box,
                _ => RaycastableShapeType::InferFromBaseMesh,
            };
            add!(RaycastableShapeComponent::new(shape))
        }
        "Collision" => match ctor {
            Some("static") => add!(CollisionComponent::STATIC()),
            Some("kinematic") => add!(CollisionComponent::KINEMATIC()),
            Some("rigged") => add!(CollisionComponent::RIGGED()),
            _ => add!(CollisionComponent::STATIC()),
        },
        "Gravity" => {
            let id = world.add_component(GravityComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "SkinnedMesh" => match ctor {
            Some("new") => add!(SkinnedMeshComponent::new(arg_f32(args, 0)? as usize)),
            _ => add!(SkinnedMeshComponent::new(0)),
        },
        "Vector3TemporalFilter" => {
            let mut c = Vector3TemporalFilterComponent::new();
            if let Some("smoothing_factor") = ctor {
                c = c.with_smoothing_factor(arg_f32(args, 0)?);
            }
            add!(c)
        }
        "QuatYawFollow" => match ctor {
            Some("new") => add!(QuatYawFollowComponent::new(arg_f32(args, 0)?, arg_f32(args, 1)?)),
            _ => add!(QuatYawFollowComponent::default()),
        },
        "SignalRouteUpward" => match ctor {
            Some("new") => add!(SignalRouteUpwardComponent::new(
                arg_str(args, 0)?, arg_str(args, 1)?
            )),
            _ => add!(SignalRouteUpwardComponent::default()),
        },
        "AvatarBodyYaw" => {
            let id = world.add_component(AvatarBodyYawComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Raycast" => match ctor {
            Some("continuous") => add!(RayCastComponent::continuous()),
            Some("event_driven") => add!(RayCastComponent::event_driven()),
            _ => add!(RayCastComponent::event_driven()),
        },
        "MusicNote" => {
            let pitch = match ctor {
                Some("a") | Some("b") | Some("c") | Some("d") | Some("e") | Some("f") | Some("g") => ctor.unwrap(),
                _ => "a",
            };
            let octave = arg_f32(args, 0)? as u16;
            let duration = arg_f32(args, 1)?;
            let note = match pitch {
                "a" => MusicNote::a(octave, duration),
                "b" => MusicNote::b(octave, duration),
                "c" => MusicNote::c(octave, duration),
                "d" => MusicNote::d(octave, duration),
                "e" => MusicNote::e(octave, duration),
                "f" => MusicNote::f(octave, duration),
                "g" => MusicNote::g(octave, duration),
                _ => MusicNote::default(),
            };
            add!(MusicNoteComponent::new(note))
        }
        "IKChain" => {
            let solver = match ctor {
                Some("aim_constraint") => IKSolver::AimConstraint {
                    offset_yaw: arg_f32(args, 0)?,
                },
                Some("two_bone_ik") => IKSolver::TwoBoneIK {
                    pole_direction: arg_f32_arr::<3>(args, 0)?,
                    copy_end_rotation: arg_bool(args, 1)?,
                },
                Some("fabrik") => IKSolver::Fabrik {
                    max_iterations: arg_f32(args, 0)? as u32,
                    tolerance: arg_f32(args, 1)?,
                },
                _ => IKSolver::AimConstraint { offset_yaw: 0.0 },
            };
            // target_id and end_effector_id are runtime-wired by AvatarControlSystem;
            // pass a sentinel for now.
            use slotmap::Key;
            let sentinel = ComponentId::null();
            add!(IKChainComponent::new(solver, sentinel, sentinel))
        }
        "TransformGizmo" => {
            let id = world.add_component(TransformGizmoComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "TransformGizmoTranslate" => add!(TransformGizmoTranslateComponent::new(
            parse_gizmo_axis(ctor)
        )),
        "TransformGizmoRotate" => add!(TransformGizmoRotateComponent::new(
            parse_gizmo_axis(ctor)
        )),
        "TransformGizmoScale" => add!(TransformGizmoScaleComponent::new(
            parse_gizmo_axis(ctor)
        )),
        "KineticResponse" => {
            let c = match ctor {
                Some("push") => KineticResponseComponent::push(),
                Some("slide") => KineticResponseComponent::slide(),
                _ => KineticResponseComponent::slide(),
            };
            let id = world.add_component(c);
            Ok(id)
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
    if let Some(style) = world.get_component_by_id_as_mut::<StyleComponent>(id) {
        match name {
            "background_color" => {
                style.background_color = Some(val_as_f32_array::<4>(val)?);
                return Ok(());
            }
            "background_z" => {
                style.background_z = val_as_f32(val)?;
                return Ok(());
            }
            "color" => {
                style.color = Some(val_as_f32_array::<4>(val)?);
                return Ok(());
            }
            _ => {}
        }
    }

    if let Some(router) = world.get_component_by_id_as_mut::<RouterComponent>(id) {
        match name {
            "target" => {
                router.target_name = Some(val_as_str(val)?.to_string());
            }
            "ignore" => {
                let Value::Array(items) = val else {
                    return Err(format!("expected array for Router.ignore, got {val:?}"));
                };
                router.ignore_names = items
                    .iter()
                    .map(val_as_str)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(str::to_string)
                    .collect();
            }
            _ => {}
        }
        return Ok(());
    }

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
            "rotation_quat" => *t = t.clone().with_rotation_quat([
                arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?, arg_f32(args, 3)?,
            ]),
            _ => {}
        }
        return Ok(());
    }
    if let Some(l) = world.get_component_by_id_as_mut::<LayoutComponent>(id) {
        match method {
            "width" | "available_width" => {
                l.available_width = arg_f32(args, 0)?;
                l.dirty = true;
            }
            "height" | "available_height" => {
                l.available_height = Some(arg_f32(args, 0)?);
                l.dirty = true;
            }
            "unit_scale" => l.unit_scale = arg_f32(args, 0)?,
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
            "output_texture" => {
                *bloom = bloom.clone().with_output_texture(arg_str(args, 0)?);
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(sc) = world.get_component_by_id_as_mut::<StencilClipComponent>(id) {
        if method == "stencil_ref" {
            sc.stencil_ref = arg_f32(args, 0)? as u8;
        }
        return Ok(());
    }
    if let Some(g) = world.get_component_by_id_as_mut::<GravityComponent>(id) {
        match method {
            "enabled" => g.enabled = arg_bool(args, 0)?,
            "coefficient" => g.coefficient = arg_f32(args, 0)?,
            _ => {}
        }
        return Ok(());
    }
    if let Some(aby) = world.get_component_by_id_as_mut::<AvatarBodyYawComponent>(id) {
        match method {
            "threshold" => *aby = aby.clone().with_threshold(arg_f32(args, 0)?),
            "rate" => *aby = aby.clone().with_rate(arg_f32(args, 0)?),
            "forward_plus_z" => *aby = aby.clone().with_forward_plus_z(),
            _ => {}
        }
        return Ok(());
    }
    if let Some(gz) = world.get_component_by_id_as_mut::<TransformGizmoComponent>(id) {
        if method == "scale" {
            *gz = gz.clone().with_scale(arg_f32(args, 0)?);
        }
        return Ok(());
    }
    if let Some(kr) = world.get_component_by_id_as_mut::<KineticResponseComponent>(id) {
        match method {
            "enabled" => kr.enabled = arg_bool(args, 0)?,
            "max_iterations" => kr.max_iterations = arg_f32(args, 0)? as u32,
            "push_out_epsilon" => kr.push_out_epsilon = arg_f32(args, 0)?,
            "push_strength" => kr.push_strength = arg_f32(args, 0)?,
            "friction" => kr.friction = arg_f32(args, 0)?,
            "friction_y" => kr.friction_y = arg_f32(args, 0)?,
            "max_speed" => kr.max_speed = arg_f32(args, 0)?,
            _ => {}
        }
        return Ok(());
    }
    if let Some(rs) = world.get_component_by_id_as_mut::<RendererStatsComponent>(id) {
        match method {
            "enabled" => rs.enabled = arg_bool(args, 0)?,
            "update_interval_sec" => rs.update_interval_sec = arg_f32(args, 0)?,
            "smoothing" => rs.smoothing = arg_f32(args, 0)?,
            "color" => rs.color = arg_f32_arr::<4>(args, 0)?,
            "emissive" => rs.emissive = arg_bool(args, 0)?,
            "camera_target" => {
                let target = match arg_str(args, 0)? {
                    "Xr" | "xr" => CameraTarget::Xr,
                    _ => CameraTarget::Window,
                };
                *rs = rs.clone().with_camera_target(target);
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(mn) = world.get_component_by_id_as_mut::<MusicNoteComponent>(id) {
        if method == "velocity" {
            mn.note = mn.note.with_velocity(arg_f32(args, 0)?);
        }
        return Ok(());
    }
    if world.get_component_by_id_as::<IKChainComponent>(id).is_some() {
        match method {
            "weight" => {
                let w = arg_f32(args, 0)?;
                if let Some(ik) = world.get_component_by_id_as_mut::<IKChainComponent>(id) {
                    *ik = ik.clone().with_weight(w);
                }
            }
            "target" => {
                let src = arg_component_ref(world, args, 0)?;
                let resolved = resolve_component_ref(world, &src);
                if let Some(ik) = world.get_component_by_id_as_mut::<IKChainComponent>(id) {
                    ik.target_source = Some(src);
                    if let Some(r) = resolved {
                        ik.target_id = r;
                    }
                }
            }
            "end_effector" => {
                let src = arg_component_ref(world, args, 0)?;
                let resolved = resolve_component_ref(world, &src);
                if let Some(ik) = world.get_component_by_id_as_mut::<IKChainComponent>(id) {
                    ik.end_effector_source = Some(src);
                    if let Some(r) = resolved {
                        ik.end_effector_id = r;
                    }
                }
            }
            _ => {}
        }
        return Ok(());
    }
    if world.get_component_by_id_as::<TransformParentComponent>(id).is_some() {
        match method {
            "target" => {
                let src = arg_component_ref(world, args, 0)?;
                if let Some(tp) = world.get_component_by_id_as_mut::<TransformParentComponent>(id) {
                    tp.target_source = Some(src);
                }
            }
            "root" => {
                let src = arg_component_ref(world, args, 0)?;
                if let Some(tp) = world.get_component_by_id_as_mut::<TransformParentComponent>(id) {
                    tp.root_source = Some(src);
                }
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(rc) = world.get_component_by_id_as_mut::<RayCastComponent>(id) {
        if method == "max_distance" {
            *rc = rc.with_max_distance(arg_f32(args, 0)?);
        }
        return Ok(());
    }
    if let Some(yf) = world.get_component_by_id_as_mut::<QuatYawFollowComponent>(id) {
        match method {
            "forward_plus_z" => *yf = yf.with_forward_plus_z(),
            "initial_yaw" => *yf = yf.with_initial_yaw(arg_f32(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(v3f) = world.get_component_by_id_as_mut::<Vector3TemporalFilterComponent>(id) {
        if method == "smoothing_factor" {
            *v3f = v3f.with_smoothing_factor(arg_f32(args, 0)?);
        }
        return Ok(());
    }
    if let Some(c3) = world.get_component_by_id_as_mut::<Camera3DComponent>(id) {
        match method {
            "fov" => *c3 = c3.clone().with_fov(arg_f32(args, 0)?),
            "near" => *c3 = c3.clone().with_near(arg_f32(args, 0)?),
            "far" => *c3 = c3.clone().with_far(arg_f32(args, 0)?),
            "target" => {
                c3.target = match arg_str(args, 0)? {
                    "xr" => CameraTarget::Xr,
                    _ => CameraTarget::Window,
                };
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(c2) = world.get_component_by_id_as_mut::<Camera2DComponent>(id) {
        if method == "target" {
            c2.target = match arg_str(args, 0)? {
                "xr" => CameraTarget::Xr,
                _ => CameraTarget::Window,
            };
        }
        return Ok(());
    }
    if let Some(cxr) = world.get_component_by_id_as_mut::<CameraXRComponent>(id) {
        if method == "target" {
            cxr.target = match arg_str(args, 0)? {
                "window" => CameraTarget::Window,
                _ => CameraTarget::Xr,
            };
        }
        return Ok(());
    }
    if let Some(bg) = world.get_component_by_id_as_mut::<BackgroundComponent>(id) {
        match method {
            "occlusion_and_lighting" => bg.occlusion_and_lighting = true,
            "ray_casting" => bg.ray_casting = true,
            _ => {}
        }
        return Ok(());
    }
    if let Some(ed) = world.get_component_by_id_as_mut::<EditorComponent>(id) {
        match method {
            "translation_space" => {
                let space = match arg_str(args, 0)? {
                    "local" => TransformGizmoCoordSpace::Local,
                    _ => TransformGizmoCoordSpace::World,
                };
                ed.transform_gizmo_translation_space = space;
            }
            "rotation_space" => {
                let space = match arg_str(args, 0)? {
                    "local" => TransformGizmoCoordSpace::Local,
                    _ => TransformGizmoCoordSpace::World,
                };
                ed.transform_gizmo_rotation_space = space;
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(rc) = world.get_component_by_id_as_mut::<RaycastableComponent>(id) {
        if method == "pointer_events" {
            rc.pointer_events = match arg_str(args, 0)? {
                "drag_only" => PointerEvents::DragOnly,
                "click_only" => PointerEvents::ClickOnly,
                "pass_through" => PointerEvents::PassThrough,
                _ => PointerEvents::All,
            };
        }
        return Ok(());
    }
    if let Some(op) = world.get_component_by_id_as_mut::<OpacityComponent>(id) {
        match method {
            "opacity" => *op = op.with_opacity(arg_f32(args, 0)?),
            "multiple_layers" => *op = op.with_multiple_layers(),
            _ => {}
        }
        return Ok(());
    }
    if let Some(em) = world.get_component_by_id_as_mut::<EmissiveComponent>(id) {
        if method == "intensity" {
            em.intensity = arg_f32(args, 0)?.max(0.0);
        }
        return Ok(());
    }
    if let Some(gltf) = world.get_component_by_id_as_mut::<GLTFComponent>(id) {
        if method == "with_visualized_transforms" {
            *gltf = gltf.clone().with_visualized_transforms(arg_bool(args, 0)?);
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
            "offset" => {
                let arr = arg_f32_arr::<3>(args, 0)?;
                *ts = ts.clone().with_offset(arr);
            }
            "rgba" => {
                let arr = arg_f32_arr::<4>(args, 0)?;
                *ts = ts.clone().with_rgba(arr);
            }
            "scale" => *ts = ts.clone().with_scale(arg_f32(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(router) = world.get_component_by_id_as_mut::<RouterComponent>(id) {
        match method {
            "target" => router.target_name = Some(arg_str(args, 0)?.to_string()),
            "ignore" => router.ignore_names = arg_str_vec(args, 0)?,
            _ => {}
        }
        return Ok(());
    }
    if let Some(text) = world.get_component_by_id_as_mut::<TextComponent>(id) {
        match method {
            "font_size" => text.set_font_size(arg_f32(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(editor) = world.get_component_by_id_as_mut::<EditorComponent>(id) {
        match method {
            "panels" => editor.spawn_panels = arg_bool(args, 0)?,
            "serialize_editor_panels" => editor.serialize_editor_panels = arg_bool(args, 0)?,
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
        use crate::engine::ecs::component::ResolveTargetsMode;
        match method {
            "playing" => *anim = anim.clone().with_state(AnimationState::Playing),
            "looping" => *anim = anim.clone().with_state(AnimationState::Looping),
            "paused"  => *anim = anim.clone().with_state(AnimationState::Paused),
            "resolve_targets" => {
                let mode = match arg_str(args, 0)? {
                    "on_attach" => ResolveTargetsMode::OnAttach,
                    "on_play"   => ResolveTargetsMode::OnPlay,
                    other => return Err(format!(
                        "Animation.resolve_targets: expected 'on_attach' or 'on_play', got {other:?}"
                    )),
                };
                *anim = anim.clone().with_resolve_targets(mode);
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(st) = world.get_component_by_id_as_mut::<StyleComponent>(id) {
        match method {
            "display" => {
                st.display = match arg_str(args, 0)? {
                    "block"                     => Some(Display::Block),
                    "inline"                    => Some(Display::Inline),
                    "inline_block"|"inline-block" => Some(Display::InlineBlock),
                    "flex"                      => Some(Display::Flex),
                    "none"                      => Some(Display::None),
                    _                           => None,
                };
            }
            "width"       => st.width  = arg_size_dimension(args, 0)?,
            "height"      => st.height = arg_size_dimension(args, 0)?,
            "box_sizing" => {
                st.box_sizing = match arg_str(args, 0)? {
                    "border_box"|"border-box" => BoxSizing::BorderBox,
                    "content_box"|"content-box" => BoxSizing::ContentBox,
                    _ => return Ok(()),
                };
            }
            "padding"     => st.padding = EdgeInsets::all_dim(arg_size_dimension(args, 0)?),
            "padding_xy"  => st.padding = EdgeInsets::axes_dim(arg_size_dimension(args, 0)?, arg_size_dimension(args, 1)?),
            "margin"      => st.margin  = EdgeInsets::all_dim(arg_size_dimension(args, 0)?),
            "margin_xy"   => st.margin  = EdgeInsets::axes_dim(arg_size_dimension(args, 0)?, arg_size_dimension(args, 1)?),
            "background_color" => st.background_color = Some(arg_f32_arr::<4>(args, 0)?),
            "background_z" => st.background_z = arg_f32(args, 0)?,
            "color" => st.color = Some(arg_f32_arr::<4>(args, 0)?),
            "flex_direction" => {
                st.flex_direction = match arg_str(args, 0)? {
                    "row"|"Row"                       => FlexDirection::Row,
                    "column"|"Column"                 => FlexDirection::Column,
                    "row_reverse"|"RowReverse"        => FlexDirection::RowReverse,
                    "column_reverse"|"ColumnReverse"  => FlexDirection::ColumnReverse,
                    _                                 => return Ok(()),
                };
            }
            "justify_content" => {
                st.justify_content = match arg_str(args, 0)? {
                    "flex_start"|"start"  => JustifyContent::FlexStart,
                    "flex_end"|"end"      => JustifyContent::FlexEnd,
                    "center"              => JustifyContent::Center,
                    "space_between"       => JustifyContent::SpaceBetween,
                    "space_around"        => JustifyContent::SpaceAround,
                    "space_evenly"        => JustifyContent::SpaceEvenly,
                    _                     => return Ok(()),
                };
            }
            "align_items" => {
                st.align_items = match arg_str(args, 0)? {
                    "stretch"              => AlignItems::Stretch,
                    "flex_start"|"start"  => AlignItems::FlexStart,
                    "flex_end"|"end"      => AlignItems::FlexEnd,
                    "center"              => AlignItems::Center,
                    "baseline"            => AlignItems::Baseline,
                    _                     => return Ok(()),
                };
            }
            "text_align" => {
                st.text_align = match arg_str(args, 0)? {
                    "left"           => TextAlign::Left,
                    "center"         => TextAlign::Center,
                    "right"          => TextAlign::Right,
                    "auto" | "none"  => TextAlign::Auto,
                    _                => return Ok(()),
                };
            }
            "flex_grow"   => st.flex_grow   = arg_f32(args, 0)?,
            "flex_shrink" => st.flex_shrink = arg_f32(args, 0)?,
            "gap"         => { st.row_gap = arg_f32(args, 0)?; st.column_gap = st.row_gap; }
            "row_gap"     => st.row_gap    = arg_f32(args, 0)?,
            "column_gap"  => st.column_gap = arg_f32(args, 0)?,
            "position"    => {
                st.position = match arg_str(args, 0)? {
                    "static"   => Position::Static,
                    "relative" => Position::Relative,
                    "absolute" => Position::Absolute,
                    "fixed"    => Position::Fixed,
                    _          => return Ok(()),
                };
            }
            "top"    => st.top    = Some(arg_size_dimension(args, 0)?),
            "right"  => st.right  = Some(arg_size_dimension(args, 0)?),
            "bottom" => st.bottom = Some(arg_size_dimension(args, 0)?),
            "left"   => st.left   = Some(arg_size_dimension(args, 0)?),
            "font_size" => st.font_size = arg_f32(args, 0)?,
            "overflow" => {
                st.overflow = match arg_str(args, 0)? {
                    "visible" => Overflow::Visible,
                    "hidden"  => Overflow::Hidden,
                    "scroll"  => Overflow::Scroll,
                    "auto"    => Overflow::Auto,
                    _         => return Ok(()),
                };
            }
            "z_index" => st.z_index = Some(arg_f32(args, 0)? as i32),
            "flex_wrap" => {
                st.flex_wrap = match arg_str(args, 0)? {
                    "nowrap"|"no_wrap"     => FlexWrap::NoWrap,
                    "wrap"                 => FlexWrap::Wrap,
                    "wrap_reverse"         => FlexWrap::WrapReverse,
                    _                      => return Ok(()),
                };
            }
            "word_wrap" => {
                st.word_wrap = match arg_str(args, 0)? {
                    "normal"                       => Some(WordWrapMode::Normal),
                    "break_word" | "break-word"    => Some(WordWrapMode::BreakWord),
                    _                              => None,
                };
            }
            "word_wrap_tokens" => {
                st.word_wrap_tokens = Some(arg_str_vec(args, 0)?);
            }
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
            t.text = s.clone();
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
        // Lossless quaternion form — used by `to_mms_ast` so saved/cloned
        // transforms reproduce exactly, including arbitrary axis rotations.
        "rotation_quat" => Ok(c.with_rotation_quat([
            arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?, arg_f32(args, 3)?,
        ])),
        other => {
            println!("[registry] unknown Transform builder: '{other}'");
            Ok(c)
        }
    }
}

