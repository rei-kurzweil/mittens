#version 450

layout(location = 0) in vec3 v_world_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;

layout(location = 0) out vec4 f_color;

// Compile-time debug selector:
// 0 = normal lighting
// 1 = show SSBO light0.pos_intensity.rgb
// 2 = show SSBO light0.color_distance.rgb
// 3 = show interpolated normal (remapped)
// 4 = show light_count as grayscale
const uint LC_DEBUG_OUTPUT = 0u;

struct PointLight {
    vec4 pos_intensity;  // xyz position (world), w intensity
    vec4 color_distance; // rgb color, w distance
};

layout(set = 0, binding = 1, std430) readonly buffer LightsSSBO {
    uint count;
    // IMPORTANT: keep this header exactly 16 bytes to match the Rust side.
    // Using `uvec3` here changes alignment/offset rules in std430 and will shift `lights`.
    uint _pad0;
    uint _pad1;
    uint _pad2;
    PointLight lights[64];
} g_lights;

// Set 1: material params (no textures yet; those can be added later).
layout(set = 1, binding = 0) uniform MaterialUBO {
    vec4 base_color;
    float quant_steps;
    uint emissive;
    uvec2 _pad0;
} mat;

layout(set = 1, binding = 1) uniform sampler2D base_tex;

float quantize(float x, float steps) {
    float s = max(1.0, steps);
    return floor(clamp(x, 0.0, 1.0) * s) / s;
}

void main() {
    vec4 tex_rgba = texture(base_tex, v_uv);
    vec4 base_rgba = tex_rgba * v_color;
    vec3 base = base_rgba.rgb;

    if (mat.emissive != 0u) {
        f_color = vec4(base, base_rgba.a);
        return;
    }

    uint light_count = min(g_lights.count, 64u);
    if (light_count == 0u) {
        // No lights: show black so it's obvious.
        f_color = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    vec3 light_pos = g_lights.lights[0].pos_intensity.xyz;
    vec3 light_color = g_lights.lights[0].color_distance.rgb;

    // get dot product of normal and light direction
    vec3 N = normalize(v_normal);
    vec3 L = normalize(light_pos - v_world_pos);
    float NdotL = max(dot(N, L), 0.0);


    vec3 out_rgb = base * NdotL * light_color;
    f_color = vec4(out_rgb, base_rgba.a);
}
