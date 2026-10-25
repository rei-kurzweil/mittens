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

const uint LIGHT_TYPE_POINT = 1u;
const uint LIGHT_TYPE_DIRECTIONAL = 2u;
const uint LIGHT_TYPE_SPOT = 3u;

struct Light {
    vec4 pos_intensity;
    vec4 color_distance;
    vec4 direction_angle;
    uvec4 meta;
};

layout(set = 0, binding = 1, std430) readonly buffer LightsSSBO {
    uint count;
    uint _pad0;
    uint _pad1;
    uint _pad2;
    Light lights[64];
} g_lights;


layout(set=1,binding=0,std140) uniform WaterInputs {
    float time_seconds;
    float wave_scale;
    float wave_strength;
    float specular_strength;
    float shininess;
} water;

// MeshSurfaceV1 world-space lighting. Return a surface-to-light direction and
// distance/cone-attenuated intensity; eligibility is tested by the material.
float light_influence(uint i, out vec3 L) {
    Light light=g_lights.lights[i];
    uint kind=light.meta.x;
    float attenuation=1.0;
    if(kind==2u) { L=normalize(light.pos_intensity.xyz); }
    else {
        vec3 delta=light.pos_intensity.xyz-v_world_pos;
        float distance_to_light=length(delta);
        L=delta/max(distance_to_light,0.00001);
        float range=light.color_distance.w;
        float fade=clamp(1.0-distance_to_light/max(range,0.001),0.0,1.0);
        attenuation=range>0.001 ? fade*fade : 1.0/(1.0+distance_to_light*distance_to_light);
        if(kind==3u) {
            float outer=light.direction_angle.w;
            float inner=uintBitsToFloat(light.meta.y);
            attenuation*=smoothstep(outer,max(inner,outer+0.00001),dot(-L,normalize(light.direction_angle.xyz)));
        }
    }
    return max(light.pos_intensity.w,0.0)*attenuation;
}
vec3 view_direction() {
    vec3 p=(ubo.view*vec4(v_world_pos,1.0)).xyz;
    return normalize(transpose(mat3(ubo.view))*(-p));
}

void main() {
    // Analytic normal ripples only: the geometry stays flat in this slice.
    vec2 q=v_world_pos.xz*water.wave_scale;
    float t=water.time_seconds;
    vec2 slope=vec2(cos(q.x+t*0.7)+0.45*cos(q.x*1.7+q.y-t*0.4),
        sin(q.y*1.2-t*0.6)+0.45*cos(q.x*1.7+q.y-t*0.4))*water.wave_strength;
    vec3 N=normalize(vec3(-slope.x,1.0,-slope.y));
    vec3 eye=view_direction();
    vec3 reflection=vec3(0.0);
    for(uint i=0u;i<min(g_lights.count,64u);i++) {
        vec3 L;float amount=light_influence(i,L);
        if(dot(N,L)>0.0) {
            float highlight=pow(max(dot(N,normalize(L+eye)),0.0),max(water.shininess,1.0));
            reflection+=g_lights.lights[i].color_distance.rgb*highlight*amount*water.specular_strength;
        }
    }
    float fresnel=0.02+0.98*pow(1.0-max(dot(N,eye),0.0),5.0);
    vec3 color=vec3(0.12,0.62,0.73)*v_color.rgb+reflection*(0.35+fresnel);
    f_color=vec4(color,v_color.a*(0.7+0.3*fresnel));
}
