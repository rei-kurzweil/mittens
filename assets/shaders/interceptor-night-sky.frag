#version 450

layout(location=0) in vec3 v_world_pos;
layout(location=0) out vec4 f_color;
layout(set=1,binding=0,std140) uniform NightSky {
    vec3 horizon_color;
    float moon_radius;
    vec3 zenith_color;
    vec3 moon_direction;
} sky;

void main() {
    vec3 direction = normalize(v_world_pos);
    float below = smoothstep(-0.21, 0.0, direction.y);
    float above = smoothstep(0.008, 0.25, direction.y);
    vec3 color = mix(vec3(0.005, 0.003, 0.007), sky.horizon_color, below);
    color = mix(color, sky.zenith_color, above);

    // Chord distance on the unit sphere gives a circular moon in both eyes.
    float radius = length(direction - normalize(sky.moon_direction)) / sky.moon_radius;
    float aa = max(fwidth(radius), 0.001);
    float disk = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, radius);
    float halo = 0.26 * exp(-0.48 * radius * radius)
               + 0.055 * exp(-0.085 * radius * radius);
    color += vec3(0.92, 0.80, 1.0) * disk + vec3(0.65, 0.44, 1.0) * halo;
    f_color = vec4(color, 1.0);
}
