use crate::engine::ecs::SignalEmitter;
use crate::engine::ecs::component::AssetPayloadComponent;
use crate::engine::ecs::component::style::VerticalAlign;
/// Component registry: maps MMS type names to engine component constructors.
///
/// This is the bridge between a `MaterializedCE` (fully-evaluated on the MMS thread)
/// and live engine components (created on the main thread).
///
/// `spawn_tree` is the only public entry point. It creates the component from the
/// ctor info, applies builder calls, named assignments, and positionals, then
/// recurses into children.
use crate::engine::ecs::component::{
    ActionComponent, AlignItems, AmbientLightComponent, AnimationComponent, AnimationState,
    AudioClipComponent, AudioOscillator, AudioOscillatorComponent, AudioOutputComponent,
    AudioTriggerMode, AvatarBodyYawComponent, AvatarControlComponent, BackgroundColorComponent,
    BackgroundComponent, BloomComponent, BlurPassComponent, BoundsComponent, BoxSizing,
    Camera2DComponent, Camera3DComponent, CameraXRComponent, ClockComponent, CollisionComponent,
    CollisionShape, CollisionShapeComponent, ColorComponent, ControllerHand, ControllerPoseKind,
    DataComponent, DataValue, DirectionalLightComponent, Display, EdgeInsets, EditorComponent,
    EditorInteractionMode, ElementType, EmissiveComponent, EmissivePassComponent,
    FitBoundsComponent, FitBoundsMode, FitBoundsTarget, FlexDirection, FlexWrap, GLTFComponent,
    GestureCoordTypeComponent, GravityComponent, GridComponent, HtmlElementComponent,
    IKChainComponent, IKSolver, InputComponent, InputTransformModeComponent, InputXRComponent,
    InputXRGamepadComponent, InspectLayoutComponent, JustifyContent, KeyframeComponent,
    KineticResponseComponent, LayoutBoundsComponent, LayoutComponent, LightQuantizationComponent,
    MeshComponent, MirrorComponent, MusicNote, MusicNoteComponent,
    NormalVisualisationComponent, OpacityComponent, OptionComponent, OscillatorType, Overflow,
    OverlayComponent, PointLightComponent, PointerComponent, PointerEvents, PoseCaptureComponent,
    PoseCaptureLibraryComponent, PoseCapturePoseComponent, Position, QuatTemporalFilterComponent,
    QuatYawFollowComponent, RayCastComponent, RaycastableComponent, RaycastableShapeComponent,
    RaycastableShapeType, RenderGraphComponent, RenderableComponent, RendererSettingsComponent,
    RendererStatsComponent, RouterComponent, ScrollingComponent, SelectableComponent,
    SelectionComponent, SerializeComponent, SignalObserverRouterComponent,
    SignalRouteUpwardComponent, SizeDimension, SkinnedMeshComponent, StencilClipComponent,
    StyleComponent, TextAlign, TextComponent, TextInputComponent, TextShadowComponent,
    TextureComponent, TextureFilteringComponent, TransformComponent, TransformDropComponent,
    TransformForkTRSComponent, TransformGizmoAxis, TransformGizmoComponent,
    TransformGizmoCoordSpace, TransformGizmoRotateComponent, TransformGizmoScaleComponent,
    TransformGizmoTranslateComponent, TransformMapRotationComponent, TransformMapScaleComponent,
    TransformMapTranslationComponent, TransformMergeTRSComponent, TransformParentComponent,
    TransformSampleAncestorComponent, TransitionComponent, TransitionEasing,
    TransitionReplacePolicy, TransparentCutoutComponent, UVComponent,
    Vector3TemporalFilterComponent, WordWrapMode, XRHandComponent, XrComponent, XrHandPreference,
};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::CameraTarget;
use crate::engine::graphics::bounds::Aabb;
use crate::engine::graphics::render_assets::RenderAssets;
use crate::meow_meow::ast::{
    BlockStatement, ComponentExpression, Expression, Ident, Statement, UnaryOpKind,
};
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
use crate::meow_meow::token::expand_component_shortform;
use std::cell::RefCell;

thread_local! {
    static LIVE_RENDER_ASSETS: RefCell<Option<*mut RenderAssets>> = const { RefCell::new(None) };
}

pub fn with_live_render_assets<R>(render_assets: &mut RenderAssets, f: impl FnOnce() -> R) -> R {
    LIVE_RENDER_ASSETS.with(|slot| {
        let prev = slot.replace(Some(render_assets as *mut RenderAssets));
        let result = f();
        let _ = slot.replace(prev);
        result
    })
}

fn with_render_assets_mut<R>(
    f: impl FnOnce(&mut RenderAssets) -> Result<R, String>,
) -> Result<R, String> {
    LIVE_RENDER_ASSETS.with(|slot| {
        let ptr = (*slot.borrow()).ok_or_else(|| {
            "procedural Renderable constructors require live RenderAssets".to_string()
        })?;
        // SAFETY: callers install the pointer for the duration of spawn/materialization on the
        // main thread, and no nested aliasing mutable access to the same RenderAssets occurs.
        unsafe { f(&mut *ptr) }
    })
}

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

    if let Some(block) = &ce.deferred_block {
        if let Some(keyframe) = world.get_component_by_id_as_mut::<KeyframeComponent>(id) {
            keyframe.callback = Some(block.clone());
        }
    }

    // Extra ctor calls + body builder calls (already evaluated).
    for (method, args) in &ce.calls {
        apply_call(world, id, method, args)?;
    }

    // Named property assignments — intercept node-level fields first.
    for (prop, val) in &ce.named {
        match prop.as_str() {
            "name" | "id" => {
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
                            node.classes = arr
                                .iter()
                                .filter_map(|v| {
                                    if let Value::String(s) = v {
                                        Some(s.clone())
                                    } else {
                                        None
                                    }
                                })
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
                    return Err(format!(
                        "attach existing child {:?} failed: {e}",
                        existing_id
                    ));
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

    if let Some(block) = &ce.deferred_block {
        if let Some(keyframe) = world.get_component_by_id_as_mut::<KeyframeComponent>(id) {
            keyframe.callback = Some(block.clone());
        }
    }

    for (method, args) in &ce.calls {
        apply_call(world, id, method, args)?;
    }

    for (prop, val) in &ce.named {
        match prop.as_str() {
            "name" | "id" => {
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
                            node.classes = arr
                                .iter()
                                .filter_map(|v| {
                                    if let Value::String(s) = v {
                                        Some(s.clone())
                                    } else {
                                        None
                                    }
                                })
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
                    return Err(format!(
                        "attach existing child {:?} failed: {e}",
                        existing_id
                    ));
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
    if raw
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
    {
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
    subtree_to_ce_ast_limited(world, root, usize::MAX)
}

pub fn filtered_world_to_ce_ast(world: &World) -> Result<Vec<ComponentExpression>, String> {
    let roots: Vec<ComponentId> = world
        .all_components()
        .filter(|&cid| world.parent_of(cid).is_none())
        .collect();
    filtered_roots_to_ce_ast(world, &roots)
}

pub fn filtered_world_root_ids(world: &World) -> Vec<ComponentId> {
    let roots: Vec<ComponentId> = world
        .all_components()
        .filter(|&cid| world.parent_of(cid).is_none())
        .collect();
    filtered_root_ids_for_roots(world, &roots)
}

pub fn filtered_roots_to_ce_ast(
    world: &World,
    roots: &[ComponentId],
) -> Result<Vec<ComponentExpression>, String> {
    let mut referenced_guids: std::collections::HashSet<uuid::Uuid> =
        std::collections::HashSet::new();
    for &root in roots {
        collect_referenced_guids_filtered(world, root, &mut referenced_guids);
    }

    let mut out = Vec::new();
    for &root in roots {
        out.extend(filtered_ce_ast_inner(world, root, &referenced_guids)?);
    }
    Ok(out)
}

pub fn filtered_root_ids_for_roots(world: &World, roots: &[ComponentId]) -> Vec<ComponentId> {
    let mut out = Vec::new();
    for &root in roots {
        collect_filtered_root_ids(world, root, &mut out);
    }
    out
}

fn immediate_child_serialize_override(world: &World, node: ComponentId) -> Option<bool> {
    world.children_of(node).iter().find_map(|&child| {
        world
            .get_component_by_id_as::<SerializeComponent>(child)
            .map(|serialize| serialize.enabled)
    })
}

fn nearest_serialize_override(world: &World, node: ComponentId) -> Option<bool> {
    let mut current = Some(node);
    while let Some(component_id) = current {
        if let Some(enabled) = immediate_child_serialize_override(world, component_id) {
            return Some(enabled);
        }
        current = world.parent_of(component_id);
    }
    None
}

fn filtered_save_visibility(world: &World, node: ComponentId) -> bool {
    nearest_serialize_override(world, node).unwrap_or(true)
}

fn collect_filtered_root_ids(world: &World, node: ComponentId, out: &mut Vec<ComponentId>) {
    let visible = filtered_save_visibility(world, node);
    if visible {
        out.push(node);
        return;
    }

    let children: Vec<ComponentId> = world.children_of(node).to_vec();
    for child in children {
        collect_filtered_root_ids(world, child, out);
    }
}

fn collect_referenced_guids_filtered(
    world: &World,
    node: ComponentId,
    out: &mut std::collections::HashSet<uuid::Uuid>,
) {
    use crate::engine::ecs::component::{
        ActionComponent, ComponentRef, IKChainComponent, TransformParentComponent,
    };

    let visible = filtered_save_visibility(world, node);
    if visible {
        if let Some(action) = world.get_component_by_id_as::<ActionComponent>(node) {
            for src in &action.target_sources {
                if let ComponentRef::Guid(u) = src {
                    out.insert(*u);
                }
            }
        }
        if let Some(ik) = world.get_component_by_id_as::<IKChainComponent>(node) {
            for src in [&ik.target_source, &ik.end_effector_source]
                .iter()
                .copied()
                .flatten()
            {
                if let ComponentRef::Guid(u) = src {
                    out.insert(*u);
                }
            }
        }
        if let Some(tp) = world.get_component_by_id_as::<TransformParentComponent>(node) {
            for src in [&tp.target_source, &tp.root_source]
                .iter()
                .copied()
                .flatten()
            {
                if let ComponentRef::Guid(u) = src {
                    out.insert(*u);
                }
            }
        }
    }

    let children: Vec<ComponentId> = world.children_of(node).to_vec();
    for child in children {
        collect_referenced_guids_filtered(world, child, out);
    }
}

fn filtered_ce_ast_inner(
    world: &World,
    root: ComponentId,
    referenced_guids: &std::collections::HashSet<uuid::Uuid>,
) -> Result<Vec<ComponentExpression>, String> {
    let node = world
        .get_component_record(root)
        .ok_or_else(|| format!("filtered_ce_ast_inner: missing component {root:?}"))?;
    let visible = filtered_save_visibility(world, root);

    let mut child_components = Vec::new();
    for &child_id in &node.children {
        child_components.extend(filtered_ce_ast_inner(world, child_id, referenced_guids)?);
    }

    if !visible {
        return Ok(child_components);
    }

    let mut ce = node.component.to_mms_ast(world);

    if !node.name.is_empty() {
        ce.body.statements.push(Statement::Reassign {
            target: Expression::Identifier(crate::meow_meow::ast::Ident("name".to_string())),
            value: Expression::String(node.name.clone()),
        });
    }

    if referenced_guids.contains(&node.guid) {
        ce.body.statements.push(Statement::Reassign {
            target: Expression::Identifier(crate::meow_meow::ast::Ident("guid".to_string())),
            value: Expression::String(node.guid.to_string()),
        });
    }

    for child_ce in child_components {
        ce.body
            .statements
            .push(Statement::Expression(Expression::Component(child_ce)));
    }

    Ok(vec![ce])
}

pub fn subtree_to_ce_ast_limited(
    world: &World,
    root: ComponentId,
    max_depth: usize,
) -> Result<ComponentExpression, String> {
    // First pass: collect every GUID referenced by any ActionComponent in
    // the subtree via `ComponentRef::Guid`. These are the targets that
    // need their GUID preserved across save/load so the dumped
    // `@uuid:<g>` selector still resolves on reload.
    let mut referenced_guids: std::collections::HashSet<uuid::Uuid> =
        std::collections::HashSet::new();
    collect_referenced_guids_limited(world, root, 0, max_depth, &mut referenced_guids);

    subtree_to_ce_ast_inner_limited(world, root, &referenced_guids, 0, max_depth)
}

fn collect_referenced_guids_limited(
    world: &World,
    node: ComponentId,
    depth: usize,
    max_depth: usize,
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
        for src in [&ik.target_source, &ik.end_effector_source]
            .iter()
            .copied()
            .flatten()
        {
            if let ComponentRef::Guid(u) = src {
                out.insert(*u);
            }
        }
    }
    if let Some(tp) = world.get_component_by_id_as::<TransformParentComponent>(node) {
        for src in [&tp.target_source, &tp.root_source]
            .iter()
            .copied()
            .flatten()
        {
            if let ComponentRef::Guid(u) = src {
                out.insert(*u);
            }
        }
    }
    let children: Vec<ComponentId> = world
        .get_component_record(node)
        .map(|n| n.children.clone())
        .unwrap_or_default();
    if depth >= max_depth {
        return;
    }
    for child in children {
        collect_referenced_guids_limited(world, child, depth + 1, max_depth, out);
    }
}

fn subtree_to_ce_ast_inner_limited(
    world: &World,
    root: ComponentId,
    referenced_guids: &std::collections::HashSet<uuid::Uuid>,
    depth: usize,
    max_depth: usize,
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
            target: Expression::Identifier(crate::meow_meow::ast::Ident("name".to_string())),
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
            target: Expression::Identifier(crate::meow_meow::ast::Ident("guid".to_string())),
            value: Expression::String(node.guid.to_string()),
        });
    }

    let children: Vec<ComponentId> = node.children.clone();
    if depth >= max_depth {
        return Ok(ce);
    }
    for child_id in children {
        let child_ce = subtree_to_ce_ast_inner_limited(
            world,
            child_id,
            referenced_guids,
            depth + 1,
            max_depth,
        )?;
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
    let component_property_assignment_only =
        component_expr_uses_property_assignment_only(&ce.component_type.0);
    let is_keyframe = matches!(ce.component_type.0.as_str(), "KF" | "Keyframe");
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

    if is_keyframe {
        return Ok(MaterializedCE {
            component_type: ce.component_type.0.clone(),
            component_property_assignment_only,
            ctor_method,
            ctor_args,
            calls,
            named: Vec::new(),
            positionals: Vec::new(),
            deferred_block: Some(crate::meow_meow::object::RuntimeClosure {
                body: ce.body.clone(),
                captured_env: std::sync::Arc::new(std::collections::HashMap::new()),
                analysis: Some(
                    crate::meow_meow::block_effect_analyzer::BlockEffectAnalyzer::analyze_keyframe_block(&ce.body),
                ),
            }),
            children: Vec::new(),
        });
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
            Statement::Reassign { target, value } => {
                let Expression::Identifier(name) = target else {
                    continue;
                };
                if component_property_assignment_only || is_universal_component_named_prop(&name.0)
                {
                    // Named-prop in a property-bag CE body, e.g. `row_name = "hero"`.
                    // The full evaluator handles this via builder.named.push in
                    // evaluator.rs; replicate the same mapping here so the
                    // ground-CE dump path preserves named props on round-trip.
                    let val = expression_to_value(value)?;
                    named.push((name.0.clone(), val));
                }
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
        component_property_assignment_only,
        ctor_method,
        ctor_args,
        calls,
        named,
        positionals: Vec::new(),
        deferred_block: None,
        children,
    })
}

pub fn component_expr_uses_property_assignment_only(raw: &str) -> bool {
    matches!(
        resolve_type_name(raw).as_str(),
        "Data" | "DataComponent" | "Style" | "StyleComponent"
    )
}

pub fn is_universal_component_named_prop(name: &str) -> bool {
    matches!(name, "name" | "id" | "guid" | "class")
}

fn expression_to_value(e: &Expression) -> Result<Value, String> {
    match e {
        Expression::Number(n) => Ok(Value::Number(*n)),
        Expression::String(s) => Ok(Value::String(s.clone())),
        Expression::Bool(b) => Ok(Value::Bool(*b)),
        Expression::Null => Ok(Value::Null),
        Expression::Dimension(n, u) => Ok(Value::Dimension {
            value: *n,
            unit: *u,
        }),
        Expression::Identifier(Ident(s)) => Ok(Value::Identifier(s.clone())),
        Expression::Array(items) => {
            let vals: Vec<Value> = items
                .iter()
                .map(expression_to_value)
                .collect::<Result<_, _>>()?;
            Ok(Value::Array(vals))
        }
        Expression::Table(_) => {
            Err("expression_to_value: table literals are not supported yet".into())
        }
        Expression::Index { base, index } => {
            let base = expression_to_value(base)?;
            let index = expression_to_value(index)?;
            let Value::Array(items) = base else {
                return Err(format!("index: expected array, got {:?}", base));
            };
            let Value::Number(n) = index else {
                return Err(format!("index: expected numeric index, got {:?}", index));
            };
            if n.fract() != 0.0 || n < 0.0 {
                return Err(format!("index: expected non-negative integer, got {n}"));
            }
            items
                .get(n as usize)
                .cloned()
                .ok_or_else(|| format!("index: {n} out of bounds for array of {}", items.len()))
        }
        Expression::UnaryOp {
            op: UnaryOpKind::Neg,
            operand,
        } => match expression_to_value(operand)? {
            Value::Number(n) => Ok(Value::Number(-n)),
            Value::Dimension { value, unit } => Ok(Value::Dimension {
                value: -value,
                unit,
            }),
            v => Err(format!("cannot negate value: {v:?}")),
        },
        Expression::Component(child_ce) => {
            let m = ce_ast_to_materialized(child_ce)?;
            Ok(Value::ComponentExpr(Box::new(m)))
        }
        other => Err(format!(
            "expression_to_value: unsupported expression {other:?}"
        )),
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

fn val_as_world_f32(v: &Value) -> Result<f32, String> {
    use crate::meow_meow::token::Unit;
    match v {
        Value::Number(n) => Ok(*n as f32),
        Value::Dimension {
            value,
            unit: Unit::WorldUnits,
        } => Ok(*value as f32),
        Value::Dimension { unit, .. } => {
            Err(format!("expected number or wu dimension, got {:?}", unit))
        }
        other => Err(format!("expected number or wu dimension, got {other:?}")),
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
    args.get(i)
        .ok_or_else(|| format!("expected at least {} arg(s), got {}", i + 1, args.len()))
}

fn arg_f32(args: &[Value], i: usize) -> Result<f32, String> {
    val_as_f32(arg(args, i)?)
}
fn arg_u32(args: &[Value], i: usize) -> Result<u32, String> {
    match arg(args, i)? {
        Value::Number(n) if *n >= 0.0 && *n <= u32::MAX as f64 && n.fract() == 0.0 => Ok(*n as u32),
        other => Err(format!(
            "arg {i}: expected non-negative integer, got {other:?}"
        )),
    }
}
fn arg_world_f32(args: &[Value], i: usize) -> Result<f32, String> {
    val_as_world_f32(arg(args, i)?)
}
fn arg_bool(args: &[Value], i: usize) -> Result<bool, String> {
    val_as_bool(arg(args, i)?)
}
fn arg_str(args: &[Value], i: usize) -> Result<&str, String> {
    val_as_str(arg(args, i)?)
}
fn arg_f32_arr<const N: usize>(args: &[Value], i: usize) -> Result<[f32; N], String> {
    val_as_f32_array(arg(args, i)?)
}
fn arg_str_vec(args: &[Value], i: usize) -> Result<Vec<String>, String> {
    val_as_str_vec(arg(args, i)?)
}

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
        Value::Array(items) => items
            .iter()
            .map(|v| value_to_component_ref(world, v))
            .collect(),
        other => value_to_component_ref(world, other).map(|t| vec![t]),
    }
}

/// Handle `guid = "8c4f3e72-..."` on a component CE. Replaces the freshly
/// minted GUID with the authored one so `@uuid:` selectors saved against
/// this component still resolve across save/load.
fn apply_guid_named_prop(world: &mut World, id: ComponentId, val: &Value) -> Result<(), String> {
    let s = val_as_str(val).map_err(|e| format!("guid prop: {e}"))?;
    let parsed =
        uuid::Uuid::parse_str(s).map_err(|e| format!("guid prop: invalid uuid '{s}': {e}"))?;
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

/// Accept either a unit-literal (`50%`, `20gu`, `0.08wu`) or a bare number
/// (interpreted as glyph units) and produce a `SizeDimension`. Used by Style
/// sizing setters.
fn arg_size_dimension(args: &[Value], i: usize) -> Result<SizeDimension, String> {
    use crate::meow_meow::token::Unit;
    match arg(args, i)? {
        Value::Number(n) => Ok(SizeDimension::GlyphUnits(*n as f32)),
        Value::Dimension { value, unit } => match unit {
            Unit::Percent => Ok(SizeDimension::Percent(*value as f32)),
            Unit::GlyphUnits => Ok(SizeDimension::GlyphUnits(*value as f32)),
            Unit::WorldUnits => Ok(SizeDimension::WorldUnits(*value as f32)),
            Unit::Degrees | Unit::Radians => Err(format!(
                "expected length unit (gu, wu, %) for size, got {:?}",
                unit
            )),
        },
        v => Err(format!(
            "expected number or dimension for size, got {:?}",
            v
        )),
    }
}

fn arg_layout_length(args: &[Value], i: usize) -> Result<SizeDimension, String> {
    match arg_size_dimension(args, i)? {
        SizeDimension::GlyphUnits(v) => Ok(SizeDimension::GlyphUnits(v)),
        SizeDimension::WorldUnits(v) => Ok(SizeDimension::WorldUnits(v)),
        SizeDimension::Percent(_) => {
            Err("expected gu or wu length for LayoutRoot size, got percent".into())
        }
        SizeDimension::Auto => Err("expected gu or wu length for LayoutRoot size, got auto".into()),
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
        "FitBounds" => {
            let mut c = FitBoundsComponent::new();
            if let Some(method) = ctor {
                apply_fit_bounds_ctor(&mut c, method, args)?;
            }
            add!(c)
        }
        "LayoutBounds" => {
            let mut c = LayoutBoundsComponent::new(
                crate::engine::graphics::bounds::Aabb {
                    min: [0.0, 0.0, 0.0],
                    max: [0.0, 0.0, 0.0],
                },
                crate::engine::graphics::bounds::Aabb {
                    min: [0.0, 0.0, 0.0],
                    max: [0.0, 0.0, 0.0],
                },
            );
            if let Some(method) = ctor {
                apply_layout_bounds_ctor(&mut c, method, args)?;
            }
            add!(c)
        }
        "Color" => match ctor {
            Some("rgba") => add!(ColorComponent::rgba(
                arg_f32(args, 0)?,
                arg_f32(args, 1)?,
                arg_f32(args, 2)?,
                arg_f32(args, 3)?
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
            Some("partial_annulus_2d") => with_render_assets_mut(|render_assets| {
                let inner_radius = arg_f32(args, 0).unwrap_or(0.28);
                let outer_radius = arg_f32(args, 1).unwrap_or(0.52);
                let start_angle = arg_f32(args, 2).unwrap_or(0.0);
                let end_angle = arg_f32(args, 3).unwrap_or(4.71239);
                let segments = arg_u32(args, 4).unwrap_or(64);
                Ok(world.add_component(RenderableComponent::partial_annulus_2d(
                    render_assets,
                    inner_radius,
                    outer_radius,
                    start_angle,
                    end_angle,
                    segments,
                )))
            }),
            Some("star") => with_render_assets_mut(|render_assets| {
                let points = arg_u32(args, 0).unwrap_or(5);
                let inner_radius = arg_f32(args, 1).unwrap_or(0.45);
                let skip = arg_u32(args, 2).unwrap_or(2);
                let phase = arg_u32(args, 3).unwrap_or(1);
                Ok(world.add_component(RenderableComponent::star(
                    render_assets,
                    points,
                    inner_radius,
                    skip,
                    phase,
                )))
            }),
            Some("heart") => with_render_assets_mut(|render_assets| {
                let segments = arg_u32(args, 0).unwrap_or(64);
                Ok(world.add_component(RenderableComponent::heart(render_assets, segments)))
            }),
            _ => Err(format!(
                "Renderable: unknown constructor '{}'",
                ctor.unwrap_or("")
            )),
        },
        "Grid" => {
            let mut c = GridComponent::default();
            if let Some(method) = ctor {
                match method {
                    "spacing" => c = c.with_spacing(arg_f32(args, 0)?),
                    "size_x" => c = c.with_size_x(arg_f32(args, 0)? as u32),
                    "size_z" => c = c.with_size_z(arg_f32(args, 0)? as u32),
                    "hidden" => c = c.with_hidden(arg_bool(args, 0)?),
                    _ => return Err(format!("Grid: unknown constructor '{method}'")),
                }
            }
            add!(c)
        }
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
        "InspectLayout" => add!(InspectLayoutComponent::new()),
        "BackgroundColor" => add!(BackgroundColorComponent::new()),
        "AmbientLight" => match ctor {
            Some("rgb") => add!(AmbientLightComponent::rgb(
                arg_f32(args, 0)?,
                arg_f32(args, 1)?,
                arg_f32(args, 2)?
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
        "InputXR" | "InputVR" => match ctor {
            Some("on") => add!(InputXRComponent::on()),
            Some("off") => add!(InputXRComponent::off()),
            _ => add!(InputXRComponent::on()),
        },
        "InputXRGamepad" | "InputXrGamepad" | "InputVRGamepad" | "InputVrGamepad" => {
            let id = world.add_component(InputXRGamepadComponent::new());
            if let Some(method) = ctor {
                if method != "new" {
                    apply_call(world, id, method, args)?;
                }
            }
            Ok(id)
        }
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
        "XR" | "VR" | "OpenXR" => match ctor {
            Some("off") => add!(XrComponent::off()),
            Some("on") | Some("auto") | Some("openxr") => add!(XrComponent::on()),
            _ => add!(XrComponent::on()),
        },
        "XRHand" | "XrHand" | "VRHand" | "VrHand" => match ctor {
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
                add!(XRHandComponent::new(true, hand, pose))
            }
            _ => Err("XRHand requires .new(enabled, hand, pose)".into()),
        },
        "TransformParent" => match ctor {
            Some("target") => add!(
                TransformParentComponent::new()
                    .with_target_source(arg_component_ref(world, args, 0)?)
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
        "TextInput" => add!(TextInputComponent::new("")),
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
        "AssetPayload" => {
            let id = world.add_component(match ctor {
                Some("new") => AssetPayloadComponent::new(arg_str(args, 0)?, arg_str(args, 1)?),
                _ => AssetPayloadComponent::new("", ""),
            });
            if let Some(method) = ctor {
                if method != "new" {
                    apply_call(world, id, method, args)?;
                }
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
        "Selection" => {
            let id = world.add_component(match ctor {
                Some("multiple") => SelectionComponent::multiple(),
                Some("optional") => SelectionComponent::optional(),
                _ => SelectionComponent::new(),
            });
            if let Some(method) = ctor {
                if method != "multiple" && method != "optional" {
                    apply_call(world, id, method, args)?;
                }
            }
            Ok(id)
        }
        "Option" => add!(OptionComponent::new()),
        "ObserverRouter" => {
            let id = world.add_component(SignalObserverRouterComponent::new());
            if let Some(method) = ctor {
                apply_call(world, id, method, args)?;
            }
            Ok(id)
        }
        "Serialize" => match ctor {
            Some("off") => add!(SerializeComponent::off()),
            _ => add!(SerializeComponent::on()),
        },
        "Scrolling" => match ctor {
            Some("new") => add!(ScrollingComponent::new(
                arg_f32(args, 0)?,
                arg_f32(args, 1)?
            )),
            _ => add!(ScrollingComponent::new(0.1, 0.1)),
        },
        "HtmlElement" => {
            let c = match ctor {
                Some("div") => HtmlElementComponent::div(),
                Some("span") => HtmlElementComponent::span(),
                Some("body") => HtmlElementComponent::body(),
                Some("header") => HtmlElementComponent::header(),
                Some("p") => HtmlElementComponent::p(),
                Some("section") => HtmlElementComponent::new(ElementType::Section),
                Some("article") => HtmlElementComponent::new(ElementType::Article),
                Some("footer") => HtmlElementComponent::new(ElementType::Footer),
                Some("nav") => HtmlElementComponent::new(ElementType::Nav),
                Some("aside") => HtmlElementComponent::new(ElementType::Aside),
                Some("main") => HtmlElementComponent::new(ElementType::Main),
                Some("h1") => HtmlElementComponent::new(ElementType::H1),
                Some("h2") => HtmlElementComponent::new(ElementType::H2),
                Some("h3") => HtmlElementComponent::new(ElementType::H3),
                Some("h4") => HtmlElementComponent::new(ElementType::H4),
                Some("h5") => HtmlElementComponent::new(ElementType::H5),
                Some("h6") => HtmlElementComponent::new(ElementType::H6),
                _ => HtmlElementComponent::new(ElementType::Element),
            };
            add!(c)
        }
        "Style" => add!(StyleComponent::new()),
        "LayoutRoot" => {
            let mut layout = LayoutComponent::new(80.0);
            if !args.is_empty() {
                layout.set_available_width_dimension(arg_layout_length(args, 0)?);
            }
            add!(layout)
        }
        "Raycastable" => match ctor {
            Some("disabled") => add!(RaycastableComponent::disabled()),
            Some("drag_only") => add!(RaycastableComponent::drag_only()),
            Some("click_only") => add!(RaycastableComponent::click_only()),
            Some("enabled") => add!(RaycastableComponent::enabled()),
            _ => add!(RaycastableComponent::enabled()),
        },
        "PoseCapture" => add!(PoseCaptureComponent::new()),
        "PoseCaptureLibrary" => {
            add!(PoseCaptureLibraryComponent::new(
                crate::engine::ecs::component::PoseTargetRef::Query("TODO".to_string())
            ))
        }
        "PoseCapturePose" => {
            add!(PoseCapturePoseComponent::new(
                arg_str(args, 0)?,
                crate::engine::ecs::component::PoseTargetRef::Query("TODO".to_string()),
                Vec::new()
            ))
        }
        "TextureFiltering" => match ctor {
            Some("linear") => add!(TextureFilteringComponent::linear()),
            Some("nearest_magnification") => {
                add!(TextureFilteringComponent::nearest_magnification())
            }
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
            let mut c = AnimationComponent::new();
            match ctor {
                Some("playing") => c = c.with_state(AnimationState::Playing),
                Some("paused") => c = c.with_state(AnimationState::Paused),
                Some("looping") => c = c.with_state(AnimationState::Looping),
                Some("length") => c = c.with_length_beats(arg_f32(args, 0)? as f64),
                Some("scope") => c = c.with_scope_source(arg_component_ref(world, args, 0)?),
                _ => {}
            }
            add!(c)
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
                    let signal = IV::SetColor {
                        component_ids: null_ids(targets.len()),
                        rgba,
                    };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("set_text") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let text = arg_str(args, 1)?.to_string();
                    let signal = IV::SetText {
                        component_ids: null_ids(targets.len()),
                        text,
                    };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("set_emissive_intensity") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let intensity = arg_f32(args, 1)?.max(0.0);
                    let signal = IV::SetEmissiveIntensity {
                        component_ids: null_ids(targets.len()),
                        intensity,
                    };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("set_position") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let position = arg_f32_arr::<3>(args, 1)?;
                    let signal = IV::SetPosition {
                        component_ids: null_ids(targets.len()),
                        position,
                    };
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
                    let signal = IV::Detach {
                        component_ids: null_ids(targets.len()),
                    };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("remove_subtree") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let signal = IV::RemoveSubtree {
                        component_ids: null_ids(targets.len()),
                    };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("request_raycast") => {
                    let targets = arg_component_ref_vec(world, args, 0)?;
                    let signal = IV::RequestRaycast {
                        component_ids: null_ids(targets.len()),
                    };
                    add!(ActionComponent::new_authored(signal, targets))
                }
                Some("update_transform") => {
                    let target = arg_component_ref(world, args, 0)?;
                    let translation = arg_f32_arr::<3>(args, 1)?;
                    let rotation_euler = arg_f32_arr::<3>(args, 2)?;
                    let scale = arg_f32_arr::<3>(args, 3)?;
                    let transform = TransformComponent::new()
                        .with_position(translation[0], translation[1], translation[2])
                        .with_rotation_euler(
                            rotation_euler[0],
                            rotation_euler[1],
                            rotation_euler[2],
                        )
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
        }
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
        "Mirror" => {
            let mut c = MirrorComponent::default();
            if let Some("quality") = ctor {
                c = MirrorComponent::new(arg_f32(args, 0)? as i32);
            }
            add!(c)
        }
        "GestureCoordType" => match ctor {
            Some("screen_space_1d_slider") => {
                add!(GestureCoordTypeComponent::screen_space_1d_slider())
            }
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
            Some("sphere") => add!(CollisionShapeComponent::new(CollisionShape::sphere_radius(
                arg_f32(args, 0)?
            ))),
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
            Some("new") => add!(QuatYawFollowComponent::new(
                arg_f32(args, 0)?,
                arg_f32(args, 1)?
            )),
            _ => add!(QuatYawFollowComponent::default()),
        },
        "SignalRouteUpward" => match ctor {
            Some("new") => add!(SignalRouteUpwardComponent::new(
                arg_str(args, 0)?,
                arg_str(args, 1)?
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
                Some("a") | Some("b") | Some("c") | Some("d") | Some("e") | Some("f")
                | Some("g") => ctor.unwrap(),
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
            let mut mn = MusicNoteComponent::new(note);
            // Optional 3rd positional arg: target audio source ref/query.
            if let Ok(v) = arg(args, 2) {
                if let Ok(src) = value_to_component_ref(world, v) {
                    mn.target_source = Some(src);
                }
            }
            add!(mn)
        }
        "AudioOutput" => {
            add!(AudioOutputComponent::new())
        }
        "AudioOscillator" => {
            let kind = match ctor {
                Some("sin") => OscillatorType::Sin,
                Some("triangle") => OscillatorType::Triangle,
                Some("square") => OscillatorType::Square,
                Some("square_3") => OscillatorType::Square3,
                Some("saw") => OscillatorType::Saw,
                Some("noise") => OscillatorType::Noise,
                Some("drum") => OscillatorType::Drum,
                _ => OscillatorType::Sin,
            };
            add!(AudioOscillatorComponent::single(AudioOscillator::new(kind)))
        }
        "AudioClip" => {
            // Cloning is a *method* on a live AudioClip handle, not a
            // ctor — see `eval_method_call` + `HostCallKind::AudioClipInstance`.
            //
            // Constructor variants accept the URI as positional arg 0. The
            // ctor name (`wav`/`opus`/`ogg`/`mp3`/`flac`/`new`) is purely
            // documentation — codec is detected from the file by the
            // decode pipeline. `latched`/`one_shot` flip the trigger mode.
            let uri = arg_str(args, 0).unwrap_or("").to_string();
            let mut c = AudioClipComponent::new(uri);
            match ctor {
                Some("one_shot") => c = c.with_trigger_mode(AudioTriggerMode::OneShot),
                Some("latched") => c = c.with_trigger_mode(AudioTriggerMode::Latched),
                _ => {}
            }
            add!(c)
        }
        "IKChain" => {
            let solver = match ctor {
                Some("aim_constraint") => IKSolver::AimConstraint {
                    offset_yaw: arg_f32(args, 0)?,
                    copy_position: arg_bool(args, 1).unwrap_or(false),
                    target_position_offset: arg_f32_arr::<3>(args, 2).unwrap_or([0.0, 0.0, 0.0]),
                },
                Some("two_bone_ik") => {
                    use slotmap::Key;
                    IKSolver::TwoBoneIK {
                        // root_joint_id and mid_joint_id are runtime-wired by
                        // AvatarControlSystem; MMS-authored TwoBoneIK chains are
                        // not currently supported (would need name resolution).
                        root_joint_id: ComponentId::null(),
                        mid_joint_id: ComponentId::null(),
                        pole_direction: arg_f32_arr::<3>(args, 0)?,
                        copy_end_rotation: arg_bool(args, 1)?,
                    }
                }
                Some("fabrik") => IKSolver::Fabrik {
                    max_iterations: arg_f32(args, 0)? as u32,
                    tolerance: arg_f32(args, 1)?,
                    target_position_offset: arg_f32_arr::<3>(args, 2).unwrap_or([0.0, 0.0, 0.0]),
                },
                _ => IKSolver::AimConstraint {
                    offset_yaw: 0.0,
                    copy_position: false,
                    target_position_offset: [0.0, 0.0, 0.0],
                },
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
        "TransformGizmoTranslate" => add!(TransformGizmoTranslateComponent::new(parse_gizmo_axis(
            ctor
        ))),
        "TransformGizmoRotate" => add!(TransformGizmoRotateComponent::new(parse_gizmo_axis(ctor))),
        "TransformGizmoScale" => add!(TransformGizmoScaleComponent::new(parse_gizmo_axis(ctor))),
        "KineticResponse" => {
            let c = match ctor {
                Some("push") => KineticResponseComponent::push(),
                Some("slide") => KineticResponseComponent::slide(),
                _ => KineticResponseComponent::slide(),
            };
            let id = world.add_component(c);
            Ok(id)
        }
        "Data" => {
            add!(DataComponent::new())
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
                style.background_z = Some(val_as_f32(val)?);
                return Ok(());
            }
            "color" => {
                style.color = Some(val_as_f32_array::<4>(val)?);
                return Ok(());
            }
            _ => {}
        }
    }

    if let Some(data) = world.get_component_by_id_as_mut::<DataComponent>(id) {
        data.insert(
            name.to_string(),
            match val {
                Value::String(s) => DataValue::Text(s.clone()),
                Value::Bool(b) => DataValue::Bool(*b),
                Value::Number(n) => DataValue::Integer(*n as i64),
                _ => DataValue::Text(format!("{val:?}")),
            },
        );
        return Ok(());
    }

    if name == "root"
        && world
            .get_component_by_id_as::<SelectionComponent>(id)
            .is_some()
    {
        let src = value_to_component_ref(world, val)?;
        if let Some(selection) = world.get_component_by_id_as_mut::<SelectionComponent>(id) {
            selection.target_root_source = Some(src);
        }
        return Ok(());
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
    if let Some(fit_bounds) = world.get_component_by_id_as_mut::<FitBoundsComponent>(id) {
        match method {
            "renderable_only" => fit_bounds.mode = FitBoundsMode::RenderableOnly,
            "layout_aware" => fit_bounds.mode = FitBoundsMode::LayoutAware,
            "to" => {
                fit_bounds.target = FitBoundsTarget::ExplicitBounds;
                fit_bounds.target_bounds = val_as_f32_array::<6>(&Value::Array(args.to_vec()))?;
            }
            "to_container" => fit_bounds.target = FitBoundsTarget::ParentPaddingBox,
            _ => {}
        }
        return Ok(());
    }

    if let Some(layout_bounds) = world.get_component_by_id_as_mut::<LayoutBoundsComponent>(id) {
        match method {
            "content_box" => {
                layout_bounds.content_local = crate::engine::graphics::bounds::Aabb {
                    min: val_as_f32_array::<3>(&args[0])?,
                    max: val_as_f32_array::<3>(&args[1])?,
                }
            }
            "padding_box" => {
                layout_bounds.padding_local = crate::engine::graphics::bounds::Aabb {
                    min: val_as_f32_array::<3>(&args[0])?,
                    max: val_as_f32_array::<3>(&args[1])?,
                }
            }
            _ => {}
        }
        return Ok(());
    }

    if let Some(pc) = world.get_component_by_id_as_mut::<PoseCaptureComponent>(id) {
        match method {
            "with_label" | "label" => pc.label = Some(arg_str(args, 0)?.to_string()),
            _ => {}
        }
        return Ok(());
    }

    // Transform builders
    if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(id) {
        match method {
            "position" => {
                *t = t.clone().with_position(
                    arg_world_f32(args, 0)?,
                    arg_world_f32(args, 1)?,
                    arg_world_f32(args, 2)?,
                )
            }
            "scale" => {
                *t = t.clone().with_scale(
                    arg_world_f32(args, 0)?,
                    arg_world_f32(args, 1)?,
                    arg_world_f32(args, 2)?,
                )
            }
            "rotation" | "rotation_euler" => {
                *t = t.clone().with_rotation_euler(
                    arg_f32(args, 0)?,
                    arg_f32(args, 1)?,
                    arg_f32(args, 2)?,
                )
            }
            "rotation_quat" => {
                *t = t.clone().with_rotation_quat([
                    arg_f32(args, 0)?,
                    arg_f32(args, 1)?,
                    arg_f32(args, 2)?,
                    arg_f32(args, 3)?,
                ])
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(l) = world.get_component_by_id_as_mut::<LayoutComponent>(id) {
        match method {
            "width" | "available_width" => {
                l.set_available_width_dimension(arg_layout_length(args, 0)?);
            }
            "height" | "available_height" => {
                l.set_available_height_dimension(arg_layout_length(args, 0)?);
            }
            "unit_scale" => l.set_unit_scale(arg_f32(args, 0)?),
            _ => {}
        }
        return Ok(());
    }
    if let Some(text_input) = world.get_component_by_id_as_mut::<TextInputComponent>(id) {
        match method {
            "read_only" => text_input.read_only = arg_bool(args, 0)?,
            _ => {}
        }
        return Ok(());
    }
    if let Some(asset_payload) = world.get_component_by_id_as_mut::<AssetPayloadComponent>(id) {
        match method {
            "asset_key" => asset_payload.asset_key = arg_str(args, 0)?.to_string(),
            "title" => asset_payload.title = arg_str(args, 0)?.to_string(),
            _ => {}
        }
        return Ok(());
    }
    if world
        .get_component_by_id_as::<SelectionComponent>(id)
        .is_some()
    {
        match method {
            "optional" => {
                if let Some(selection) = world.get_component_by_id_as_mut::<SelectionComponent>(id)
                {
                    selection.allow_empty_single = true;
                }
            }
            "root" => {
                let src = arg_component_ref(world, args, 0)?;
                if let Some(selection) = world.get_component_by_id_as_mut::<SelectionComponent>(id)
                {
                    selection.target_root_source = Some(src);
                }
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(router) = world.get_component_by_id_as_mut::<SignalObserverRouterComponent>(id) {
        match method {
            "blacklist" => {
                router.blacklist = arg_str_vec(args, 0)?;
            }
            "whitelist" => {
                router.whitelist = arg_str_vec(args, 0)?;
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(dl) = world.get_component_by_id_as_mut::<DirectionalLightComponent>(id) {
        match method {
            "intensity" => *dl = dl.clone().with_intensity(arg_f32(args, 0)?),
            "color" => {
                *dl = dl
                    .clone()
                    .with_color(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?)
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(pl) = world.get_component_by_id_as_mut::<PointLightComponent>(id) {
        match method {
            "intensity" => *pl = pl.clone().with_intensity(arg_f32(args, 0)?),
            "distance" => *pl = pl.clone().with_distance(arg_f32(args, 0)?),
            "color" => {
                *pl = pl
                    .clone()
                    .with_color(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?)
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(render_graph) = world.get_component_by_id_as_mut::<RenderGraphComponent>(id) {
        match method {
            "on" => *render_graph = render_graph.clone().with_enabled(true),
            "off" => *render_graph = render_graph.clone().with_enabled(false),
            "enabled" => *render_graph = render_graph.clone().with_enabled(arg_bool(args, 0)?),
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
            "radius_ndc" => *bloom = bloom.clone().with_radius_ndc(arg_f32(args, 0)?),
            "emissive_scale" => *bloom = bloom.clone().with_emissive_scale(arg_f32(args, 0)?),
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
    if world
        .get_component_by_id_as::<MusicNoteComponent>(id)
        .is_some()
    {
        match method {
            "velocity" => {
                if let Some(mn) = world.get_component_by_id_as_mut::<MusicNoteComponent>(id) {
                    mn.note = mn.note.with_velocity(arg_f32(args, 0)?);
                }
            }
            "play_on_attach" => {
                if let Some(mn) = world.get_component_by_id_as_mut::<MusicNoteComponent>(id) {
                    mn.play_on_attach = true;
                }
            }
            "at_beat" => {
                let b = arg_f32(args, 0)? as f64;
                if let Some(mn) = world.get_component_by_id_as_mut::<MusicNoteComponent>(id) {
                    mn.scheduled_beat = Some(b);
                }
            }
            "target" => {
                // target(ref) — explicit ComponentRef override.
                let src = arg_component_ref(world, args, 0)?;
                if let Some(mn) = world.get_component_by_id_as_mut::<MusicNoteComponent>(id) {
                    mn.target_source = Some(src);
                }
            }
            _ => {}
        }
        return Ok(());
    }
    if world
        .get_component_by_id_as::<AudioOscillatorComponent>(id)
        .is_some()
    {
        match method {
            "frequency" => {
                let hz = arg_f32(args, 0)?;
                if let Some(c) = world.get_component_by_id_as_mut::<AudioOscillatorComponent>(id) {
                    for o in c.oscillators.iter_mut() {
                        o.frequency = hz;
                    }
                }
            }
            "amplitude" => {
                let a = arg_f32(args, 0)?;
                if let Some(c) = world.get_component_by_id_as_mut::<AudioOscillatorComponent>(id) {
                    for o in c.oscillators.iter_mut() {
                        o.amplitude = a;
                    }
                }
            }
            "enabled" => {
                let en = arg_bool(args, 0)?;
                if let Some(c) = world.get_component_by_id_as_mut::<AudioOscillatorComponent>(id) {
                    for o in c.oscillators.iter_mut() {
                        o.enabled = en;
                    }
                }
            }
            _ => {}
        }
        return Ok(());
    }
    if world
        .get_component_by_id_as::<AudioClipComponent>(id)
        .is_some()
    {
        match method {
            "one_shot" => {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioClipComponent>(id) {
                    c.trigger_mode = AudioTriggerMode::OneShot;
                }
            }
            "latched" => {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioClipComponent>(id) {
                    c.trigger_mode = AudioTriggerMode::Latched;
                }
            }
            "retrigger" => {
                if let Some(c) = world.get_component_by_id_as_mut::<AudioClipComponent>(id) {
                    c.trigger_mode = AudioTriggerMode::Retrigger;
                }
            }
            _ => {}
        }
        return Ok(());
    }
    if world
        .get_component_by_id_as::<IKChainComponent>(id)
        .is_some()
    {
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
    if world
        .get_component_by_id_as::<TransformParentComponent>(id)
        .is_some()
    {
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
            "enabled" => *c3 = c3.clone().with_enabled(arg_bool(args, 0)?),
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
        match method {
            "enabled" => cxr.enabled = arg_bool(args, 0)?,
            "target" => {
                cxr.target = match arg_str(args, 0)? {
                    "window" => CameraTarget::Window,
                    _ => CameraTarget::Xr,
                };
            }
            _ => {}
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
            "interaction_mode" => {
                ed.interaction_mode = match arg_str(args, 0)? {
                    "cursor_3d" => EditorInteractionMode::Cursor3d,
                    "select_cursor" => EditorInteractionMode::SelectAndCursor,
                    _ => EditorInteractionMode::Select,
                };
            }
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
        } else if method == "interaction_priority" {
            rc.interaction_priority = arg_f32(args, 0)?.clamp(0.0, u8::MAX as f32) as u8;
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
    if let Some(inp) = world.get_component_by_id_as_mut::<InputXRGamepadComponent>(id) {
        match method {
            "enabled" => inp.enabled = arg_bool(args, 0)?,
            "hand" => {
                inp.hand = match arg_str(args, 0)?.to_ascii_lowercase().as_str() {
                    "default" => XrHandPreference::Default,
                    "left" => XrHandPreference::Left,
                    "right" => XrHandPreference::Right,
                    "either" => XrHandPreference::Either,
                    s => return Err(format!("unknown XrHandPreference: {s}")),
                }
            }
            "locomotion" => {
                inp.locomotion = if args.is_empty() {
                    true
                } else {
                    arg_bool(args, 0)?
                };
            }
            "speed" => inp.speed = arg_f32(args, 0)?,
            "deadzone" => inp.deadzone = arg_f32(args, 0)?,
            _ => {}
        }
        return Ok(());
    }
    if world
        .get_component_by_id_as::<InputTransformModeComponent>(id)
        .is_some()
    {
        let translation_basis_src = if method == "translation_basis" {
            Some(arg_component_ref(world, args, 0)?)
        } else {
            None
        };
        let updated = {
            let itm = world
                .get_component_by_id_as::<InputTransformModeComponent>(id)
                .expect("checked above")
                .clone();
            match method {
                "fps_rotation" => itm.with_fps_rotation(),
                "roll_axis_y" => itm.with_roll_axis_y(),
                "rotation_disabled" => itm.with_rotation_disabled(),
                "translation_basis" => itm
                    .with_translation_basis_source(translation_basis_src.expect("computed above")),
                _ => itm,
            }
        };
        if let Some(itm) = world.get_component_by_id_as_mut::<InputTransformModeComponent>(id) {
            *itm = updated;
        }
        return Ok(());
    }
    if let Some(s) = world.get_component_by_id_as_mut::<RendererSettingsComponent>(id) {
        if method == "window_size" {
            *s = s
                .clone()
                .with_window_size(arg_f32(args, 0)? as u32, arg_f32(args, 1)? as u32);
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
            "head_bone" => *avc = avc.clone().with_head_bone(arg_str(args, 0)?),
            "left_hand_bone" => *avc = avc.clone().with_left_hand_bone(arg_str(args, 0)?),
            "right_hand_bone" => *avc = avc.clone().with_right_hand_bone(arg_str(args, 0)?),
            "left_upper_arm_bone" => *avc = avc.clone().with_left_upper_arm_bone(arg_str(args, 0)?),
            "left_lower_arm_bone" => *avc = avc.clone().with_left_lower_arm_bone(arg_str(args, 0)?),
            "right_upper_arm_bone" => {
                *avc = avc.clone().with_right_upper_arm_bone(arg_str(args, 0)?)
            }
            "right_lower_arm_bone" => {
                *avc = avc.clone().with_right_lower_arm_bone(arg_str(args, 0)?)
            }
            "left_arm_pole_direction" => {
                *avc = avc
                    .clone()
                    .with_left_arm_pole_direction(arg_f32_arr::<3>(args, 0)?)
            }
            "right_arm_pole_direction" => {
                *avc = avc
                    .clone()
                    .with_right_arm_pole_direction(arg_f32_arr::<3>(args, 0)?)
            }
            "initial_yaw" => *avc = avc.clone().with_initial_yaw(arg_f32(args, 0)?),
            "forward_plus_z" => *avc = avc.clone().with_forward_plus_z(),
            "ik_debug" => *avc = avc.clone().with_ik_debug(),
            "calibrate_hand_transforms" => *avc = avc.clone().with_calibrate_hand_transforms(),
            "body_yaw_threshold" => *avc = avc.clone().with_body_yaw_threshold(arg_f32(args, 0)?),
            "body_yaw_rate" => *avc = avc.clone().with_body_yaw_rate(arg_f32(args, 0)?),
            "hand_rotation_smoothing" => {
                *avc = avc.clone().with_hand_rotation_smoothing(arg_f32(args, 0)?)
            }
            "camera_bone" => *avc = avc.clone().with_camera_bone(arg_str(args, 0)?),
            "avatar_height" => *avc = avc.clone().with_avatar_height(arg_f32(args, 0)?),
            "eye_height_from_head_bone" => {
                *avc = avc
                    .clone()
                    .with_eye_height_from_head_bone(arg_f32(args, 0)?)
            }
            "head_ik_eye_height" => *avc = avc.clone().with_head_ik_eye_height(arg_f32(args, 0)?),
            "hand_grip_rotation_left" => {
                *avc = avc
                    .clone()
                    .with_hand_grip_rotation_left(arg_f32_arr::<4>(args, 0)?)
            }
            "hand_grip_rotation_right" => {
                *avc = avc
                    .clone()
                    .with_hand_grip_rotation_right(arg_f32_arr::<4>(args, 0)?)
            }
            "hips_bone" => *avc = avc.clone().with_hips_bone(arg_str(args, 0)?),
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
    if let Some(grid) = world.get_component_by_id_as_mut::<GridComponent>(id) {
        match method {
            "spacing" => *grid = grid.with_spacing(arg_f32(args, 0)?),
            "size_x" => *grid = grid.with_size_x(arg_f32(args, 0)? as u32),
            "size_z" => *grid = grid.with_size_z(arg_f32(args, 0)? as u32),
            "enabled" => *grid = grid.with_enabled(arg_bool(args, 0)?),
            "hidden" => *grid = grid.with_hidden(arg_bool(args, 0)?),
            "selectable" => *grid = grid.with_selectable(arg_bool(args, 0)?),
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
            "ease_in_quad" => *transition = transition.with_easing(TransitionEasing::EaseInQuad),
            "ease_out_quad" => *transition = transition.with_easing(TransitionEasing::EaseOutQuad),
            "ease_in_out_quad" => {
                *transition = transition.with_easing(TransitionEasing::EaseInOutQuad)
            }
            "ease_in_cubic" => *transition = transition.with_easing(TransitionEasing::EaseInCubic),
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
    if world
        .get_component_by_id_as::<AnimationComponent>(id)
        .is_some()
    {
        use crate::engine::ecs::component::ResolveTargetsMode;
        let scope_src = if method == "scope" {
            Some(arg_component_ref(world, args, 0)?)
        } else {
            None
        };
        let Some(anim) = world.get_component_by_id_as_mut::<AnimationComponent>(id) else {
            return Ok(());
        };
        match method {
            "playing" => *anim = anim.clone().with_state(AnimationState::Playing),
            "looping" => *anim = anim.clone().with_state(AnimationState::Looping),
            "paused" => *anim = anim.clone().with_state(AnimationState::Paused),
            "length" => {
                let n = arg_f32(args, 0)? as f64;
                *anim = anim.clone().with_length_beats(n);
            }
            "scope" => {
                *anim = anim
                    .clone()
                    .with_scope_source(scope_src.expect("scope arg pre-parsed"));
            }
            "resolve_targets" => {
                let mode = match arg_str(args, 0)? {
                    "on_attach" => ResolveTargetsMode::OnAttach,
                    "on_play" => ResolveTargetsMode::OnPlay,
                    other => {
                        return Err(format!(
                            "Animation.resolve_targets: expected 'on_attach' or 'on_play', got {other:?}"
                        ));
                    }
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
                    "block" => Some(Display::Block),
                    "inline" => Some(Display::Inline),
                    "inline_block" | "inline-block" => Some(Display::InlineBlock),
                    "flex" => Some(Display::Flex),
                    "none" => Some(Display::None),
                    _ => None,
                };
            }
            "width" => st.width = arg_size_dimension(args, 0)?,
            "height" => st.height = arg_size_dimension(args, 0)?,
            "box_sizing" => {
                st.box_sizing = match arg_str(args, 0)? {
                    "border_box" | "border-box" => BoxSizing::BorderBox,
                    "content_box" | "content-box" => BoxSizing::ContentBox,
                    _ => return Ok(()),
                };
            }
            "padding" => st.padding = EdgeInsets::all_dim(arg_size_dimension(args, 0)?),
            "padding_xy" => {
                st.padding =
                    EdgeInsets::axes_dim(arg_size_dimension(args, 0)?, arg_size_dimension(args, 1)?)
            }
            "margin" => st.margin = EdgeInsets::all_dim(arg_size_dimension(args, 0)?),
            "margin_xy" => {
                st.margin =
                    EdgeInsets::axes_dim(arg_size_dimension(args, 0)?, arg_size_dimension(args, 1)?)
            }
            "background_color" => st.background_color = Some(arg_f32_arr::<4>(args, 0)?),
            "background_z" => st.background_z = Some(arg_f32(args, 0)?),
            "color" => st.color = Some(arg_f32_arr::<4>(args, 0)?),
            "flex_direction" => {
                st.flex_direction = match arg_str(args, 0)? {
                    "row" | "Row" => FlexDirection::Row,
                    "column" | "Column" => FlexDirection::Column,
                    "row_reverse" | "RowReverse" => FlexDirection::RowReverse,
                    "column_reverse" | "ColumnReverse" => FlexDirection::ColumnReverse,
                    _ => return Ok(()),
                };
            }
            "justify_content" => {
                st.justify_content = match arg_str(args, 0)? {
                    "flex_start" | "start" => JustifyContent::FlexStart,
                    "flex_end" | "end" => JustifyContent::FlexEnd,
                    "center" => JustifyContent::Center,
                    "space_between" => JustifyContent::SpaceBetween,
                    "space_around" => JustifyContent::SpaceAround,
                    "space_evenly" => JustifyContent::SpaceEvenly,
                    _ => return Ok(()),
                };
            }
            "align_items" => {
                st.align_items = match arg_str(args, 0)? {
                    "stretch" => AlignItems::Stretch,
                    "flex_start" | "start" => AlignItems::FlexStart,
                    "flex_end" | "end" => AlignItems::FlexEnd,
                    "center" => AlignItems::Center,
                    "baseline" => AlignItems::Baseline,
                    _ => return Ok(()),
                };
            }
            "text_align" => {
                st.text_align = match arg_str(args, 0)? {
                    "left" => TextAlign::Left,
                    "center" => TextAlign::Center,
                    "right" => TextAlign::Right,
                    "auto" | "none" => TextAlign::Auto,
                    _ => return Ok(()),
                };
            }
            "font_size" => st.font_size = arg_size_dimension(args, 0)?,
            "vertical_align" => {
                st.vertical_align = match arg_str(args, 0)? {
                    "top" => VerticalAlign::Top,
                    "middle" | "center" => VerticalAlign::Middle,
                    "bottom" => VerticalAlign::Bottom,
                    "auto" | "none" => VerticalAlign::Auto,
                    _ => return Ok(()),
                };
            }
            "flex_grow" => st.flex_grow = arg_f32(args, 0)?,
            "flex_shrink" => st.flex_shrink = arg_f32(args, 0)?,
            "gap" => {
                st.row_gap = arg_f32(args, 0)?;
                st.column_gap = st.row_gap;
            }
            "row_gap" => st.row_gap = arg_f32(args, 0)?,
            "column_gap" => st.column_gap = arg_f32(args, 0)?,
            "position" => {
                st.position = match arg_str(args, 0)? {
                    "static" => Position::Static,
                    "relative" => Position::Relative,
                    "absolute" => Position::Absolute,
                    "fixed" => Position::Fixed,
                    _ => return Ok(()),
                };
            }
            "top" => st.top = Some(arg_size_dimension(args, 0)?),
            "right" => st.right = Some(arg_size_dimension(args, 0)?),
            "bottom" => st.bottom = Some(arg_size_dimension(args, 0)?),
            "left" => st.left = Some(arg_size_dimension(args, 0)?),
            "overflow" => {
                st.overflow = match arg_str(args, 0)? {
                    "visible" => Overflow::Visible,
                    "hidden" => Overflow::Hidden,
                    "scroll" => Overflow::Scroll,
                    "auto" => Overflow::Auto,
                    _ => return Ok(()),
                };
            }
            "z_index" => st.z_index = Some(arg_f32(args, 0)? as i32),
            "flex_wrap" => {
                st.flex_wrap = match arg_str(args, 0)? {
                    "nowrap" | "no_wrap" => FlexWrap::NoWrap,
                    "wrap" => FlexWrap::Wrap,
                    "wrap_reverse" => FlexWrap::WrapReverse,
                    _ => return Ok(()),
                };
            }
            "word_wrap" => {
                st.word_wrap = match arg_str(args, 0)? {
                    "normal" => Some(WordWrapMode::Normal),
                    "break_word" | "break-word" => Some(WordWrapMode::BreakWord),
                    "break_all" | "break-all" => Some(WordWrapMode::BreakAll),
                    _ => None,
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
        if let Some(ti) = world.get_component_by_id_as_mut::<TextInputComponent>(id) {
            ti.set_text(s.clone());
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
        "position" => Ok(c.with_position(
            arg_world_f32(args, 0)?,
            arg_world_f32(args, 1)?,
            arg_world_f32(args, 2)?,
        )),
        "scale" => Ok(c.with_scale(
            arg_world_f32(args, 0)?,
            arg_world_f32(args, 1)?,
            arg_world_f32(args, 2)?,
        )),
        "rotation" | "rotation_euler" => {
            Ok(c.with_rotation_euler(arg_f32(args, 0)?, arg_f32(args, 1)?, arg_f32(args, 2)?))
        }
        // Lossless quaternion form — used by `to_mms_ast` so saved/cloned
        // transforms reproduce exactly, including arbitrary axis rotations.
        "rotation_quat" => Ok(c.with_rotation_quat([
            arg_f32(args, 0)?,
            arg_f32(args, 1)?,
            arg_f32(args, 2)?,
            arg_f32(args, 3)?,
        ])),
        other => {
            println!("[registry] unknown Transform builder: '{other}'");
            Ok(c)
        }
    }
}

fn apply_fit_bounds_ctor(
    c: &mut FitBoundsComponent,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    match method {
        "renderable_only" => c.mode = FitBoundsMode::RenderableOnly,
        "layout_aware" => c.mode = FitBoundsMode::LayoutAware,
        "to" => {
            c.target = FitBoundsTarget::ExplicitBounds;
            c.target_bounds = val_as_f32_array::<6>(&Value::Array(args.to_vec()))?;
        }
        "to_container" => c.target = FitBoundsTarget::ParentPaddingBox,
        other => {
            println!("[registry] unknown FitBounds builder: '{other}'");
        }
    }
    Ok(())
}

fn apply_layout_bounds_ctor(
    c: &mut LayoutBoundsComponent,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    match method {
        "content_box" => {
            c.content_local = crate::engine::graphics::bounds::Aabb {
                min: val_as_f32_array::<3>(&args[0])?,
                max: val_as_f32_array::<3>(&args[1])?,
            };
        }
        "padding_box" => {
            c.padding_local = crate::engine::graphics::bounds::Aabb {
                min: val_as_f32_array::<3>(&args[0])?,
                max: val_as_f32_array::<3>(&args[1])?,
            };
        }
        other => {
            println!("[registry] unknown LayoutBounds builder: '{other}'");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meow_meow::ast::BlockStatement;
    use crate::meow_meow::object::RuntimeClosure;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn spawn_tree_installs_keyframe_callback() {
        let ce = MaterializedCE {
            component_type: "Keyframe".to_string(),
            component_property_assignment_only: false,
            ctor_method: Some("at".to_string()),
            ctor_args: vec![Value::Number(1.0)],
            calls: vec![],
            named: vec![],
            positionals: vec![],
            deferred_block: Some(RuntimeClosure {
                body: BlockStatement { statements: vec![] },
                captured_env: Arc::new(HashMap::new()),
                analysis: None,
            }),
            children: vec![],
        };

        let mut world = World::default();
        let mut emit = crate::engine::ecs::RxWorld::default();
        let id = spawn_tree(&ce, None, &mut world, &mut emit).expect("spawn keyframe");

        let keyframe = world
            .get_component_by_id_as::<KeyframeComponent>(id)
            .expect("spawned keyframe exists");
        assert!(keyframe.callback.is_some());
    }
}
