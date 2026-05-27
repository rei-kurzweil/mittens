#version 450

layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 4) flat in float v_emissive;

layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 1) uniform sampler2D base_tex;

void main() {
    vec4 base_rgba = texture(base_tex, v_uv) * v_color;
    if (base_rgba.a <= 0.001) {
        discard;
    }

    float intensity = max(v_emissive, 0.0);
    f_color = vec4(base_rgba.rgb * intensity, base_rgba.a);
}
