#version 450
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform BurningInputs {
    float burn_progress;
    vec2 burn_axis;
    float burn_start;
    float burn_end;
    float ember_width;
    float char_width;
    float edge_noise;
    float time_seconds;
    vec3 ember_color;
    vec3 char_color;
    vec3 ash_color;
    float shade_strength;
} burning;
layout(set = 1, binding = 1) uniform sampler2D base_tex;

void main() {
    vec4 base = texture(base_tex, v_uv) * v_color;
    float span = burning.burn_end - burning.burn_start;
    if (abs(span) < 0.0001) { span = 0.0001; }
    float coordinate = (dot(v_uv, burning.burn_axis) - burning.burn_start) / span;
    float progress = clamp(burning.burn_progress, 0.0, 1.0);
    // Periodic across the circumference, so a usual cylinder's UV seam agrees.
    vec2 across_axis = vec2(burning.burn_axis.y, -burning.burn_axis.x);
    float across = dot(v_uv, across_axis);
    float noise = sin(across * 37.699112) * 0.65
                + sin(across * 81.681409) * 0.35;
    float distance_to_front = coordinate - progress
                            + noise * max(burning.edge_noise, 0.0);
    float ember_width = max(burning.ember_width, 0.0001);
    float char_width = max(burning.char_width, 0.0001);
    float edge = 0.003;
    float char_mask = 1.0 - smoothstep(0.0, edge, distance_to_front);
    float ash_mask = 1.0 - smoothstep(-ember_width - char_width,
                                    -ember_width - char_width + edge,
                                    distance_to_front);
    float ember_mask = 1.0 - smoothstep(ember_width, ember_width + edge,
                                       abs(distance_to_front));
    // Endpoints have exact meanings: fresh paper at 0, entirely ash at 1.
    if (progress <= 0.0) { char_mask = 0.0; ash_mask = 0.0; ember_mask = 0.0; }
    if (progress >= 1.0) { char_mask = 1.0; ash_mask = 1.0; ember_mask = 0.0; }
    vec3 color = mix(base.rgb, burning.char_color, char_mask);
    color = mix(color, burning.ash_color, ash_mask);
    float shade = step(0.1, dot(normalize(v_normal), normalize(vec3(-0.4, 0.8, 0.5))));
    color *= 1.0 - clamp(burning.shade_strength, 0.0, 1.0) * (1.0 - shade);
    float pulse = 0.90 + 0.10 * sin(burning.time_seconds * 7.0 + across * 6.283185);
    color = mix(color, burning.ember_color * pulse, ember_mask);
    f_color = vec4(color, base.a);
}
