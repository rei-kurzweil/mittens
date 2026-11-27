#version 450
// Cartoon grass on a dome: cartoon-grass.frag for a sphere mesh scaled into an
// ellipsoid (rolling hills). The shared vertex shader's normal is wrong under
// that non-uniform scale, so the normal here comes from the ellipsoid itself:
// normalize((P - dome_center) / dome_radii^2). Keep the rest in step with
// cartoon-grass.frag, which this copies.
//
// Cartoon grass ground. The ground itself is the albedo (the `albedo` input
// times the instance colour, C.rgba) times a soft procedural lightness
// pattern. Tufts come from a 2x2 atlas
// (generate_grass_tufts.py): the ground is divided into cells in world XZ,
// and each cell may hold one tuft at a hashed spot, size and variant. A tuft
// is drawn upright toward the viewer, curved side up, as if it stood on the
// ground: its width runs across the view and its height runs away from the
// viewer, stretched by how flat the ground is seen there. So tufts keep
// their places on the ground but always face the camera. Tufts fade out by
// `fade_far`; nearer ones cover farther ones. Only up-facing surfaces get
// tufts.
// Lighting is the Anime ramp: never brighter than the albedo, and toward the
// shade colour as the light reaching the surface (each light's intensity,
// attenuated by distance and spot cone) drops below the lit threshold. A
// light counts only on the side it faces, ramped across N.L = 0 over
// +-facing_softness instead of switching at the terminator.
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

layout(set = 1, binding = 0, std140) uniform CartoonGrass {
    vec3 shade_color; float shade_strength;
    vec3 albedo; float facing_softness;
    float shade_threshold; float lit_threshold; float cell_size; float density;
    float tuft_min; float tuft_max; float fade_near; float fade_far;
    float patch_strength; float patch_scale; float ground_lightness; float line_strength;
    vec3 dome_center; float dome_pad0;
    vec3 dome_radii; float dome_pad1;
} mat;

// The tuft atlas: RGB is outline lightness, alpha its coverage.
layout(set = 1, binding = 1) uniform sampler2D tufts;

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

vec2 hash22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

float value_noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash12(i), hash12(i + vec2(1, 0)), u.x),
               mix(hash12(i + vec2(0, 1)), hash12(i + vec2(1, 1)), u.x), u.y);
}

float anime_ramp(float light_amount, float shade_threshold, float lit_threshold) {
    if (lit_threshold <= shade_threshold) {
        return step(lit_threshold, light_amount);
    }
    return smoothstep(shade_threshold, lit_threshold, light_amount);
}

void main() {
    vec3 P = v_world_pos;
    vec3 radii = max(mat.dome_radii, vec3(1e-3));
    vec3 N = normalize((P - mat.dome_center) / (radii * radii));
    vec3 eye = -transpose(mat3(ubo.view)) * ubo.view[3].xyz;

    float scale = max(mat.patch_scale, 0.01);
    float lightness = mat.ground_lightness + mat.patch_strength
        * ((value_noise(P.xz / scale) - 0.5) * 1.4 + (value_noise(P.xz / (scale * 0.31)) - 0.5) * 0.6);

    // Derivatives before any divergent branch, for the atlas gradients.
    vec2 dx = dFdx(P.xz);
    vec2 dy = dFdy(P.xz);
    vec2 from_eye = P.xz - eye.xz;
    float distance = length(from_eye);
    float fade = 1.0 - smoothstep(mat.fade_near, max(mat.fade_far, mat.fade_near + 0.001), distance);
    float cell = max(mat.cell_size, 0.05);
    if (fade > 0.0 && N.y > 0.6 && distance > 1e-3) {
        vec2 away = from_eye / distance;
        vec2 across = vec2(-away.y, away.x);
        float nearest = 1e9;
        vec4 hit = vec4(0.0);
        // A tuft rises away from the viewer, so its anchor is at or nearer
        // than this point: search cells back toward the eye and to the sides.
        for (int k = 0; k < 5; k++) {
            for (int l = -1; l <= 1; l++) {
                vec2 c = floor((P.xz - away * (float(k) * cell) + across * (float(l) * cell)) / cell);
                if (hash12(c) > mat.density) {
                    continue;
                }
                vec2 anchor = (c + hash22(c)) * cell;
                vec2 to_anchor = anchor - eye.xz;
                float anchor_distance = length(to_anchor);
                if (anchor_distance >= nearest || anchor_distance < 1e-3) {
                    continue;
                }
                vec2 f = to_anchor / anchor_distance;
                vec2 r = vec2(-f.y, f.x);
                float width = mix(mat.tuft_min, mat.tuft_max, hash12(c + 17.0));
                // Height on screen per metre of ground away from the viewer.
                float rise = max(eye.y - P.y, 0.05) / max(anchor_distance, 0.05);
                vec2 o = P.xz - anchor;
                vec2 uv = vec2(dot(o, r) / width + 0.5, dot(o, f) * rise / width);
                if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) {
                    continue;
                }
                float variant = floor(hash12(c + 31.0) * 4.0);
                vec2 corner = vec2(mod(variant, 2.0), floor(variant / 2.0)) * 0.5;
                vec2 atlas = corner + vec2(uv.x, 1.0 - uv.y) * 0.5;
                vec2 gx = vec2(dot(dx, r) / width, -dot(dx, f) * rise / width) * 0.5;
                vec2 gy = vec2(dot(dy, r) / width, -dot(dy, f) * rise / width) * 0.5;
                vec4 t = textureGrad(tufts, atlas, gx, gy);
                if (t.a > 0.02) {
                    nearest = anchor_distance;
                    hit = t;
                }
            }
        }
        lightness = mix(lightness, hit.r, hit.a * mat.line_strength * fade);
    }

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
            vec3 to_light = lp - P;
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
                attenuation *= smoothstep(outer_cos, max(inner_cos, outer_cos + 1e-5), dot(-L, spot_direction));
            }
        }
        float softness = max(mat.facing_softness, 0.0);
        float ndotl = dot(N, L);
        float facing = softness > 1e-4 ? smoothstep(-softness, softness, ndotl) : step(0.0, ndotl);
        facing_amount += max(intensity, 0.0) * attenuation * facing;
    }

    vec3 base = mat.albedo * v_color.rgb * clamp(lightness, 0.0, 1.0);
    vec3 shade = mix(vec3(1.0), clamp(mat.shade_color, vec3(0.0), vec3(1.0)), clamp(mat.shade_strength, 0.0, 1.0));
    vec3 color = mix(base * shade, base, anime_ramp(facing_amount, mat.shade_threshold, mat.lit_threshold));
    f_color = vec4(color, 1.0);
}
