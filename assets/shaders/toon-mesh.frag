#version 450

layout(location = 0) in vec3 v_world_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 4) flat in uint v_emissive;

layout(location = 0) out vec4 f_color;


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

    // IMPORTANT: with depth testing enabled, fully-transparent texels must not write depth.
    // Otherwise later geometry gets depth-rejected and you see the clear color (often black)
    // in places that should be transparent.
    if (base_rgba.a <= 0.001) {
        discard;
    }

    if (mat.emissive != 0u || v_emissive != 0u) {
        f_color = vec4(base, base_rgba.a);
        return;
    }

    uint light_count = min(g_lights.count, 64u);
    if (light_count == 0u) {
        // No lights: show black so it's obvious.
        f_color = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    vec3 N = normalize(v_normal);
    vec3 out_rgb = vec3(0.0);

    for (uint i = 0u; i < light_count; i++) {
        vec3 lp = g_lights.lights[i].pos_intensity.xyz;
        float intensity = g_lights.lights[i].pos_intensity.w;
        vec3 lc = g_lights.lights[i].color_distance.rgb;
        float range = g_lights.lights[i].color_distance.w;

        vec3 toL = lp - v_world_pos;
        float dist = length(toL);
        if (dist <= 1e-5) {
            continue;
        }

        vec3 L = toL / dist;
        float ndotl = max(dot(N, L), 0.0);
        float q = quantize(ndotl, mat.quant_steps);

        // Attenuation:
        // - if range is provided, fade out to 0 at range (smooth)
        // - otherwise fall back to inverse-square-ish
        float att = 1.0;
        if (range > 1e-3) {
            float t = clamp(1.0 - (dist / range), 0.0, 1.0);
            att = t * t;
        } else {
            att = 1.0 / (1.0 + dist * dist);
        }

        out_rgb += base * q * lc * intensity * att;
    }

    f_color = vec4(out_rgb, base_rgba.a);
}
