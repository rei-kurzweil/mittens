#version 450
#extension GL_EXT_multiview : require

// XR multiview variant of toon-mesh.frag.
// Identical lighting logic — only the camera UBO struct differs so that
// `ambient_light` is at the correct byte offset for the XR UBO layout
// (view[2] + proj[2] up front).

layout(location = 0) in vec3 v_world_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 4) flat in float v_emissive;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform CameraXrUBO {
    mat4 view[2];
    mat4 proj[2];
    mat3 camera2d;
    vec2 viewport;
    vec2 _pad0;
    vec3 ambient_light;
    float _pad1;
} ubo;

const uint LIGHT_TYPE_POINT = 1u;
const uint LIGHT_TYPE_DIRECTIONAL = 2u;

struct Light {
    vec4 pos_intensity;
    vec4 color_distance;
    uvec4 meta;
};

layout(set = 0, binding = 1, std430) readonly buffer LightsSSBO {
    uint count;
    uint _pad0;
    uint _pad1;
    uint _pad2;
    Light lights[64];
} g_lights;

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

    float best_q = 0.0;
    vec3 best_lc = vec3(0.0);

    for (uint i = 0u; i < light_count; i++) {
        uint light_type = g_lights.lights[i].meta.x;
        vec3 lp = g_lights.lights[i].pos_intensity.xyz;
        float intensity = g_lights.lights[i].pos_intensity.w;
        vec3 lc = g_lights.lights[i].color_distance.rgb;
        float range = g_lights.lights[i].color_distance.w;

        vec3 L;
        float att = 1.0;

        if (light_type == LIGHT_TYPE_DIRECTIONAL) {
            float len = length(lp);
            if (len <= 1e-5) {
                continue;
            }
            L = lp / len;
            att = 1.0;
        } else {
            vec3 toL = lp - v_world_pos;
            float dist = length(toL);
            if (dist <= 1e-5) {
                continue;
            }
            L = toL / dist;

            if (range > 1e-3) {
                float t = clamp(1.0 - (dist / range), 0.0, 1.0);
                att = t * t;
            } else {
                att = 1.0 / (1.0 + dist * dist);
            }
        }

        float ndotl = max(dot(N, L), 0.0);
        float q = quantize(ndotl * intensity * att, mat.quant_steps);

        if (q > best_q + 1e-6) {
            best_q = q;
            best_lc = lc;
        } else if (abs(q - best_q) <= 1e-6) {
            best_lc = max(best_lc, lc);
        }
    }

    vec3 out_rgb = ambient_rgb + (base * best_lc * best_q);
    f_color = vec4(out_rgb, base_rgba.a);
}
