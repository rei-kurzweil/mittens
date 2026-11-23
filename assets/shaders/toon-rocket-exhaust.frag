#version 450
layout(location = 0) in vec3 v_world_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;
layout(set = 0, binding = 0, std140) uniform CameraUBO {
    mat4 view; mat4 proj; mat3 camera2d; vec2 viewport; vec2 _pad0;
    vec3 ambient_light; uint renderer_flags;
} camera;
layout(set = 1, binding = 0, std140) uniform RocketExhaustInputs {
    vec3 launch_origin;
    vec3 flight_direction;
    vec3 light_color;
    vec3 shadow_color;
    float wave_frequency;
    float wave_amplitude;
    float trail_length;
    float trail_radius;
} exhaust;

// Evaluate the original ribbon profile on its old 0.9 m wide projection.
vec4 ribbon_smoke(vec3 projected, vec3 center, vec3 axis, vec3 side) {
    float t = 0.5 - dot(projected - center, axis) / exhaust.trail_length;
    float along = dot(projected - exhaust.launch_origin, axis);
    if (t < 0.0 || t > 1.0 || along <= 0.0) return vec4(exhaust.shadow_color, 0.0);

    float expansion = smoothstep(0.0, 0.5, t * exhaust.trail_length);
    float phase = along * exhaust.wave_frequency;
    float primary = sin(phase);
    float secondary = sin(phase * 2.3 + 1.7);
    float sway = expansion * exhaust.wave_amplitude * (primary + 0.4 * secondary);
    float half_width = mix(0.021, 0.22, expansion);
    half_width *= 1.0 + expansion * (0.13 * primary - 0.09 * secondary);
    float across = (dot(projected - center, side) / 0.9 - sway) / half_width;
    float outer = 1.0 - smoothstep(0.86, 1.0, abs(across));
    float middle = 1.0 - smoothstep(0.63, 0.73, abs(across + 0.12 * expansion * secondary));
    float core = 1.0 - smoothstep(0.27, 0.37, abs(across - 0.10 * expansion * primary));
    vec3 grey = mix(exhaust.shadow_color, exhaust.light_color, 0.5 * (middle + core));
    float alpha = outer * (1.0 - smoothstep(0.2, 1.0, t)) * smoothstep(0.0, 0.12, along);
    return vec4(grey, alpha);
}

vec4 cap_smoke(vec3 radial, vec3 to_eye, float t, float along) {
    float radius = length(radial) / exhaust.trail_radius;
    float facing = dot(normalize(v_normal), normalize(to_eye));
    float edge = (1.0 - smoothstep(0.60, 1.0, radius)) * smoothstep(0.0, 0.55, facing);
    float shade = 1.0 - smoothstep(0.15, 0.8, radius);
    vec3 grey = mix(exhaust.shadow_color, exhaust.light_color, shade);
    float alpha = edge * mix(0.8, 0.40, t) * smoothstep(0.0, 0.12, along);
    return vec4(grey, alpha);
}

void main() {
    if (!gl_FrontFacing) discard;
    vec3 axis = normalize(exhaust.flight_direction);
    vec3 offset = v_world_pos - exhaust.launch_origin;
    float along = dot(offset, axis);
    vec3 radial = offset - along * axis;
    // Capsule V is linear along the axis. It locates the mesh center without
    // using the cylindrical U coordinate or the surface normal for the bands.
    float t = v_uv.y;
    vec3 center = v_world_pos - radial + (t - 0.5) * exhaust.trail_length * axis;
    vec3 eye = -transpose(mat3(camera.view)) * camera.view[3].xyz;
    vec3 to_eye = eye - v_world_pos;
    vec3 side = cross(axis, normalize(eye - center));
    float side_length = length(side);
    float ribbon_weight = smoothstep(0.08, 0.26, side_length);

    vec4 smoke = vec4(exhaust.shadow_color, 0.0);
    if (ribbon_weight > 0.0) {
        side /= side_length;
        vec3 plane_normal = cross(side, axis);
        float denom = dot(plane_normal, to_eye);
        if (abs(denom) > 0.0001) {
            // Find where this pixel's ray hits the old billboard. This keeps
            // both waves continuous in perspective, even at an oblique angle.
            vec3 projected = v_world_pos + to_eye * (dot(plane_normal, center - v_world_pos) / denom);
            smoke = ribbon_smoke(projected, center, axis, side);
        }
    }
    if (ribbon_weight < 1.0) {
        smoke = mix(cap_smoke(radial, to_eye, t, along), smoke, ribbon_weight);
    }
    smoke *= v_color;
    if (smoke.a < 0.001) discard;
    f_color = smoke;
}
