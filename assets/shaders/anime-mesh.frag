#version 450

layout(location = 0) in vec3 v_world_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 4) flat in float v_emissive;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform CameraUBO {
    mat4 view;
    mat4 proj;
    mat3 camera2d;
    vec2 viewport;
    vec2 _pad0;
    vec3 ambient_light;
    uint renderer_flags;
} ubo;

const uint LIGHT_TYPE_POINT = 1u;
const uint LIGHT_TYPE_DIRECTIONAL = 2u;
const uint LIGHT_TYPE_SPOT = 3u;

struct Light {
    vec4 pos_intensity;
    vec4 color_distance;
    vec4 direction_angle;
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
    uint _pad0;
    uint _pad1;
    vec4 anime_shade_color_strength;
    vec4 anime_rim_color;
    vec4 anime_controls;
} mat;

layout(set = 1, binding = 1) uniform sampler2D base_tex;

float anime_ramp(float light_amount, float shade_threshold, float lit_threshold) {
    if (lit_threshold <= shade_threshold) {
        return step(lit_threshold, light_amount);
    }
    return smoothstep(shade_threshold, lit_threshold, light_amount);
}

void main() {
    vec4 base_rgba = texture(base_tex, v_uv) * v_color;
    if (base_rgba.a <= 0.001) {
        discard;
    }

    vec3 N = normalize(v_normal);
    float light_amount = 0.0;
    float distance_amount = 0.0;
    float facing_amount = 0.0;
    uint light_count = min(g_lights.count, 64u);

    for (uint i = 0u; i < light_count; i++) {
        uint light_type = g_lights.lights[i].meta.x;
        vec3 lp = g_lights.lights[i].pos_intensity.xyz;
        float intensity = g_lights.lights[i].pos_intensity.w;
        float range = g_lights.lights[i].color_distance.w;
        vec4 direction_angle = g_lights.lights[i].direction_angle;

        vec3 L;
        float attenuation = 1.0;
        if (light_type == LIGHT_TYPE_DIRECTIONAL) {
            float len = length(lp);
            if (len <= 1e-5) {
                continue;
            }
            L = lp / len;
        } else {
            vec3 to_light = lp - v_world_pos;
            float distance_to_light = length(to_light);
            if (distance_to_light <= 1e-5) {
                continue;
            }
            L = to_light / distance_to_light;
            if (range > 1e-3) {
                float range_factor = clamp(1.0 - distance_to_light / range, 0.0, 1.0);
                attenuation = range_factor * range_factor;
            } else {
                attenuation = 1.0 / (1.0 + distance_to_light * distance_to_light);
            }

            if (light_type == LIGHT_TYPE_SPOT) {
                vec3 spot_direction = normalize(direction_angle.xyz);
                float outer_cos = direction_angle.w;
                float inner_cos = uintBitsToFloat(g_lights.lights[i].meta.y);
                float cone_cos = dot(-L, spot_direction);
                attenuation *= smoothstep(
                    outer_cos,
                    max(inner_cos, outer_cos + 1e-5),
                    cone_cos
                );
            }
        }

        float amount = max(intensity, 0.0) * attenuation;
        distance_amount += amount;
        float ndotl = dot(N, L);
        light_amount += max(ndotl, 0.0) * amount;
        // Fixed 90-degree eligibility gate. Once facing a light, the Anime
        // ramp receives its full distance/cone-attenuated intensity, without
        // cosine weighting. Tangent and back-facing surfaces receive none.
        if (ndotl > 0.0) {
            facing_amount += amount;
        }
    }

    vec3 base = base_rgba.rgb;
    // Both diagnostics use the actual engine light buffer and world position.
    // They bypass the Anime ramp and rim without changing draw/pipeline identity.
    float lighting_debug = mat.anime_rim_color.w;
    if (lighting_debug > 0.5) {
        float amount = lighting_debug < 1.5 ? distance_amount : light_amount;
        f_color = vec4(base * clamp(amount, 0.0, 1.0), base_rgba.a);
        return;
    }
    float shade_strength = clamp(mat.anime_shade_color_strength.a, 0.0, 1.0);
    vec3 shade_multiplier = mix(
        vec3(1.0),
        clamp(mat.anime_shade_color_strength.rgb, vec3(0.0), vec3(1.0)),
        shade_strength
    );
    vec3 shaded = base * shade_multiplier;
    float ramp = anime_ramp(
        facing_amount,
        mat.anime_controls.x,
        mat.anime_controls.y
    );
    vec3 color = mix(shaded, base, ramp);

    vec3 view_position = (ubo.view * vec4(v_world_pos, 1.0)).xyz;
    vec3 view_normal = normalize(mat3(ubo.view) * N);
    vec3 V = normalize(-view_position);
    float rim_power = max(mat.anime_controls.w, 0.01);
    float rim = pow(1.0 - max(dot(view_normal, V), 0.0), rim_power)
        * clamp(mat.anime_controls.z, 0.0, 1.0);
    color = min(base, color + clamp(mat.anime_rim_color.rgb, vec3(0.0), vec3(1.0)) * rim);

    f_color = vec4(color, base_rgba.a);
}
