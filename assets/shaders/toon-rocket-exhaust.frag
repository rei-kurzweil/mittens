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

void main() {
    if (!gl_FrontFacing) discard;
    vec3 direction = exhaust.flight_direction;
    direction /= max(length(direction), 0.0001);
    vec3 offset = v_world_pos - exhaust.launch_origin;
    float along = dot(offset, direction);
    if (along <= 0.0) discard;

    float t = clamp(v_uv.y, 0.0, 1.0);
    vec3 radial = offset - along * direction;
    vec3 center = v_world_pos - radial + (t - 0.5) * exhaust.trail_length * direction;
    vec3 eye = -transpose(mat3(camera.view)) * camera.view[3].xyz;
    vec3 to_eye = eye - center;

    // Project onto the side of an axial billboard without rotating the mesh.
    // As that side collapses end-on, use the cap's radial profile.
    vec3 side = cross(direction, to_eye / max(length(to_eye), 0.0001));
    float side_length = length(side);
    float side_view = smoothstep(0.10, 0.38, side_length);
    float lateral = dot(radial, side) / max(side_length, 0.0001);
    float phase = along * exhaust.wave_frequency;
    float primary = sin(phase);
    float secondary = sin(phase * 2.3 + 1.7);
    float expansion = smoothstep(0.0, 0.5, t * exhaust.trail_length);
    float width = exhaust.trail_radius * mix(0.08, 0.84, expansion);
    width *= 1.0 + expansion * (0.13 * primary - 0.09 * secondary);
    float sway = exhaust.trail_radius * 2.8 * exhaust.wave_amplitude
        * expansion * (primary + 0.4 * secondary);
    float across = (lateral - sway) / width;
    float cap = length(radial) / (exhaust.trail_radius * (0.96 + 0.04 * secondary));
    float outer = 1.0 - smoothstep(0.82, 1.04, mix(cap, abs(across), side_view));

    // Keep the original nested waves and two gray tones across the smoke.
    float middle = 1.0 - smoothstep(0.63, 0.75,
        mix(cap, abs(across + 0.12 * expansion * secondary), side_view));
    float core = 1.0 - smoothstep(0.27, 0.40,
        mix(cap, abs(across - 0.10 * expansion * primary), side_view));
    vec3 grey = mix(exhaust.shadow_color, exhaust.light_color, 0.5 * (middle + core));

    vec3 fragment_to_eye = eye - v_world_pos;
    float facing = dot(normalize(v_normal), fragment_to_eye / max(length(fragment_to_eye), 0.0001));
    float softness = smoothstep(0.0, 0.48, facing);
    float tail = mix(0.42, 0.03, side_view);
    float age = mix(1.0, tail, smoothstep(0.2, 1.0, t));
    float muzzle = smoothstep(0.0, 0.12, along);
    float alpha = outer * softness * age * muzzle * v_color.a;
    if (alpha < 0.001) discard;
    f_color = vec4(grey * v_color.rgb, alpha);
}
