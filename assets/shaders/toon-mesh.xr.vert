#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 in_pos;
layout(location = 5) in vec2 in_uv;
layout(location = 8) in vec3 in_normal;

layout(location = 1) in vec4 i_model_c0;
layout(location = 2) in vec4 i_model_c1;
layout(location = 3) in vec4 i_model_c2;
layout(location = 4) in vec4 i_model_c3;
layout(location = 6) in vec4 i_color;
layout(location = 7) in float i_emissive;
layout(location = 9) in float i_opacity;

// Set 0: global camera (XR multiview variant).
// view[i] / proj[i] are indexed by gl_ViewIndex (one entry per eye).
layout(set = 0, binding = 0) uniform CameraXrUBO {
    mat4 view[2];
    mat4 proj[2];
    mat3 camera2d;
    vec2 viewport;
    vec2 _pad0;
    vec3 ambient_light;
    float _pad1;
} ubo;

layout(location = 0) out vec3 v_world_pos;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec2 v_uv;
layout(location = 3) out vec4 v_color;
layout(location = 4) flat out float v_emissive;

void main() {
    mat4 model = mat4(i_model_c0, i_model_c1, i_model_c2, i_model_c3);

    vec4 world = model * vec4(in_pos, 1.0);

    v_world_pos = world.xyz;
    v_normal = normalize(mat3(model) * in_normal);
    v_uv = in_uv;
    v_color = i_color;
    v_color.a *= i_opacity;
    v_emissive = i_emissive;

    gl_Position = ubo.proj[gl_ViewIndex] * ubo.view[gl_ViewIndex] * world;
}
