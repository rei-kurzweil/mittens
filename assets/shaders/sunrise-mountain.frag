#version 450
// Background mountains at sunrise (Background occlusion+lighting only). Faces
// are lit by the scene's directional lights in their own colours, so a gold
// sun on one side and a red counter-light on the other paint the two sides of
// each ridge; point and spot lights live in foreground space and are skipped.
// The normal comes from screen derivatives, not the vertex normal, so the
// sheared cubes the ridges are built from shade correctly as flat facets.
// Haze pulls distant and low geometry toward the sky's horizon colour, and
// silhouettes near the sun get a gold rim. The camera is at the origin here,
// since Background removes camera translation.
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

layout(set = 1, binding = 0, std140) uniform SunriseMountain {
    vec3 haze_color; float haze_near;
    vec3 haze_sun_color; float haze_far;
    vec3 rim_color; float haze_strength;
    vec3 sun_direction; float rim_strength;
    float floor_y; float valley_mist;
} mat;

void main() {
    vec3 p = v_world_pos;
    vec3 N = normalize(cross(dFdx(p), dFdy(p)));
    vec3 V = normalize(-p);
    if (dot(N, V) < 0.0) {
        N = -N;
    }

    vec3 light = max(ubo.ambient_light, vec3(0.0));
    uint light_count = min(g_lights.count, 64u);
    for (uint i = 0u; i < light_count; i++) {
        if (g_lights.lights[i].meta.x != LIGHT_TYPE_DIRECTIONAL) {
            continue;
        }
        vec3 lp = g_lights.lights[i].pos_intensity.xyz;
        float len = length(lp);
        if (len <= 1e-5) {
            continue;
        }
        float ndotl = dot(N, lp / len);
        // A soft toon step: facets read as lit or shaded, with a narrow blend.
        float band = smoothstep(-0.05, 0.30, ndotl);
        light += g_lights.lights[i].color_distance.xyz
            * max(g_lights.lights[i].pos_intensity.w, 0.0) * band;
    }
    vec3 color = v_color.rgb * light;

    vec3 sun = length(mat.sun_direction) > 0.0001 ? normalize(mat.sun_direction) : vec3(0.0, 0.05, -1.0);
    vec3 direction = normalize(p);
    vec2 flat_direction = direction.xz / max(length(direction.xz), 0.0001);
    vec2 flat_sun = sun.xz / max(length(sun.xz), 0.0001);
    float sun_side = dot(flat_direction, flat_sun) * 0.5 + 0.5;

    // Silhouettes toward the sun catch a gold rim.
    float edge = pow(1.0 - abs(dot(N, V)), 3.0);
    color += mat.rim_color * edge * pow(sun_side, 4.0) * mat.rim_strength;

    float distance_haze = smoothstep(mat.haze_near, max(mat.haze_far, mat.haze_near + 0.001), length(p));
    float mist = (1.0 - smoothstep(mat.floor_y, mat.floor_y + 6.0, p.y)) * mat.valley_mist;
    float haze = clamp(max(distance_haze, mist) * mat.haze_strength, 0.0, 1.0);
    vec3 haze_color = mix(mat.haze_color, mat.haze_sun_color, sun_side * sun_side);
    color = mix(color, haze_color, haze);

    f_color = vec4(color, 1.0);
}
