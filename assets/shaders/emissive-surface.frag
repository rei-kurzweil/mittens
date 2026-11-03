#version 450
// Unshaded emitted radiance, independent of the ship's default surface material.
layout(location=2) in vec2 v_uv;
layout(location=3) in vec4 v_color;
layout(location=4) flat in float v_emissive;
layout(location=0) out vec4 f_color;
layout(set=1,binding=0,std140) uniform Emission { float gain; } emission;
layout(set=1,binding=1) uniform sampler2D base_tex;
void main() {
    vec4 base = texture(base_tex, v_uv) * v_color;
    if (base.a <= 0.001) discard;
    f_color = vec4(base.rgb * max(v_emissive, 0.0) * emission.gain, base.a);
}
