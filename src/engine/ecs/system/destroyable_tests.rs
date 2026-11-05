use super::*;
use crate::engine::graphics::{MeshHandle, MeshUploader, TextureHandle, TextureUploader};
#[derive(Default)]
struct Uploader {
    next: u32,
    released: usize,
    fail: bool,
}
impl MeshUploader for Uploader {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        if self.fail {
            return Err("injected upload failure".into());
        }
        self.next += 1;
        Ok(MeshHandle(self.next))
    }
    fn release_mesh(&mut self, _: MeshHandle) {
        self.released += 1;
    }
}
impl TextureUploader for Uploader {
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        Ok(TextureHandle(0))
    }
}
fn surface(
    world: &mut World,
    visuals: &mut VisualWorld,
    assets: &mut RenderAssets,
    uploader: &mut Uploader,
    x: f32,
) -> (ComponentId, ComponentId) {
    let mut t = TransformComponent::new()
        .with_position(x, 0., 0.)
        .with_scale(2., 0.10, 2.);
    t.transform.matrix_world = t.transform.model;
    let model = t.transform.model;
    let frame = world.add_component(t);
    let marker = attach(world, frame, DestroyableComponent::default());
    let r = attach(world, frame, RenderableComponent::cube());
    attach(world, r, RaycastableComponent::enabled());
    let zone = attach(world, frame, ZoneComponent::cube([0.5; 3]));
    attach(world, zone, CollidableComponent::static_());
    let gpu = assets
        .gpu_mesh_handle(uploader, CpuMeshHandle::CUBE)
        .unwrap();
    let handle = visuals.register(
        r,
        GpuRenderable::new(gpu, crate::engine::graphics::MaterialHandle::TOON_MESH),
        crate::engine::graphics::primitives::Transform {
            model,
            matrix_world: model,
            ..Default::default()
        },
        [1.; 4],
        1.,
        false,
        false,
        false,
        false,
        false,
        0.,
        None,
        3.,
    );
    world
        .get_component_by_id_as_mut::<RenderableComponent>(r)
        .unwrap()
        .handle = Some(handle);
    (marker, r)
}
fn event(world: &mut World, position: [f32; 3], amount: f32) {
    world.add_component(DamageComponent {
        position,
        radius: 0.28,
        amount,
    });
}
fn pump(
    d: &mut DestroyableSystem,
    s: &mut SystemWorld,
    w: &mut World,
    v: &mut VisualWorld,
    a: &mut RenderAssets,
    u: &mut Uploader,
) {
    d.prepare(s, w, v, a, u);
}
fn finish(
    d: &mut DestroyableSystem,
    s: &mut SystemWorld,
    w: &mut World,
    v: &mut VisualWorld,
    a: &mut RenderAssets,
    u: &mut Uploader,
) {
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    while d.surfaces.values().any(|s| s.busy) {
        assert!(Instant::now() < deadline, "worker did not finish");
        std::thread::sleep(std::time::Duration::from_millis(2));
        pump(d, s, w, v, a, u);
    }
}
#[test]
fn sparse_damage_local_chunks_debris_and_owner_cleanup() {
    let (mut w, mut v, mut a, mut u, mut s, mut d) = (
        World::default(),
        VisualWorld::default(),
        RenderAssets::new(),
        Uploader::default(),
        SystemWorld::default(),
        DestroyableSystem::default(),
    );
    let (owner, _) = surface(&mut w, &mut v, &mut a, &mut u, 0.);
    let (_, other) = surface(&mut w, &mut v, &mut a, &mut u, 4.);
    let authored_frame = w.parent_of(owner).unwrap();
    let authored_before =
        crate::scripting::component_registry::filtered_roots_to_ce_ast(&w, &[authored_frame])
            .unwrap()
            .iter()
            .map(crate::scripting::unparser::unparse_component)
            .collect::<Vec<_>>();
    assert!(d.surfaces.is_empty());
    for _ in 0..2 {
        event(&mut w, [0., 0.05, 0.], 1.);
        pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    }
    assert!(d.surfaces[&owner].cells.is_empty());
    assert!(!d.surfaces[&owner].damage.is_empty());
    assert!(d.debris.is_empty());
    event(&mut w, [0., 0.05, 0.], 1.);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    finish(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert_eq!(d.cuts, 1);
    assert_eq!(d.surfaces[&owner].cells.len(), 4);
    assert!(!d.debris.is_empty());
    assert_eq!(d.debris.len(), 3);
    assert!(d.surfaces[&owner].damage.is_empty());
    let mut hit = false;
    for cell in &d.surfaces[&owner].cells {
        if let Some(mesh) = crate::engine::ecs::system::triangle_collision::on(&w, cell.zone) {
            hit |= mesh
                .world(TransformSystem::world_model(&w, cell.frame).unwrap())
                .ray([0., 1., 0.], [0., -1., 0.])
                .is_some();
        }
    }
    assert!(
        !hit,
        "ray must pass through all chunks touched by a cross-boundary cut"
    );
    assert_eq!(
        w.get_component_by_id_as::<RenderableComponent>(other)
            .unwrap()
            .renderable
            .base_mesh,
        CpuMeshHandle::CUBE
    );
    assert!(
        w.get_component_by_id_as::<RenderableComponent>(other)
            .unwrap()
            .handle
            .is_some()
    );
    s.bvh.flush_pending(&w);
    assert!(
        s.raycast
            .cast_against_renderables_bvh(&w, &s.bvh, [0., 1., 0.], [0., -1., 0.], 0., 5.)
            .is_empty()
    );
    assert!(
        s.raycast
            .cast_against_renderables(&w, [0., 1., 0.], [0., -1., 0.], 0., 5.)
            .is_empty()
    );
    assert!(
        !s.raycast
            .cast_against_renderables_bvh(&w, &s.bvh, [0.8, 1., 0.], [0., -1., 0.], 0., 5.)
            .is_empty()
    );
    let capsule_frame = w.add_component(TransformComponent::new());
    let capsule = attach(&mut w, capsule_frame, ZoneComponent::capsule_y(0.1, 0.3));
    for cell in &d.surfaces[&owner].cells {
        assert!(
            !crate::engine::ecs::system::zone_query::overlap_zones(&w, capsule, cell.zone)
                .unwrap()
                .intersects
        );
    }
    let authored_after =
        crate::scripting::component_registry::filtered_roots_to_ce_ast(&w, &[authored_frame])
            .unwrap()
            .iter()
            .map(crate::scripting::unparser::unparse_component)
            .collect::<Vec<_>>();
    assert_eq!(
        authored_before, authored_after,
        "runtime geometry and suppression must not leak into filtered saves"
    );
    let frame = w.parent_of(owner).unwrap();
    s.remove_subtree_immediate(&mut w, &mut v, frame);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert!(d.surfaces.is_empty());
    assert!(u.released > 0);
    for debris in &mut d.debris {
        debris.expires = Instant::now();
    }
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert!(d.debris.is_empty());
    assert_eq!(v.instance_count(), 1);
}
#[test]
fn failed_upload_keeps_previous_geometry_and_can_retry() {
    let (mut w, mut v, mut a, mut u, mut s, mut d) = (
        World::default(),
        VisualWorld::default(),
        RenderAssets::new(),
        Uploader::default(),
        SystemWorld::default(),
        DestroyableSystem::default(),
    );
    let (owner, _) = surface(&mut w, &mut v, &mut a, &mut u, 0.);
    event(&mut w, [0., 0.05, 0.], 3.);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    u.fail = true;
    finish(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert_eq!(d.cuts, 0);
    assert!(d.debris.is_empty());
    assert!(d.surfaces[&owner].cells.iter().all(|c| c.mesh.is_none()));
    u.fail = false;
    event(&mut w, [0., 0.05, 0.], 1.);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    finish(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert_eq!(d.cuts, 1);
}

#[test]
fn fully_consumed_surface_releases_state_and_cannot_resurrect() {
    let (mut w, mut v, mut a, mut u, mut s, mut d) = (
        World::default(),
        VisualWorld::default(),
        RenderAssets::new(),
        Uploader::default(),
        SystemWorld::default(),
        DestroyableSystem::default(),
    );
    let (owner, _) = surface(&mut w, &mut v, &mut a, &mut u, 0.);
    w.add_component(DamageComponent {
        position: [0.; 3],
        radius: 2.,
        amount: 3.,
    });
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    finish(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert!(d.surfaces.is_empty());
    assert!(
        w.get_component_by_id_as::<DestroyableComponent>(owner)
            .unwrap()
            .exhausted
    );
    assert_eq!(d.debris.len(), 3);
    event(&mut w, [0.; 3], 3.);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert!(d.surfaces.is_empty());
    assert_eq!(d.cuts, 1);
    s.bvh.flush_pending(&w);
    assert!(
        s.raycast
            .cast_against_renderables(&w, [0., 1., 0.], [0., -1., 0.], 0., 5.)
            .is_empty()
    );
}

#[test]
fn unpacking_damage_group_preserves_imported_mesh_overrides() {
    let (mut w, mut v, mut a, mut u, mut s) = (
        World::default(),
        VisualWorld::default(),
        RenderAssets::new(),
        Uploader::default(),
        SystemWorld::default(),
    );
    let root = w.add_component(TransformComponent::new());
    let group = attach(&mut w, root, DestroyableComponent::default());
    let batch = attach(&mut w, root, CombineMeshComponent::keep_transforms());
    // Match a suppressed imported body: its Renderable has never resolved the
    // GLTF MeshComponent override and still contains a placeholder CPU handle.
    let source = attach(
        &mut w,
        batch,
        RenderableComponent::new(crate::engine::graphics::primitives::Renderable::new(
            CpuMeshHandle(0),
            crate::engine::graphics::MaterialHandle::TOON_MESH,
        )),
    );
    attach(&mut w, source, MeshComponent::new("imported_body"));
    let cpu = a.register_imported_mesh(
        "imported_body",
        crate::engine::graphics::mesh::MeshFactory::cube(),
    );
    let gpu = a.gpu_mesh_handle(&mut u, cpu).unwrap();
    s.combine_mesh.reconcile_and_build(
        &mut w,
        &mut v,
        &mut a,
        &mut u,
        &mut s.renderable,
        &mut s.mesh_bounds,
    );
    assert!(v.instance_handle_for_component(batch).is_some());
    let mut groups = super::super::destroyable_group::DestroyableGroups::default();
    groups.reconcile(&mut w);
    assert!(groups.activate(group, &mut s, &mut w, &mut v, &mut a, &mut u));
    assert!(v.instance_handle_for_component(batch).is_none());
    let intact = v
        .instance(v.instance_handle_for_component(source).unwrap())
        .unwrap();
    assert_eq!(
        intact.renderable.mesh, gpu,
        "unpacking must publish the imported body, not its placeholder"
    );
}

#[test]
fn floor_batches_unpack_only_after_success_and_leave_other_floors_combined() {
    let (mut w, mut v, mut a, mut u, mut s, mut d) = (
        World::default(),
        VisualWorld::default(),
        RenderAssets::new(),
        Uploader::default(),
        SystemWorld::default(),
        DestroyableSystem::default(),
    );
    let mut floors = Vec::new();
    let mut batches = Vec::new();
    let mut sources = Vec::new();
    for floor in 0..2 {
        let root = w.add_component(TransformComponent::new());
        let group = attach(&mut w, root, DestroyableComponent::default());
        floors.push(group);
        for material in 0..2 {
            let combine = attach(&mut w, root, CombineMeshComponent::keep_transforms());
            batches.push(combine);
            let (marker, source) = surface(
                &mut w,
                &mut v,
                &mut a,
                &mut u,
                floor as f32 * 8. + material as f32 * 3.,
            );
            let frame = w.parent_of(marker).unwrap();
            if floor != 0 || material != 0 {
                w.remove_component_subtree(marker).unwrap();
            }
            w.set_parent(frame, Some(combine)).unwrap();
            attach(
                &mut w,
                source,
                ColorComponent::rgba(1., material as f32, 0., 1.),
            );
            sources.push(source);
        }
    }
    s.combine_mesh.reconcile_and_build(
        &mut w,
        &mut v,
        &mut a,
        &mut u,
        &mut s.renderable,
        &mut s.mesh_bounds,
    );
    assert_eq!(v.instance_count(), 4);
    let authored = |w: &World| {
        crate::scripting::component_registry::filtered_roots_to_ce_ast(
            w,
            &[w.parent_of(floors[0]).unwrap()],
        )
        .unwrap()
        .iter()
        .map(crate::scripting::unparser::unparse_component)
        .collect::<Vec<_>>()
    };
    let before = authored(&w);
    event(&mut w, [0., 0.05, 0.], 1.);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert_eq!(v.instance_count(), 4, "sub-threshold hits keep every batch");
    event(&mut w, [0., 0.05, 0.], 2.);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert_eq!(
        v.instance_count(),
        4,
        "pending cuts must not draw duplicate cells"
    );
    s.combine_mesh.reconcile_and_build(
        &mut w,
        &mut v,
        &mut a,
        &mut u,
        &mut s.renderable,
        &mut s.mesh_bounds,
    );
    u.fail = true;
    finish(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert_eq!(d.cuts, 0);
    assert_eq!(
        v.instance_count(),
        4,
        "failed upload retains combined floor"
    );
    assert_eq!(
        before,
        authored(&w),
        "inherited piece markers are runtime only"
    );
    u.fail = false;
    event(&mut w, [0., 0.05, 0.], 1.);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    finish(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert_eq!(d.cuts, 1);
    for (i, &batch) in batches.iter().enumerate() {
        assert_eq!(v.instance_handle_for_component(batch).is_some(), i >= 2);
    }
    assert!(
        w.get_component_by_id_as::<RenderableComponent>(sources[0])
            .unwrap()
            .handle
            .is_none(),
        "cut source remains suppressed"
    );
    let intact = w
        .get_component_by_id_as::<RenderableComponent>(sources[1])
        .unwrap()
        .handle
        .unwrap();
    assert_eq!(v.instance(intact).unwrap().color, [1., 1., 0., 1.]);
    assert!(
        w.get_component_by_id_as::<RenderableComponent>(sources[2])
            .unwrap()
            .handle
            .is_none()
    );
    assert_eq!(before, authored(&w));
    let count = v.instance_count();
    s.combine_mesh.reconcile_and_build(
        &mut w,
        &mut v,
        &mut a,
        &mut u,
        &mut s.renderable,
        &mut s.mesh_bounds,
    );
    assert_eq!(
        v.instance_count(),
        count,
        "unpacked floor must never recombine"
    );
    s.bvh.flush_pending(&w);
    assert!(
        s.raycast
            .cast_against_renderables_bvh(&w, &s.bvh, [0., 1., 0.], [0., -1., 0.], 0., 5.)
            .is_empty()
    );
    assert!(
        d.surfaces
            .values()
            .filter(|surface| surface.cells.iter().any(|c| !c.solid.empty()))
            .all(|surface| surface.view.is_some())
    );
    assert!(
        d.surfaces
            .values()
            .flat_map(|surface| &surface.cells)
            .all(|cell| cell.solid.empty()
                || w.get_component_by_id_as::<RenderableComponent>(cell.render)
                    .unwrap()
                    .handle
                    .is_none()),
        "spatial chunks must not add individual draw instances"
    );
    // Intact pieces remain independently damageable after the floor activates.
    event(&mut w, [3., 0.05, 0.], 3.);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    finish(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert_eq!(d.cuts, 2);
    let root = w.parent_of(floors[0]).unwrap();
    s.remove_subtree_immediate(&mut w, &mut v, root);
    for debris in &mut d.debris {
        debris.expires = Instant::now();
    }
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert!(
        d.surfaces.is_empty(),
        "removing a floor retires its inherited surfaces"
    );
    assert_eq!(
        v.instance_count(),
        2,
        "only the untouched floor's batches remain"
    );
}

#[test]
fn empty_cells_retire_immediately_and_sources_cannot_reenter_the_bvh() {
    let (mut w, mut v, mut a, mut u, mut s, mut d) = (
        World::default(),
        VisualWorld::default(),
        RenderAssets::new(),
        Uploader::default(),
        SystemWorld::default(),
        DestroyableSystem::default(),
    );
    let (owner, source) = surface(&mut w, &mut v, &mut a, &mut u, 0.);
    w.add_component(DamageComponent {
        position: [-0.5, 0., -0.5],
        radius: 0.8,
        amount: 3.,
    });
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    finish(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    let cells = &d.surfaces[&owner].cells;
    assert!(cells.iter().any(|c| c.solid.empty()));
    assert!(cells.iter().any(|c| !c.solid.empty()));
    for cell in cells.iter().filter(|c| c.solid.empty()) {
        assert!(w.get_component_record(cell.frame).is_none());
        assert!(w.get_component_record(cell.zone).is_none());
        assert!(w.get_component_record(cell.render).is_none());
    }
    s.bvh.queue_renderable_added(source);
    s.bvh.flush_pending(&w);
    assert!(
        s.bvh.debug_renderable_bounds(source).is_none(),
        "suppressed original must stay out of the index even if registration is retried"
    );
    event(&mut w, [0.7, 0.05, 0.7], 3.);
    pump(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    finish(&mut d, &mut s, &mut w, &mut v, &mut a, &mut u);
    assert_eq!(
        d.cuts, 2,
        "empty stable slots must not invalidate later cuts"
    );
}
