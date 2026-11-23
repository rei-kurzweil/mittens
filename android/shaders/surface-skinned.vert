#version 450
// surface.vert for compute-deformed meshes: position and normal come from the
// persistent deformation output (written by mesh-deformation.comp) instead of
// the static vertex buffer. UVs still come from the mesh's vertex buffer.
layout(location = 5) in vec2 uv;
layout(location = 1) in mat4 model;
layout(location = 6) in vec4 color;
layout(location = 7) in uint deformed_base;
layout(set = 0, binding = 0, std140) uniform Camera {
    mat4 view; mat4 proj; mat3 camera2d; vec2 viewport; vec2 pad;
    vec3 ambient; uint renderer_flags;
} camera;
struct DeformedVertex { vec3 position; uint packed_normal; };
layout(set = 0, binding = 2, std430) readonly buffer DeformedVertices {
    DeformedVertex vertices[];
} deformed;
layout(location = 0) out vec3 v_world_pos;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec2 v_uv;
layout(location = 3) out vec4 v_color;
layout(location = 4) flat out float v_emissive;
layout(location = 5) flat out vec4 v_transmission;
layout(location = 6) flat out float v_roughness;
float sign_not_zero(float value) { return value < 0.0 ? -1.0 : 1.0; }
// Octahedral SNORM16x2, matching engine::graphics::deformation::oct_encode_normal.
vec3 decode_oct_normal(uint packed) {
    if (packed == 0x80008000u) { return vec3(0.0); }
    vec2 oct = vec2(float(int(packed << 16) >> 16), float(int(packed) >> 16)) / 32767.0;
    vec3 n = vec3(oct, 1.0 - abs(oct.x) - abs(oct.y));
    if (n.z < 0.0) {
        float old_x = n.x;
        n.x = (1.0 - abs(n.y)) * sign_not_zero(old_x);
        n.y = (1.0 - abs(old_x)) * sign_not_zero(n.y);
    }
    return normalize(n);
}
void main() {
    DeformedVertex vertex = deformed.vertices[deformed_base + uint(gl_VertexIndex)];
    vec4 world = model * vec4(vertex.position, 1.0);
    gl_Position = camera.proj * camera.view * world;
    v_world_pos = world.xyz;
    v_normal = normalize(transpose(inverse(mat3(model))) * decode_oct_normal(vertex.packed_normal));
    v_uv = uv; v_color = color;
    v_emissive = 0.0; v_transmission = vec4(0.0); v_roughness = 0.0;
}
