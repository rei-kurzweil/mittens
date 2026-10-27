#version 450
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform UiInputs { float brightness; } ui;
layout(set = 1, binding = 1) uniform sampler2D base_tex;

void main() {
    vec4 color = texture(base_tex, v_uv) * v_color;
    if (color.a <= 0.001) { discard; }
    f_color = vec4(color.rgb * ui.brightness, color.a);
}
