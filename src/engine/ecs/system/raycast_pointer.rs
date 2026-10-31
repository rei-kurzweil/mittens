use super::*;

impl RayCastSystem {
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        rx: &mut RxWorld,
        bvh: &BvhSystem,
        activations: &crate::engine::ecs::system::pointer_system::PointerActivations,
        pointer_system: &crate::engine::ecs::system::pointer_system::PointerSystem,
        _dt_sec: f32,
    ) {
        self.current_rays.clear();
        self.climbing_hits.clear();
        let profile = Self::profile_spatial_enabled();
        if profile {
            self.profile_frames += 1;
        }
        // Cursor ray (if needed by any raycaster this frame).
        let cursor_ray = Self::ray_from_cursor(visuals, input);

        if !self.raycasters.is_empty() {
            // Iterate over a stable snapshot so removal during iteration is safe.
            let raycasters: Vec<ComponentId> = self.raycasters.iter().copied().collect();
            for rcid in raycasters {
                let Some(rc) = world.get_component_by_id_as::<RayCastComponent>(rcid) else {
                    self.raycasters.remove(&rcid);
                    self.last_hit.remove(&rcid);
                    continue;
                };

                // Copy out what we need so we can mutably borrow `world` later.
                let mode = rc.mode;
                let min_distance = rc.min_distance;
                let max_distance = rc.max_distance;
                let cast_requested = rc.cast_requests > 0;

                let source = Self::inferred_source_kind(world, rcid);

                // Either XR action must be able to acquire its own interaction target.
                // In particular, grip grabbing must not depend on the trigger first
                // waking this event-driven raycaster.
                let pointer_action_active = pointer_system
                    .raycast_to_pointer(rcid)
                    .map(|ptr| activations.raycast_active(ptr))
                    .unwrap_or(false);

                let (origin, dir) = match source {
                    RaySourceKind::CursorThroughActiveCamera => {
                        let Some(r) = cursor_ray else {
                            continue;
                        };
                        (r.origin, r.dir)
                    }
                    RaySourceKind::ParentForward => {
                        let Some((o, d)) = Self::ray_from_parent_forward(world, rcid) else {
                            continue;
                        };
                        (o, d)
                    }
                };

                if !Self::should_cast(mode, input, cast_requested, source, pointer_action_active) {
                    continue;
                }

                self.current_rays.insert(
                    rcid,
                    PointerRaySnapshot {
                        origin_world: origin,
                        direction_world: dir,
                    },
                );

                let query_started = profile.then(Instant::now);
                // Climbing alone can acquire surfaces just behind the fingertip.
                // Trigger/UI rays and the visible beam keep the ordinary pointer origin.
                if pointer_system
                    .raycast_to_pointer(rcid)
                    .is_some_and(|pointer| {
                        activations.grip_pressed.contains(&pointer)
                            && pointer_topology_context(world, pointer).has_controller_driver
                    })
                {
                    self.cast_climbing_ray(world, bvh, rcid, origin, dir, max_distance);
                }

                let mut hits = self.cast_against_renderables_bvh(
                    world,
                    bvh,
                    origin,
                    dir,
                    min_distance,
                    max_distance,
                );
                if profile {
                    self.profile_rays += 1;
                    self.profile_bvh_hits += hits.len() as u64;
                }
                if hits.is_empty() && !bvh.has_index() {
                    hits = self.cast_against_renderables(
                        world,
                        origin,
                        dir,
                        min_distance,
                        max_distance,
                    );
                    if profile {
                        self.profile_fallbacks += 1;
                        self.profile_fallback_candidates += self.eligible_renderables.len() as u64;
                    }
                }
                if let Some(started) = query_started {
                    self.profile_query_time += started.elapsed();
                }
                if BvhSystem::debug_grid_raycast_enabled()
                    && pointer_system
                        .raycast_to_pointer(rcid)
                        .is_some_and(|pointer| {
                            activations.pressed.contains(&pointer)
                                || activations.grip_pressed.contains(&pointer)
                        })
                {
                    self.trace_grid_press(world, bvh, rcid, origin, dir, max_distance, &hits);
                }
                if Self::debug_raycast_enabled() {
                    let summary: Vec<String> = hits
                        .iter()
                        .take(8)
                        .map(|(cid, t)| {
                            let rc = Self::raycastable_for_renderable(world, *cid);
                            format!(
                                "{:?} '{}' t={:.3} pri={} pe={:?}",
                                cid,
                                Self::debug_component_label(world, *cid),
                                t,
                                rc.map(|r| r.interaction_priority).unwrap_or(0),
                                rc.map(|r| r.pointer_events)
                                    .unwrap_or(crate::engine::ecs::component::PointerEvents::All)
                            )
                        })
                        .collect();
                    eprintln!(
                        "[raycast] rc={:?} source={:?} origin=[{:+.3},{:+.3},{:+.3}] dir=[{:+.3},{:+.3},{:+.3}] interval=[{:.3},{:.3}] hits={}",
                        rcid,
                        source,
                        origin[0],
                        origin[1],
                        origin[2],
                        dir[0],
                        dir[1],
                        dir[2],
                        min_distance,
                        max_distance,
                        if summary.is_empty() {
                            "<none>".to_string()
                        } else {
                            summary.join(" | ")
                        }
                    );
                }

                if world
                    .get_component_by_id_as::<RayCastComponent>(rcid)
                    .is_some_and(|ray| ray.nearest_hit_only)
                {
                    hits.sort_by(|a, b| a.1.total_cmp(&b.1));
                    hits.truncate(1);
                }
                // Emit one RayIntersected per hit (front-to-back). GestureSystem accumulates
                // all of them within the frame and picks drag/click targets independently.
                for &(hit_cid, t) in &hits {
                    rx.push_event(
                        hit_cid,
                        EventSignal::RayIntersected {
                            raycaster: rcid,
                            renderable: hit_cid,
                            t,
                            origin,
                            dir,
                        },
                    );
                }

                let best = hits.first().copied();
                match mode {
                    RayCastMode::Continuous => {
                        let next = best.map(|(cid, _)| cid);
                        self.last_hit.insert(rcid, next);
                    }
                    RayCastMode::EventDriven => {}
                }

                if cast_requested {
                    if let Some(rc) = world.get_component_by_id_as_mut::<RayCastComponent>(rcid) {
                        rc.cast_requests = 0;
                    }
                }
            }
        }

        if profile && self.profile_frames >= 120 {
            let line = format!(
                "[spatial-profile][raycast] frames={} rays={} bvh_hits={} fallbacks={} fallback_scan_candidates={} query_ms={:.3}",
                self.profile_frames,
                self.profile_rays,
                self.profile_bvh_hits,
                self.profile_fallbacks,
                self.profile_fallback_candidates,
                self.profile_query_time.as_secs_f64() * 1000.0
            );
            eprintln!("{line}");
            crate::utils::profile_log::append("spatial-profile", &line);
            self.profile_frames = 0;
            self.profile_rays = 0;
            self.profile_bvh_hits = 0;
            self.profile_fallbacks = 0;
            self.profile_fallback_candidates = 0;
            self.profile_query_time = Duration::ZERO;
        }
    }
}
