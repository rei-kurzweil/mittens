use super::*;
use crate::engine::ecs::system::climbing_system::climbable_owner_for_hit;

fn grip_owner(world: &World, hit: ComponentId, desktop: bool) -> Option<(ComponentId, bool)> {
    if !desktop {
        if let Some(owner) = climbable_owner_for_hit(world, hit) {
            return Some((owner, true));
        }
    }
    grabbable_owner_for_hit(world, hit).map(|owner| (owner, false))
}

type PointerHit = (
    u8,
    f32,
    ComponentId,
    ComponentId,
    [f32; 3],
    [f32; 3],
    PointerEvents,
);

impl GestureSystem {
    pub(super) fn snapshot_pointer_hits(&self) -> Vec<PointerHit> {
        self.ray_hits_sorted
            .lock()
            .map(|hits| hits.clone())
            .unwrap_or_default()
    }
    pub(super) fn process_grips(
        &mut self,
        world: &mut World,
        activations: &PointerActivations,
        pointer_system: &PointerSystem,
        attachment_system: &mut AttachmentSystem,
        rx: &mut RxWorld,
        hits: &[PointerHit],
        raycasts: &RayCastSystem,
    ) -> HashSet<ComponentId> {
        // --- Grip (XR) and left mouse (desktop): capture Grabbable targets. ---
        let mut grab_presses: Vec<(ComponentId, bool)> = activations
            .grip_pressed
            .iter()
            .copied()
            .map(|p| (p, false))
            .collect();
        grab_presses.extend(
            activations
                .pressed
                .iter()
                .copied()
                .filter(|p| {
                    let topology = pointer_topology_context(world, *p);
                    !topology.has_controller_driver
                        && !topology.has_xr_camera_anchor
                        && !topology.has_xr_input_driver
                })
                .map(|p| (p, true)),
        );
        let mut desktop_grab_consumed = HashSet::new();
        for (pointer_cid, desktop) in grab_presses {
            if self.grip_states.contains_key(&pointer_cid) {
                continue;
            }
            if attachment_system.try_dismount_for_pointer(world, pointer_cid, rx) {
                if desktop {
                    desktop_grab_consumed.insert(pointer_cid);
                }
                continue;
            }
            let Some(raycaster) = pointer_system.raycast_for_pointer(pointer_cid) else {
                continue;
            };
            let mounted = hits
                .iter()
                .filter(|hit| hit.2 == raycaster && hit.6.captures_drag())
                .any(|hit| attachment_system.try_mount_from_hit(world, pointer_cid, hit.3, rx));
            if mounted {
                if desktop {
                    desktop_grab_consumed.insert(pointer_cid);
                }
                continue;
            }
            let ordinary_hit = hits
                .iter()
                .find(|h| {
                    h.2 == raycaster
                        && h.6.captures_drag()
                        && grip_owner(world, h.3, desktop).is_some_and(|(owner, climbing)| {
                            climbing || !self.grabbed_owners.contains(&owner)
                        })
                })
                .copied();
            let climbing_hit = (!desktop)
                .then(|| raycasts.climbing_hit(raycaster))
                .flatten()
                .map(|hit| {
                    (
                        hit.priority,
                        hit.distance_from_pointer,
                        raycaster,
                        hit.renderable,
                        hit.ray.origin_world,
                        hit.ray.direction_world,
                        hit.pointer_events,
                    )
                });
            let hit = match (ordinary_hit, climbing_hit) {
                (Some(ordinary), Some(climbing)) => {
                    if climbing.0 > ordinary.0
                        || (climbing.0 == ordinary.0 && climbing.1 <= ordinary.1)
                        || climbing.3 == ordinary.3
                    {
                        climbing
                    } else {
                        ordinary
                    }
                }
                (Some(hit), None) | (None, Some(hit)) => hit,
                (None, None) => continue,
            };
            let Some((owner, climbing)) = grip_owner(world, hit.3, desktop) else {
                continue;
            };
            if !climbing {
                self.grabbed_owners.insert(owner);
            }
            self.grip_states.insert(
                pointer_cid,
                GripGestureState {
                    renderable: hit.3,
                    owner,
                    desktop,
                    climbing,
                },
            );
            if desktop {
                desktop_grab_consumed.insert(pointer_cid);
            }
            rx.push_event(
                hit.3,
                EventSignal::GrabStart {
                    pointer: pointer_cid,
                    raycaster,
                    renderable: hit.3,
                    target: owner,
                    ray_origin_world: hit.4,
                    ray_dir_world: hit.5,
                },
            );
        }

        let mut grab_releases = activations.grip_released.clone();
        grab_releases.extend(
            activations
                .released
                .iter()
                .copied()
                .filter(|p| self.grip_states.get(p).is_some_and(|s| s.desktop)),
        );
        for pointer_cid in grab_releases {
            if let Some(state) = self.grip_states.remove(&pointer_cid) {
                if !state.climbing {
                    self.grabbed_owners.remove(&state.owner);
                }
                rx.push_event(
                    state.renderable,
                    EventSignal::GrabEnd {
                        pointer: pointer_cid,
                        target: state.owner,
                    },
                );
            }
        }

        desktop_grab_consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{
        ClimbableComponent, ControllerHand, ControllerPoseKind, ControllerXRComponent,
        TransformComponent,
    };

    #[test]
    fn xr_grips_capture_the_same_surface_independently_from_ray_hits() {
        let mut world = World::default();
        let surface = world.add_component(TransformComponent::new());
        let marker = world.add_component(ClimbableComponent::new());
        world.add_child(surface, marker).unwrap();
        let mut pointers = PointerSystem::default();
        let mut rx = RxWorld::default();
        let mut hands = Vec::new();
        let mut hits = Vec::new();
        for side in [ControllerHand::Left, ControllerHand::Right] {
            let hand = world.add_component(ControllerXRComponent::new(
                true,
                side,
                ControllerPoseKind::GripAim,
            ));
            let pointer = world.add_component(PointerComponent::new());
            world.add_child(hand, pointer).unwrap();
            pointers.register_pointer(&mut world, pointer, &mut rx);
            let ray = pointers.raycast_for_pointer(pointer).unwrap();
            hits.push((
                0,
                1.0,
                ray,
                surface,
                [0.0; 3],
                [0.0, 0.0, -1.0],
                PointerEvents::All,
            ));
            hands.push(pointer);
        }
        let mut gestures = GestureSystem::default();
        let mut attachment = AttachmentSystem::default();
        gestures.process_grips(
            &mut world,
            &PointerActivations {
                grip_pressed: hands.clone(),
                ..Default::default()
            },
            &pointers,
            &mut attachment,
            &mut rx,
            &hits,
            &RayCastSystem::default(),
        );
        assert_eq!(gestures.grip_states.len(), 2);
        assert_eq!(
            rx.drain_ready_events()
                .iter()
                .filter(|s| matches!(s.event, Some(EventSignal::GrabStart { .. })))
                .count(),
            2
        );
        gestures.process_grips(
            &mut world,
            &PointerActivations {
                grip_released: hands.clone(),
                ..Default::default()
            },
            &pointers,
            &mut attachment,
            &mut rx,
            &[],
            &RayCastSystem::default(),
        );
        assert!(gestures.grip_states.is_empty());
        gestures.process_grips(
            &mut world,
            &PointerActivations {
                grip_pressed: hands,
                ..Default::default()
            },
            &pointers,
            &mut attachment,
            &mut rx,
            &[],
            &RayCastSystem::default(),
        );
        assert!(
            gestures.grip_states.is_empty(),
            "a ray miss cannot acquire a climbable"
        );
    }
}
