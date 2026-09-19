#version 450

layout(location = 1) in vec4 i_model_c0;
layout(location = 2) in vec4 i_model_c1;
layout(location = 3) in vec4 i_model_c2;
layout(location = 4) in vec4 i_model_c3;
layout(location = 10) in uint i_deformed_base;
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

struct DeformedVertex {
    vec3 position;
    uint packed_normal;
};
layout(std430, set = 2, binding = 1) readonly buffer DeformedVertices {
    DeformedVertex vertices[];
} deformed_vertices;

layout(location = 0) flat out vec4 v_outline_color;

float sign_not_zero(float value) {
    return value < 0.0 ? -1.0 : 1.0;
}

vec3 decode_oct_normal(uint packed) {
    if (packed == 0x80008000u) {
        return vec3(0.0);
    }
    vec2 oct = vec2(
        float(int(packed << 16) >> 16),
        float(int(packed) >> 16)
    ) / 32767.0;
    vec3 normal = vec3(oct, 1.0 - abs(oct.x) - abs(oct.y));
    if (normal.z < 0.0) {
        float old_x = normal.x;
        normal.x = (1.0 - abs(normal.y)) * sign_not_zero(old_x);
        normal.y = (1.0 - abs(old_x)) * sign_not_zero(normal.y);
    }
    return normalize(normal);
}

void main() {
    mat4 model = mat4(i_model_c0, i_model_c1, i_model_c2, i_model_c3);
    DeformedVertex deformed = deformed_vertices.vertices[i_deformed_base + gl_VertexIndex];
    vec4 world = model * vec4(deformed.position, 1.0);
    // The compute output already contains the post-morph/post-skin normal.
    // Match the foreground transform and avoid doing a matrix inverse per vertex.
    vec3 world_normal = normalize(mat3(model) * decode_oct_normal(deformed.packed_normal));
    world.xyz += world_normal * i_outline_width;
    v_outline_color = i_outline_color;
    gl_Position = ubo.proj * ubo.view * world;
}
