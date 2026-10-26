#version 450
// Unlit, two-stop sky gradient. Background removes camera translation.
layout(location = 0) in vec3 v_world_pos;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform SkyGradient {
    vec3 low_color;
    float low_point;
    vec3 high_color;
    float high_point;
} sky;

void main() {
    float altitude = normalize(v_world_pos).y;
    float span = sky.high_point - sky.low_point;
    float ramp = abs(span) > 0.00001
        ? clamp((altitude - sky.low_point) / span, 0.0, 1.0)
        : step(sky.low_point, altitude);
    // Smooth endpoints without requiring extra gradient stops or textures.
    ramp = ramp * ramp * (3.0 - 2.0 * ramp);
    f_color = vec4(mix(sky.low_color, sky.high_color, ramp), 1.0);
}
