#version 450

// Position-only vertex format.
// Matches the current mesh upload path (via `MeshUploader`), which packs `CpuVertex.pos` as 3x f32.
layout(location = 0) in vec3 in_pos;

// Per-instance model matrix, provided as 4 vec4 vertex attributes.
// These should come from a vertex input binding configured with
// VK_VERTEX_INPUT_RATE_INSTANCE.
layout(location = 1) in vec4 i_model_c0;
layout(location = 2) in vec4 i_model_c1;
layout(location = 3) in vec4 i_model_c2;
layout(location = 4) in vec4 i_model_c3;

// Uniform buffer: camera data comes from set=0,binding=0.
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

void main() {
    mat4 model = mat4(i_model_c0, i_model_c1, i_model_c2, i_model_c3);

    vec4 world = model * vec4(in_pos, 1.0);

    gl_Position = ubo.proj * ubo.view * world;
}
