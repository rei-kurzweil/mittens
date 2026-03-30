#version 450

// Skinned toon vertex shader.
//
// Uses per-vertex JOINTS_0/WEIGHTS_0 (separate vertex buffer) and a shared bones palette SSBO.

layout(location = 0) in vec3 in_pos;
layout(location = 5) in vec2 in_uv;
layout(location = 8) in vec3 in_normal;

// Skinning attributes (glTF: JOINTS_0 / WEIGHTS_0).
layout(location = 12) in uvec4 in_joints0;
layout(location = 13) in vec4 in_weights0;

// Per-instance model matrix.
layout(location = 1) in vec4 i_model_c0;
layout(location = 2) in vec4 i_model_c1;
layout(location = 3) in vec4 i_model_c2;
layout(location = 4) in vec4 i_model_c3;
layout(location = 6) in vec4 i_color;
layout(location = 7) in float i_emissive;
layout(location = 9) in float i_opacity;
layout(location = 10) in uint i_bones_base;
layout(location = 11) in uint i_bones_count;

// Set 0: global camera.
// Unified camera path: clip = proj * view * world.
layout(set = 0, binding = 0) uniform CameraUBO {
    mat4 view;
    mat4 proj;
    mat3 camera2d;
    vec2 viewport;
    vec2 _pad0;
    vec3 ambient_light;
    float _pad1;
} ubo;

// Set 2: rig data.
// Layout is shared with `PipelineDescriptorSetLayouts::rig`.
layout(set = 2, binding = 1) readonly buffer BonesSSBO {
    mat4 bones[];
} bones_ssbo;

layout(location = 0) out vec3 v_world_pos;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec2 v_uv;
layout(location = 3) out vec4 v_color;
layout(location = 4) flat out float v_emissive;

void main() {
    mat4 model = mat4(i_model_c0, i_model_c1, i_model_c2, i_model_c3);

    // Compute mesh-local skinned position/normal.
    // Bones are uploaded as mesh-local skin matrices, and instances address them via i_bones_base.
    mat4 skin = mat4(1.0);
    float wsum = in_weights0.x + in_weights0.y + in_weights0.z + in_weights0.w;
    if (i_bones_count > 0u && wsum > 0.0) {
        uint b0 = i_bones_base + in_joints0.x;
        uint b1 = i_bones_base + in_joints0.y;
        uint b2 = i_bones_base + in_joints0.z;
        uint b3 = i_bones_base + in_joints0.w;
        skin =
            bones_ssbo.bones[b0] * in_weights0.x +
            bones_ssbo.bones[b1] * in_weights0.y +
            bones_ssbo.bones[b2] * in_weights0.z +
            bones_ssbo.bones[b3] * in_weights0.w;
    }

    vec4 skinned_local = skin * vec4(in_pos, 1.0);
    vec3 skinned_normal = normalize(mat3(skin) * in_normal);

    vec4 world = model * skinned_local;

    // World-space outputs (lighting expects world-space lights).
    v_world_pos = world.xyz;

    // Transform into world space.
    v_normal = normalize(mat3(model) * skinned_normal);
    v_uv = in_uv;
    v_color = i_color;
    v_color.a *= i_opacity;
    v_emissive = i_emissive;

    gl_Position = ubo.proj * ubo.view * world;
}
