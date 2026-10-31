//! Grip acquisition starts slightly behind the normal pointer's fingertip origin.
use super::*;
use crate::engine::ecs::system::climbing_system::climbable_owner_for_hit;

pub(crate) const CLIMB_RAY_BACKOFF_METERS: f32 = 0.09;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ClimbingRayHit {
    pub renderable: ComponentId,
    pub ray: PointerRaySnapshot,
    /// Signed distance from the ordinary pointer origin for interaction ordering.
    pub distance_from_pointer: f32,
    pub priority: u8,
    pub pointer_events: crate::engine::ecs::component::PointerEvents,
}

impl RayCastSystem {
    pub(crate) fn climbing_hit(&self, raycaster: ComponentId) -> Option<ClimbingRayHit> {
        self.climbing_hits.get(&raycaster).copied()
    }

    pub(super) fn cast_climbing_ray(
        &mut self,
        world: &World,
        bvh: &BvhSystem,
        raycaster: ComponentId,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
    ) {
        let backed_origin =
            std::array::from_fn(|axis| origin[axis] - direction[axis] * CLIMB_RAY_BACKOFF_METERS);
        // Preserve the forward endpoint while including the extra hand-sized segment.
        let max_distance = max_distance + CLIMB_RAY_BACKOFF_METERS;
        let hits = if bvh.has_index() {
            self.cast_against_renderables_bvh(
                world,
                bvh,
                backed_origin,
                direction,
                0.0,
                max_distance,
            )
        } else {
            self.cast_against_renderables(world, backed_origin, direction, 0.0, max_distance)
        };
        if Self::profile_spatial_enabled() {
            self.profile_rays += 1;
            if bvh.has_index() {
                self.profile_bvh_hits += hits.len() as u64;
            } else {
                self.profile_fallbacks += 1;
                self.profile_fallback_candidates += self.eligible_renderables.len() as u64;
            }
        }
        let hit = hits.into_iter().find_map(|(renderable, distance)| {
            climbable_owner_for_hit(world, renderable)?;
            let policy = BvhSystem::find_raycastable_for_renderable(world, renderable)?;
            policy
                .pointer_events
                .captures_drag()
                .then_some(ClimbingRayHit {
                    renderable,
                    ray: PointerRaySnapshot {
                        origin_world: backed_origin,
                        direction_world: direction,
                    },
                    distance_from_pointer: distance - CLIMB_RAY_BACKOFF_METERS,
                    priority: policy.interaction_priority,
                    pointer_events: policy.pointer_events,
                })
        });
        if Self::debug_raycast_enabled() {
            eprintln!(
                "[raycast][climbing] rc={raycaster:?} backoff_m={CLIMB_RAY_BACKOFF_METERS:.3} origin={backed_origin:?} dir={direction:?} hit={hit:?}"
            );
        }
        if let Some(hit) = hit {
            self.climbing_hits.insert(raycaster, hit);
        }
    }
}

#[cfg(test)]
#[path = "climbing_raycast_tests.rs"]
mod tests;
