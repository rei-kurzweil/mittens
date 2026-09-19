#version 450

layout(location = 0) in vec3 in_pos;
layout(location = 8) in vec3 in_normal;

layout(location = 1) in vec4 i_model_c0;
layout(location = 2) in vec4 i_model_c1;
layout(location = 3) in vec4 i_model_c2;
layout(location = 4) in vec4 i_model_c3;
layout(location = 14) in float i_outline_width;
layout(location = 15) in vec4 i_outline_color;

layout(set = 0, binding = 0) uniform CameraUBO {
    mat4 view;
    mat4 proj;
    mat3 camera2d;
    vec2 viewport;
    vec2 _pad0;
    vec3 ambient_light;
    uint renderer_flags;
} ubo;

layout(location = 0) flat out vec4 v_outline_color;

void main() {
    mat4 model = mat4(i_model_c0, i_model_c1, i_model_c2, i_model_c3);
    vec4 world = model * vec4(in_pos, 1.0);
    // Match the foreground path for the first slice. An inverse-transpose
    // instance matrix can be added later if non-uniform scale needs exact normals.
    vec3 world_normal = normalize(mat3(model) * in_normal);
    world.xyz += world_normal * i_outline_width;
    v_outline_color = i_outline_color;
    gl_Position = ubo.proj * ubo.view * world;
}
