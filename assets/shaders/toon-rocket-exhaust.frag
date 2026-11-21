#version 450
layout(location = 0) in vec3 v_world_pos;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform RocketExhaustInputs {
    vec3 launch_origin;
    vec3 flight_direction;
    vec3 light_color;
    vec3 shadow_color;
    float wave_frequency;
    float wave_amplitude;
    float trail_length;
} exhaust;

// UV y=0 is the nozzle and y=1 is the trailing end. The ribbon moves with the
// rocket, but the waves are phased by world distance along the flight line so
// the smoke appears to stay where it was laid down. Fragments behind the
// launch point are discarded, so the fixed-length ribbon never pokes back
// through the launcher.
void main() {
    vec3 direction = exhaust.flight_direction;
    direction /= max(length(direction), 0.0001);
    float along = dot(v_world_pos - exhaust.launch_origin, direction);
    if (along <= 0.0) discard;
    float t = clamp(v_uv.y, 0.0, 1.0);
    float expansion = smoothstep(0.0, 0.5, t * exhaust.trail_length);
    float phase = along * exhaust.wave_frequency;
    float primary = sin(phase);
    float secondary = sin(phase * 2.3 + 1.7);
    // Match the nozzle at the start, then let the waves shape the silhouette.
    float sway = expansion * exhaust.wave_amplitude * (primary + 0.4 * secondary);
    float half_width = mix(0.021, 0.22, expansion);
    half_width *= 1.0 + expansion * (0.13 * primary - 0.09 * secondary);
    float across = (v_uv.x - 0.5 - sway) / half_width;
    float outer = 1.0 - smoothstep(0.86, 1.0, abs(across));
    float muzzle = smoothstep(0.0, 0.12, along);
    float alpha = outer * (1.0 - smoothstep(0.2, 1.0, t)) * muzzle * v_color.a;
    if (alpha < 0.001) discard;
    // Three nested bands with independently shifting boundaries.
    float middle = 1.0 - smoothstep(0.63, 0.73, abs(across + 0.12 * expansion * secondary));
    float core = 1.0 - smoothstep(0.27, 0.37, abs(across - 0.1 * expansion * primary));
    vec3 grey = mix(exhaust.shadow_color, exhaust.light_color, 0.5 * (middle + core));
    f_color = vec4(grey * v_color.rgb, alpha);
}
