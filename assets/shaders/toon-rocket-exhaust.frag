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
} exhaust;

// UV y=0 is the rocket; y=1 is the trailing end. The ribbon moves with the
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
    float x = v_uv.x - 0.5;
    float phase = along * exhaust.wave_frequency;
    // Straight at the nozzle, increasingly sinuous further back.
    float sway = exhaust.wave_amplitude * t;
    float wave = sway * (sin(phase) + 0.4 * sin(phase * 2.3 + 1.7));
    float half_width = mix(0.07, 0.22, t);
    float outer = 1.0 - smoothstep(half_width - 0.04, half_width, abs(x - wave));
    float core = abs(x - wave + 0.5 * sway * sin(phase * 0.7 + 0.9));
    float inner = 1.0 - smoothstep(half_width * 0.35, half_width * 0.55, core);
    float muzzle = smoothstep(0.0, 0.12, along);
    float alpha = outer * (1.0 - smoothstep(0.2, 1.0, t)) * muzzle * v_color.a;
    if (alpha < 0.001) discard;
    vec3 grey = mix(exhaust.shadow_color, exhaust.light_color, inner);
    f_color = vec4(grey * v_color.rgb, alpha);
}
