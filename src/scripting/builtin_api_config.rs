//! Global engine APIs, separate from component authoring vocabulary.
use super::*;
pub(super) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    builder.namespace("mittens", |namespace| {
        namespace.api(
            "smoke",
            mms::ValueSignature::new(Vec::new(), mms::ValueType::Null),
            MittensBinding::Api(MittensApi::Smoke),
        );
    });

    builder.host_api(
        "emit_data",
        mms::ValueSignature::with_optional(
            vec![
                mms::ValueType::Component,
                mms::ValueType::String,
                mms::ValueType::Any,
            ],
            2,
            mms::ValueType::Null,
        ),
        MittensBinding::Api(MittensApi::EmitData),
    );

    crate::scripting::host::xr_settings_api::register(builder);

    crate::scripting::host::graphics_settings_api::register(builder);
    crate::scripting::host::benchmark_api::register(builder);

    crate::scripting::file_api::register(builder);
    crate::scripting::file_browser::register(builder);
    crate::scripting::json_api::register(builder);

    builder.namespace("AudioInput", |namespace| {
        namespace.api(
            "devices",
            mms::ValueSignature::new(Vec::new(), mms::ValueType::Array),
            MittensBinding::Api(MittensApi::AudioInputDevices),
        );
    });
    builder.namespace("AudioOutput", |namespace| {
        namespace.api(
            "devices",
            mms::ValueSignature::new(Vec::new(), mms::ValueType::Array),
            MittensBinding::Api(MittensApi::AudioOutputDevices),
        );
    });
}
