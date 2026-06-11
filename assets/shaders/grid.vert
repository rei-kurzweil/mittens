#version 450

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

layout(set = 0, binding = 0) uniform CameraUBO {
    mat4 view;
    mat4 proj;
    mat3 camera2d;
    vec2 viewport;
    vec2 _pad0;
    vec3 ambient_light;
    float _pad1;
} ubo;

layout(location = 0) out vec3 v_world_pos;
layout(location = 1) out vec2 v_uv;
layout(location = 2) out vec4 v_color;

void main() {
    mat4 model = mat4(i_model_c0, i_model_c1, i_model_c2, i_model_c3);
    vec4 world = model * vec4(in_pos, 1.0);

    v_world_pos = world.xyz;
    v_uv = in_uv;
    v_color = i_color;
    v_color.a *= i_opacity;

    gl_Position = ubo.proj * ubo.view * world;
}
