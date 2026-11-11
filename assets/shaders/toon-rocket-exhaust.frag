#version 450
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;

// UV y=0 is the head; y=1 is the trailing end.
void main() {
    float t = clamp(v_uv.y, 0.0, 1.0);
    float x = v_uv.x - 0.5;
    float wave = 0.085 * sin(t * 19.0) + 0.035 * sin(t * 37.0);
    float outer = 1.0 - smoothstep(0.19, 0.25, abs(x - wave));
    float inner = 1.0 - smoothstep(0.065, 0.12, abs(x - wave + 0.06 * sin(t * 13.0)));
    float alpha = outer * (1.0 - smoothstep(0.25, 1.0, t)) * v_color.a;
    if (alpha < 0.001) discard;
    vec3 grey = mix(vec3(0.49, 0.50, 0.52), vec3(0.72, 0.73, 0.74), inner);
    f_color = vec4(grey * v_color.rgb, alpha);
}
