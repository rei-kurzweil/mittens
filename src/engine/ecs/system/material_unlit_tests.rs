use super::*;
use crate::engine::ecs::component::{RenderableComponent, TransformComponent};
use crate::engine::ecs::system::RenderableSystem;
use crate::engine::ecs::{CommandQueue, World};
use crate::engine::graphics::{CpuMesh, MeshHandle, MeshUploader, RenderAssets, VisualWorld};

#[derive(Default)]
struct Uploader(u32);
impl MeshUploader for Uploader {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(MeshHandle(self.0))
    }
}

#[test]
fn inherited_unlit_rebinds_existing_draws_and_does_not_bloom() {
    let mut world = World::default();
    let mut visuals = VisualWorld::default();
    let mut system = RenderableSystem::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let mut uploader = Uploader::default();
    let root = world.add_component(TransformComponent::new());
    let frame = world.add_component(TransformComponent::new());
    let renderable = world.add_component(RenderableComponent::cube());
    world.add_child(root, frame).unwrap();
    world.add_child(frame, renderable).unwrap();
    world.init_component_tree(root, &mut crate::engine::ecs::RxWorld::default());
    system.register_renderable_from_world(&mut world, &mut visuals, renderable);
    system.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut uploader,
        &mut queue,
    );
    let handle = world
        .get_component_by_id_as::<RenderableComponent>(renderable)
        .unwrap()
        .get_handle()
        .unwrap();
    assert_eq!(
        visuals.instance(handle).unwrap().renderable.material,
        MaterialHandle::TOON_MESH
    );
    let unlit = world.add_component(UnlitComponent);
    world.add_child(root, unlit).unwrap();
    system.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut uploader,
        &mut queue,
    );
    assert_eq!(
        visuals.instance(handle).unwrap().renderable.material,
        MaterialHandle::UNLIT_MESH
    );
    let emissive = world.add_component(crate::engine::ecs::component::EmissiveComponent::new(2.0));
    world.add_child(renderable, emissive).unwrap();
    system.register_emissive(&mut world, &mut visuals, emissive);
    system.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut uploader,
        &mut queue,
    );
    visuals.prepare_draw_cache();
    assert_eq!(
        visuals.instance(handle).unwrap().renderable.material,
        MaterialHandle::UNLIT_MESH
    );
    assert!(visuals.emissive_draw_batches().is_empty());
    world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::EmissiveComponent>(emissive)
        .unwrap()
        .intensity = 0.0;
    system.register_emissive(&mut world, &mut visuals, emissive);
    system.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut uploader,
        &mut queue,
    );
    world.detach_child(renderable, emissive);
    world.detach_child(root, unlit);
    system.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut uploader,
        &mut queue,
    );
    assert_eq!(
        visuals.instance(handle).unwrap().renderable.material,
        MaterialHandle::TOON_MESH
    );
    let unlit_root = world.add_component(TransformComponent::new());
    world.add_child(unlit_root, unlit).unwrap();
    world.add_child(unlit_root, frame).unwrap();
    system.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut uploader,
        &mut queue,
    );
    assert_eq!(
        visuals.instance(handle).unwrap().renderable.material,
        MaterialHandle::UNLIT_MESH
    );
    world.add_child(root, frame).unwrap();
    system.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut uploader,
        &mut queue,
    );
    assert_eq!(
        visuals.instance(handle).unwrap().renderable.material,
        MaterialHandle::TOON_MESH
    );
}

#[test]
fn local_material_overrides_parent_unlit_scope() {
    let mut world = World::default();
    let root = world.add_component(TransformComponent::new());
    let marker = world.add_component(UnlitComponent);
    let cube = world.add_component(RenderableComponent::cube());
    let local = world.add_component(ShadingComponent::toon());
    world.add_child(root, marker).unwrap();
    world.add_child(root, cube).unwrap();
    world.add_child(cube, local).unwrap();
    assert_eq!(
        MaterialSystem::resolve_surface(&world, cube),
        Some(SurfaceSource::Material(local))
    );
}
