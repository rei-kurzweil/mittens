#version 450
layout(location=0) in vec3 v_world_pos;
layout(location=1) in vec3 v_normal;
layout(location=2) in vec2 v_uv;
layout(location=3) in vec4 v_color;
layout(location=0) out vec4 f_color;
layout(set = 0, binding = 0) uniform CameraUBO {
    mat4 view;
    mat4 proj;
    mat3 camera2d;
    vec2 viewport;
    vec2 _pad0;
    vec3 ambient_light;
    uint renderer_flags;
} ubo;


layout(set=1,binding=1) uniform sampler2D matcap;
layout(set=1,binding=0,std140) uniform MetalInputs {float brightness;} metal;
void main() {
    vec3 normal=normalize(mat3(ubo.view)*normalize(v_normal));
    vec2 uv=clamp(normal.xy*vec2(0.5,-0.5)+0.5,vec2(0.001),vec2(0.999));
    vec3 reflection=texture(matcap,uv).rgb;
    f_color=vec4(reflection*metal.brightness*v_color.rgb,v_color.a);
}
