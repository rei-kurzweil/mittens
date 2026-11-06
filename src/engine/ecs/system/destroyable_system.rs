//! Sparse CPU damage, stable per-surface grid, and transactional local mesh cuts.
use super::{
    SystemWorld, TransformSystem,
    csg_system::{CsgSystem, CutJob, MeshCutJob},
    triangle_collision::TriangleMesh,
};
use crate::engine::ecs::component::*;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{
    RenderAssets, RenderUploader, VisualWorld,
    bounds::Aabb,
    csg_mesh::Solid,
    mesh::CpuMesh,
    primitives::{CpuMeshHandle, GpuRenderable, InstanceHandle, TransformMatrix},
    visual_world::VisualInstance,
};
#[path = "destroyable_imported.rs"]
mod imported;
use crate::utils::math::{mat4_inverse, mat4_mul, mat4_mul_vec4};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Instant,
};
// Enough source slots for every piece of the eight-storey player building.
// Chunk and triangle budgets still independently bound generated geometry.
const MAX_SURFACES: usize = 512;
const MAX_CHUNKS: usize = 512;
const MAX_DAMAGE_CELLS: usize = 4096;
const MAX_DEBRIS: usize = 96;
const MAX_SCENE_TRIANGLES: usize = 262_144;
const MAX_SCENE_CHUNKS: usize = 4096;

#[derive(Debug)]
struct Cell {
    frame: ComponentId,
    render: ComponentId,
    zone: ComponentId,
    solid: Solid,
    mesh: Option<CpuMeshHandle>,
}
#[derive(Debug)]
struct Surface {
    source: ComponentId,
    zone: ComponentId,
    frame: ComponentId,
    style: VisualInstance,
    damage_step: [f32; 3],
    local_bounds: Aabb,
    initial_mesh: Option<Arc<CpuMesh>>,
    imported: bool,
    cells: Vec<Cell>,
    damage: HashMap<[i32; 3], f32>,
    pending: VecDeque<DamageComponent>,
    busy: bool,
    revision: u64,
    clear: Vec<[i32; 3]>,
    suppression: Option<ComponentId>,
    zone_suppression: Option<ComponentId>,
    view: Option<SurfaceView>,
}
#[derive(Debug)]
struct SurfaceView {
    frame: ComponentId,
    render: ComponentId,
    mesh: CpuMeshHandle,
}
#[derive(Debug)]
struct Debris {
    root: ComponentId,
    render: ComponentId,
    mesh: CpuMeshHandle,
    velocity: [f32; 3],
    expires: Instant,
}
#[derive(Debug, Default)]
pub struct DestroyableSystem {
    imported: imported::ImportedSources,
    groups: super::destroyable_group::DestroyableGroups,
    csg: Option<CsgSystem>,
    surfaces: HashMap<ComponentId, Surface>,
    debris: VecDeque<Debris>,
    last_tick: Option<Instant>,
    pub cuts: u64,
    pub last_cut_time: std::time::Duration,
}
impl DestroyableSystem {
    pub fn prepare(
        &mut self,
        systems: &mut SystemWorld,
        world: &mut World,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        uploader: &mut dyn RenderUploader,
    ) {
        self.groups.reconcile(world);
        self.imported.prepare(systems, world, assets);
        let now = Instant::now();
        let dt = self
            .last_tick
            .replace(now)
            .map(|t| now.duration_since(t).as_secs_f32().min(0.1))
            .unwrap_or(0.);
        let removed: Vec<_> = self
            .surfaces
            .keys()
            .copied()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<DestroyableComponent>(id)
                    .is_none()
            })
            .collect();
        for id in removed {
            if let Some(surface) = self.surfaces.remove(&id) {
                if let Some(view) = surface.view {
                    retire(
                        systems,
                        world,
                        visuals,
                        assets,
                        uploader,
                        view.frame,
                        view.render,
                        Some(view.mesh),
                    );
                }
                for cell in surface.cells {
                    retire(
                        systems,
                        world,
                        visuals,
                        assets,
                        uploader,
                        cell.frame,
                        cell.render,
                        cell.mesh,
                    );
                }
                if let Some(marker) = surface.suppression {
                    let _ = world.remove_component_subtree(marker);
                }
                if let Some(marker) = surface.zone_suppression {
                    let _ = world.remove_component_subtree(marker);
                }
                if world
                    .get_component_by_id_as::<RenderableComponent>(surface.source)
                    .is_some()
                {
                    systems
                        .renderable
                        .register_renderable(world, visuals, surface.source);
                    systems.bvh.queue_renderable_added(surface.source);
                    systems
                        .raycast
                        .notify_renderable_added(world, surface.source);
                }
            }
        }
        let exhausted: Vec<_> = self
            .surfaces
            .iter()
            .filter(|(_, s)| {
                !s.busy && !s.cells.is_empty() && s.cells.iter().all(|c| c.solid.empty())
            })
            .map(|(&id, _)| id)
            .collect();
        for id in exhausted {
            let surface = self.surfaces.remove(&id).unwrap();
            if let Some(view) = surface.view {
                retire(
                    systems,
                    world,
                    visuals,
                    assets,
                    uploader,
                    view.frame,
                    view.render,
                    Some(view.mesh),
                );
            }
            if let Some(config) = world.get_component_by_id_as_mut::<DestroyableComponent>(id) {
                config.exhausted = true;
            }
            for cell in surface.cells {
                retire(
                    systems,
                    world,
                    visuals,
                    assets,
                    uploader,
                    cell.frame,
                    cell.render,
                    cell.mesh,
                );
            }
        }
        while let Some(result) = self.csg.as_ref().and_then(CsgSystem::poll) {
            self.last_cut_time = result.elapsed;
            let total_triangles: usize = self
                .surfaces
                .values()
                .flat_map(|s| &s.cells)
                .filter(|c| c.mesh.is_some())
                .map(|c| c.solid.triangle_count())
                .sum();
            let Some(surface) = self.surfaces.get_mut(&result.owner) else {
                continue;
            };
            if !surface.busy || surface.revision != result.revision {
                continue;
            }
            if world
                .get_component_by_id_as::<TransformComponent>(surface.frame)
                .is_none()
                || surface.cells.iter().filter(|c| !c.solid.empty()).any(|c| {
                    world
                        .get_component_by_id_as::<RenderableComponent>(c.render)
                        .is_none()
                })
            {
                surface.busy = false;
                surface.pending.clear();
                surface.clear.clear();
                continue;
            }
            surface.busy = false;
            let changes = match result.result {
                Ok(c) => c,
                Err(e) => {
                    surface.clear.clear();
                    eprintln!("[Destroyable] cut failed: {e}");
                    continue;
                }
            };
            let old_triangles: usize = changes
                .cells
                .iter()
                .map(|c| &surface.cells[c.index])
                .filter(|c| c.mesh.is_some())
                .map(|c| c.solid.triangle_count())
                .sum();
            let new_triangles: usize = changes.cells.iter().map(|c| c.solid.triangle_count()).sum();
            if total_triangles - old_triangles + new_triangles > MAX_SCENE_TRIANGLES {
                surface.clear.clear();
                eprintln!(
                    "[Destroyable] scene triangle budget reached; previous geometry retained"
                );
                continue;
            }
            // Upload one batched visual before publishing any cell changes.
            let changed = !changes.cells.is_empty();
            if !changed {
                surface.clear.clear();
                continue;
            }
            let render_mesh = changes
                .render_mesh
                .expect("changed cells have a batched visual");
            let render_upload = if render_mesh.vertices.is_empty() {
                None
            } else {
                let cpu = assets.register_transient_mesh(render_mesh);
                match assets.gpu_mesh_handle(uploader, cpu) {
                    Ok(gpu) => Some((cpu, gpu)),
                    Err(e) => {
                        assets.release_transient_mesh(uploader, cpu);
                        surface.clear.clear();
                        eprintln!("[Destroyable] upload failed: {e}");
                        continue;
                    }
                }
            };
            let pending_group = self.groups.pending_group(result.owner).or_else(|| {
                systems
                    .combine_mesh
                    .owner_of(world, surface.source)
                    .and_then(|_| self.groups.pending_source_group(world, surface.source))
            });
            if let Some(group) = pending_group {
                if !self
                    .groups
                    .activate(group, systems, world, visuals, assets, uploader)
                {
                    if let Some((cpu, _)) = render_upload {
                        assets.release_transient_mesh(uploader, cpu);
                    }
                    surface.clear.clear();
                    continue;
                }
            }
            let prepared: Vec<_> = changes
                .cells
                .into_iter()
                .map(|change| {
                    let cpu = (!change.mesh.vertices.is_empty())
                        .then(|| assets.register_transient_mesh(change.mesh.clone()));
                    (change, cpu)
                })
                .collect();
            surface.initial_mesh = None;
            for (change, upload) in prepared {
                let cell = &mut surface.cells[change.index];
                let old_mesh = cell.mesh.take();
                systems
                    .renderable
                    .remove_renderable(world, visuals, cell.render);
                systems.bvh.queue_renderable_removed(cell.render);
                systems.raycast.notify_renderable_removed(cell.render);
                let geometry_ids: Vec<_> = world
                    .children_of(cell.zone)
                    .iter()
                    .chain(world.children_of(cell.render))
                    .copied()
                    .filter(|&id| {
                        world
                            .get_component_by_id_as::<CutGeometryComponent>(id)
                            .is_some()
                    })
                    .collect();
                for id in geometry_ids {
                    let _ = world.remove_component_subtree(id);
                }
                if let Some(cpu) = upload {
                    let geometry = Arc::new(TriangleMesh::new(&change.mesh));
                    let bounds = world
                        .children_of(cell.render)
                        .iter()
                        .copied()
                        .find(|&id| {
                            world
                                .get_component_by_id_as::<BoundsComponent>(id)
                                .is_some()
                        })
                        .unwrap();
                    world
                        .get_component_by_id_as_mut::<BoundsComponent>(bounds)
                        .unwrap()
                        .local = geometry.bounds;
                    for owner in [cell.zone, cell.render] {
                        attach(
                            world,
                            owner,
                            CutGeometryComponent {
                                mesh: geometry.clone(),
                            },
                        );
                    }
                    let r = world
                        .get_component_by_id_as_mut::<RenderableComponent>(cell.render)
                        .unwrap();
                    r.renderable.mesh = cpu;
                    r.renderable.base_mesh = cpu;
                    cell.mesh = Some(cpu);
                    systems.bvh.queue_renderable_added(cell.render);
                    systems.raycast.notify_renderable_added(world, cell.render);
                } else {
                    // Keep the stable worker slot, but retire all empty-cell
                    // scene resources immediately instead of retaining dead ECS
                    // subtrees until the entire authored primitive disappears.
                    retire(
                        systems,
                        world,
                        visuals,
                        assets,
                        uploader,
                        cell.frame,
                        cell.render,
                        None,
                    );
                }
                cell.solid = change.solid;
                if let Some(old) = old_mesh {
                    assets.release_transient_mesh(uploader, old);
                }
            }
            systems
                .renderable
                .suppress_renderable(world, visuals, surface.source);
            if let Some(view) = surface.view.take() {
                retire(
                    systems,
                    world,
                    visuals,
                    assets,
                    uploader,
                    view.frame,
                    view.render,
                    Some(view.mesh),
                );
            }
            let model = TransformSystem::world_model(world, surface.frame).unwrap();
            if let Some((cpu, gpu)) = render_upload {
                let mut transform = TransformComponent::new();
                transform.transform.matrix_world = model;
                let frame = attach(world, surface.frame, transform);
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
                        cpu,
                        surface.style.renderable.material,
                    ),
                );
                if surface.imported {
                    imported::inherit_material_scope(world, surface.source, render);
                }
                attach(world, render, RaycastableComponent::disabled());
                let handle = register_visual(visuals, render, gpu, model, surface.style);
                world
                    .get_component_by_id_as_mut::<RenderableComponent>(render)
                    .unwrap()
                    .handle = Some(handle);
                surface.view = Some(SurfaceView {
                    frame,
                    render,
                    mesh: cpu,
                });
            }
            for mesh in changes.debris {
                while self.debris.len() >= MAX_DEBRIS {
                    let d = self.debris.pop_front().unwrap();
                    retire(
                        systems,
                        world,
                        visuals,
                        assets,
                        uploader,
                        d.root,
                        d.render,
                        Some(d.mesh),
                    );
                }
                if let Some(d) = spawn_debris(
                    world,
                    visuals,
                    assets,
                    uploader,
                    mesh,
                    model,
                    surface.style,
                    now,
                ) {
                    self.debris.push_back(d);
                }
            }
            for key in surface.clear.drain(..) {
                surface.damage.remove(&key);
            }
            if changed {
                self.cuts += 1;
            }
        }
        for _ in 0..self.debris.len() {
            let mut d = self.debris.pop_front().unwrap();
            if now >= d.expires
                || world
                    .get_component_by_id_as::<TransformComponent>(d.root)
                    .is_none()
            {
                retire(
                    systems,
                    world,
                    visuals,
                    assets,
                    uploader,
                    d.root,
                    d.render,
                    Some(d.mesh),
                );
                continue;
            }
            d.velocity[1] -= 9.81 * dt;
            let t = world
                .get_component_by_id_as_mut::<TransformComponent>(d.root)
                .unwrap();
            for i in 0..3 {
                t.transform.translation[i] += d.velocity[i] * dt;
            }
            t.transform.recompute_model();
            t.transform.matrix_world = t.transform.model;
            if let Some(handle) = world
                .get_component_by_id_as::<RenderableComponent>(d.render)
                .and_then(|r| r.handle)
            {
                visuals.update_model(handle, TransformSystem::world_model(world, d.root).unwrap());
            }
            self.debris.push_back(d);
        }
        let mut events: Vec<_> = world
            .damage_ids()
            .filter_map(|id| {
                world
                    .get_component_by_id_as::<DamageComponent>(id)
                    .copied()
                    .map(|e| (id, e))
            })
            .collect();
        events.sort_unstable_by_key(|(id, _)| *id);
        for (id, event) in events {
            let _ = world.remove_component_subtree(id);
            if event.radius <= 0.
                || event.amount <= 0.
                || !event
                    .position
                    .into_iter()
                    .chain([event.radius, event.amount])
                    .all(f32::is_finite)
            {
                continue;
            }
            let owners: Vec<_> = world
                .destroyable_ids()
                .filter(|&id| {
                    world
                        .get_component_by_id_as::<DestroyableComponent>(id)
                        .is_some_and(|c| !c.exhausted)
                })
                .collect();
            for owner in owners {
                if !self.surfaces.contains_key(&owner) {
                    if self.surfaces.len() >= MAX_SURFACES {
                        continue;
                    }
                    if let Some(surface) = self
                        .imported
                        .discover(world, visuals, owner, event)
                        .or_else(|| discover(world, visuals, owner, event))
                    {
                        self.surfaces.insert(owner, surface);
                    }
                }
                if let Some(surface) = self.surfaces.get_mut(&owner) {
                    let Some(model) = TransformSystem::world_model(world, surface.frame) else {
                        continue;
                    };
                    if !blast_overlaps_bounds(event, model, surface.local_bounds) {
                        continue;
                    }
                    if surface.pending.len() < 32 {
                        let mut queued = event;
                        if surface.imported {
                            let Some(inverse) = mat4_inverse(model) else {
                                continue;
                            };
                            queued.position = point(inverse, event.position);
                        }
                        surface.pending.push_back(queued);
                    }
                }
            }
        }
        let mut chunk_budget = MAX_SCENE_CHUNKS.saturating_sub(
            self.surfaces
                .values()
                .map(|s| s.cells.iter().filter(|c| !c.solid.empty()).count())
                .sum(),
        );
        for (&owner, surface) in &mut self.surfaces {
            if surface.busy {
                continue;
            }
            let Some(config) = world
                .get_component_by_id_as::<DestroyableComponent>(owner)
                .copied()
            else {
                continue;
            };
            let Some(queued_event) = surface.pending.pop_front() else {
                continue;
            };
            let model = TransformSystem::world_model(world, surface.frame).unwrap();
            let mut event = queued_event;
            if surface.imported {
                event.position = point(model, queued_event.position);
            }
            let Some(inverse) = mat4_inverse(model) else {
                continue;
            };
            let local = point(inverse, event.position);
            let scale: [f32; 3] =
                std::array::from_fn(|i| dot3([model[i][0], model[i][1], model[i][2]]).sqrt());
            if scale.iter().any(|&s| s < 1e-5) {
                continue;
            }
            let step = surface.damage_step;
            let bounds = surface.local_bounds;
            let extent: [f32; 3] = std::array::from_fn(|i| bounds.max[i] - bounds.min[i]);
            let lo: [i32; 3] = std::array::from_fn(|i| {
                (((local[i] - bounds.min[i] - event.radius / scale[i]) / step[i]).floor() as i32)
                    .max(0)
            });
            let hi: [i32; 3] = std::array::from_fn(|i| {
                (((local[i] - bounds.min[i] + event.radius / scale[i]) / step[i]).floor() as i32)
                    .min((extent[i] / step[i]).ceil() as i32 - 1)
            });
            if (0..3)
                .map(|i| (hi[i] as i64 - lo[i] as i64 + 1).max(0) as u64)
                .product::<u64>()
                > MAX_DAMAGE_CELLS as u64
            {
                continue;
            }
            let mut covered = Vec::new();
            let mut crossed = false;
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        let key = [x, y, z];
                        let p = std::array::from_fn(|i| {
                            bounds.min[i]
                                + (key[i] as f32 * step[i]
                                    + ((key[i] + 1) as f32 * step[i]).min(extent[i]))
                                    * 0.5
                        });
                        if (0..3).any(|i| p[i] < bounds.min[i] || p[i] > bounds.max[i]) {
                            continue;
                        }
                        let wp = point(model, p);
                        if (0..3)
                            .map(|i| (wp[i] - event.position[i]).powi(2))
                            .sum::<f32>()
                            > (event.radius + config.cell_size).powi(2)
                        {
                            continue;
                        }
                        if surface.damage.len() >= MAX_DAMAGE_CELLS
                            && !surface.damage.contains_key(&key)
                        {
                            continue;
                        }
                        let value = surface.damage.entry(key).or_default();
                        *value += event.amount;
                        crossed |= *value >= config.threshold;
                        covered.push(key);
                    }
                }
            }
            if !crossed {
                continue;
            }
            if surface.cells.is_empty() {
                let partitioned = if surface.imported {
                    if chunk_budget == 0 {
                        Err("scene chunk budget reached".into())
                    } else {
                        self.imported.partition(surface, systems, world)
                    }
                } else {
                    partition(surface, config, chunk_budget, systems, world)
                };
                if let Err(e) = partitioned {
                    eprintln!("[Destroyable] {e}");
                    continue;
                }
                chunk_budget = chunk_budget.saturating_sub(surface.cells.len());
            }
            let mut cells = Vec::new();
            for (index, cell) in surface.cells.iter().enumerate() {
                if cell.solid.empty() {
                    continue;
                }
                let model = TransformSystem::world_model(world, cell.frame).unwrap();
                if !if surface.imported {
                    blast_overlaps_bounds(event, model, surface.local_bounds)
                } else {
                    blast_overlaps(event, model)
                } {
                    continue;
                }
                let Some(inverse) = mat4_inverse(model) else {
                    continue;
                };
                cells.push((
                    index,
                    cell.solid.clone(),
                    Solid::world_blast(event.position, event.radius, inverse),
                    world
                        .get_component_by_id_as::<TransformComponent>(cell.frame)
                        .unwrap()
                        .transform
                        .model,
                ));
            }
            if cells.is_empty() {
                for key in covered {
                    surface.damage.remove(&key);
                }
                continue;
            }
            surface.revision += 1;
            let job = CutJob {
                owner,
                revision: surface.revision,
                cells,
                render_cells: surface
                    .cells
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| !c.solid.empty())
                    .map(|(i, c)| {
                        (
                            i,
                            c.solid.clone(),
                            world
                                .get_component_by_id_as::<TransformComponent>(c.frame)
                                .unwrap()
                                .transform
                                .model,
                        )
                    })
                    .collect(),
            };
            let worker = self.csg.get_or_insert_with(CsgSystem::default);
            let submitted = if let Some(mesh) = &surface.initial_mesh {
                let inverse = mat4_inverse(model).unwrap();
                worker
                    .submit_mesh(MeshCutJob {
                        owner,
                        revision: surface.revision,
                        mesh: mesh.clone(),
                        cutter: Solid::world_blast(event.position, event.radius, inverse),
                    })
                    .is_ok()
            } else {
                worker.submit(job).is_ok()
            };
            if submitted {
                surface.busy = true;
                surface.clear = covered;
            } else {
                // Backpressure retries the event without counting its damage twice.
                for key in covered {
                    if let Some(value) = surface.damage.get_mut(&key) {
                        *value = (*value - event.amount).max(0.);
                    }
                }
                surface.damage.retain(|_, value| *value > 0.);
                surface.pending.push_front(queued_event);
            }
        }
    }
}
fn dot3(a: [f32; 3]) -> f32 {
    a.into_iter().map(|v| v * v).sum()
}
fn point(m: TransformMatrix, p: [f32; 3]) -> [f32; 3] {
    let q = mat4_mul_vec4(m, [p[0], p[1], p[2], 1.]);
    [q[0], q[1], q[2]]
}
pub(super) fn attach<T: Component>(
    world: &mut World,
    parent: ComponentId,
    value: T,
) -> ComponentId {
    let id = world.add_component(value);
    world.set_parent(id, Some(parent)).unwrap();
    id
}
fn blast_overlaps(event: DamageComponent, model: TransformMatrix) -> bool {
    blast_overlaps_bounds(
        event,
        model,
        Aabb {
            min: [-0.5; 3],
            max: [0.5; 3],
        },
    )
}
fn blast_overlaps_bounds(event: DamageComponent, model: TransformMatrix, bounds: Aabb) -> bool {
    let bounds = bounds.transformed(model);
    (0..3)
        .map(|i| {
            (event.position[i] - event.position[i].clamp(bounds.min[i], bounds.max[i])).powi(2)
        })
        .sum::<f32>()
        <= event.radius * event.radius
}
fn discover(
    world: &World,
    visuals: &VisualWorld,
    owner: ComponentId,
    event: DamageComponent,
) -> Option<Surface> {
    let frame = world.parent_of(owner)?;
    world.get_component_by_id_as::<TransformComponent>(frame)?;
    let source = world.children_of(frame).iter().copied().find(|&id| {
        world
            .get_component_by_id_as::<RenderableComponent>(id)
            .is_some_and(|r| r.renderable.base_mesh == CpuMeshHandle::CUBE)
    })?;
    let zone = world.children_of(frame).iter().copied().find(|&id| {
        world
            .get_component_by_id_as::<ZoneComponent>(id)
            .is_some_and(|z| z.enabled && z.shape == CollisionShape::CUBE())
    })?;
    if !blast_overlaps(event, TransformSystem::world_model(world, frame)?) {
        return None;
    }
    // Combined sources deliberately have no individual draw handle. Their
    // batch carries the shared appearance until this floor is unpacked.
    let mut visual_source = Some(source);
    let mut style = None;
    while let Some(id) = visual_source {
        if let Some(instance) = visuals
            .instance_handle_for_component(id)
            .and_then(|h| visuals.instance(h))
        {
            style = Some(*instance);
            break;
        }
        visual_source = world.parent_of(id);
    }
    let style = style?;
    let model = TransformSystem::world_model(world, frame)?;
    let config = world.get_component_by_id_as::<DestroyableComponent>(owner)?;
    let damage_step = std::array::from_fn(|i| {
        (config.cell_size / dot3([model[i][0], model[i][1], model[i][2]]).sqrt()).min(1.)
    });
    Some(Surface {
        source,
        zone,
        frame,
        style,
        damage_step,
        local_bounds: Aabb {
            min: [-0.5; 3],
            max: [0.5; 3],
        },
        initial_mesh: None,
        imported: false,
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
fn partition(
    surface: &mut Surface,
    config: DestroyableComponent,
    budget: usize,
    systems: &mut SystemWorld,
    world: &mut World,
) -> Result<(), String> {
    let model = TransformSystem::world_model(world, surface.frame).unwrap();
    let counts: [usize; 3] = std::array::from_fn(|i| {
        (dot3([model[i][0], model[i][1], model[i][2]]).sqrt() / config.chunk_size)
            .ceil()
            .max(1.) as usize
    });
    if counts
        .iter()
        .try_fold(1usize, |total, n| total.checked_mul(*n))
        .is_none_or(|n| n > MAX_CHUNKS.min(budget))
    {
        return Err("surface exceeds 512 chunks; increase chunk_size".into());
    }
    let intact = Solid::box_between([-0.5; 3], [0.5; 3]);
    for x in 0..counts[0] {
        for y in 0..counts[1] {
            for z in 0..counts[2] {
                let index = [x, y, z];
                let size = counts.map(|n| 1. / n as f32);
                let center: [f32; 3] =
                    std::array::from_fn(|i| -0.5 + (index[i] as f32 + 0.5) * size[i]);
                let mut t = TransformComponent::new()
                    .with_position(center[0], center[1], center[2])
                    .with_scale(size[0], size[1], size[2]);
                t.transform.matrix_world = mat4_mul(model, t.transform.model);
                let frame = attach(world, surface.frame, t);
                attach(world, frame, SerializeComponent::off());
                attach(
                    world,
                    frame,
                    crate::engine::ecs::component::destroyable::DestructionCellComponent,
                );
                let render = attach(world, frame, RenderableComponent::cube());
                attach(world, render, RaycastableComponent::enabled());
                attach(
                    world,
                    render,
                    BoundsComponent::new(crate::engine::graphics::bounds::Aabb {
                        min: [-0.5; 3],
                        max: [0.5; 3],
                    }),
                );
                systems.bvh.queue_renderable_added(render);
                systems.raycast.notify_renderable_added(world, render);
                let zone = attach(world, frame, ZoneComponent::cube([0.5; 3]));
                attach(
                    world,
                    zone,
                    CollidableComponent::static_().with_friction(0.85).unwrap(),
                );
                surface.cells.push(Cell {
                    frame,
                    render,
                    zone,
                    solid: intact.clone(),
                    mesh: None,
                });
            }
        }
    }
    systems.bvh.queue_renderable_removed(surface.source);
    systems.raycast.notify_renderable_removed(surface.source);
    let suppression = attach(
        world,
        surface.source,
        crate::engine::ecs::component::destroyable::DestroyedSourceComponent,
    );
    attach(world, suppression, SerializeComponent::off());
    surface.suppression = Some(suppression);
    let suppression = attach(
        world,
        surface.zone,
        crate::engine::ecs::component::destroyable::DestroyedSourceComponent,
    );
    attach(world, suppression, SerializeComponent::off());
    surface.zone_suppression = Some(suppression);
    Ok(())
}
pub(super) fn register_visual(
    visuals: &mut VisualWorld,
    render: ComponentId,
    mesh: crate::engine::graphics::MeshHandle,
    model: TransformMatrix,
    style: VisualInstance,
) -> InstanceHandle {
    let mut transform = style.transform;
    transform.model = model;
    transform.matrix_world = model;
    let handle = visuals.register(
        render,
        GpuRenderable::new(mesh, style.renderable.material),
        transform,
        style.color,
        style.opacity,
        style.multiple_layers,
        style.transparent_cutout,
        style.background,
        style.background_occluded_lit,
        style.overlay,
        style.emissive,
        style.texture,
        style.quant_steps,
    );
    visuals.bind_material_instance(handle, style.material_instance);
    visuals.update_texture_filtering(handle, style.texture_filtering);
    handle
}
fn spawn_debris(
    world: &mut World,
    visuals: &mut VisualWorld,
    assets: &mut RenderAssets,
    uploader: &mut dyn RenderUploader,
    mut mesh: CpuMesh,
    model: TransformMatrix,
    style: VisualInstance,
    now: Instant,
) -> Option<Debris> {
    // Bake into world coordinates; debris is independent of subsequent owner motion.
    let inverse = mat4_inverse(model)?;
    for v in &mut mesh.vertices {
        v.pos = point(model, v.pos);
        v.normal = crate::utils::math::vec3_normalize(std::array::from_fn(|i| {
            (0..3).map(|j| inverse[i][j] * v.normal[j]).sum()
        }));
    }
    let cpu = assets.register_transient_mesh(mesh);
    let gpu = match assets.gpu_mesh_handle(uploader, cpu) {
        Ok(g) => g,
        Err(_) => {
            assets.release_transient_mesh(uploader, cpu);
            return None;
        }
    };
    let root = world.add_component(TransformComponent::new());
    attach(world, root, SerializeComponent::off());
    let render = attach(
        world,
        root,
        RenderableComponent::from_cpu_mesh_handle(cpu, style.renderable.material),
    );
    let handle = register_visual(
        visuals,
        render,
        gpu,
        crate::utils::math::mat4_identity(),
        style,
    );
    world
        .get_component_by_id_as_mut::<RenderableComponent>(render)
        .unwrap()
        .handle = Some(handle);
    Some(Debris {
        root,
        render,
        mesh: cpu,
        velocity: [0.3, 1.2, 0.2],
        expires: now + std::time::Duration::from_secs(4),
    })
}
fn retire(
    systems: &mut SystemWorld,
    world: &mut World,
    visuals: &mut VisualWorld,
    assets: &mut RenderAssets,
    uploader: &mut dyn RenderUploader,
    root: ComponentId,
    render: ComponentId,
    mesh: Option<CpuMeshHandle>,
) {
    if let Some(handle) = visuals.instance_handle_for_component(render) {
        visuals.remove(handle);
    }
    systems.bvh.queue_renderable_removed(render);
    systems.raycast.notify_renderable_removed(render);
    systems.remove_subtree_immediate(world, visuals, root);
    if let Some(mesh) = mesh {
        assets.release_transient_mesh(uploader, mesh);
    }
}

#[cfg(test)]
#[path = "destroyable_tests.rs"]
mod tests;
