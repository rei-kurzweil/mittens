#version 450
layout(location = 0) in vec3 v_world_pos;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform SkyInputs {
    vec3 low_color; float low_point;
    vec3 high_color; float high_point;
    vec3 sun_direction; float sun_angular_radius;
    vec3 sun_color; float sun_strength;
} sky;
void main() {
    vec3 direction = normalize(v_world_pos);
    float span = sky.high_point - sky.low_point;
    float t = abs(span) > 0.00001 ? clamp((direction.y - sky.low_point) / span, 0.0, 1.0) : step(sky.low_point, direction.y);
    t = t * t * (3.0 - 2.0 * t);
    vec3 gradient = mix(sky.low_color, sky.high_color, t);
    float radius = max(sky.sun_angular_radius, 0.0001);
    float alignment = dot(direction, normalize(sky.sun_direction));
    float glow = smoothstep(cos(radius * 2.0), cos(radius), alignment);
    vec3 background = mix(gradient, sky.sun_color, clamp(glow * sky.sun_strength, 0.0, 1.0));
    float edge = alignment - cos(radius);
    float aa = max(fwidth(edge), 0.000001);
    float disc = smoothstep(-aa, aa, edge);
    f_color = vec4(mix(background, vec3(1.0), disc * clamp(sky.sun_strength, 0.0, 1.0)), 1.0);
}
