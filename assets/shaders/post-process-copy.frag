#version 450

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D src_main;

layout(push_constant) uniform Params {
    vec2 direction;
    float bloom_intensity;
    float radius_pixels;
} params;

void main() {
    f_color = texture(src_main, v_uv);
}
