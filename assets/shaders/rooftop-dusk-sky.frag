#version 450
// Unlit dusk gradient: beige horizon band, lilac above it, darkening toward
// the zenith. Below the horizon it settles into a muted ground haze.
// Stop points are viewing-direction Y (-1 nadir, 0 horizon, +1 zenith).
layout(location=0) in vec3 v_world_pos;
layout(location=0) out vec4 f_color;
layout(set=1,binding=0,std140) uniform DuskSky {
    vec3 ground_color; float horizon_point;
    vec3 horizon_color; float lilac_point;
    vec3 lilac_color; float zenith_point;
    vec3 zenith_color; float haze_depth;
} sky;

void main() {
    float y = normalize(v_world_pos).y;
    float haze = smoothstep(sky.horizon_point - max(sky.haze_depth, 0.001), sky.horizon_point, y);
    float lilac = smoothstep(sky.horizon_point, max(sky.lilac_point, sky.horizon_point + 0.001), y);
    float zenith = smoothstep(sky.lilac_point, max(sky.zenith_point, sky.lilac_point + 0.001), y);
    vec3 color = mix(sky.ground_color, sky.horizon_color, haze);
    color = mix(color, sky.lilac_color, lilac);
    color = mix(color, sky.zenith_color, zenith);
    f_color = vec4(color, 1.0);
}
