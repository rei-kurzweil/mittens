//! Avatar collision capsule: routing, contact validity, init, readiness, diagnostics.
use super::*;

pub(super) fn refresh_capsule_routing(avc_id: ComponentId, world: &mut World) {
    let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
        return;
    };
    let Some(collidable) = avatar.capsule_collidable_id else {
        return;
    };
    if avatar.capsule_routing_revision == world.query_revision()
        && avatar.capsule_route_ref == avatar.movement_target
        && avatar.capsule_route_response == Some(avatar.collision_response)
    {
        return;
    }
    let response = avatar.collision_response;
    let previous_response = avatar.capsule_route_response;
    let sample = avatar.capsule_sample_id;
    let previous_source = avatar.capsule_vertical_source;
    let previous_target = world
        .get_component_by_id_as::<CollidableComponent>(collidable)
        .and_then(|component| component.movement_target_id);
    let target = automatic_avc_movement_target(world, avc_id);
    let source = owning_input_xr(world, avc_id)
        .and_then(|input| xr_locomotion_target_transform(world, input));
    let source = source.filter(|source| {
        response == CollisionResponse::ReadOnly
            || target.is_some_and(|target| ancestor_or_self(world, target, *source))
    });
    let authored_target = avatar.movement_target.clone();
    if let Some(sample) = sample {
        let reference = source
            .and_then(|source| world.get_component_record(source))
            .map(|node| super::super::super::component::ComponentRef::Guid(node.guid));
        if let Some(settings) = world
            .get_component_by_id_as_mut::<TransformOperatorComponent>(sample)
            .and_then(|operator| operator.sample_translation_settings_mut())
        {
            settings.source = reference;
        }
    }
    if let Some(component) = world.get_component_by_id_as_mut::<CollidableComponent>(collidable) {
        component.movement_target_id = target;
        component.movement_target_required = true;
        component.response = response;
    }
    let revision = world.query_revision();
    if let Some(avatar) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        avatar.capsule_vertical_source = source;
        avatar.capsule_routing_revision = revision;
        avatar.capsule_route_ref = authored_target;
        avatar.capsule_route_response = Some(response);
        avatar.capsule_reset_contact_history |= previous_source != source
            || previous_target != target
            || previous_response != Some(response);
    }
}

pub(super) fn capsule_contact_valid(
    world: &World,
    avatar: &AvatarControlComponent,
    operators: &super::super::TransformOperatorSystem,
) -> bool {
    if !avatar.collision_enabled
        || (avatar.collision_response == CollisionResponse::Slide
            && avatar.capsule_route_response != Some(CollisionResponse::Slide))
        || avatar.model_root_id.is_none_or(|id| {
            world
                .get_component_by_id_as::<TransformComponent>(id)
                .is_none()
        })
    {
        return false;
    }
    if avatar.humanoid_map_gltf.is_some_and(|id| {
        world
            .get_component_by_id_as::<GLTFComponent>(id)
            .is_none_or(|gltf| !gltf.spawned)
    }) {
        return false;
    }
    let Some(sample) = avatar.capsule_sample_id else {
        return true;
    }; // Preserve desktop routing.
    let Some(source) = avatar.capsule_vertical_source else {
        return false;
    };
    let Some(settings) = world
        .get_component_by_id_as::<TransformOperatorComponent>(sample)
        .and_then(|operator| operator.sample_translation_settings())
    else {
        return false;
    };
    let expected = world
        .get_component_record(source)
        .map(|node| super::super::super::component::ComponentRef::Guid(node.guid));
    if settings.source != expected
        || settings.axes != super::super::super::component::transform_operator::TranslationAxes::Y
    {
        return false;
    }
    world
        .get_component_by_id_as::<TransformComponent>(source)
        .is_some()
        && avatar
            .model_root_id
            .and_then(|id| uniform_world_scale(world, id))
            .is_some_and(|scale| (scale - avatar.capsule_world_scale).abs() <= scale * 1e-4)
        && operators.translation_sample_status(world, sample)
            == super::super::transform_operator::TranslationSampleStatus::Ready
}

pub(super) fn try_init_or_route_capsule(
    avc_id: ComponentId,
    world: &mut World,
    render_assets: &RenderAssets,
    emit: &mut dyn SignalEmitter,
) {
    let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
        return;
    };
    if !avatar.collision_enabled {
        return;
    }
    if avatar.capsule_collidable_id.is_some_and(|id| {
        world
            .get_component_by_id_as::<CollidableComponent>(id)
            .is_some()
    }) {
        refresh_capsule_routing(avc_id, world);
        return;
    }
    let avc = avatar.clone();
    let model_root_id = avc.model_root_id.or_else(|| {
        world.children_of(avc_id).iter().copied().find(|child| {
            world
                .get_component_by_id_as::<TransformComponent>(*child)
                .is_some()
        })
    });
    let Some(model_root_id) = model_root_id else {
        return;
    };
    let movement_target = automatic_avc_movement_target(world, avc_id);
    let xr = owning_input_xr(world, avc_id);
    let vertical_source = xr.and_then(|input| xr_locomotion_target_transform(world, input));
    if xr.is_some()
        && vertical_source.is_none_or(|source| {
            avc.collision_response == CollisionResponse::Slide
                && movement_target.is_none_or(|target| !ancestor_or_self(world, target, source))
        })
    {
        return;
    }
    let world_scale = if xr.is_some() {
        let Some(scale) = uniform_world_scale(world, model_root_id) else {
            return;
        };
        scale
    } else {
        1.0
    };

    let measured_bounds =
        BoundsSystem::measure_renderable_subtree_bounds(world, render_assets, model_root_id);
    let measured_inference = match measured_bounds {
        RenderableBoundsMeasure::Measured(bounds) => {
            infer_upright_capsule(&bounds, avc.capsule_radius).map(|inferred| (bounds, inferred))
        }
        RenderableBoundsMeasure::Unmeasurable => None,
    };
    let (bounds_source, inference_bounds, inferred) =
        if let Some((bounds, inferred)) = measured_inference {
            ("aggregate_render_bounds", bounds, inferred)
        } else {
            let Some(height) = spawned_gltf_exists(world, model_root_id)
                .then(|| {
                    if xr.is_some() {
                        avc.avatar_height.or_else(|| {
                            (avc.model_root_local_y < 0.0).then_some(-avc.model_root_local_y)
                        })
                    } else {
                        fallback_avatar_height(world, avc_id, model_root_id, &avc)
                    }
                })
                .flatten()
            else {
                return;
            };
            let bounds = crate::engine::graphics::bounds::Aabb {
                min: [0.0, 0.0, 0.0],
                max: [0.0, height.max(0.0), 0.0],
            };
            let Some(inferred) = infer_upright_capsule(&bounds, avc.capsule_radius) else {
                return;
            };
            ("fallback_avatar_height", bounds, inferred)
        };
    let inferred = if xr.is_some() {
        // STAGE's floor-relative origin is the pre-tracking frame's Y. Geometry
        // supplies a stable rest-pose offset; never calibrate from live head Y.
        let scaled = crate::engine::graphics::bounds::Aabb {
            min: inference_bounds.min.map(|value| value * world_scale),
            max: inference_bounds.max.map(|value| value * world_scale),
        };
        let Some(inferred) = infer_upright_capsule(&scaled, avc.capsule_radius) else {
            return;
        };
        inferred
    } else {
        inferred
    };
    let (radius, half_segment) = match inferred.shape {
        CollisionShape::CapsuleY {
            radius,
            half_segment,
        } => (radius, half_segment),
        _ => unreachable!("upright capsule inference returned a non-capsule shape"),
    };
    let half_height = radius + half_segment;
    eprintln!(
        "[AVC][capsule][inferred] avc={avc_id:?} model_root={model_root_id:?} source={bounds_source} bounds_min={:?} bounds_max={:?} center_y={:.6} radius={radius:.6} half_segment={half_segment:.6} local_bottom={:.6} local_top={:.6} movement_target={movement_target:?}",
        inference_bounds.min,
        inference_bounds.max,
        inferred.center_y,
        inferred.center_y - half_height,
        inferred.center_y + half_height,
    );

    use crate::engine::ecs::component::transform_operator::TransformChannel;
    let fork = world.add_component(
        TransformOperatorComponent::fork_trs()
            .with_drop(TransformChannel::Rotation)
            .with_drop(TransformChannel::Scale),
    );
    let capsule_t =
        world.add_component(TransformComponent::new().with_position(0.0, inferred.center_y, 0.0));
    let serialize = world.add_component(SerializeComponent::off());
    let zone = world.add_component(ZoneComponent::new(inferred.shape));
    let mut collider = CollidableComponent::slide().with_runtime_movement_target(movement_target);
    collider.response = avc.collision_response;
    collider.query_owner = Some(avc_id);
    // The generic unresolved-source fallback preserves tracked Y. Never let
    // an XR proxy use it for contact while its sample is still pending.
    collider.enabled = xr.is_none();
    let collidable = world.add_component(collider);

    let _ = world.set_parent(fork, Some(model_root_id));
    let sample = vertical_source.map(|source| {
        let guid = world.get_component_record(source).unwrap().guid;
        let sample = world.add_component(
            TransformOperatorComponent::sample_translation(
                super::super::super::component::ComponentRef::Guid(guid),
            )
            .with_axes(super::super::super::component::transform_operator::TranslationAxes::Y),
        );
        let _ = world.set_parent(sample, Some(fork));
        emit.push_intent_now(
            sample,
            IntentValue::UpdateTransformWorld {
                component_id: sample,
            },
        );
        sample
    });
    let _ = world.set_parent(capsule_t, Some(sample.unwrap_or(fork)));
    // Omit the entire generated XR branch from scene saves, including its
    // source reference. Desktop keeps the previous serialization boundary.
    let _ = world.set_parent(serialize, Some(if xr.is_some() { fork } else { capsule_t }));
    let _ = world.set_parent(zone, Some(capsule_t));
    let _ = world.set_parent(collidable, Some(zone));

    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        avc.model_root_id = Some(model_root_id);
        avc.capsule_transform_id = Some(capsule_t);
        avc.capsule_collidable_id = Some(collidable);
        avc.capsule_sample_id = sample;
        avc.capsule_vertical_source = vertical_source;
        avc.capsule_world_scale = world_scale;
    }
    refresh_capsule_routing(avc_id, world);
    // Untracked rigs do not emit head/body splice transforms. Publish the new
    // proxy's world frame explicitly before contact can consume it; otherwise
    // its identity cache would push a correctly placed NPC up from the floor.
    emit.push_intent_now(
        model_root_id,
        IntentValue::UpdateTransformWorld {
            component_id: model_root_id,
        },
    );
}

pub(super) fn publish_capsule_readiness(
    avc_id: ComponentId,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    operators: &super::super::TransformOperatorSystem,
) {
    let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
        return;
    };
    let previously_ready = avatar.capsule_ready;
    let usable_zone = (|| {
        if !capsule_contact_valid(world, avatar, operators) {
            return None;
        }
        let collidable_id = avatar.capsule_collidable_id?;
        let collidable = world.get_component_by_id_as::<CollidableComponent>(collidable_id)?;
        if !collidable.enabled {
            return None;
        }
        if avatar.capsule_routing_revision != world.query_revision()
            || avatar.capsule_route_ref != avatar.movement_target
        {
            return None;
        }
        if avatar.collision_response == CollisionResponse::Slide {
            let target = collidable.movement_target_id?;
            if super::super::static_contact_system::movement_target(
                world,
                collidable_id,
                world.parent_of(collidable_id)?,
            ) != Some(target)
            {
                return None;
            }
        }
        let zone_id = world.parent_of(collidable_id)?;
        let zone = world.get_component_by_id_as::<ZoneComponent>(zone_id)?;
        if !zone.enabled {
            return None;
        }
        let frame = super::super::zone_query::resolve_zone_frame(world, zone_id, zone).ok()?;
        let model = super::super::TransformSystem::world_model(world, frame)?;
        super::super::collision_geometry::axis_aligned_world_shape(zone.shape, model)?;
        Some(zone_id)
    })();
    world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avc_id)
        .unwrap()
        .capsule_ready = usable_zone.is_some();
    if !previously_ready {
        if let Some(zone) = usable_zone {
            emit.push_event(
                avc_id,
                crate::engine::ecs::EventSignal::DataEvent {
                    name: "CapsuleReady".into(),
                    payload: Some(zone),
                },
            );
        }
    }
}

pub(super) fn log_settled_capsule_diagnostics(avc_id: ComponentId, world: &World) {
    let Some(avc) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
        return;
    };
    let Some(capsule_transform) = avc.capsule_transform_id else {
        return;
    };
    let Some(zone) = world
        .children_of(capsule_transform)
        .iter()
        .copied()
        .find(|id| world.get_component_by_id_as::<ZoneComponent>(*id).is_some())
    else {
        return;
    };
    let Some(shape) = world
        .get_component_by_id_as::<ZoneComponent>(zone)
        .map(|zone| zone.shape)
    else {
        return;
    };
    let CollisionShape::CapsuleY {
        radius,
        half_segment,
    } = shape
    else {
        return;
    };
    let local_center_y = world
        .get_component_by_id_as::<TransformComponent>(capsule_transform)
        .map(|transform| transform.transform.translation[1]);
    let world_center =
        crate::engine::ecs::system::TransformSystem::world_position(world, capsule_transform);
    let world_extents = world_center.map(|center| {
        let half_height = radius + half_segment;
        (center[1] - half_height, center[1] + half_height)
    });
    eprintln!(
        "[AVC][capsule][settled] avc={avc_id:?} capsule_transform={capsule_transform:?} zone={zone:?} local_center_y={local_center_y:?} world_center={world_center:?} radius={radius:.6} half_segment={half_segment:.6} world_bottom_top={world_extents:?}"
    );
}

pub(super) fn automatic_avc_movement_target(
    world: &World,
    avc_id: ComponentId,
) -> Option<ComponentId> {
    if let Some(source) = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .and_then(|avc| avc.movement_target.as_ref())
    {
        return resolve_component_ref(world, source, Some(avc_id), QueryRootMode::WorldRoot)
            .filter(|id| {
                world
                    .get_component_by_id_as::<TransformComponent>(*id)
                    .is_some()
            });
    }

    let mut current = Some(avc_id);
    while let Some(id) = current {
        if world
            .get_component_by_id_as::<InputXRComponent>(id)
            .is_some()
        {
            return xr_locomotion_target_transform(world, id);
        }
        current = world.parent_of(id);
    }
    world.parent_of(avc_id).filter(|id| {
        world
            .get_component_by_id_as::<TransformComponent>(*id)
            .is_some()
    })
}

pub(super) fn spawned_gltf_exists(world: &World, root: ComponentId) -> bool {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if world
            .get_component_by_id_as::<GLTFComponent>(id)
            .is_some_and(|gltf| gltf.spawned)
        {
            return true;
        }
        stack.extend(world.children_of(id).iter().copied());
    }
    false
}

pub(super) fn fallback_avatar_height(
    world: &World,
    _avc_id: ComponentId,
    model_root_id: ComponentId,
    avc: &AvatarControlComponent,
) -> Option<f32> {
    if let Some(height) = avc.avatar_height {
        return Some(height.max(0.0));
    }
    let bone = avc.splice_camera_bone.or(avc.displaced_head)?;
    let root_y = world
        .get_component_by_id_as::<TransformComponent>(model_root_id)?
        .transform
        .matrix_world[3][1];
    let bone_y = world
        .get_component_by_id_as::<TransformComponent>(bone)?
        .transform
        .matrix_world[3][1];
    Some((bone_y - root_y).abs())
}

pub(super) fn ancestor_input_xr_is_ready(world: &World, start: ComponentId) -> bool {
    let mut current = Some(start);
    while let Some(component) = current {
        if let Some(input) = world
            .get_component_by_id_as::<crate::engine::ecs::component::InputXRComponent>(component)
        {
            return input.pose_valid;
        }
        current = world.parent_of(component);
    }
    true
}
