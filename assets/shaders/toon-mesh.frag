#version 450

layout(location = 0) in vec3 v_world_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 4) flat in float v_emissive;

layout(location = 0) out vec4 f_color;

// Set 0: global camera.
layout(set = 0, binding = 0) uniform CameraUBO {
    mat4 view;
    mat4 proj;
    mat3 camera2d;
    vec2 viewport;
    vec2 _pad0;
    vec3 ambient_light;
    float _pad1;
} ubo;


const uint LIGHT_TYPE_POINT = 1u;
const uint LIGHT_TYPE_DIRECTIONAL = 2u;
const uint LIGHT_TYPE_SPOT = 3u;

struct Light {
    vec4 pos_intensity;  // xyz position OR direction, w intensity
    vec4 color_distance; // rgb color, w distance (point range)
    vec4 direction_angle; // xyz spot direction, w outer cone cosine
    uvec4 meta;          // x light_type, y inner cone cosine as float bits
};

layout(set = 0, binding = 1, std430) readonly buffer LightsSSBO {
    uint count;
    // IMPORTANT: keep this header exactly 16 bytes to match the Rust side.
    // Using `uvec3` here changes alignment/offset rules in std430 and will shift `lights`.
    uint _pad0;
    uint _pad1;
    uint _pad2;
    Light lights[64];
} g_lights;

// Set 1: material params (no textures yet; those can be added later).
layout(set = 1, binding = 0) uniform MaterialUBO {
    vec4 base_color;
    float quant_steps;
    uint emissive;
    uint _pad0;
    uint _pad1;
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

    if (mat.emissive != 0u || v_emissive > 0.0) {
        f_color = vec4(base, base_rgba.a);
        return;
    }

    uint light_count = min(g_lights.count, 64u);

    vec3 N = normalize(v_normal);
    vec3 ambient_rgb = base * max(ubo.ambient_light, vec3(0.0));

    // Accumulate lights before quantization so overlapping light footprints and
    // differently-colored lights combine normally. Quantization is applied once
    // to the combined intensity below, preserving the accumulated color ratio.
    float light_amount = 0.0;
    vec3 light_rgb = vec3(0.0);

    for (uint i = 0u; i < light_count; i++) {
        uint light_type = g_lights.lights[i].meta.x;
        vec3 lp = g_lights.lights[i].pos_intensity.xyz;
        float intensity = g_lights.lights[i].pos_intensity.w;
        vec3 lc = g_lights.lights[i].color_distance.rgb;
        float range = g_lights.lights[i].color_distance.w;
        vec4 direction_angle = g_lights.lights[i].direction_angle;

        vec3 L;
        float att = 1.0;

        if (light_type == LIGHT_TYPE_DIRECTIONAL) {
            // Directional light: pos_intensity.xyz stores a direction vector.
            // Normalize on-GPU, per user convention.
            float len = length(lp);
            if (len <= 1e-5) {
                continue;
            }
            L = lp / len;
            att = 1.0;
        } else {
            // Point light: pos_intensity.xyz stores world-space position.
            vec3 toL = lp - v_world_pos;
            float dist = length(toL);
            if (dist <= 1e-5) {
                continue;
            }
            L = toL / dist;

            // Attenuation:
            // - if range is provided, fade out to 0 at range (smooth)
            // - otherwise fall back to inverse-square-ish
            if (range > 1e-3) {
                float t = clamp(1.0 - (dist / range), 0.0, 1.0);
                att = t * t;
            } else {
                att = 1.0 / (1.0 + dist * dist);
            }

            if (light_type == LIGHT_TYPE_SPOT) {
                vec3 spot_direction = normalize(direction_angle.xyz);
                float outer_cos = direction_angle.w;
                float inner_cos = uintBitsToFloat(g_lights.lights[i].meta.y);
                float cone_cos = dot(-L, spot_direction);
                att *= smoothstep(outer_cos, max(inner_cos, outer_cos + 1e-5), cone_cos);
            }
        }

        float ndotl = max(dot(N, L), 0.0);

        float amount = max(ndotl * intensity * att, 0.0);
        light_amount += amount;
        light_rgb += lc * amount;
    }

    vec3 mixed_light = vec3(0.0);
    if (light_amount > 1e-6) {
        mixed_light = (light_rgb / light_amount)
            * quantize(light_amount, mat.quant_steps);
    }

    vec3 out_rgb = ambient_rgb + (base * mixed_light);
    f_color = vec4(out_rgb, base_rgba.a);
}
