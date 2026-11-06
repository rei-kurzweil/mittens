//! Rigid imported geometry adapter for runtime destruction. Asset data and
//! local triangle hierarchies are shared; damage state belongs to each marker.
use super::*;
use crate::engine::graphics::bounds::Aabb;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
struct Geometry {
    cpu: CpuMeshHandle,
    mesh: Arc<CpuMesh>,
    triangles: Arc<TriangleMesh>,
}
#[derive(Clone, Debug)]
struct Source {
    geometry: Geometry,
    zone: Option<ComponentId>,
}
#[derive(Debug, Default)]
pub(super) struct ImportedSources {
    geometry: HashMap<CpuMeshHandle, Geometry>,
    sources: HashMap<ComponentId, Source>,
    queried_gltfs: HashSet<ComponentId>,
}

fn bounds_component(world: &mut World, source: ComponentId, bounds: Aabb) {
    if let Some(id) = world.children_of(source).iter().copied().find(|&id| {
        world
            .get_component_by_id_as::<BoundsComponent>(id)
            .is_some()
    }) {
        world
            .get_component_by_id_as_mut::<BoundsComponent>(id)
            .unwrap()
            .local = bounds;
    } else {
        attach(world, source, BoundsComponent::new(bounds));
    }
}
fn zone(
    world: &mut World,
    parent: ComponentId,
    geometry: &Arc<TriangleMesh>,
    collision: Option<CollidableComponent>,
) -> ComponentId {
    let half = std::array::from_fn(|i| {
        geometry.bounds.min[i]
            .abs()
            .max(geometry.bounds.max[i].abs())
            .max(0.0001)
    });
    let zone = attach(world, parent, ZoneComponent::cube(half));
    attach(world, zone, SerializeComponent::off());
    if let Some(collision) = collision {
        attach(world, zone, collision);
    }
    attach(
        world,
        zone,
        CutGeometryComponent {
            mesh: geometry.clone(),
        },
    );
    zone
}
impl ImportedSources {
    fn ensure_source(
        &mut self,
        systems: &mut SystemWorld,
        world: &mut World,
        assets: &RenderAssets,
        source: ComponentId,
    ) -> bool {
        if self.sources.contains_key(&source) {
            return true;
        }
        if self.sources.len() >= MAX_SURFACES {
            return false;
        }
        if !world
            .children_of(source)
            .iter()
            .any(|&id| world.get_component_by_id_as::<MeshComponent>(id).is_some())
        {
            return false;
        }
        let Some(cpu) =
            crate::engine::ecs::system::combine_mesh_system::source_mesh(world, assets, source)
        else {
            return false;
        };
        let geometry = if let Some(geometry) = self.geometry.get(&cpu) {
            geometry.clone()
        } else {
            let Some(mesh) = assets.cpu_mesh(cpu) else {
                return false;
            };
            // Large/animated sources need a different strategy. The truck's
            // rigid primitives are below this cap; don't build unbounded BVHs.
            if mesh.indices_u32.len() > 16_384 * 3
                || mesh.joints0.is_some()
                || mesh.weights0.is_some()
                || !mesh.morph_targets.is_empty()
            {
                return false;
            }
            let mesh = Arc::new(mesh.clone());
            let geometry = Geometry {
                cpu,
                triangles: Arc::new(TriangleMesh::new(&mesh)),
                mesh,
            };
            self.geometry.insert(cpu, geometry.clone());
            geometry
        };
        attach(
            world,
            source,
            CutGeometryComponent {
                mesh: geometry.triangles.clone(),
            },
        );
        bounds_component(world, source, geometry.triangles.bounds);
        systems.bvh.queue_renderable_added(source);
        systems.raycast.notify_renderable_added(world, source);
        let destroyable = world.children_of(source).iter().any(|&id| {
            world
                .get_component_by_id_as::<DestroyableComponent>(id)
                .is_some()
        });
        let collision = world.children_of(source).iter().find_map(|&id| {
            world
                .get_component_by_id_as::<CollidableComponent>(id)
                .cloned()
        });
        let zone = destroyable.then(|| zone(world, source, &geometry.triangles, collision));
        self.sources.insert(source, Source { geometry, zone });
        true
    }
    pub(super) fn prepare(
        &mut self,
        systems: &mut SystemWorld,
        world: &mut World,
        assets: &RenderAssets,
    ) {
        self.sources.retain(|&source, _| {
            world
                .get_component_by_id_as::<RenderableComponent>(source)
                .is_some()
        });
        self.queried_gltfs.retain(|&gltf| {
            world
                .get_component_by_id_as::<GLTFComponent>(gltf)
                .is_some()
        });
        let owners: Vec<_> = world.destroyable_ids().collect();
        for owner in owners {
            let Some(source) = world.parent_of(owner) else {
                continue;
            };
            let Some(renderable) = world.get_component_by_id_as::<RenderableComponent>(source)
            else {
                continue;
            };
            let gltf = renderable.material_scope;
            if !self.ensure_source(systems, world, assets, source) {
                continue;
            }
            // Accurate rays must also see the excluded open cabin, rather than
            // letting its bounding box stop shots aimed through window openings.
            if let Some(gltf) = gltf.filter(|gltf| !self.queried_gltfs.contains(gltf)) {
                let mut stack = world.scripting_query_roots(gltf);
                let mut visited = HashSet::new();
                let mut ready = true;
                while let Some(node) = stack.pop() {
                    if !visited.insert(node) {
                        continue;
                    }
                    stack.extend_from_slice(world.children_of(node));
                    if world
                        .get_component_by_id_as::<RenderableComponent>(node)
                        .is_some()
                    {
                        ready &= self.ensure_source(systems, world, assets, node);
                    }
                }
                if ready {
                    self.queried_gltfs.insert(gltf);
                }
            }
        }
    }
    pub(super) fn discover(
        &self,
        world: &World,
        visuals: &VisualWorld,
        owner: ComponentId,
        event: DamageComponent,
    ) -> Option<Surface> {
        let source = world.parent_of(owner)?;
        let record = self.sources.get(&source)?;
        let zone = record.zone?;
        let frame = crate::engine::ecs::system::editor_scene_hit::nearest_transform_ancestor(
            world, source,
        )?;
        let bounds = record.geometry.triangles.bounds;
        let model = TransformSystem::world_model(world, frame)?;
        if !blast_overlaps_bounds(event, model, bounds) {
            return None;
        }
        let mut ancestor = Some(source);
        let mut style = None;
        while let Some(id) = ancestor {
            if let Some(value) = visuals
                .instance_handle_for_component(id)
                .and_then(|h| visuals.instance(h))
            {
                style = Some(*value);
                break;
            }
            ancestor = world.parent_of(id);
        }
        let config = world.get_component_by_id_as::<DestroyableComponent>(owner)?;
        let damage_step = std::array::from_fn(|i| {
            let scale = dot3([model[i][0], model[i][1], model[i][2]])
                .sqrt()
                .max(1e-6);
            (config.cell_size / scale)
                .min(bounds.max[i] - bounds.min[i])
                .max(1e-6)
        });
        Some(Surface {
            source,
            zone,
            frame,
            style: style?,
            damage_step,
            local_bounds: bounds,
            initial_mesh: Some(record.geometry.mesh.clone()),
            imported: true,
            cells: vec![],
            damage: HashMap::new(),
            pending: VecDeque::new(),
            busy: false,
            revision: 0,
            clear: vec![],
            suppression: None,
            zone_suppression: None,
            view: None,
        })
    }
    pub(super) fn partition(
        &self,
        surface: &mut Surface,
        systems: &mut SystemWorld,
        world: &mut World,
    ) -> Result<(), String> {
        let record = self
            .sources
            .get(&surface.source)
            .ok_or("imported source disappeared")?;
        let model = TransformSystem::world_model(world, surface.frame)
            .ok_or("imported source has no transform")?;
        let mut t = TransformComponent::new();
        t.transform.matrix_world = model;
        let frame = attach(world, surface.frame, t);
        attach(world, frame, SerializeComponent::off());
        attach(
            world,
            frame,
            crate::engine::ecs::component::destroyable::DestructionCellComponent,
        );
        let render = attach(
            world,
            frame,
            RenderableComponent::from_cpu_mesh_handle(
                record.geometry.cpu,
                surface.style.renderable.material,
            ),
        );
        inherit_material_scope(world, surface.source, render);
        attach(world, render, RaycastableComponent::enabled());
        bounds_component(world, render, surface.local_bounds);
        attach(
            world,
            render,
            CutGeometryComponent {
                mesh: record.geometry.triangles.clone(),
            },
        );
        let collision = record.zone.and_then(|zone| {
            world.children_of(zone).iter().find_map(|&id| {
                world
                    .get_component_by_id_as::<CollidableComponent>(id)
                    .cloned()
            })
        });
        let zone = zone(world, frame, &record.geometry.triangles, collision);
        // Placeholder state only: never used for imported subtraction or drawn.
        // First import/cut runs on the worker and replaces it with the true solid.
        surface.cells.push(Cell {
            frame,
            render,
            zone,
            solid: Solid::box_between(surface.local_bounds.min, surface.local_bounds.max),
            mesh: None,
        });
        systems.bvh.queue_renderable_added(render);
        systems.raycast.notify_renderable_added(world, render);
        for (target, is_zone) in [(surface.source, false), (surface.zone, true)] {
            let marker = attach(
                world,
                target,
                crate::engine::ecs::component::destroyable::DestroyedSourceComponent,
            );
            attach(world, marker, SerializeComponent::off());
            if is_zone {
                surface.zone_suppression = Some(marker)
            } else {
                surface.suppression = Some(marker)
            }
        }
        systems.bvh.queue_renderable_removed(surface.source);
        systems.raycast.notify_renderable_removed(surface.source);
        Ok(())
    }
}

pub(super) fn inherit_material_scope(world: &mut World, source: ComponentId, target: ComponentId) {
    let scope = world
        .get_component_by_id_as::<RenderableComponent>(source)
        .and_then(|r| r.material_scope);
    if let Some(renderable) = world.get_component_by_id_as_mut::<RenderableComponent>(target) {
        renderable.material_scope = scope;
    }
}
