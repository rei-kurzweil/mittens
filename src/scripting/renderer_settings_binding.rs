use super::{
    MittensBinding, component_signature, host_builder_call, host_constructor,
    host_constructor_and_builder,
};
use meow_meow_script as mms;

pub(super) fn register(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
) {
    host_constructor_and_builder(
        component,
        canonical,
        "preferred_xr_refresh_rate",
        component_signature([mms::ValueType::F32]),
    );
    host_constructor(component, canonical, "msaa_off", component_signature([]));
    host_constructor_and_builder(
        component,
        canonical,
        "window_size",
        component_signature(vec![mms::ValueType::U32; 2]),
    );
    host_constructor_and_builder(
        component,
        canonical,
        "transmission_depth_compare",
        component_signature([mms::ValueType::Bool]),
    );
    host_constructor(
        component,
        canonical,
        "desired_xr_resolution",
        component_signature([mms::ValueType::Array]),
    );
    host_builder_call(
        component,
        canonical,
        "desired_xr_resolution",
        component_signature([mms::ValueType::Array]),
    );
}

#[cfg(test)]
mod tests {
    use crate::engine::{
        Universe,
        ecs::{World, component::RendererSettingsComponent},
    };

    #[test]
    fn refresh_preference_supports_constructor_builder_and_serialization() {
        for source in [
            "RendererSettings.preferred_xr_refresh_rate(75) {}",
            "RendererSettings { preferred_xr_refresh_rate(75) }",
        ] {
            let mut scene = Universe::new(World::default());
            scene
                .load_mms_source_at_path(source, "examples/shell-client.mms")
                .unwrap();
            let settings = scene
                .world
                .all_components()
                .find_map(|id| {
                    scene
                        .world
                        .get_component_by_id_as::<RendererSettingsComponent>(id)
                })
                .unwrap();
            assert_eq!(settings.preferred_xr_refresh_rate, Some(75.0));
            use crate::engine::ecs::component::Component;
            let ast = format!("{:?}", settings.to_mms_ast(&scene.world));
            assert!(ast.contains("preferred_xr_refresh_rate"));
        }
        for rate in [0.0, -75.0, f32::NAN, f32::INFINITY] {
            assert!(
                RendererSettingsComponent::new()
                    .with_preferred_xr_refresh_rate(rate)
                    .is_err()
            );
        }
    }
}
