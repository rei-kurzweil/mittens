#version 450
// Unlit sunrise sky, all analytic (no textures). The horizon runs from red
// on the sun's side to pink opposite it, rising through a warm band into a
// dusky zenith. A gold glow sits around the sun, brightest along the
// horizon, and a few thin cloud streaks pick up that gold near the sun. The
// sun's disc is a separate emissive background draw so it can bloom.
// Background removes camera translation.
layout(location = 0) in vec3 v_world_pos;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform SunriseSky {
    vec3 zenith_color; float zenith_point;
    vec3 upper_color; float upper_point;
    vec3 horizon_color; float sun_side_power;
    vec3 sun_horizon_color; float glow_strength;
    vec3 glow_color; float glow_width;
    vec3 sun_direction; float cloud_strength;
    vec3 cloud_color; float below_point;
    vec3 below_color; float cloud_seed;
} sky;

// One streak band: a thin horizontal stripe, broken up along its length.
float streak(float azimuth, float altitude, float center, float width, float phase) {
    float across = (altitude - center) / width;
    float band = exp(-across * across);
    float along = sin(azimuth * 3.0 + phase) * 0.6 + sin(azimuth * 7.0 + phase * 2.3) * 0.4;
    return band * smoothstep(0.05, 0.75, along);
}

void main() {
    vec3 direction = normalize(v_world_pos);
    vec3 sun = length(sky.sun_direction) > 0.0001 ? normalize(sky.sun_direction) : vec3(0.0, 0.05, -1.0);
    float altitude = direction.y;

    // How far round the sky this direction is toward the sun (1 = its azimuth).
    vec2 flat_direction = direction.xz / max(length(direction.xz), 0.0001);
    vec2 flat_sun = sun.xz / max(length(sun.xz), 0.0001);
    float toward = dot(flat_direction, flat_sun) * 0.5 + 0.5;
    float sun_side = pow(toward, max(sky.sun_side_power, 0.01));

    vec3 horizon = mix(sky.horizon_color, sky.sun_horizon_color, sun_side);
    vec3 color = mix(horizon, sky.upper_color,
        smoothstep(0.0, max(sky.upper_point, 0.001), altitude));
    color = mix(color, sky.zenith_color,
        smoothstep(sky.upper_point, max(sky.zenith_point, sky.upper_point + 0.001), altitude));
    color = mix(color, sky.below_color,
        smoothstep(0.0, min(sky.below_point, -0.001), altitude));

    // Gold around the sun: a wide warm wash, a tighter halo, and a bright
    // band hugging the horizon on the sun's side.
    float facing = max(dot(direction, sun), 0.0);
    float width = max(sky.glow_width, 0.01);
    float glow = 0.30 * pow(facing, 6.0 / width)
               + 0.55 * pow(facing, 60.0 / width)
               + 0.45 * exp(-abs(altitude - sun.y) * 22.0) * pow(sun_side, 6.0);
    color += sky.glow_color * glow * sky.glow_strength;

    // Thin streaks low in the sky: lavender away from the sun, gold toward it.
    float azimuth = atan(direction.z, direction.x);
    float clouds = streak(azimuth, altitude, 0.07, 0.012, sky.cloud_seed)
                 + streak(azimuth, altitude, 0.15, 0.018, sky.cloud_seed + 1.7)
                 + 0.7 * streak(azimuth, altitude, 0.26, 0.025, sky.cloud_seed + 4.1);
    vec3 cloud = mix(sky.cloud_color, sky.glow_color, pow(sun_side, 3.0));
    color = mix(color, cloud, clamp(clouds * sky.cloud_strength, 0.0, 1.0));

    f_color = vec4(color, 1.0);
}
