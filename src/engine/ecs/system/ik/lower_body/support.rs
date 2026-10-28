//! Read-only flat support queries against the shared, retained contact BVH.
use super::LowerBodyStatus;
use super::rig::{below, matrix};
use crate::engine::ecs::component::{AvatarControlComponent, ZoneComponent};
use crate::engine::ecs::system::{collidable_bvh::CollidableBvh, collision_geometry};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::transform::TransformTrs;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Support {
    pub zone: ComponentId,
    pub height: f32,
}

pub(super) struct SupportQuery<'a> {
    world: &'a World,
    index: &'a CollidableBvh,
    candidates: &'a [usize],
    owner: ComponentId,
    center: [f32; 3],
    bottom: f32,
    margin: f32,
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
        scale: f32,
        horizontal_padding: f32,
        stack: &mut Vec<usize>,
        candidates: &'a mut Vec<usize>,
    ) -> Result<Self, LowerBodyStatus> {
        use LowerBodyStatus::{Pending, UnsupportedRig};
        if !avatar.capsule_ready || !avatar.collision_enabled {
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
        let (mut min, mut max) = collision_geometry::transformed_aabb(
            shape,
            TransformTrs::new(center, [0.0, 0.0, 0.0, 1.0], [1.0; 3])
                .to_matrix()
                .unwrap(),
        );
        let bottom = min[1];
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
        let mut support: Option<Support> = None;
        for &candidate in self.candidates {
            let surface = self.index.surface(candidate);
            if zone.is_some_and(|zone| zone != surface.zone)
                || below(self.world, surface.zone, self.owner)
                || below(self.world, surface.frame_id, self.owner)
            {
                continue;
            }
            let Some(height) = surface.flat_support(sole, self.center, self.bottom, self.margin)
            else {
                continue;
            };
            if support
                .is_none_or(|old| (height - self.bottom).abs() < (old.height - self.bottom).abs())
            {
                support = Some(Support {
                    zone: surface.zone,
                    height,
                });
            }
        }
        support
    }
}
