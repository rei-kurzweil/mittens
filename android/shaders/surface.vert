#version 450
layout(location = 0) in vec3 position;
layout(location = 5) in vec2 uv;
layout(location = 8) in vec3 normal;
layout(location = 1) in mat4 model;
layout(location = 6) in vec4 color;
layout(set = 0, binding = 0, std140) uniform Camera {
    mat4 view; mat4 proj; mat3 camera2d; vec2 viewport; vec2 pad;
    vec3 ambient; uint renderer_flags;
} camera;
layout(location = 0) out vec3 v_world_pos;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec2 v_uv;
layout(location = 3) out vec4 v_color;
layout(location = 4) flat out float v_emissive;
layout(location = 5) flat out vec4 v_transmission;
layout(location = 6) flat out float v_roughness;
void main() {
    vec4 world = model * vec4(position, 1.0);
    gl_Position = camera.proj * camera.view * world;
    // Plain sky backgrounds follow rotation and occupy the far depth plane.
    if ((camera.renderer_flags & 256u) != 0u) { gl_Position.z = gl_Position.w; }
    v_world_pos = world.xyz;
    v_normal = normalize(transpose(inverse(mat3(model))) * normal);
    v_uv = uv; v_color = color;
    v_emissive = 0.0; v_transmission = vec4(0.0); v_roughness = 0.0;
}
