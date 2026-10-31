#version 450
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform BlossomInputs { float brightness; } blossom;
layout(set = 1, binding = 1) uniform sampler2D base_tex;
void main() {
    vec4 flower = texture(base_tex, v_uv);
    if (flower.a < 0.5) { discard; }
    f_color = vec4(flower.rgb * v_color.rgb * blossom.brightness, 1.0);
}
