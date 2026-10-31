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
    boxes.push(bounds, 2);
    assert_eq!(boxes.boxes.len(), 1);
    let mesh = boxes.mesh();
    assert_eq!(mesh.indices_u32.len(), 4 * 36);
    for i in 1..5000 {
        boxes.push(
            crate::engine::graphics::bounds::Aabb {
                min: [i as f32, 0., 0.],
                max: [i as f32 + 0.5, 1., 1.],
            },
            0,
        );
    }
    assert_eq!(
        boxes.boxes.len(),
        super::super::bvh_snapshot_mesh::MAX_BOXES
    );
    assert!(boxes.truncated);
}
