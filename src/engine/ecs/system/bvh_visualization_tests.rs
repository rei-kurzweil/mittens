use super::*;
use crate::engine::ecs::component::*;
use crate::engine::graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader};
#[derive(Default)]
struct Uploader {
    uploads: u32,
    releases: u32,
    fail: bool,
}
impl MeshUploader for Uploader {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        if self.fail {
            return Err("injected failure".into());
        }
        self.uploads += 1;
        Ok(MeshHandle(self.uploads))
    }
    fn release_mesh(&mut self, _: MeshHandle) {
        self.releases += 1;
    }
}
impl TextureUploader for Uploader {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
}
fn child<T: Component>(world: &mut World, parent: ComponentId, value: T) -> ComponentId {
    let id = world.add_component(value);
    world.add_child(parent, id).unwrap();
    id
}
#[test]
fn bvh_snapshot_is_lazy_frozen_refreshable_and_released() {
    let (mut w, mut v, mut a, mut u, mut bvh, mut contacts, mut viz) = (
        World::default(),
        VisualWorld::default(),
        RenderAssets::new(),
        Uploader::default(),
        BvhSystem::default(),
        StaticContactSystem::default(),
        BvhVisualizationSystem::default(),
    );
    let editor = w.add_component(EditorComponent::new());
    let owner = w.add_component(EditorUIComponent::new());
    let frame = child(&mut w, editor, TransformComponent::new());
    let r = child(&mut w, frame, RenderableComponent::cube());
    child(&mut w, r, RaycastableComponent::enabled());
    let zone = child(&mut w, frame, ZoneComponent::cube([0.5; 3]));
    child(&mut w, zone, CollidableComponent::static_());
    let solid = crate::engine::graphics::csg_mesh::Solid::box_between([-0.5; 3], [0.5; 3]);
    let cutter =
        crate::engine::graphics::csg_mesh::Solid::box_between([-0.2, -1., -0.2], [0.2, 1., 0.2]);
    let mesh = std::sync::Arc::new(super::super::triangle_collision::TriangleMesh::new(
        &solid.cut(&cutter).unwrap().0.mesh(),
    ));
    child(&mut w, zone, CutGeometryComponent { mesh });
    bvh.queue_renderable_added(r);
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(viz.captures, 0);
    assert_eq!(u.uploads, 0);
    {
        let ui = w
            .get_component_by_id_as_mut::<EditorUIComponent>(owner)
            .unwrap();
        ui.bvh_visible = true;
        ui.bvh_revision = 1;
        ui.bvh_scope_roots = vec![editor];
    }
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(viz.captures, 1);
    assert!(viz.box_count(owner) > 1);
    assert_eq!(v.instance_count(), 1);
    // Wait for the real shader compile before process teardown, and validate
    // the custom surface shader without requiring a Vulkan device.
    let definition = v.materials.source_definition(owner).unwrap();
    let custom = v
        .materials
        .definition(definition)
        .unwrap()
        .custom
        .as_ref()
        .unwrap()
        .clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while custom.compiled.get().is_none() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(
        custom.compiled.get().unwrap().is_ok(),
        "{:?}",
        custom.compiled.get()
    );
    let original = viz.snapshots[&owner].output.unwrap();
    assert_eq!(
        v.instance(original.0).unwrap().renderable.material,
        v.materials.definition(definition).unwrap().static_program,
        "desktop draws must select the snapshot shader, not only bind its parameters"
    );

    let uploads = u.uploads;
    let captures = viz.captures;
    w.get_component_by_id_as_mut::<EditorUIComponent>(owner)
        .unwrap()
        .set_bvh_animation_enabled(true);
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(viz.snapshots[&owner].active_depth, Some(0));
    assert_eq!(u.uploads, uploads, "animation must reuse the captured mesh");
    assert_eq!(
        viz.captures, captures,
        "animation must not recapture live BVHs"
    );
    let source = v.materials.source_instance(owner).unwrap();
    assert_eq!(
        v.materials.instance(source).unwrap().values.as_ref(),
        &[1.4, 0.]
    );
    let revision = v.materials.instance(source).unwrap().revision;
    viz.snapshots.get_mut(&owner).unwrap().animation_step = Some(Instant::now() - STEP);
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(
        v.materials.instance(source).unwrap().values.as_ref(),
        &[1.4, 1.]
    );
    assert_eq!(v.materials.instance(source).unwrap().revision, revision + 1);
    assert_eq!(u.uploads, uploads);
    assert_eq!(viz.captures, captures);

    w.get_component_by_id_as_mut::<EditorUIComponent>(owner)
        .unwrap()
        .set_bvh_animation_enabled(false);
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(
        v.materials.instance(source).unwrap().values.as_ref(),
        &[1.4, -1.]
    );
    assert_eq!(
        viz.snapshots[&owner].output,
        Some(original),
        "stopping animation retains every wire box"
    );

    assert!(
        w.children_of(owner).is_empty(),
        "snapshot draw must not add pick/collision/save nodes"
    );
    let frame2 = child(
        &mut w,
        editor,
        TransformComponent::new().with_position(3., 0., 0.),
    );
    let r2 = child(&mut w, frame2, RenderableComponent::cube());
    child(&mut w, r2, RaycastableComponent::enabled());
    bvh.queue_renderable_added(r2);
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(viz.captures, 1);
    assert_eq!(viz.snapshots[&owner].output, Some(original));
    w.get_component_by_id_as_mut::<EditorUIComponent>(owner)
        .unwrap()
        .bvh_revision += 1;
    u.fail = true;
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(viz.snapshots[&owner].output, Some(original));
    assert_eq!(viz.captures, 1);
    u.fail = false;
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(viz.captures, 2);
    assert_eq!(v.instance_count(), 1);
    assert_eq!(u.releases, 1);
    w.get_component_by_id_as_mut::<EditorUIComponent>(owner)
        .unwrap()
        .bvh_visible = false;
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(v.instance_count(), 0);
    assert_eq!(u.releases, 2);
    w.get_component_by_id_as_mut::<EditorUIComponent>(owner)
        .unwrap()
        .bvh_visible = true;
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(viz.captures, 3);
    w.remove_component_subtree(owner).unwrap();
    viz.prepare(&w, &mut v, &mut a, &mut u, &mut bvh, &mut contacts);
    assert_eq!(v.instance_count(), 0);
    assert!(viz.snapshots.is_empty());
}
#[test]
fn bvh_snapshot_mesh_handles_planar_and_degenerate_bounds_and_limits() {
    let mut boxes = SnapshotBoxes::default();
    let bounds = crate::engine::graphics::bounds::Aabb {
        min: [0., 0., 0.],
        max: [1., 0., 1.],
    };
    boxes.push(bounds, 0);
    boxes.push(bounds, 0);
    assert_eq!(boxes.boxes.len(), 1);
    // Equal bounds at different depths are distinct animation steps.
    boxes.push(bounds, 2);
    assert_eq!(boxes.boxes.len(), 2);
    let mesh = boxes.mesh();
    assert_eq!(mesh.indices_u32.len(), 2 * 4 * 24);
    for i in 1..5000 {
        boxes.push(
            crate::engine::graphics::bounds::Aabb {
                min: [i as f32, 0., 0.],
                max: [i as f32 + 0.5, 1., 1.],
            },
            0,
        );
    }
    boxes.limit();
    assert_eq!(
        boxes.boxes.len(),
        super::super::bvh_snapshot_mesh::MAX_BOXES
    );
    assert!(boxes.truncated);
}

#[test]
fn bvh_animation_visits_all_levels_before_looping_and_never_skips_slow_frames() {
    let mut state = Snapshot {
        output: Some((InstanceHandle(0), CpuMeshHandle::CUBE)),
        max_depth: 4,
        ..Default::default()
    };
    let mut visuals = VisualWorld::default();
    let owner = ComponentId::default();
    let start = Instant::now();
    state.animate(owner, true, start, &mut visuals);
    assert_eq!(state.active_depth, Some(0));
    state.animate(
        owner,
        true,
        start + STEP - Duration::from_millis(1),
        &mut visuals,
    );
    assert_eq!(state.active_depth, Some(0));
    // A leaf at level one does not restart the animation of deeper branches.
    for depth in 1..=4 {
        state.animate(owner, true, start + STEP * depth, &mut visuals);
        assert_eq!(state.active_depth, Some(depth));
    }
    state.animate(owner, true, start + STEP * 5, &mut visuals);
    assert_eq!(state.active_depth, Some(0));
    state.animate(owner, true, start + Duration::from_secs(100), &mut visuals);
    assert_eq!(
        state.active_depth,
        Some(1),
        "a stalled frame must not skip unvisited levels"
    );
    state.animate(owner, false, start + Duration::from_secs(101), &mut visuals);
    assert_eq!(state.active_depth, None);
    state.animate(owner, true, start + Duration::from_secs(102), &mut visuals);
    assert_eq!(
        state.active_depth,
        Some(0),
        "enabling restarts from the root"
    );
}

#[test]
fn bvh_snapshot_attaches_triangle_tree_below_its_collision_leaf() {
    let mut world = World::default();
    let root = world.add_component(TransformComponent::new());
    let zone = child(&mut world, root, ZoneComponent::cube([0.5; 3]));
    child(&mut world, zone, CollidableComponent::static_());
    let mesh = crate::engine::graphics::mesh::MeshFactory::cube();
    let triangles = std::sync::Arc::new(super::super::triangle_collision::TriangleMesh::new(&mesh));
    child(&mut world, zone, CutGeometryComponent { mesh: triangles });
    let mut contacts = StaticContactSystem::default();
    let mut boxes = SnapshotBoxes::default();
    contacts
        .support_index(&world)
        .snapshot(&world, &[root], &mut boxes);
    assert_eq!(boxes.boxes[0].1, 0, "outer collider starts at the root");
    assert_eq!(
        boxes.boxes.iter().filter(|(_, depth)| *depth == 0).count(),
        1
    );
    assert!(boxes.boxes.iter().any(|(_, depth)| *depth > 1));
    let encoded = boxes.mesh();
    assert!(encoded.vertices.iter().any(|v| v.uv[1] > 1.));
}

#[test]
fn bvh_snapshot_budget_keeps_old_and_new_damaged_regions() {
    let mut boxes = SnapshotBoxes::default();
    let bounds = |x| crate::engine::graphics::bounds::Aabb {
        min: [x, 0., 0.],
        max: [x + 1., 1., 1.],
    };
    boxes.push_important(bounds(-10.), 0);
    boxes.push_important(bounds(-9.), 12); // Older damaged chunk.
    for index in 0..9000 {
        boxes.push(bounds(index as f32), (index % 20 + 1) as u32);
    }
    boxes.push_important(bounds(10000.), 15); // Later damage, after the old cap.
    boxes.limit();
    assert_eq!(
        boxes.boxes.len(),
        super::super::bvh_snapshot_mesh::MAX_BOXES
    );
    assert!(boxes.truncated);
    for x in [-10., -9., 10000.] {
        assert!(boxes.boxes.iter().any(|(b, _)| b.min[0] == x));
    }
}
