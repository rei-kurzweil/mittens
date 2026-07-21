use mittens_engine::engine::ecs::component::{
    Camera3DComponent, CameraXRComponent, ComponentRef, EditorComponent, GLTFComponent,
    PointerComponent, RenderableComponent, TransformComponent, TransformGizmoComponent,
    TransformParentComponent,
};
use mittens_engine::engine::ecs::system::editor_scene_hit::resolve_world_scene_hit;
use mittens_engine::engine::ecs::system::{EditorContextSystem, PointerSystem, XrInputState};
use mittens_engine::engine::ecs::{CommandQueue, IntentValue, SignalEmitter, SystemWorld, World};
use mittens_engine::engine::graphics::{RenderAssets, VisualWorld};
use mittens_engine::engine::user_input::InputState;
use winit::event::MouseButton;

fn translation(world: &World, component: mittens_engine::engine::ecs::ComponentId) -> [f32; 3] {
    let matrix = world
        .get_component_by_id_as::<TransformComponent>(component)
        .expect("transform")
        .transform
        .matrix_world;
    [matrix[3][0], matrix[3][1], matrix[3][2]]
}

#[test]
fn editor_startup_activates_identity_without_semantically_selecting_it() {
    let mut world = World::default();
    let editor = world.add_component(EditorComponent::new());
    let mut context = EditorContextSystem::new();

    context.register_editor_identity(&world, editor);

    let state = context.shared_state();
    let state = state.lock().unwrap();
    assert_eq!(state.active_editor, Some(editor));
    assert_eq!(state.selected_component, None);
}

#[test]
fn gltf_bounds_are_invisible_by_default() {
    assert!(!GLTFComponent::new("test.glb").bounds_visible);
}

#[test]
fn wireframe_box_mesh_is_cached_by_thickness_and_retains_unit_extents() {
    let mut assets = RenderAssets::new();
    let thin = mittens_engine::engine::ecs::component::RenderableComponent::wireframe_box(
        &mut assets,
        0.02,
    );
    let thin_again = mittens_engine::engine::ecs::component::RenderableComponent::wireframe_box(
        &mut assets,
        0.02,
    );
    let thick = mittens_engine::engine::ecs::component::RenderableComponent::wireframe_box(
        &mut assets,
        0.08,
    );

    assert_eq!(thin.renderable.mesh, thin_again.renderable.mesh);
    assert_ne!(thin.renderable.mesh, thick.renderable.mesh);

    let mesh = assets
        .cpu_mesh(thin.renderable.mesh)
        .expect("wireframe mesh");
    assert_eq!(mesh.vertices.len(), 12 * 24);
    assert_eq!(mesh.indices_u32.len(), 12 * 36);
    for axis in 0..3 {
        let min = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.pos[axis])
            .fold(f32::INFINITY, f32::min);
        let max = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.pos[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((min + 0.5).abs() < 1.0e-6);
        assert!((max - 0.5).abs() < 1.0e-6);
    }
}

#[test]
fn wireframe_square_mesh_is_cached_and_lies_in_the_unit_xy_plane() {
    let mut assets = RenderAssets::new();
    let thin = RenderableComponent::wireframe_square(&mut assets, 0.1);
    let thin_again = RenderableComponent::wireframe_square(&mut assets, 0.1);
    let thick = RenderableComponent::wireframe_square(&mut assets, 0.2);

    assert_eq!(thin.renderable.mesh, thin_again.renderable.mesh);
    assert_ne!(thin.renderable.mesh, thick.renderable.mesh);

    let mesh = assets
        .cpu_mesh(thin.renderable.mesh)
        .expect("wireframe square mesh");
    assert_eq!(mesh.vertices.len(), 8);
    assert_eq!(mesh.indices_u32.len(), 24);
    for axis in 0..2 {
        let min = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.pos[axis])
            .fold(f32::INFINITY, f32::min);
        let max = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.pos[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((min + 0.5).abs() < 1.0e-6);
        assert!((max - 0.5).abs() < 1.0e-6);
    }
    assert!(mesh.vertices.iter().all(|vertex| vertex.pos[2] == 0.0));
}

#[test]
fn desktop_mouse_does_not_activate_xr_camera_pointer() {
    let mut world = World::default();
    let mut queue = CommandQueue::new();
    let mut pointers = PointerSystem::default();

    let desktop_camera = world.add_component(Camera3DComponent::new());
    let desktop_pointer = world.add_component(PointerComponent::new());
    world.add_child(desktop_camera, desktop_pointer).unwrap();
    pointers.register_pointer(&mut world, desktop_pointer, &mut queue);

    let xr_camera = world.add_component(CameraXRComponent::on());
    let xr_pointer = world.add_component(PointerComponent::new());
    world.add_child(xr_camera, xr_pointer).unwrap();
    pointers.register_pointer(&mut world, xr_pointer, &mut queue);

    let mut input = InputState::default();
    input.mouse_pressed.insert(MouseButton::Left);
    input.mouse_down.insert(MouseButton::Left);
    input.mouse_released.insert(MouseButton::Left);

    let activations = pointers.build_activations(&world, &input, &XrInputState::default());
    assert_eq!(activations.pressed, vec![desktop_pointer]);
    assert_eq!(activations.down, vec![desktop_pointer]);
    assert_eq!(activations.released, vec![desktop_pointer]);
}

#[test]
fn panel_title_click_target_is_not_a_world_scene_hit() {
    let mut world = World::default();
    let runtime_ui_root = world.add_component_boxed_named(
        "editor_runtime_ui_root",
        Box::new(TransformComponent::new()),
    );
    let title = world.add_component(TransformComponent::new());
    let title_background = world.add_component(RenderableComponent::cube());
    world.add_child(runtime_ui_root, title).unwrap();
    world.add_child(title, title_background).unwrap();

    assert!(resolve_world_scene_hit(&world, title_background).is_none());
}

#[test]
fn attaching_gizmo_does_not_move_target_or_unrelated_follower() {
    let mut world = World::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut systems = SystemWorld::default();
    let mut queue = CommandQueue::new();

    let avatar_root = world.add_component(TransformComponent::new().with_position(0.0, 2.0, 0.0));
    let tail = world.add_component(TransformComponent::new().with_position(0.0, 1.0, 0.0));
    let mesh = world.add_component(RenderableComponent::cube());
    world.add_child(avatar_root, tail).unwrap();
    world.add_child(tail, mesh).unwrap();
    systems.transform_changed(&mut world, &mut visuals, avatar_root);

    let target_guid = world.get_component_record(mesh).unwrap().guid;
    let follower = world.add_component(
        TransformParentComponent::new().with_target_source(ComponentRef::Guid(target_guid)),
    );
    let follower_local = world.add_component(TransformComponent::new());
    world.add_child(follower, follower_local).unwrap();
    world.init_component_tree(follower, &mut queue);
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    let avatar_before = translation(&world, avatar_root);
    let tail_before = translation(&world, tail);
    let follower_before = translation(&world, follower_local);

    let gizmo = world.add_component(TransformGizmoComponent::new());
    queue.push_intent_now(
        avatar_root,
        IntentValue::Attach {
            parents: vec![tail],
            child: gizmo,
        },
    );
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    assert_eq!(translation(&world, avatar_root), avatar_before);
    assert_eq!(translation(&world, tail), tail_before);
    assert_eq!(translation(&world, follower_local), follower_before);
}

#[test]
fn follower_updates_when_an_ancestor_of_its_target_moves() {
    let mut world = World::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut systems = SystemWorld::default();
    let mut queue = CommandQueue::new();

    let model_root = world.add_component(TransformComponent::new().with_position(0.0, 2.0, 0.0));
    let mesh_node = world.add_component(TransformComponent::new().with_position(0.0, 1.0, 0.0));
    let mesh = world.add_component(RenderableComponent::cube());
    world.add_child(model_root, mesh_node).unwrap();
    world.add_child(mesh_node, mesh).unwrap();
    systems.transform_changed(&mut world, &mut visuals, model_root);

    let target_guid = world.get_component_record(mesh).unwrap().guid;
    let follower = world.add_component(
        TransformParentComponent::new().with_target_source(ComponentRef::Guid(target_guid)),
    );
    let follower_local = world.add_component(TransformComponent::new());
    world.add_child(follower, follower_local).unwrap();
    world.init_component_tree(follower, &mut queue);
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
    assert_eq!(translation(&world, follower_local), [0.0, 3.0, 0.0]);

    queue.push_intent_now(
        model_root,
        IntentValue::UpdateTransform {
            component_ids: vec![model_root],
            translation: [0.0, 4.0, 0.0],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        },
    );
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    assert_eq!(translation(&world, mesh_node), [0.0, 5.0, 0.0]);
    assert_eq!(translation(&world, follower_local), [0.0, 5.0, 0.0]);
}

#[test]
fn enabling_armature_markers_preserves_joint_world_transform() {
    let mut world = World::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut systems = SystemWorld::default();
    let mut queue = CommandQueue::new();

    let model_root = world.add_component(TransformComponent::new().with_position(0.0, 2.0, 0.0));
    let gltf = world.add_component(GLTFComponent::new("test.glb"));
    let joint = world.add_component(TransformComponent::new().with_position(0.0, 1.0, 0.0));
    world.add_child(model_root, gltf).unwrap();
    world.add_child(model_root, joint).unwrap();
    systems.transform_changed(&mut world, &mut visuals, model_root);

    let before = translation(&world, joint);
    let component = world
        .get_component_by_id_as_mut::<GLTFComponent>(gltf)
        .unwrap();
    component.spawned = true;
    component.armature_visible = true;
    component.armature_joint_transforms = vec![joint];
    systems.gltf.register_component(gltf);

    systems.armature_visualization.tick_with_queue(
        &mut world,
        &systems.gltf,
        &mut visuals,
        &mut queue,
        0.016,
    );
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    assert_eq!(translation(&world, joint), before);
}
