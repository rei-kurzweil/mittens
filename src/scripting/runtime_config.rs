//! The crate-owned MMS runtime specification used by Mittens.
//!
//! Component names, aliases, and the migrated constructor/body vocabulary are
//! authoritative here. The legacy registry remains the host implementation
//! while its declarations move into this specification.

use meow_meow_script as mms;

/// Engine implementations attached to host-effectful runtime declarations.
///
/// Categories encode receiver lifecycle. Constructors have no receiver,
/// initializers receive a newly created component during tree assembly, and
/// methods receive a checked live component handle. Keeping this type and the
/// completed table in the build result prevents callers from reconstructing
/// either half later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MittensBinding {
    ComponentConstructor {
        component: &'static str,
        name: Option<&'static str>,
    },
    ComponentInitializer {
        component: &'static str,
        name: &'static str,
        kind: ComponentInitializerKind,
    },
    ComponentMethod {
        component: &'static str,
        name: &'static str,
    },
    Api(MittensApi),
    Signal {
        name: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentInitializerKind {
    Call,
    Property,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MittensApi {
    Smoke,
    FileReadText,
    JsonParse,
    JsonStringify,
    AudioInputDevices,
    AudioOutputDevices,
}

/// The crate-owned runtime and matching Mittens bindings from one build.
pub type MittensRuntime = mms::ConfiguredRuntime<MittensBinding>;

fn component_signature(arguments: impl Into<Vec<mms::ValueType>>) -> mms::ValueSignature {
    mms::ValueSignature::new(arguments, mms::ValueType::Component)
}

fn optional_component_signature(
    arguments: impl Into<Vec<mms::ValueType>>,
    minimum_arguments: usize,
) -> mms::ValueSignature {
    mms::ValueSignature::with_optional(arguments, minimum_arguments, mms::ValueType::Component)
}

fn constructor_and_builder(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    name: &str,
    signature: mms::ValueSignature,
) {
    component
        .constructor(name, signature.clone())
        .builder_call(name, signature);
}

fn host_constructor(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
    name: &'static str,
    signature: mms::ValueSignature,
) {
    component.host_constructor(
        name,
        signature,
        MittensBinding::ComponentConstructor {
            component: canonical,
            name: Some(name),
        },
    );
}

fn host_builder_call(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
    name: &'static str,
    signature: mms::ValueSignature,
) {
    component.host_builder_call(
        name,
        signature,
        MittensBinding::ComponentInitializer {
            component: canonical,
            name,
            kind: ComponentInitializerKind::Call,
        },
    );
}

fn host_property(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
    name: &'static str,
    value_type: mms::ValueType,
) {
    component.host_property(
        name,
        value_type,
        MittensBinding::ComponentInitializer {
            component: canonical,
            name,
            kind: ComponentInitializerKind::Property,
        },
    );
}

fn host_method(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
    name: &'static str,
    signature: mms::ValueSignature,
) {
    component.method(
        name,
        signature,
        MittensBinding::ComponentMethod {
            component: canonical,
            name,
        },
    );
}

fn host_constructor_and_builder(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
    name: &'static str,
    signature: mms::ValueSignature,
) {
    host_constructor(component, canonical, name, signature.clone());
    host_builder_call(component, canonical, name, signature);
}

fn no_arg_constructors(component: &mut mms::ComponentBuilder<'_, MittensBinding>, names: &[&str]) {
    for name in names {
        component.constructor(*name, component_signature([]));
    }
}

fn no_arg_builders(component: &mut mms::ComponentBuilder<'_, MittensBinding>, names: &[&str]) {
    for name in names {
        component.builder_call(*name, component_signature([]));
    }
}

fn no_arg_constructors_and_builders(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    names: &[&str],
) {
    no_arg_constructors(component, names);
    no_arg_builders(component, names);
}

/// Build the strict MMS vocabulary and its opaque engine binding table.
pub fn build_mittens_runtime() -> Result<MittensRuntime, mms::RuntimeSpecError> {
    let mut builder = mms::RuntimeSpec::builder::<MittensBinding>();
    builder
        .component_name_policy(mms::ComponentNamePolicy::StrictRegistered)
        .with_standard_builtins();

    for &canonical in crate::scripting::component_registry::SUPPORTED_COMPONENT_NAMES {
        // `MusicNote` is currently both a standard builtin table and an
        // engine component name. The RuntimeSpec correctly rejects that
        // ambiguous global declaration; resolving the language spelling is a
        // separate vocabulary decision. The `NOTE` component shortform stays
        // on the legacy path until then.
        if canonical == "MusicNote" {
            continue;
        }
        builder.component(canonical, |component| {
            component.host_default_constructor(MittensBinding::ComponentConstructor {
                component: canonical,
                name: None,
            });
            if matches!(canonical, "Data" | "Style") {
                component.body_mode(mms::ComponentBodyMode::PropsOnly);
            } else if canonical == "Keyframe" {
                component.body_mode(mms::ComponentBodyMode::Deferred);
            }
            for shortform in mms::COMPONENT_SHORTFORMS.iter().filter(|entry| {
                entry.full == canonical && !entry.short.eq_ignore_ascii_case(canonical)
            }) {
                component.alias(shortform.short);
            }
            host_property(component, canonical, "name", mms::ValueType::String);
            host_property(component, canonical, "id", mms::ValueType::String);
            host_property(component, canonical, "class", mms::ValueType::Any);

            let floats = |count| component_signature(vec![mms::ValueType::F32; count]);
            let unsigned = |count| component_signature(vec![mms::ValueType::U32; count]);
            let booleans = |count| component_signature(vec![mms::ValueType::Bool; count]);
            let strings = |count| component_signature(vec![mms::ValueType::String; count]);
            let any = |count| component_signature(vec![mms::ValueType::Any; count]);
            let no_args = || component_signature([]);

            let method = |arguments, result| mms::ValueSignature::new(arguments, result);
            for (name, signature) in [
                (
                    "attach",
                    method(vec![mms::ValueType::Component], mms::ValueType::Null),
                ),
                (
                    "attach_clone",
                    method(vec![mms::ValueType::Component], mms::ValueType::Null),
                ),
                ("detach", method(vec![], mms::ValueType::Null)),
                (
                    "remove_child",
                    method(vec![mms::ValueType::U32], mms::ValueType::Null),
                ),
                ("remove_subtree", method(vec![], mms::ValueType::Null)),
                (
                    "set_color",
                    method(vec![mms::ValueType::Array], mms::ValueType::Null),
                ),
            ] {
                host_method(component, canonical, name, signature);
            }

            match canonical {
                "Transform" => {
                    host_method(
                        component,
                        canonical,
                        "local_bounds",
                        method(vec![], mms::ValueType::Any),
                    );
                    host_method(
                        component,
                        canonical,
                        "update_transform",
                        method(vec![mms::ValueType::Array; 3], mms::ValueType::Null),
                    );
                    host_method(
                        component,
                        canonical,
                        "rest_relative_rotation",
                        method(vec![mms::ValueType::Array], mms::ValueType::Null),
                    );
                    host_method(
                        component,
                        canonical,
                        "look_at",
                        method(vec![mms::ValueType::Array], mms::ValueType::Null),
                    );
                    host_method(
                        component,
                        canonical,
                        "translation",
                        method(vec![], mms::ValueType::Array),
                    );
                    host_method(
                        component,
                        canonical,
                        "trs",
                        mms::ValueSignature::with_optional(
                            vec![mms::ValueType::Any],
                            0,
                            mms::ValueType::Any,
                        ),
                    );
                }
                "TransformWorld" => {
                    host_method(
                        component,
                        canonical,
                        "trs",
                        mms::ValueSignature::with_optional(
                            vec![mms::ValueType::Any],
                            0,
                            mms::ValueType::Any,
                        ),
                    );
                }
                "PoseCapturePose" => {
                    for name in ["apply", "overlay"] {
                        host_method(
                            component,
                            canonical,
                            name,
                            method(vec![mms::ValueType::Component], mms::ValueType::Null),
                        );
                    }
                    host_method(
                        component,
                        canonical,
                        "apply_blended",
                        method(
                            vec![mms::ValueType::Component, mms::ValueType::F32],
                            mms::ValueType::Null,
                        ),
                    );
                }
                "Emissive" => {
                    host_method(
                        component,
                        canonical,
                        "set_intensity",
                        method(vec![mms::ValueType::F32], mms::ValueType::Null),
                    );
                    for name in ["on", "off"] {
                        host_method(
                            component,
                            canonical,
                            name,
                            method(vec![], mms::ValueType::Null),
                        );
                    }
                }
                "Raycast" => host_method(
                    component,
                    canonical,
                    "request_raycast",
                    method(vec![], mms::ValueType::Null),
                ),
                "AudioBandPassFilter" => host_method(
                    component,
                    canonical,
                    "set_center_hz",
                    method(vec![mms::ValueType::F32], mms::ValueType::Null),
                ),
                "HttpClient" => {
                    for name in ["get", "delete"] {
                        host_method(
                            component,
                            canonical,
                            name,
                            method(vec![mms::ValueType::String], mms::ValueType::Null),
                        );
                    }
                    for name in ["post", "put"] {
                        host_method(
                            component,
                            canonical,
                            name,
                            method(vec![mms::ValueType::String; 2], mms::ValueType::Null),
                        );
                    }
                }
                "HttpServer" => host_method(
                    component,
                    canonical,
                    "reply_text",
                    method(
                        vec![
                            mms::ValueType::Table,
                            mms::ValueType::U16,
                            mms::ValueType::String,
                        ],
                        mms::ValueType::Null,
                    ),
                ),
                _ => {}
            }

            match canonical {
                "Transform" => {
                    for method in ["position", "scale", "rotation", "rotation_euler"] {
                        host_constructor_and_builder(component, canonical, method, floats(3));
                    }
                    for method in ["rotation_quat", "quaternion", "quat"] {
                        constructor_and_builder(component, method, floats(4));
                    }
                    constructor_and_builder(component, "looking_at", any(1));
                }
                "Renderable" => {
                    host_constructor(component, canonical, "cube", no_args());
                    no_arg_constructors(
                        component,
                        &[
                            "circle2d",
                            "sphere",
                            "triangle",
                            "square",
                            "plane",
                            "tetrahedron",
                        ],
                    );
                    component
                        .constructor(
                            "cone",
                            optional_component_signature([mms::ValueType::U32], 0),
                        )
                        .constructor(
                            "icosahedron",
                            optional_component_signature(
                                [mms::ValueType::U32, mms::ValueType::F32],
                                0,
                            ),
                        )
                        .constructor(
                            "wireframe_sphere",
                            optional_component_signature(
                                [
                                    mms::ValueType::U32,
                                    mms::ValueType::U32,
                                    mms::ValueType::F32,
                                ],
                                0,
                            ),
                        )
                        .constructor(
                            "wireframe_icosahedron",
                            optional_component_signature(
                                [
                                    mms::ValueType::U32,
                                    mms::ValueType::F32,
                                    mms::ValueType::F32,
                                ],
                                0,
                            ),
                        )
                        .constructor(
                            "wireframe_box",
                            optional_component_signature([mms::ValueType::F32], 0),
                        )
                        .constructor(
                            "wireframe_square",
                            optional_component_signature([mms::ValueType::F32], 0),
                        )
                        .constructor(
                            "heart",
                            optional_component_signature([mms::ValueType::U32], 0),
                        )
                        // Polygon has a stable mesh key plus its 2D point list.
                        // Keep the strict runtime signature aligned with
                        // component_registry::polygon_args.
                        .constructor("polygon", any(2))
                        .constructor(
                            "partial_annulus_2d",
                            optional_component_signature(
                                [
                                    mms::ValueType::F32,
                                    mms::ValueType::F32,
                                    mms::ValueType::F32,
                                    mms::ValueType::F32,
                                    mms::ValueType::U32,
                                ],
                                0,
                            ),
                        )
                        .constructor(
                            "star",
                            optional_component_signature(
                                [
                                    mms::ValueType::U32,
                                    mms::ValueType::F32,
                                    mms::ValueType::U32,
                                    mms::ValueType::U32,
                                ],
                                0,
                            ),
                        );
                }
                "CombineMesh" => no_arg_constructors(component, &["keep_transforms"]),
                "ImplicitSurface" => {
                    for (name, signature) in [
                        ("bounds", floats(6)),
                        ("voxel_size", floats(1)),
                        ("iso_level", floats(1)),
                        ("smooth_min_radius", floats(1)),
                    ] {
                        host_constructor_and_builder(component, canonical, name, signature);
                    }
                }
                "ImplicitSphere" => {
                    host_constructor_and_builder(component, canonical, "radius", floats(1));
                }
                "Color" => {
                    host_constructor(component, canonical, "rgba", floats(4));
                }
                "Refraction" | "RoughTransmission" => {
                    for method in ["ior", "thickness", "strength", "edge_fade"] {
                        host_constructor_and_builder(component, canonical, method, floats(1));
                    }
                    if canonical == "RoughTransmission" {
                        host_constructor_and_builder(component, canonical, "roughness", floats(1));
                    }
                }
                "BackgroundColor" => {
                    host_constructor(component, canonical, "rgba", floats(4));
                }
                "Camera3D" => {
                    for method in ["enabled"] {
                        host_constructor_and_builder(component, canonical, method, booleans(1));
                    }
                    for method in ["fov", "near", "far"] {
                        host_constructor_and_builder(component, canonical, method, floats(1));
                    }
                    constructor_and_builder(component, "target", strings(1));
                }
                "Camera2D" => constructor_and_builder(component, "target", strings(1)),
                "CameraXR" => {
                    no_arg_constructors(component, &["on", "off"]);
                    component
                        .builder_call("enabled", booleans(1))
                        .builder_call("target", strings(1));
                }
                "Emissive" => {
                    host_constructor(component, canonical, "on", no_args());
                    host_constructor(component, canonical, "off", no_args());
                    host_builder_call(component, canonical, "intensity", floats(1));
                }
                "AmbientLight" => {
                    host_constructor(component, canonical, "rgb", floats(3));
                }
                "DirectionalLight" | "PointLight" | "SpotLight" => {
                    if canonical == "DirectionalLight" {
                        host_constructor_and_builder(component, canonical, "intensity", floats(1));
                        host_constructor_and_builder(component, canonical, "color", floats(3));
                    } else {
                        constructor_and_builder(component, "intensity", floats(1));
                        constructor_and_builder(component, "color", floats(3));
                    }
                    if canonical != "DirectionalLight" {
                        constructor_and_builder(component, "distance", floats(1));
                    }
                    if canonical == "SpotLight" {
                        constructor_and_builder(component, "angle", floats(1));
                        constructor_and_builder(component, "penumbra", floats(1));
                    }
                }
                "Bloom" => {
                    host_constructor(component, canonical, "on", no_args());
                    host_constructor(component, canonical, "off", no_args());
                    host_constructor(component, canonical, "intensity", floats(1));
                    host_constructor(component, canonical, "radius_ndc", floats(1));
                    host_constructor(component, canonical, "emissive_scale", floats(1));
                    host_constructor(component, canonical, "half_res", booleans(1));
                    host_constructor(component, canonical, "output_texture", strings(1));
                    host_builder_call(component, canonical, "enabled", booleans(1));
                    host_builder_call(component, canonical, "intensity", floats(1));
                    host_builder_call(component, canonical, "radius_ndc", floats(1));
                    host_builder_call(component, canonical, "emissive_scale", floats(1));
                    host_builder_call(component, canonical, "half_res", booleans(1));
                    host_builder_call(component, canonical, "output_texture", strings(1));
                }
                "BlurPass" => {
                    host_constructor(component, canonical, "on", no_args());
                    host_constructor(component, canonical, "off", no_args());
                    host_constructor(component, canonical, "enabled", booleans(1));
                    host_constructor(component, canonical, "radius_ndc", floats(1));
                    host_constructor(component, canonical, "half_res", booleans(1));
                    host_builder_call(component, canonical, "enabled", booleans(1));
                    host_builder_call(component, canonical, "radius_ndc", floats(1));
                    host_builder_call(component, canonical, "half_res", booleans(1));
                }
                "RenderGraph" => {
                    host_constructor(component, canonical, "on", no_args());
                    host_constructor(component, canonical, "off", no_args());
                    component.builder_call("enabled", booleans(1));
                }
                "RendererSettings" => {
                    host_constructor(component, canonical, "msaa_off", no_args());
                    host_constructor(component, canonical, "window_size", unsigned(2));
                    host_builder_call(component, canonical, "window_size", unsigned(2));
                    host_constructor_and_builder(
                        component,
                        canonical,
                        "transmission_depth_compare",
                        booleans(1),
                    );
                }
                "Grid" => {
                    constructor_and_builder(component, "spacing", floats(1));
                    for method in ["size_x", "size_z"] {
                        constructor_and_builder(component, method, unsigned(1));
                    }
                    constructor_and_builder(component, "hidden", booleans(1));
                    component
                        .builder_call("enabled", booleans(1))
                        .builder_call("selectable", booleans(1))
                        .builder_call("visual_space", strings(1));
                }
                "HttpClient" => {
                    constructor_and_builder(component, "enabled", booleans(1));
                    constructor_and_builder(
                        component,
                        "timeout_ms",
                        component_signature([mms::ValueType::U64]),
                    );
                }
                "HttpServer" => {
                    constructor_and_builder(component, "bind", strings(1));
                    constructor_and_builder(component, "enabled", booleans(1));
                }
                "XREyeTracking" => {
                    component.constructor("on", no_args());
                    component.constructor(
                        "listen",
                        component_signature([mms::ValueType::String, mms::ValueType::U16]),
                    );
                    component.builder_call("priority", any(1));
                    component.builder_call("enable_pupil_direction_tracking", booleans(1));
                    component.builder_call("head_rotation_compensation", strings(1));
                    component.builder_call("rotation_limits", floats(4));
                    component.builder_call("rotation_limits_per_eye", any(2));
                }
                "XREyeTrackingHTC" | "VRChatOSCEyeTracking" | "HTCEyeTracking" => {
                    component.constructor("on", no_args());
                    component.constructor(
                        "listen",
                        component_signature([mms::ValueType::String, mms::ValueType::U16]),
                    );
                    component.builder_call("enable_pupil_direction_tracking", booleans(1));
                    component.builder_call("head_rotation_compensation", strings(1));
                    component.builder_call("rotation_limits", floats(4));
                    component.builder_call("rotation_limits_per_eye", any(2));
                }
                "MediaPipeEyeTracking" => {
                    component.constructor("on", no_args());
                    component.builder_call("head_rotation_compensation", strings(1));
                    component.builder_call("rotation_limits", floats(4));
                    component.builder_call("rotation_limits_per_eye", any(2));
                }
                "GridBinding" => {
                    component.constructor("grid", any(1));
                }
                "Opacity" => {
                    constructor_and_builder(component, "opacity", floats(1));
                    no_arg_constructors(component, &["multiple_layers"]);
                    no_arg_builders(component, &["multiple_layers"]);
                }
                "Input" => {
                    constructor_and_builder(component, "speed", floats(1));
                    component.builder_call("enabled", booleans(1));
                    component.builder_call("translation_enabled", booleans(1));
                    component.builder_call("rotation_enabled", booleans(1));
                    for method_name in ["enable", "disable"] {
                        host_method(
                            component,
                            canonical,
                            method_name,
                            method(vec![], mms::ValueType::Null),
                        );
                    }
                    for method_name in ["set_translation_enabled", "set_rotation_enabled"] {
                        host_method(
                            component,
                            canonical,
                            method_name,
                            method(vec![mms::ValueType::Bool], mms::ValueType::Null),
                        );
                    }
                }
                "InputXR" => no_arg_constructors(component, &["on", "off"]),
                "XR" => no_arg_constructors(component, &["on", "off", "auto", "openxr"]),
                "InputXRGamepad" => {
                    component.constructor("new", no_args());
                    constructor_and_builder(component, "enabled", booleans(1));
                    constructor_and_builder(component, "hand", strings(1));
                    constructor_and_builder(
                        component,
                        "locomotion",
                        optional_component_signature([mms::ValueType::Bool], 0),
                    );
                    for method in ["speed", "deadzone"] {
                        constructor_and_builder(component, method, floats(1));
                    }
                    for method_name in ["enable", "disable"] {
                        host_method(
                            component,
                            canonical,
                            method_name,
                            method(vec![], mms::ValueType::Null),
                        );
                    }
                }
                "InputTransformMode" => {
                    no_arg_constructors(component, &["forward_y", "forward_z"]);
                    no_arg_builders(
                        component,
                        &[
                            "fps_rotation",
                            "roll_axis_y",
                            "roll_axis_z",
                            "rotation_disabled",
                        ],
                    );
                    component.builder_call("translation_basis", any(1));
                }
                "Pointer" => {
                    host_constructor(component, canonical, "disabled", no_args());
                    constructor_and_builder(component, "debug_enable", booleans(1));
                    for method in [
                        "min_grab_distance",
                        "click_max_screen_distance_px",
                        "click_max_ray_angle_deg",
                        "click_max_origin_distance",
                    ] {
                        constructor_and_builder(component, method, floats(1));
                    }
                }
                "TransformParent" => {
                    component.constructor("target", any(1));
                    component
                        .builder_call("target", any(1))
                        .builder_call("root", any(1));
                }
                "TransformApplyInverseLocal" => {
                    component.constructor("source", any(1));
                    component.builder_call("source", any(1));
                }
                "TransformCameraSpecific" => {
                    component.constructor("active_stereoscopic", no_args());
                }
                "TransformSampleAncestor" => {
                    component.constructor("skip", unsigned(1));
                }
                "QuatTemporalFilter" | "Vector3TemporalFilter" => {
                    constructor_and_builder(component, "smoothing_factor", floats(1));
                }
                "GLTF" => {
                    component.constructor("new", strings(1));
                    component.builder_call("with_visualized_transforms", booleans(1));
                }
                "Clock" => constructor_and_builder(component, "bpm", floats(1)),
                "Animation" => {
                    for name in ["playing", "paused", "looping"] {
                        host_constructor_and_builder(component, canonical, name, no_args());
                    }
                    host_constructor_and_builder(component, canonical, "length", floats(1));
                    host_constructor_and_builder(component, canonical, "scope", any(1));
                    for name in ["play", "loop_anim", "pause", "next", "previous"] {
                        host_method(component, canonical, name, no_args());
                    }
                }
                "TextureFiltering" => {
                    no_arg_constructors(component, &["linear", "nearest_magnification", "nearest"]);
                }
                "Texture" => {
                    for method in ["render_image", "with_uri", "uri", "from_png", "from_dds"] {
                        constructor_and_builder(component, method, strings(1));
                    }
                }
                "Transition" => {
                    no_arg_constructors(component, &["on", "off"]);
                    no_arg_builders(
                        component,
                        &[
                            "on",
                            "off",
                            "step",
                            "linear",
                            "ease_in_quad",
                            "ease_out_quad",
                            "ease_in_out_quad",
                            "ease_in_cubic",
                            "ease_out_cubic",
                            "ease_in_out_cubic",
                            "ease_in_out_sine",
                            "replace_same_target",
                            "allow_parallel",
                        ],
                    );
                    for method in ["enabled", "capture_from_current"] {
                        constructor_and_builder(component, method, booleans(1));
                    }
                    constructor_and_builder(component, "duration_beats", floats(1));
                }
                "UV" => constructor_and_builder(component, "uv", floats(2)),
                "Text" => {
                    component.positional(mms::ValueType::String);
                    component.builder_call("font_size", floats(1));
                    host_method(
                        component,
                        canonical,
                        "set_text",
                        method(vec![mms::ValueType::String], mms::ValueType::Null),
                    );
                }
                "TextInput" => {
                    component
                        .positional(mms::ValueType::String)
                        .builder_call("read_only", booleans(1));
                }
                "FitBounds" => {
                    no_arg_constructors_and_builders(
                        component,
                        &["renderable_only", "layout_aware", "to_container"],
                    );
                    constructor_and_builder(component, "to", floats(6));
                }
                "LayoutBounds" => {
                    for method in ["content_box", "padding_box"] {
                        constructor_and_builder(component, method, any(2));
                    }
                }
                "Background" => {
                    no_arg_constructors_and_builders(
                        component,
                        &["occlusion_and_lighting", "ray_casting"],
                    );
                }
                "StencilClip" => {
                    constructor_and_builder(
                        component,
                        "stencil_ref",
                        component_signature([mms::ValueType::U8]),
                    );
                }
                "XRHand" => {
                    component.constructor(
                        "new",
                        component_signature([
                            mms::ValueType::Bool,
                            mms::ValueType::String,
                            mms::ValueType::String,
                        ]),
                    );
                    component.builder_call("laser", no_args());
                }
                "JointRetargetBasis" => {
                    component.constructor("new", any(5));
                }
                "HumanoidBoneMap" => {
                    component.constructor("new", no_args());
                    component
                        .builder_call(
                            "slot",
                            component_signature([mms::ValueType::String, mms::ValueType::Any]),
                        )
                        .builder_call("absent", strings(1))
                        .builder_call("automap_disable", no_args());
                }
                "MorphTargetMap" => {
                    component.constructor("new", no_args());
                    component.builder_call(
                        "slot",
                        component_signature([mms::ValueType::String, mms::ValueType::String]),
                    );
                }
                "RestAttachment" => {
                    component.constructor("new", any(2));
                }
                "SpringCollider" => {
                    component
                        .constructor(
                            "sphere",
                            component_signature([mms::ValueType::Any, mms::ValueType::F32]),
                        )
                        .constructor(
                            "spheres",
                            component_signature([mms::ValueType::Any, mms::ValueType::F32]),
                        );
                }
                "SpringBone" => {
                    component
                        .constructor("new", strings(1))
                        .constructor("from_root", any(1))
                        .builder_call("center", any(1))
                        .builder_call("colliders", any(1))
                        .builder_call("enabled", booleans(1));
                    for method in [
                        "virtual_end_length_ratio",
                        "stiffness",
                        "drag_force",
                        "hit_radius",
                    ] {
                        component.builder_call(method, floats(1));
                    }
                    // The host accepts either (power, vec3) or
                    // (power, x, y, z); overloads remain a tracked seam.
                    component.builder_call("gravity", any(2));
                }
                "SpringJoint" => {
                    component.constructor("new", any(1));
                    for method in ["stiffness", "drag_force"] {
                        component.builder_call(method, floats(1));
                    }
                    component.builder_call("gravity", any(2));
                }
                "RendererStats" => {
                    for method in ["enabled", "emissive"] {
                        constructor_and_builder(component, method, booleans(1));
                    }
                    for method in ["update_interval_sec", "smoothing"] {
                        constructor_and_builder(component, method, floats(1));
                    }
                    constructor_and_builder(component, "color", any(1));
                    constructor_and_builder(component, "camera_target", strings(1));
                }
                "TextShadow" => {
                    for method in ["offset_xy", "offset", "rgba"] {
                        constructor_and_builder(component, method, any(1));
                    }
                    for method in ["z_offset", "scale"] {
                        constructor_and_builder(component, method, floats(1));
                    }
                }
                "AssetPayload" => {
                    component.constructor("new", strings(2));
                    for method in ["asset_key", "title"] {
                        constructor_and_builder(component, method, strings(1));
                    }
                }
                "Router" => {
                    constructor_and_builder(component, "target", strings(1));
                    constructor_and_builder(component, "ignore", any(1));
                    component
                        .property("target", mms::ValueType::String)
                        .property("ignore", mms::ValueType::Array);
                }
                "Selectable" | "Toggle" | "Serialize" => {
                    no_arg_constructors(component, &["on", "off"]);
                }
                "Selection" => {
                    no_arg_constructors(component, &["multiple", "optional"]);
                    no_arg_builders(component, &["optional"]);
                    component
                        .constructor("root", any(1))
                        .builder_call("root", any(1))
                        .property("root", mms::ValueType::Any);
                }
                "ObserverRouter" => {
                    for method in ["blacklist", "whitelist"] {
                        constructor_and_builder(component, method, any(1));
                    }
                }
                "Scrolling" => {
                    component.constructor("new", floats(2));
                }
                "HtmlElement" => {
                    no_arg_constructors(
                        component,
                        &[
                            "div", "span", "body", "header", "p", "section", "article", "footer",
                            "nav", "aside", "main", "h1", "h2", "h3", "h4", "h5", "h6",
                        ],
                    );
                }
                "LayoutRoot" => {
                    for method in ["width", "available_width", "height", "available_height"] {
                        component.builder_call(method, any(1));
                    }
                    component.builder_call("unit_scale", floats(1));
                }
                "Style" => {
                    for method in [
                        "display",
                        "box_sizing",
                        "flex_direction",
                        "justify_content",
                        "align_items",
                        "text_align",
                        "vertical_align",
                        "position",
                        "overflow",
                        "flex_wrap",
                        "word_wrap",
                    ] {
                        component.builder_call(method, strings(1));
                    }
                    for method in [
                        "width",
                        "height",
                        "padding",
                        "margin",
                        "font_size",
                        "top",
                        "right",
                        "bottom",
                        "left",
                    ] {
                        component.builder_call(method, any(1));
                    }
                    for method in ["padding_xy", "margin_xy"] {
                        component.builder_call(method, any(2));
                    }
                    for method in ["margin_top", "margin_right", "margin_bottom", "margin_left"] {
                        component.builder_call(method, any(1));
                    }
                    for method in [
                        "background_z",
                        "flex_grow",
                        "flex_shrink",
                        "gap",
                        "row_gap",
                        "column_gap",
                    ] {
                        component.builder_call(method, floats(1));
                    }
                    for method in ["background_color", "color"] {
                        component
                            .builder_call(method, any(1))
                            .property(method, mms::ValueType::Any);
                    }
                    component
                        .property("background_z", mms::ValueType::F32)
                        .builder_call("z_index", component_signature([mms::ValueType::I32]))
                        .builder_call("word_wrap_tokens", any(1));
                }
                "Grabbable" => {
                    no_arg_constructors(component, &["parent", "off", "on"]);
                }
                "Rider" => {
                    component
                        .constructor("anchor", any(1))
                        .builder_call("anchor", any(1))
                        .builder_call("movement_root", any(1))
                        .builder_call("input", any(1))
                        .builder_call("enabled", booleans(1));
                }
                "Mountable" => {
                    component
                        .constructor("entry_zone", any(1))
                        .builder_call("entry_zone", any(1))
                        .builder_call("mount_anchor", any(1))
                        .builder_call("dismount_anchor", any(1))
                        .builder_call("on_grip", no_args())
                        .builder_call("enabled", booleans(1));
                }
                "Draggable" => {
                    no_arg_constructors(component, &["parent", "off", "on"]);
                    component
                        .constructor("target", any(1))
                        .constructor("plane", any(1))
                        .builder_call("target", any(1))
                        .builder_call("plane", any(1));
                }
                "Slider" => {
                    constructor_and_builder(component, "range", floats(2));
                    constructor_and_builder(component, "step", floats(1));
                    constructor_and_builder(component, "value", floats(1));
                    constructor_and_builder(component, "width", floats(1));
                    constructor_and_builder(component, "disabled", booleans(1));
                    constructor_and_builder(component, "track", any(1));
                    constructor_and_builder(component, "thumb", any(1));
                    for (name, signature) in [
                        ("value", method(vec![], mms::ValueType::F32)),
                        (
                            "set_value",
                            method(vec![mms::ValueType::F32], mms::ValueType::Null),
                        ),
                        (
                            "sync_value",
                            method(vec![mms::ValueType::F32], mms::ValueType::Null),
                        ),
                        ("track_mount", method(vec![], mms::ValueType::Component)),
                        ("thumb_mount", method(vec![], mms::ValueType::Component)),
                    ] {
                        host_method(component, canonical, name, signature);
                    }
                }
                "Raycastable" => {
                    for constructor in ["disabled", "drag_only", "click_only", "enabled"] {
                        host_constructor(component, canonical, constructor, no_args());
                    }
                    for method in ["pointer_events", "drag_continuation", "drag_mapping"] {
                        component.builder_call(method, strings(1));
                    }
                    component.builder_call(
                        "interaction_priority",
                        component_signature([mms::ValueType::U8]),
                    );
                }
                "PoseCapture" => {
                    for method in ["with_label", "label", "with_asset_name", "asset_name"] {
                        constructor_and_builder(component, method, strings(1));
                    }
                }
                "PoseCapturePose" => {
                    component.constructor("new", strings(1)).builder_call(
                        "joint",
                        component_signature([
                            mms::ValueType::String,
                            mms::ValueType::Any,
                            mms::ValueType::Any,
                            mms::ValueType::Any,
                        ]),
                    );
                }
                "Keyframe" => {
                    host_constructor(component, canonical, "at", floats(1));
                }
                "NormalVis" => {
                    component.constructor("thickness", floats(1));
                }
                "TransparentCutout" => {
                    component.constructor("disabled", no_args());
                }
                "LightQuantization" => {
                    component.constructor("steps", floats(1));
                }
                "AnimeShading" | "Shading" => {
                    if canonical == "Shading" {
                        no_arg_constructors(component, &["anime", "toon"]);
                        for (getter, setter) in [
                            ("get_shade_strength", "set_shade_strength"),
                            ("get_shade_threshold", "set_shade_threshold"),
                            ("get_lit_threshold", "set_lit_threshold"),
                            ("get_rim_strength", "set_rim_strength"),
                            ("get_rim_power", "set_rim_power"),
                        ] {
                            host_method(
                                component,
                                canonical,
                                getter,
                                method(vec![], mms::ValueType::F32),
                            );
                            host_method(
                                component,
                                canonical,
                                setter,
                                method(vec![mms::ValueType::F32], mms::ValueType::Null),
                            );
                        }
                    }
                    for method in [
                        "shade_color",
                        "shade_strength",
                        "shade_threshold",
                        "lit_threshold",
                        "rim_color",
                        "rim_strength",
                        "rim_power",
                    ] {
                        if canonical == "Shading" {
                            component.builder_call(method, any(1));
                        } else {
                            constructor_and_builder(component, method, any(1));
                        }
                    }
                }
                "ToonOutline" => {
                    constructor_and_builder(component, "width", floats(1));
                    constructor_and_builder(component, "color", any(1));
                }
                "Bounds" => {
                    component.constructor("aabb", any(2));
                }
                "Mesh" => {
                    component.constructor("new", strings(1));
                }
                "Mirror" => {
                    component.constructor("quality", component_signature([mms::ValueType::I32]));
                }
                "GestureCoordType" => {
                    no_arg_constructors(component, &["screen_space_1d_slider", "world_plane"]);
                }
                "CollisionShape" => {
                    component
                        .constructor("cube", any(1))
                        .constructor("sphere", floats(1))
                        .constructor("capsule_y", floats(2));
                }
                "Zone" => {
                    component
                        .constructor("cube", any(1))
                        .constructor("sphere", floats(1))
                        .constructor("capsule_y", floats(2))
                        .builder_call("at", any(1))
                        .builder_call("role", strings(1))
                        .builder_call("enabled", booleans(1));
                }
                "RaycastableShape" => {
                    no_arg_constructors(
                        component,
                        &[
                            "aabb",
                            "cone",
                            "ring_2d",
                            "quad_2d",
                            "triangle_2d",
                            "tetrahedron",
                            "box",
                        ],
                    );
                }
                "Collision" => {
                    no_arg_constructors(component, &["static", "kinematic", "rigged"]);
                }
                "Gravity" => {
                    constructor_and_builder(component, "enabled", booleans(1));
                    constructor_and_builder(component, "coefficient", floats(1));
                }
                "SkinnedMesh" => {
                    component.constructor("new", unsigned(1));
                }
                "QuatYawFollow" => {
                    component
                        .constructor("new", floats(2))
                        .builder_call("initial_yaw", floats(1))
                        .builder_call("forward_plus_z", no_args());
                }
                "SignalRouteUpward" => {
                    component.constructor("new", strings(2));
                }
                "AvatarBodyYaw" => {
                    for method in ["threshold", "rate"] {
                        constructor_and_builder(component, method, floats(1));
                    }
                    no_arg_constructors_and_builders(component, &["forward_plus_z"]);
                }
                "Raycast" => {
                    no_arg_constructors(component, &["continuous", "event_driven"]);
                    component.builder_call("min_distance", floats(1));
                    component.builder_call("max_distance", floats(1));
                }
                "AudioOutput" => {
                    component.constructor("off", no_args());
                }
                "AudioInput" => {
                    no_arg_constructors(component, &["default"]);
                    component
                        .constructor("device_number", unsigned(1))
                        .builder_call("enabled", booleans(1));
                    host_method(
                        component,
                        canonical,
                        "select_device_number",
                        method(vec![mms::ValueType::U32], mms::ValueType::Null),
                    );
                }
                "Amplitude" => {
                    component
                        .constructor("rolling_window", floats(1))
                        .builder_call("from", any(1))
                        .builder_call("enabled", booleans(1));
                }
                "AudioGain" => {
                    // Legacy construction uses the first argument regardless
                    // of constructor spelling; `new` is the canonical form.
                    component.constructor("new", floats(1));
                }
                "AudioBandPassFilter" | "AudioLimiter" => {
                    component.constructor("new", floats(3));
                }
                "AudioOscillator" => {
                    no_arg_constructors(
                        component,
                        &[
                            "sin", "triangle", "square", "square_3", "saw", "noise", "drum",
                        ],
                    );
                    component
                        .builder_call("frequency", floats(1))
                        .builder_call("amplitude", floats(1))
                        .builder_call("enabled", booleans(1));
                }
                "AudioClip" => {
                    for method in [
                        "new", "wav", "opus", "ogg", "mp3", "flac", "one_shot", "latched",
                    ] {
                        component.constructor(method, strings(1));
                    }
                    no_arg_builders(component, &["one_shot", "latched", "retrigger"]);
                }
                "IKChain" => {
                    component
                        .constructor("aim_constraint", any(3))
                        .constructor("two_bone_ik", any(2))
                        .constructor("fabrik", any(3))
                        .builder_call("weight", floats(1))
                        .builder_call("target", any(1))
                        .builder_call("end_effector", any(1));
                }
                "TransformGizmo" => {
                    constructor_and_builder(component, "scale", floats(1));
                }
                "TransformGizmoTranslate" | "TransformGizmoRotate" | "TransformGizmoScale" => {
                    no_arg_constructors(component, &["x", "y", "z"]);
                }
                "TransformGizmoTranslatePlane" => {
                    no_arg_constructors(component, &["xy", "yz", "xz"]);
                }
                "CollisionResponse" => {
                    no_arg_constructors(component, &["push", "slide"]);
                    component
                        .builder_call("enabled", booleans(1))
                        .builder_call("max_iterations", unsigned(1))
                        .builder_call("movement_target", any(1));
                    for method in [
                        "push_out_epsilon",
                        "push_strength",
                        "friction",
                        "friction_y",
                        "max_speed",
                    ] {
                        component.builder_call(method, floats(1));
                    }
                }
                "Editor" => {
                    no_arg_constructors_and_builders(component, &["active"]);
                    for method in [
                        "interaction_mode",
                        "translation_space",
                        "rotation_space",
                        "asset_dir",
                    ] {
                        constructor_and_builder(component, method, strings(1));
                    }
                    for method in ["panels", "serialize_editor_panels"] {
                        constructor_and_builder(component, method, booleans(1));
                    }
                }
                "EditorUI" => {
                    constructor_and_builder(component, "panels", any(1));
                }
                "AvatarControl" => {
                    for method in ["left_arm_pole_direction", "right_arm_pole_direction"] {
                        constructor_and_builder(component, method, any(1));
                    }
                    for method in [
                        "initial_yaw",
                        "capsule_radius",
                        "body_yaw_threshold",
                        "body_yaw_rate",
                        "hand_rotation_smoothing",
                        "avatar_height",
                        "eye_height_from_head_bone",
                        "head_ik_eye_height",
                        "mouth_open_rms_floor",
                        "mouth_open_rms_ceiling",
                        "mouth_open_smoothing",
                    ] {
                        constructor_and_builder(component, method, floats(1));
                    }
                    constructor_and_builder(component, "head_motion_gaze_policy", strings(1));
                    constructor_and_builder(component, "mouth_open_from_amplitude", any(1));
                    no_arg_constructors_and_builders(
                        component,
                        &[
                            "forward_plus_z",
                            "ik_debug",
                            "collision_disabled",
                            "neck_pin_disabled",
                        ],
                    );
                    constructor_and_builder(component, "neck_pin_enabled", booleans(1));
                }
                _ => {}
            }
        });
    }

    // Signals are event kinds whose optional component receiver is a routing
    // scope, not an owning component type.
    for name in [
        "FrameTick",
        "KeyDown",
        "KeyUp",
        "KeyPress",
        "GLTFInitialized",
        "Click",
        "ToggleChanged",
        "SliderChanged",
        "SliderCommitted",
        "DataEvent",
        "CollisionStarted",
        "CollisionEnded",
        "DragStart",
        "DragMove",
        "DragEnd",
        "GrabStart",
        "GrabEnd",
        "MountStarted",
        "MountEnded",
        "ParentChanged",
        "RayIntersected",
        "Scrolling",
        "TextInputChanged",
        "TextInputFocusChanged",
        "SelectionAdded",
        "SelectionRemoved",
        "SelectionChanged",
        "SelectionCleared",
        "XrButtonDown",
        "XrButtonUp",
        "XrButtonChanged",
        "XrAxisChanged",
        "HttpRequest",
        "HttpResponse",
        "HttpError",
        "XrEyeTrackingUpdated",
        "XrEyeTrackingHtcUpdated",
    ] {
        builder.signal(name, Vec::new(), MittensBinding::Signal { name });
    }

    builder.namespace("mittens", |namespace| {
        namespace.api(
            "smoke",
            mms::ValueSignature::new(Vec::new(), mms::ValueType::Null),
            MittensBinding::Api(MittensApi::Smoke),
        );
    });

    builder.namespace("File", |namespace| {
        namespace.api(
            "read_text",
            mms::ValueSignature::new(vec![mms::ValueType::String], mms::ValueType::String),
            MittensBinding::Api(MittensApi::FileReadText),
        );
    });

    builder.namespace("JSON", |namespace| {
        namespace.api(
            "parse",
            mms::ValueSignature::new(vec![mms::ValueType::String], mms::ValueType::Any),
            MittensBinding::Api(MittensApi::JsonParse),
        );
        namespace.api(
            "stringify",
            mms::ValueSignature::new(vec![mms::ValueType::Any], mms::ValueType::String),
            MittensBinding::Api(MittensApi::JsonStringify),
        );
    });

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

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_spec_contains_registry_names_and_supported_shortforms() {
        let configured = build_mittens_runtime().unwrap();
        let spec = configured.runtime().spec();

        for name in crate::scripting::component_registry::SUPPORTED_COMPONENT_NAMES {
            if *name == "MusicNote" {
                continue;
            }
            assert!(spec.component(name).is_some(), "missing component {name}");
            configured
                .runtime()
                .materialize_component(&format!("{name} {{}}"))
                .unwrap_or_else(|error| panic!("component {name} is not parseable: {error}"));
            let declaration = spec.component(name).unwrap();
            let operation_id = declaration
                .default_constructor_operation_id()
                .unwrap_or_else(|| panic!("component {name} has no host default constructor"));
            assert_eq!(
                configured.bindings().get(operation_id),
                Some(&MittensBinding::ComponentConstructor {
                    component: name,
                    name: None,
                }),
                "component {name} has the wrong host default constructor binding"
            );
        }
        for shortform in mms::COMPONENT_SHORTFORMS {
            if shortform.full != "MusicNote"
                && crate::scripting::component_registry::SUPPORTED_COMPONENT_NAMES
                    .contains(&shortform.full)
            {
                assert!(
                    spec.component(shortform.short).is_some(),
                    "missing alias {} for {}",
                    shortform.short,
                    shortform.full
                );
                configured
                    .runtime()
                    .materialize_component(&format!("{} {{}}", shortform.short))
                    .unwrap_or_else(|error| {
                        panic!("alias {} is not parseable: {error}", shortform.short)
                    });
            }
        }
        let smoke_id = spec.api(Some("mittens"), "smoke").unwrap().operation_id();
        assert_eq!(
            configured.bindings().get(smoke_id),
            Some(&MittensBinding::Api(MittensApi::Smoke))
        );
        for (name, binding) in [
            ("parse", MittensApi::JsonParse),
            ("stringify", MittensApi::JsonStringify),
        ] {
            let id = spec.api(Some("JSON"), name).unwrap().operation_id();
            assert_eq!(
                configured.bindings().get(id),
                Some(&MittensBinding::Api(binding))
            );
        }
        let file_read_text = spec.api(Some("File"), "read_text").unwrap().operation_id();
        assert_eq!(
            configured.bindings().get(file_read_text),
            Some(&MittensBinding::Api(MittensApi::FileReadText))
        );
        for (component, binding) in [
            ("AudioInput", MittensApi::AudioInputDevices),
            ("AudioOutput", MittensApi::AudioOutputDevices),
        ] {
            let id = spec.api(Some(component), "devices").unwrap().operation_id();
            assert_eq!(
                configured.bindings().get(id),
                Some(&MittensBinding::Api(binding))
            );
        }
        assert!(spec.api(Some("Audio"), "input_devices").is_none());
        assert!(spec.component("DefinitelyNotAMittensComponent").is_none());
        assert!(
            configured
                .runtime()
                .materialize_component("DefinitelyNotAMittensComponent {}")
                .is_err()
        );
        assert!(
            configured
                .runtime()
                .materialize_component("RendererSettings.window_size(960, 720) {}")
                .is_ok()
        );
        assert!(
            configured
                .runtime()
                .materialize_component("RendererSettings.transmission_depth_compare(false) {}")
                .is_ok()
        );
        configured
            .runtime()
            .materialize_component(
                "RenderGraph { EmissivePass { BlurPass { radius_ndc(0.05) half_res(true) } } Bloom { intensity(1.2) } }"
            )
            .unwrap();
        configured
            .runtime()
            .materialize_component(
                "Transform { Camera3D { enabled(true) fov(55.0) near(0.05) far(250.0) } DirectionalLight { intensity(1.5) color(1.0, 0.92, 0.82) } }",
            )
            .unwrap();
        for valid in [
            "Renderable.cone() {}",
            "Renderable.cone(24) {}",
            "Renderable.heart() {}",
            "Renderable.heart(48) {}",
            "Renderable.star() {}",
            "Renderable.polygon(\"ui/test/triangle/v1\", [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]) {}",
            "Renderable.partial_annulus_2d() {}",
            "Renderable.wireframe_sphere() {}",
        ] {
            configured
                .runtime()
                .materialize_component(valid)
                .unwrap_or_else(|error| panic!("optional constructor rejected {valid}: {error}"));
        }
        for invalid in ["720.5", "-1", "4294967296"] {
            let error = configured
                .runtime()
                .materialize_component(&format!(
                    "RendererSettings.window_size(960, {invalid}) {{}}"
                ))
                .unwrap_err()
                .to_string();
            assert!(error.contains("expected u32"), "{error}");
        }
        for invalid in [
            "Bloom.intensity(\"bright\") {}",
            "BlurPass.half_res(1) {}",
            "RenderGraph { enabled(1) }",
            "Camera3D { fov(\"wide\") }",
            "DirectionalLight { color(1.0, 0.5) }",
            "Renderable.cone(12, 24) {}",
        ] {
            assert!(
                configured.runtime().materialize_component(invalid).is_err(),
                "invalid post-processing declaration was accepted: {invalid}"
            );
        }
    }

    #[test]
    fn non_marker_components_have_declarative_schema() {
        const MARKER_OR_DYNAMIC_COMPONENTS: &[&str] = &[
            "BackgroundColor",
            "Data",
            "EmissivePass",
            "InspectLayout",
            "Option",
            "Overlay",
            "PoseCaptureLibrary",
            "SecondaryMotion",
            "SpringColliders",
            "TransformDrop",
            "TransformForkTRS",
            "TransformMapRotation",
            "TransformMapScale",
            "TransformMapTranslation",
            "TransformMergeTRS",
            "Unlit",
        ];
        let configured = build_mittens_runtime().unwrap();

        let mut missing = Vec::new();
        for declaration in configured.spec().components() {
            if declaration.name() == "MusicNote"
                || MARKER_OR_DYNAMIC_COMPONENTS.contains(&declaration.name())
            {
                continue;
            }
            let component_properties = declaration
                .properties()
                .filter(|property| !matches!(property.name(), "name" | "id" | "class"))
                .count();
            let component_methods = declaration
                .methods()
                .filter(|method| {
                    !matches!(
                        method.name(),
                        "attach"
                            | "attach_clone"
                            | "detach"
                            | "remove_child"
                            | "remove_subtree"
                            | "set_color"
                    )
                })
                .count();
            let has_schema = declaration.constructors().len() > 0
                || declaration.builder_calls().len() > 0
                || declaration.positionals().len() > 0
                || component_properties > 0
                || component_methods > 0;
            if !has_schema {
                missing.push(declaration.name());
            }
        }
        assert!(
            missing.is_empty(),
            "components silently fell back to names-only registration: {missing:?}"
        );
    }

    #[test]
    fn direct_component_operations_resolve_to_matching_bindings() {
        let configured = build_mittens_runtime().unwrap();
        let tree = configured
            .runtime()
            .materialize_component(
                "T.position(1.0, 2.0, 3.0).scale(2.0, 2.0, 2.0) { name = \"root\" }",
            )
            .unwrap();

        assert_eq!(
            configured
                .bindings()
                .get(tree.constructor.operation_id.unwrap()),
            Some(&MittensBinding::ComponentConstructor {
                component: "Transform",
                name: Some("position"),
            })
        );
        assert_eq!(tree.constructor.name.as_deref(), Some("position"));
        let call = &tree.initializer_calls[0];
        assert_eq!(call.name, "scale");
        assert_eq!(
            configured.bindings().get(call.operation_id.unwrap()),
            Some(&MittensBinding::ComponentInitializer {
                component: "Transform",
                name: "scale",
                kind: ComponentInitializerKind::Call,
            })
        );
        let property = &tree.properties[0];
        assert_eq!(property.name, "name");
        assert_eq!(
            configured.bindings().get(property.operation_id.unwrap()),
            Some(&MittensBinding::ComponentInitializer {
                component: "Transform",
                name: "name",
                kind: ComponentInitializerKind::Property,
            })
        );

        for name in ["translation", "attach"] {
            let method = configured
                .spec()
                .component("Transform")
                .unwrap()
                .method(name)
                .unwrap();
            assert_eq!(
                configured.bindings().get(method.operation_id().unwrap()),
                Some(&MittensBinding::ComponentMethod {
                    component: "Transform",
                    name,
                })
            );
        }
    }
}
