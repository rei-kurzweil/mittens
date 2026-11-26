//! Read-only flat support queries against the shared, retained contact BVH.
use super::LowerBodyStatus;
use super::rig::{below, matrix};
use crate::engine::ecs::component::{AvatarControlComponent, ZoneComponent};
use crate::engine::ecs::system::{
    collidable_bvh::{CollidableBvh, SupportSurface},
    collision_geometry,
};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::transform::TransformTrs;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Support {
    pub zone: ComponentId,
    pub height: f32,
    pub probe: [f32; 2],
}

pub(super) struct SupportQuery<'a> {
    world: &'a World,
    index: &'a CollidableBvh,
    candidates: &'a [usize],
    owner: ComponentId,
    center: [f32; 3],
    bottom: f32,
    margin: f32,
    trace: Option<&'a RefCell<Vec<SupportSurface>>>,
}

impl<'a> SupportQuery<'a> {
    pub fn center(&self) -> [f32; 3] {
        self.center
    }
    pub fn new(
        world: &'a World,
        index: &'a CollidableBvh,
        avatar: &AvatarControlComponent,
        id: ComponentId,
        soles: [[f32; 3]; 2],
        previous_floor: Option<f32>,
        scale: f32,
        horizontal_padding: f32,
        stack: &mut Vec<usize>,
        candidates: &'a mut Vec<usize>,
        trace: Option<&'a RefCell<Vec<SupportSurface>>>,
    ) -> Result<Self, LowerBodyStatus> {
        use LowerBodyStatus::{Pending, UnsupportedRig};
        let (center, mut min, mut max, bottom) = if avatar.collision_enabled {
            if !avatar.capsule_ready {
                return Err(Pending);
            }
            let capsule = avatar.capsule_transform_id.ok_or(Pending)?;
            let collider = avatar.capsule_collidable_id.ok_or(Pending)?;
            let zone = world.parent_of(collider).ok_or(Pending)?;
            let shape = world
                .get_component_by_id_as::<ZoneComponent>(zone)
                .filter(|z| z.enabled)
                .ok_or(Pending)?
                .shape;
            let frame = matrix(world, capsule).map_err(UnsupportedRig)?;
            let (center, shape) = collision_geometry::axis_aligned_world_shape(shape, frame)
                .ok_or(UnsupportedRig("invalid capsule frame"))?;
            let (min, max) = collision_geometry::transformed_aabb(
                shape,
                TransformTrs::new(center, [0.0, 0.0, 0.0, 1.0], [1.0; 3])
                    .to_matrix()
                    .unwrap(),
            );
            (center, min, max, min[1])
        } else {
            // Visual/network rigs still need read-only floor support. Derive a
            // query frame from their soles without enabling collision response
            // or creating a capsule that could move the received locomotion root.
            // Rest FK follows crouching; an already acquired floor does not.
            // Re-query the BVH at that plane instead of moving the received root.
            let bottom = previous_floor.unwrap_or_else(|| soles[0][1].min(soles[1][1]));
            let center = [
                (soles[0][0] + soles[1][0]) * 0.5,
                soles[0][1].max(soles[1][1]).max(bottom) + scale * 0.5,
                (soles[0][2] + soles[1][2]) * 0.5,
            ];
            (center, [center[0], bottom, center[2]], center, bottom)
        };
        let margin = avatar.crouch_support_margin * scale;
        if !margin.is_finite() || margin <= 0.0 {
            return Err(UnsupportedRig("invalid support margin"));
        }
        for sole in soles {
            for axis in 0..3 {
                min[axis] = min[axis].min(sole[axis]);
                max[axis] = max[axis].max(sole[axis]);
            }
        }
        for axis in 0..3 {
            let padding = margin + if axis == 1 { 0.0 } else { horizontal_padding };
            min[axis] -= padding;
            max[axis] += padding;
        }
        index.query(min, max, stack, candidates);
        Ok(Self {
            world,
            index,
            candidates,
            owner: world.parent_of(id).ok_or(Pending)?,
            center,
            bottom,
            margin,
            trace,
        })
    }

    pub fn at(&self, sole: [f32; 3]) -> Option<Support> {
        self.find(sole, None)
    }

    pub fn retains(&self, sole: [f32; 3], support: Support) -> bool {
        self.find(sole, Some(support.zone))
            .is_some_and(|current| (current.height - support.height).abs() < 0.001)
    }

    fn find(&self, sole: [f32; 3], zone: Option<ComponentId>) -> Option<Support> {
        if self.trace.is_some() {
            self.find_support::<true>(sole, zone)
        } else {
            self.find_support::<false>(sole, zone)
        }
    }

    fn find_support<const FACE: bool>(
        &self,
        sole: [f32; 3],
        zone: Option<ComponentId>,
    ) -> Option<Support> {
        let mut support: Option<Support> = None;
        let mut winning = None;
        for &candidate in self.candidates {
            let surface = self.index.surface(candidate);
            if zone.is_some_and(|zone| zone != surface.zone)
                || below(self.world, surface.zone, self.owner)
                || below(self.world, surface.frame_id, self.owner)
            {
                continue;
            }
            let hit = if FACE {
                surface.flat_support_hit::<true>(sole, self.center, self.bottom, self.margin)
            } else {
                surface
                    .flat_support(sole, self.center, self.bottom, self.margin)
                    .map(|height| (height, 0))
            };
            let Some((height, triangle)) = hit else {
                continue;
            };
            if support
                .is_none_or(|old| (height - self.bottom).abs() < (old.height - self.bottom).abs())
            {
                support = Some(Support {
                    zone: surface.zone,
                    height,
                    probe: [sole[0], sole[2]],
                });
                if FACE {
                    winning = Some((surface, height, triangle));
                }
            }
        }
        if FACE
            && let Some((surface, height, triangle)) = winning
            && let Some(hit) = surface.support_surface([sole[0], height, sole[2]], triangle)
            && let Some(trace) = self.trace
        {
            trace.borrow_mut().push(hit);
        }
        support
    }
}
