#version 450
// Bliss sky: a clear blue gradient, deepest overhead and paling to a soft
// haze at the horizon, with the sun's disc and glow computed here (no
// textures). Background removes camera translation, so the world position is
// a view direction. All colours are linear.
layout(location = 0) in vec3 v_world_pos;
layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 0, std140) uniform BlissSky {
    vec3 zenith_color; float zenith_power;
    vec3 horizon_color; float haze_width;
    vec3 ground_color; float sun_radius;
    vec3 sun_direction; float sun_strength;
    vec3 glow_color; float glow_strength;
    vec3 sun_color; float glow_width;
} sky;

void main() {
    vec3 direction = normalize(v_world_pos);
    float altitude = direction.y;

    // Above the horizon the blue deepens toward the zenith; zenith_power > 1
    // keeps more of the sky pale near the horizon.
    float up = pow(clamp(altitude, 0.0, 1.0), 1.0 / max(sky.zenith_power, 0.01));
    vec3 color = mix(sky.horizon_color, sky.zenith_color, up);
    // A thin haze band along the horizon, and the ground colour below it.
    float haze = 1.0 - smoothstep(0.0, max(sky.haze_width, 0.001), abs(altitude));
    color = mix(color, sky.horizon_color, haze * 0.6);
    color = mix(color, sky.ground_color, smoothstep(0.0, -0.18, altitude));

    vec3 sun = normalize(sky.sun_direction);
    float facing = max(dot(direction, sun), 0.0);
    // A wide soft glow and a tighter bright one, then the disc itself.
    float width = max(sky.glow_width, 0.01);
    float glow = 0.35 * pow(facing, 6.0 / width) + 0.65 * pow(facing, 48.0 / width);
    color += sky.glow_color * glow * sky.glow_strength;
    float inner = cos(sky.sun_radius);
    float outer = cos(sky.sun_radius * 1.35);
    color = mix(color, sky.sun_color * sky.sun_strength, smoothstep(outer, inner, facing));

    f_color = vec4(color, 1.0);
}
