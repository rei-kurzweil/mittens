#version 450
// Binary font-atlas coverage for the existing opaque mobile pass. Thresholding
// keeps dark glyphs crisp against cream instead of filling filtered texel halos.
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform UiInputs { float brightness; } ui;
layout(set = 1, binding = 1) uniform sampler2D base_tex;
void main() {
    vec4 texel = texture(base_tex, v_uv);
    if (texel.a * v_color.a < 0.5) { discard; }
    f_color = vec4(v_color.rgb * ui.brightness, 1.0);
}
