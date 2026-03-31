#version 450

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D src_main;
layout(set = 0, binding = 1) uniform sampler2D src_bloom;

layout(push_constant) uniform Params {
    vec2 direction;
    float bloom_intensity;
    float radius_pixels;
} params;

void main() {
    vec4 main_color = texture(src_main, v_uv);
    vec4 bloom = texture(src_bloom, v_uv);
    f_color = vec4(main_color.rgb + bloom.rgb * max(params.bloom_intensity, 0.0), main_color.a);
}
