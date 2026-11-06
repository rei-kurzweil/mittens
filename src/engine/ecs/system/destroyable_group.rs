//! Floor-sized destruction owners retain their authored batches until a cut commits.
use std::collections::HashMap;

use super::destroyable_system::{attach, register_visual};
use super::{SystemWorld, TransformSystem};
use crate::engine::ecs::component::*;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::primitives::CpuMeshHandle;
use crate::engine::graphics::{RenderAssets, RenderUploader, VisualWorld};

#[derive(Debug)]
struct Group {
    pieces: Vec<ComponentId>,
    members: Vec<ComponentId>,
    active: bool,
}

#[derive(Debug, Default)]
pub(super) struct DestroyableGroups {
    groups: HashMap<ComponentId, Group>,
    owners: HashMap<ComponentId, ComponentId>,
}

impl DestroyableGroups {
    /// Expand only new aggregate owners. Piece markers are derived, never saved.
    /// A piece-local Destroyable overrides the inherited floor settings.
    pub fn reconcile(&mut self, world: &mut World) {
        let removed: Vec<_> = self
            .groups
            .keys()
            .copied()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<DestroyableComponent>(id)
                    .is_none()
            })
            .collect();
        for id in removed {
            if let Some(group) = self.groups.remove(&id) {
                for member in group.members {
                    self.owners.remove(&member);
                }
                for piece in group.pieces {
                    let _ = world.remove_component_subtree(piece);
                }
            }
        }
        let ids: Vec<_> = world.destroyable_ids().collect();
        for id in ids {
            if self.owners.contains_key(&id) {
                continue;
            }
            let config = *world
                .get_component_by_id_as::<DestroyableComponent>(id)
                .unwrap();
            if let Some(group) = self.groups.get(&id) {
                for &piece in &group.pieces {
                    if let Some(value) =
                        world.get_component_by_id_as_mut::<DestroyableComponent>(piece)
                    {
                        let exhausted = value.exhausted;
                        *value = config;
                        value.exhausted = exhausted;
                    }
                }
                continue;
            }
            let Some(frame) = world.parent_of(id) else {
                continue;
            };
            if piece_source(world, frame).is_some() {
                continue;
            }
            let mut pieces = Vec::new();
            let mut members = Vec::new();
            let mut stack = world.children_of(frame).to_vec();
            while let Some(node) = stack.pop() {
                if world.children_of(node).iter().any(|&child| {
                    world.get_component_by_id_as::<crate::engine::ecs::component::destroyable::DestructionCellComponent>(child).is_some()
                }) { continue; }
                let explicit = world.children_of(node).iter().copied().find(|&child| {
                    world
                        .get_component_by_id_as::<DestroyableComponent>(child)
                        .is_some()
                });
                if piece_source(world, node).is_some() {
                    let piece = explicit.unwrap_or_else(|| {
                        let piece = attach(world, node, config);
                        attach(world, piece, SerializeComponent::off());
                        pieces.push(piece);
                        piece
                    });
                    members.push(piece);
                    self.owners.insert(piece, id);
                } else if explicit.is_none() {
                    // A nested aggregate is governed by its own marker.
                    stack.extend_from_slice(world.children_of(node));
                }
            }
            self.groups.insert(
                id,
                Group {
                    pieces,
                    members,
                    active: false,
                },
            );
        }
    }

    pub fn pending_group(&self, piece: ComponentId) -> Option<ComponentId> {
        let group = *self.owners.get(&piece)?;
        (!self.groups.get(&group)?.active).then_some(group)
    }

    pub(super) fn pending_source_group(
        &self,
        world: &World,
        source: ComponentId,
    ) -> Option<ComponentId> {
        let mut ancestor = world.parent_of(source);
        while let Some(frame) = ancestor {
            if let Some(owner) = world
                .children_of(frame)
                .iter()
                .copied()
                .find(|id| self.groups.get(id).is_some_and(|group| !group.active))
            {
                return Some(owner);
            }
            ancestor = world.parent_of(frame);
        }
        None
    }

    /// Prepare every source before retiring combined outputs. Most reuse the
    /// uploaded cube; additional non-destroyable meshes are preserved too.
    pub fn activate(
        &mut self,
        group: ComponentId,
        systems: &mut SystemWorld,
        world: &mut World,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        uploader: &mut dyn RenderUploader,
    ) -> bool {
        let Some(state) = self.groups.get_mut(&group) else {
            return false;
        };
        if state.active {
            return true;
        }
        let Some(frame) = world.parent_of(group) else {
            return false;
        };
        let combines: Vec<_> = world
            .owned_subtree(frame)
            .into_iter()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<CombineMeshComponent>(id)
                    .is_some()
            })
            .collect();
        // Preflight the whole section before publishing any instance changes.
        let mut prepared = Vec::new();
        for &combine in &combines {
            let sources = systems.combine_mesh.sources_for_root(world, combine);
            if sources.is_empty() {
                continue;
            }
            let Some(style) = visuals
                .instance_handle_for_component(combine)
                .and_then(|h| visuals.instance(h))
                .copied()
            else {
                return false;
            };
            for source in sources {
                if let Some((_, zone)) = world
                    .parent_of(source)
                    .and_then(|frame| piece_source(world, frame))
                {
                    if crate::engine::ecs::component::destroyable::zone_suppressed(world, zone) {
                        continue;
                    }
                }
                let Some(model) = TransformSystem::world_model(world, source) else {
                    return false;
                };
                // Suppressed GLTF sources may still carry a placeholder on the
                // Renderable. Resolve the same imported override used to bake it.
                let Some(cpu) = super::combine_mesh_system::source_mesh(world, assets, source)
                else {
                    return false;
                };
                let Ok(gpu) = assets.gpu_mesh_handle(uploader, cpu) else {
                    return false;
                };
                prepared.push((source, gpu, model, style));
            }
        }
        for (source, gpu, model, style) in prepared {
            let handle = register_visual(visuals, source, gpu, model, style);
            world
                .get_component_by_id_as_mut::<RenderableComponent>(source)
                .unwrap()
                .handle = Some(handle);
            systems
                .renderable
                .register_renderable(world, visuals, source);
        }
        for combine in combines {
            systems
                .combine_mesh
                .unpack(combine, visuals, &mut systems.mesh_bounds);
        }
        state.active = true;
        true
    }
}

fn piece_source(world: &World, frame: ComponentId) -> Option<(ComponentId, ComponentId)> {
    world.get_component_by_id_as::<TransformComponent>(frame)?;
    let source = world.children_of(frame).iter().copied().find(|&id| {
        world
            .get_component_by_id_as::<RenderableComponent>(id)
            .is_some_and(|r| r.renderable.base_mesh == CpuMeshHandle::CUBE)
    })?;
    let zone = world.children_of(frame).iter().copied().find(|&id| {
        world
            .get_component_by_id_as::<ZoneComponent>(id)
            .is_some_and(|z| {
                z.shape
                    == crate::engine::ecs::system::model::collision_types::CollisionShape::CUBE()
            })
    })?;
    Some((source, zone))
}
