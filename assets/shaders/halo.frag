#version 450
layout(location=0) in vec3 v_world_pos;
layout(location=1) in vec3 v_normal;
layout(location=3) in vec4 v_color;
layout(location=0) out vec4 f_color;
layout(set=0,binding=0,std140) uniform CameraUBO {
    mat4 view; mat4 proj; mat3 camera2d; vec2 viewport; vec2 _pad0;
    vec3 ambient_light; uint renderer_flags;
} camera;
layout(set=1,binding=0,std140) uniform HaloInputs {
    float gain;
    float rim_strength;
    float rim_power;
    vec3 rim_color;
} halo;

void main() {
    // A closed ring needs only its front-facing shell for this alpha experiment.
    if (!gl_FrontFacing || v_color.a<=0.001) discard;
    vec3 position_vs=(camera.view*vec4(v_world_pos,1.0)).xyz;
    vec3 normal_vs=normalize(mat3(camera.view)*v_normal);
    vec3 view_direction=-position_vs/max(length(position_vs),0.0001);
    float facing=clamp(abs(dot(normal_vs,view_direction)),0.0,1.0);
    float rim=pow(1.0-facing,max(halo.rim_power,0.01));
    vec3 rim_color=mix(v_color.rgb,halo.rim_color,0.5);
    vec3 radiance=v_color.rgb*max(halo.gain,0.0)
        +rim_color*rim*max(halo.rim_strength,0.0);
    f_color=vec4(radiance,clamp(v_color.a,0.0,1.0));
}
