//! Registration is queued when an MMS resource expression finishes, independent
//! of scene attachment. Shader preparation never executes script code.
use crate::engine::ecs::component::material::{
    MaterialBaseComponent, MaterialComponent, MaterialDefinitionComponent,
};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{
    VisualWorld, custom_material::CustomMaterialDefinition, materials::MaterialDefinitionId,
};

#[derive(Debug, Default)]
pub struct MaterialSystem;

impl MaterialSystem {
    pub fn tick(&mut self, world: &mut World, visuals: &mut VisualWorld) {
        for source in world.take_material_changes() {
            if let Err(error) = Self::register(world, visuals, source) {
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

    fn register(
        world: &mut World,
        visuals: &mut VisualWorld,
        source: ComponentId,
    ) -> Result<(), String> {
        if world
            .get_component_by_id_as::<MaterialDefinitionComponent>(source)
            .is_some()
        {
            Self::definition(world, visuals, source)?;
        } else if let Some(material) = world.get_component_by_id_as::<MaterialComponent>(source) {
            let definition = Self::definition(world, visuals, material.definition)?;
            let fields = &visuals
                .materials
                .definition(definition)
                .unwrap()
                .custom
                .as_ref()
                .unwrap()
                .fields;
            for name in material.values.keys() {
                if !fields.contains(name) {
                    return Err(format!("unknown material field '{name}'"));
                }
            }
            let values = fields
                .iter()
                .map(|name| material.values.get(name).copied().unwrap_or(0.0))
                .collect();
            let is_new = visuals.materials.source_instance(source).is_none();
            visuals
                .materials
                .upsert_custom_source(source, definition, values);
            if is_new {
                world.mark_material_scope_changed(source);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{CommandQueue, EventSignal, RxWorld, Signal};
    use crate::engine::graphics::RenderAssets;
    use crate::scripting::runner::RuntimeSpecSession;

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
        let (mut session, _) =
            RuntimeSpecSession::start(source, &mut world, &mut rx, Some(&mut assets), &mut queue)
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
        assert_eq!(
            visuals.materials.instance(instance).unwrap().values.len(),
            5
        );
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
            visuals.materials.instance(instance).unwrap().values[0],
            0.25
        );
        assert!(visuals.materials.instance(instance).unwrap().revision > revision);
        assert_eq!(visuals.materials.instance_count(), 1);
        let definition_id = visuals.materials.source_definition(definition).unwrap();
        let registered = visuals.materials.definition(definition_id).unwrap();
        assert!(registered.supports_emission_extraction);
        assert_eq!(
            registered.family,
            visuals.materials.extracted_emissive_family
        );
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
            0,
            "a late update cannot resurrect a removed definition"
        );
    }
}
