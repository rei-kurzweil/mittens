#version 450
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec4 v_color;
layout(location = 0) out vec4 f_color;
layout(set = 1, binding = 0, std140) uniform Inputs { vec3 tint; float gain; } material;
layout(set = 1, binding = 1) uniform sampler2D base_tex;
void main() {
    vec4 texel = texture(base_tex, v_uv);
    f_color = vec4(texel.rgb * v_color.rgb * material.tint * material.gain, texel.a * v_color.a);
}
