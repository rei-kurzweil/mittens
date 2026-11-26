//! Upper body solver: the only writer for the torso bones (hips, spine, chest,
//! upper chest). It applies the torso pitch the body solver decides. Arms are
//! still solved by `ik_system` chains; they move here with planted hands.
mod rig;

pub(crate) use rig::SpineRig;

use crate::engine::ecs::component::AvatarControlComponent;
use crate::engine::ecs::system::HumanoidBoneMapSystem;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UpperBodyStatus {
    #[default]
    Pending,
    Ready,
    UnsupportedRig(&'static str),
}

#[derive(Debug, Default)]
pub struct UpperBodyIkSystem {
    avatars: HashMap<ComponentId, UpperBodyInstance>,
}

#[derive(Debug, Default)]
struct UpperBodyInstance {
    spine: Option<SpineRig>,
    binding_key: Option<(ComponentId, u64)>,
    bind_error: Option<&'static str>,
}

impl UpperBodyIkSystem {
    pub fn register(&mut self, id: ComponentId) {
        self.avatars.entry(id).or_default();
    }

    pub fn remove(&mut self, id: ComponentId) {
        self.avatars.remove(&id);
    }

    pub fn status(&self, id: ComponentId) -> UpperBodyStatus {
        match self.avatars.get(&id) {
            Some(UpperBodyInstance { spine: Some(_), .. }) => UpperBodyStatus::Ready,
            Some(UpperBodyInstance {
                bind_error: Some(reason),
                ..
            }) => UpperBodyStatus::UnsupportedRig(reason),
            _ => UpperBodyStatus::Pending,
        }
    }

    pub(crate) fn spine(&self, id: ComponentId) -> Option<&SpineRig> {
        self.avatars.get(&id)?.spine.as_ref()
    }

    /// Bind or rebind spine rigs when an avatar's model root or bone map changes.
    pub fn prepare_bindings(
        &mut self,
        world: &World,
        maps: &HumanoidBoneMapSystem,
        emit: &mut dyn SignalEmitter,
    ) {
        for (&id, instance) in &mut self.avatars {
            let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
                continue;
            };
            let Some(root) = avatar.model_root_id else {
                continue;
            };
            let Some(report) = avatar.humanoid_map_gltf.and_then(|g| maps.report(g)) else {
                continue;
            };
            let key = (root, report.generation);
            if instance.binding_key == Some(key) {
                continue;
            }
            instance.binding_key = Some(key);
            if let Some(spine) = &mut instance.spine {
                if spine.root == root && spine.matches_map(report) {
                    spine.generation = report.generation;
                    continue;
                }
                spine.restore(world, emit);
            }
            instance.spine = None;
            match SpineRig::bind(world, root, report) {
                Ok(spine) => {
                    instance.spine = Some(spine);
                    instance.bind_error = None;
                }
                Err(reason) => instance.bind_error = Some(reason),
            }
        }
    }

    /// Apply each avatar's torso pitch from the body solver.
    pub fn tick(
        &mut self,
        world: &World,
        emit: &mut dyn SignalEmitter,
        body: &super::body::BodySolverSystem,
    ) {
        for (&id, instance) in &mut self.avatars {
            let Some(spine) = &mut instance.spine else {
                continue;
            };
            if world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .is_some_and(|avatar| avatar.paused)
            {
                continue;
            }
            let pitch = body.torso_pitch(id);
            if pitch > 1e-4 {
                spine.write(world, emit, pitch);
            } else {
                spine.restore(world, emit);
            }
        }
    }
}
