//! Common material registration, source resolution and retirement. Authored
//! Shading components are compatibility adapters over the same retained records.
//! Shader preparation never executes script code.
use crate::engine::ecs::component::material::{
    MaterialBaseComponent, MaterialComponent, MaterialDefinitionComponent,
};
use crate::engine::ecs::component::{
    ShadingComponent, ShadingModel, UnlitComponent, resolve_transmissive_model,
};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{
    MaterialHandle, VisualWorld,
    custom_material::CustomMaterialDefinition,
    materials::{MaterialDefinitionId, MaterialInstanceId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceSource {
    Material(ComponentId),
    Unlit,
}

#[derive(Debug, Default)]
pub struct MaterialSystem;

impl MaterialSystem {
    pub fn tick(&mut self, world: &mut World, visuals: &mut VisualWorld) {
        for source in world.take_material_changes() {
            let result = if world
                .get_component_by_id_as::<MaterialDefinitionComponent>(source)
                .is_some()
            {
                Self::definition(world, visuals, source).map(|_| ())
            } else {
                Self::publish_source(world, visuals, source)
            };
            if let Err(error) = result {
                eprintln!("[Material] {source:?}: {error}");
            }
        }
    }

    fn definition(
        world: &World,
        visuals: &mut VisualWorld,
        source: ComponentId,
    ) -> Result<MaterialDefinitionId, String> {
        if let Some(id) = visuals.materials.source_definition(source) {
            return Ok(id);
        }
        let authored = world
            .get_component_by_id_as::<MaterialDefinitionComponent>(source)
            .ok_or("material references a missing definition")?;
        if let Some(model) = authored.builtin {
            return Ok(match model {
                ShadingModel::Anime => visuals.materials.anime_definition,
                ShadingModel::Toon => visuals.materials.toon_definition,
            });
        }
        let base = world
            .get_component_by_id_as::<MaterialBaseComponent>(authored.base)
            .ok_or("definition references a missing MaterialBase")?
            .0;
        let custom = CustomMaterialDefinition::prepare(
            authored.fragment_uri.clone(),
            authored.fields.clone(),
            base,
        )?;
        Ok(visuals.materials.register_custom_definition(source, custom))
    }

    /// Select the nearest complete material declaration for every consumer.
    /// Authored local declarations precede generated Shading projections.
    pub(crate) fn resolve_source(world: &World, consumer: ComponentId) -> Option<ComponentId> {
        match Self::resolve_surface(world, consumer) {
            Some(SurfaceSource::Material(source)) => Some(source),
            Some(SurfaceSource::Unlit) | None => None,
        }
    }

    pub(crate) fn resolve_surface(world: &World, consumer: ComponentId) -> Option<SurfaceSource> {
        let mut current = Some(consumer);
        while let Some(node) = current {
            if world
                .get_component_by_id_as::<MaterialComponent>(node)
                .is_some()
            {
                return Some(SurfaceSource::Material(node));
            }
            if let Some(source) = Self::shading_source(world, node) {
                return Some(SurfaceSource::Material(source));
            }
            if world.children_of(node).iter().any(|&child| {
                world
                    .get_component_by_id_as::<UnlitComponent>(child)
                    .is_some()
            }) {
                return Some(SurfaceSource::Unlit);
            }
            if resolve_transmissive_model(world, node)
                .ok()
                .flatten()
                .is_some()
            {
                return None;
            }
            if let Some(source) = world.children_of(node).iter().copied().find(|&child| {
                world
                    .get_component_by_id_as::<MaterialComponent>(child)
                    .is_some()
            }) {
                return Some(SurfaceSource::Material(source));
            }
            let shading = world
                .children_of(node)
                .iter()
                .filter_map(|&child| {
                    let shading = world.get_component_by_id_as::<ShadingComponent>(child)?;
                    Some((
                        shading.source_component().is_some(),
                        Self::shading_source(world, child)?,
                    ))
                })
                .min_by_key(|(projected, _)| *projected);
            if let Some((false, source)) = shading {
                return Some(SurfaceSource::Material(source));
            }
            if let Some(source) = world
                .get_component_by_id_as::<crate::engine::ecs::component::RenderableComponent>(node)
                .and_then(|renderable| renderable.material_scope)
                .and_then(|scope| Self::resolve_surface(world, scope))
            {
                return Some(source);
            }
            if let Some((_, source)) = shading {
                return Some(SurfaceSource::Material(source));
            }
            current = world.parent_of(node);
        }
        None
    }

    fn shading_source(world: &World, declaration: ComponentId) -> Option<ComponentId> {
        let shading = world.get_component_by_id_as::<ShadingComponent>(declaration)?;
        let source = shading.source_component().unwrap_or(declaration);
        world
            .get_component_by_id_as::<ShadingComponent>(source)
            .map(|_| source)
    }

    /// GLTF's existing projection authoring adapter. Resolution itself is common.
    pub(crate) fn resolve_shading(
        world: &World,
        consumer: ComponentId,
    ) -> Option<(ComponentId, ShadingComponent)> {
        let source = Self::resolve_source(world, consumer)?;
        Some((
            source,
            *world.get_component_by_id_as::<ShadingComponent>(source)?,
        ))
    }

    pub(crate) fn ensure_source(
        world: &World,
        visuals: &mut VisualWorld,
        source: ComponentId,
    ) -> Result<MaterialInstanceId, String> {
        let (source, definition, values) = if let Some(material) =
            world.get_component_by_id_as::<MaterialComponent>(source)
        {
            let definition = Self::definition(world, visuals, material.definition)?;
            let record = visuals
                .materials
                .definition(definition)
                .ok_or("missing registered definition")?;
            for name in material.values.keys() {
                if record.parameter_schema.field(name).is_none() {
                    return Err(format!("unknown material field '{name}'"));
                }
            }
            let mut values = record.defaults.to_vec();
            for field in &record.parameter_schema.fields {
                if let Some(value) = material.values.get(&field.name) {
                    if value.len() != field.ty.width()
                        || value.iter().any(|value| !value.is_finite())
                    {
                        return Err(format!(
                            "material field '{}' requires {:?}",
                            field.name, field.ty
                        ));
                    }
                    values[field.slot..field.slot + value.len()].copy_from_slice(value);
                }
            }
            (source, definition, std::sync::Arc::<[f32]>::from(values))
        } else {
            let source = Self::shading_source(world, source).ok_or("missing material source")?;
            let shading = world
                .get_component_by_id_as::<ShadingComponent>(source)
                .unwrap();
            let definition = match shading.model {
                ShadingModel::Anime => visuals.materials.anime_definition,
                ShadingModel::Toon => visuals.materials.toon_definition,
            };
            let values = shading.material_values(
                &visuals
                    .materials
                    .definition(definition)
                    .unwrap()
                    .parameter_schema,
            );
            (source, definition, values)
        };
        Ok(visuals.materials.upsert_source(source, definition, values))
    }

    pub(crate) fn resolve_instance(
        world: &World,
        visuals: &mut VisualWorld,
        consumer: ComponentId,
    ) -> Option<MaterialInstanceId> {
        let source = Self::resolve_source(world, consumer)?;
        match Self::ensure_source(world, visuals, source) {
            Ok(instance) => {
                if let Some(renderable) = world
                    .get_component_by_id_as::<crate::engine::ecs::component::RenderableComponent>(
                        consumer,
                    )
                {
                    Self::program(visuals, instance, renderable.renderable.material)?;
                }
                Some(instance)
            }
            Err(error) => {
                eprintln!("[Material] {source:?}: {error}");
                None
            }
        }
    }

    /// Register/update any authored source. Only structural edits refresh bindings.
    pub(crate) fn publish_source(
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) -> Result<(), String> {
        let source = Self::shading_source(world, component).unwrap_or(component);
        let previous = visuals
            .materials
            .source_instance(source)
            .and_then(|id| visuals.materials.instance(id))
            .map(|record| record.definition);
        let instance = Self::ensure_source(world, visuals, source)?;
        if component != source {
            let value = *world
                .get_component_by_id_as::<ShadingComponent>(source)
                .ok_or("missing Shading source")?;
            if let Some(projection) =
                world.get_component_by_id_as_mut::<ShadingComponent>(component)
            {
                *projection = value.projected_from(source);
            }
        }
        if previous
            != visuals
                .materials
                .instance(instance)
                .map(|record| record.definition)
        {
            world.mark_material_scope_changed(source);
            world.mark_material_scope_changed(component);
            let consumers: Vec<_> = visuals.material_consumer_components(instance).collect();
            for consumer in consumers {
                world.mark_material_scope_changed(consumer);
            }
        }
        Ok(())
    }

    /// Program selection depends on the resolved definition and geometry interface.
    pub(crate) fn program(
        visuals: &VisualWorld,
        instance: MaterialInstanceId,
        fallback: MaterialHandle,
    ) -> Option<MaterialHandle> {
        let definition = visuals
            .materials
            .instance(instance)
            .and_then(|record| visuals.materials.definition(record.definition))?;
        if crate::engine::graphics::materials::builtin_program_capabilities(fallback)
            .cached_deformation
        {
            if definition.deformed_program.is_none() {
                eprintln!(
                    "[Material] definition '{}' has no cached-deformation program for this geometry",
                    definition.name
                );
            }
            definition.deformed_program
        } else {
            Some(definition.static_program)
        }
    }

    pub(crate) fn with_emissive(
        material: MaterialHandle,
        emissive_intensity: f32,
    ) -> MaterialHandle {
        crate::engine::graphics::materials::builtin_emission_variant(
            material,
            emissive_intensity > 0.0,
        )
    }

    pub(crate) fn retire_source(visuals: &mut VisualWorld, source: ComponentId) {
        for instance_source in visuals.materials.remove_definition_source(source) {
            visuals.remove_material_source(instance_source);
        }
        visuals.remove_material_source(source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{CommandQueue, EventSignal, RxWorld, Signal};
    use crate::engine::graphics::RenderAssets;
    use crate::scripting::runner::RuntimeSpecSession;

    #[test]
    fn material_program_contract_rejects_unavailable_custom_deformation() {
        let mut world = World::default();
        let source = world.add_component(ShadingComponent::new());
        let mut visuals = VisualWorld::default();
        let definition = visuals.materials.register_custom_definition(
            source,
            std::sync::Arc::new(CustomMaterialDefinition {
                base: crate::engine::graphics::materials::MaterialBaseKind::Surface,
                fields: vec!["gain".into()],
                fragment_uri: "test.frag".into(),
                compiled: std::sync::OnceLock::new(),
            }),
        );
        let instance = visuals
            .materials
            .upsert_source(source, definition, vec![1.0]);
        assert!(MaterialSystem::program(&visuals, instance, MaterialHandle::TOON_MESH).is_some());
        assert!(
            MaterialSystem::program(&visuals, instance, MaterialHandle::SKINNED_TOON_MESH)
                .is_none()
        );
    }

    #[test]
    fn one_resolver_preserves_local_material_overrides_and_legacy_boundaries() {
        use crate::engine::ecs::component::{RenderableComponent, TransformComponent};
        let mut world = World::default();
        let outer = world.add_component(ShadingComponent::new());
        let branch = world.add_component(TransformComponent::new());
        let cube = world.add_component(RenderableComponent::cube());
        world.add_child(outer, branch).unwrap();
        world.add_child(branch, cube).unwrap();
        assert_eq!(MaterialSystem::resolve_source(&world, cube), Some(outer));
        let local_material = world.add_component(MaterialComponent {
            definition: ComponentId::default(),
            values: Default::default(),
        });
        world.add_child(branch, local_material).unwrap();
        assert_eq!(
            MaterialSystem::resolve_source(&world, cube),
            Some(local_material)
        );
        let local_shading = world.add_component(ShadingComponent::toon());
        world.add_child(cube, local_shading).unwrap();
        assert_eq!(
            MaterialSystem::resolve_source(&world, cube),
            Some(local_shading)
        );
        world.detach_child(cube, local_shading);
        assert_eq!(
            MaterialSystem::resolve_source(&world, cube),
            Some(local_material)
        );
        world.detach_child(branch, local_material);
        assert_eq!(MaterialSystem::resolve_source(&world, cube), Some(outer));
        let unlit = world.add_component(UnlitComponent::default());
        world.add_child(cube, unlit).unwrap();
        assert_eq!(MaterialSystem::resolve_source(&world, cube), None);
        world.detach_child(cube, unlit);
        let transmission =
            world.add_component(crate::engine::ecs::component::RefractionComponent::default());
        world.add_child(cube, transmission).unwrap();
        assert_eq!(MaterialSystem::resolve_source(&world, cube), None);
    }

    #[test]
    fn builtin_sources_initialize_detached_and_scalar_edits_keep_bindings_clean() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut system = MaterialSystem;
        let source = world.add_component(ShadingComponent::new());
        world.mark_material_changed(source);
        system.tick(&mut world, &mut visuals);
        let binding = visuals.materials.source_instance(source).unwrap();
        let revision = visuals.materials.instance(binding).unwrap().revision;
        world.take_shading_topology_changes();
        world
            .get_component_by_id_as_mut::<ShadingComponent>(source)
            .unwrap()
            .rim_strength = 0.4;
        world.mark_material_changed(source);
        system.tick(&mut world, &mut visuals);
        assert_eq!(visuals.materials.source_instance(source), Some(binding));
        assert_eq!(
            visuals.materials.instance(binding).unwrap().revision,
            revision + 1
        );
        assert!(world.take_shading_topology_changes().is_empty());
        let definition = visuals
            .materials
            .definition(visuals.materials.anime_definition)
            .unwrap();
        assert_eq!(
            definition.parameter_schema.read(
                &visuals.materials.instance(binding).unwrap().values,
                "rim_strength"
            ),
            Some([0.4].as_slice())
        );
        MaterialSystem::retire_source(&mut visuals, source);
        assert!(visuals.materials.instance(binding).is_none());
    }

    #[test]
    fn youtuber_room_view_switches_select_desktop_xr_or_both() {
        use crate::engine::ecs::component::{
            AmplitudeComponent, AvatarControlComponent, Camera3DComponent, CameraXRComponent,
            CollidableComponent, CollidableMode, GravityComponent, MorphTargetMapComponent,
            VelocityComponent, XREyeTrackingComponent, XrComponent,
        };
        for (desktop, xr) in [(true, false), (false, true), (true, true)] {
            let source = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/youtuber-room.mms"
            ))
            .lines()
            .map(|line| {
                if line.starts_with("let desktop_view_enabled =") {
                    format!("let desktop_view_enabled = {desktop}")
                } else if line.starts_with("let xr_view_enabled =") {
                    format!("let xr_view_enabled = {xr}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
            let mut world = World::default();
            let mut rx = RxWorld::default();
            let mut queue = CommandQueue::new();
            let mut assets = RenderAssets::new();
            let (_session, _) = RuntimeSpecSession::start_at_path(
                &source,
                "examples/youtuber-room.mms",
                &mut world,
                &mut rx,
                Some(&mut assets),
                &mut queue,
            )
            .unwrap();
            let desktop_count = world
                .all_components()
                .filter(|&id| {
                    world
                        .get_component_by_id_as::<Camera3DComponent>(id)
                        .is_some()
                })
                .count();
            let xr_count = world
                .all_components()
                .filter(|&id| {
                    world
                        .get_component_by_id_as::<CameraXRComponent>(id)
                        .is_some()
                })
                .count();
            assert_eq!(desktop_count, usize::from(desktop));
            assert_eq!(xr_count, usize::from(xr));
            let states: Vec<_> = world
                .all_components()
                .filter_map(|id| {
                    world
                        .get_component_by_id_as::<XrComponent>(id)
                        .map(|state| state.enabled)
                })
                .collect();
            assert_eq!(states, vec![xr]);
            let xr_only = xr && !desktop;
            let named = |name| {
                world
                    .all_components()
                    .find(|&id| world.component_label(id) == Some(name))
            };
            assert_eq!(named("room_reimu").is_some(), !xr_only);
            assert_eq!(named("room_reimu_anchor").is_some(), !xr_only);
            assert_eq!(named("xr_reimu").is_some(), xr_only);
            let avatars: Vec<_> = world
                .all_components()
                .filter_map(|id| world.get_component_by_id_as::<AvatarControlComponent>(id))
                .collect();
            assert_eq!(avatars.len(), usize::from(xr_only));
            assert_eq!(named("room_reimu_microphone").is_some(), xr_only);
            assert_eq!(
                world
                    .all_components()
                    .filter(|&id| world.component_label(id) == Some("slide_wall_collision"))
                    .count(),
                64 * 12
            );
            assert!(
                world
                    .all_components()
                    .filter_map(|id| world.get_component_by_id_as::<CollidableComponent>(id))
                    .filter(|collider| collider.mode == CollidableMode::Static && collider.enabled)
                    .count()
                    > 768
            );
            assert_eq!(named("room_reimu_gravity").is_some(), xr_only);
            if xr_only {
                let avatar = avatars[0];
                assert!(avatar.collision_enabled && avatar.movement_target.is_some());
                assert!(
                    !world
                        .get_component_by_id_as::<GravityComponent>(
                            named("room_reimu_gravity").unwrap()
                        )
                        .unwrap()
                        .enabled,
                    "gravity must wait for CapsuleReady rather than fall before tracking initializes"
                );
                assert!(
                    world
                        .get_component_by_id_as::<VelocityComponent>(
                            named("room_reimu_velocity").unwrap()
                        )
                        .is_some()
                );
                assert!(avatar.mouth_open_amplitude.is_some());
                assert!((avatar.mouth_open_rms_floor - 0.005).abs() < 0.00001);
                assert!((avatar.mouth_open_rms_ceiling - 0.09).abs() < 0.00001);
                assert_eq!(avatar.mouth_open_smoothing, 16.0);
                assert!(avatar.crouch_enabled && avatar.crouch_sole_offsets.is_some());
                let amplitude = world
                    .get_component_by_id_as::<AmplitudeComponent>(
                        named("room_reimu_voice_level").unwrap(),
                    )
                    .unwrap();
                assert!(amplitude.source.is_some());
                assert_eq!(amplitude.highpass_hz, Some(120.0));
                let gltf = named("xr_reimu").unwrap();
                let morphs = world
                    .children_of(gltf)
                    .iter()
                    .find_map(|&id| world.get_component_by_id_as::<MorphTargetMapComponent>(id))
                    .unwrap();
                assert_eq!(
                    morphs.slots().get("viseme_aa").map(String::as_str),
                    Some("Fcl_MTH_A")
                );
                assert_eq!(morphs.slots().len(), 3);
                let tracking = world
                    .all_components()
                    .find_map(|id| world.get_component_by_id_as::<XREyeTrackingComponent>(id))
                    .unwrap();
                assert!(!tracking.enable_pupil_direction_tracking);
            }
        }
    }

    #[test]
    fn youtuber_room_registers_unattached_definitions_shares_leds_and_updates_one_record() {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut queue = CommandQueue::new();
        let mut assets = RenderAssets::new();
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/youtuber-room.mms"
        ));
        let (mut session, _) = RuntimeSpecSession::start_at_path(
            source,
            "examples/youtuber-room.mms",
            &mut world,
            &mut rx,
            Some(&mut assets),
            &mut queue,
        )
        .unwrap();
        let definition = world
            .all_components()
            .find(|&id| {
                world
                    .get_component_by_id_as::<MaterialDefinitionComponent>(id)
                    .is_some()
            })
            .unwrap();
        let material = world
            .all_components()
            .find(|&id| {
                world
                    .get_component_by_id_as::<MaterialComponent>(id)
                    .is_some()
            })
            .unwrap();
        assert_eq!(world.parents_of(definition).count(), 0);
        assert!(!world.is_initialized(definition));
        assert_eq!(world.parents_of(material).count(), 4);
        let mut visuals = VisualWorld::new();
        let mut system = MaterialSystem;
        system.tick(&mut world, &mut visuals);
        let instance = visuals.materials.source_instance(material).unwrap();
        let time_index = world
            .get_component_by_id_as::<MaterialDefinitionComponent>(definition)
            .unwrap()
            .fields
            .iter()
            .position(|field| field.name == "time_seconds")
            .unwrap();
        assert_eq!(
            visuals.materials.instance(instance).unwrap().values.len(),
            5
        );
        let instance_count = visuals.materials.instance_count();
        let revision = visuals.materials.instance(instance).unwrap().revision;
        rx.dispatch_event_handlers(
            &mut world,
            &Signal::event(
                ComponentId::default(),
                EventSignal::FrameTick { dt_sec: 0.25 },
            ),
        );
        let output = session.service_callbacks(&mut world, &mut rx, Some(&mut assets), &mut queue);
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        system.tick(&mut world, &mut visuals);
        assert_eq!(visuals.materials.source_instance(material), Some(instance));
        assert_eq!(
            visuals.materials.instance(instance).unwrap().values[time_index],
            0.25
        );
        assert!(visuals.materials.instance(instance).unwrap().revision > revision);
        assert_eq!(visuals.materials.instance_count(), instance_count);
        let definition_id = visuals.materials.source_definition(definition).unwrap();
        let registered = visuals.materials.definition(definition_id).unwrap();
        let prepared = registered.custom.as_ref().unwrap().clone();
        let other_programs: Vec<_> = visuals
            .materials
            .instances()
            .filter_map(|(_, record)| {
                visuals
                    .materials
                    .definition(record.definition)?
                    .custom
                    .clone()
            })
            .collect();
        assert!(registered.capabilities.supports_emission_extraction());
        assert_eq!(registered.base, visuals.materials.extracted_emissive_base);
        crate::engine::ecs::SystemWorld::default().remove_subtree_immediate(
            &mut world,
            &mut visuals,
            definition,
        );
        assert!(visuals.materials.definition(definition_id).is_none());
        assert!(visuals.materials.instance(instance).is_none());
        world.mark_material_changed(material);
        system.tick(&mut world, &mut visuals);
        assert_eq!(
            visuals.materials.instance_count(),
            instance_count - 1,
            "a late update cannot resurrect a removed definition"
        );
        // Let the compiler worker finish before the test process tears down
        // shaderc/glslang globals. The detached resource must remain valid.
        for program in std::iter::once(prepared).chain(other_programs) {
            for _ in 0..200 {
                if program.compiled.get().is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let compiled = program
                .compiled
                .get()
                .expect("material compilation must complete");
            assert!(
                compiled.is_ok(),
                "{}: {:?}",
                program.fragment_uri,
                compiled.as_ref().err()
            );
        }
    }
}

#[cfg(test)]
#[path = "material_unlit_tests.rs"]
mod unlit_tests;
