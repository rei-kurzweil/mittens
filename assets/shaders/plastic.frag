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


// Shared definition; outer and inner shells supply independent instance values.
layout(set=1,binding=0,std140) uniform PlasticInputs {
    float diffuse_scale;
    float specular_scale;
    float ambient_strength;
    float shininess;
    float rim_strength;
    float normal_sign;
} plastic;

float light_influence(uint i, out vec3 L) {
    Light light=g_lights.lights[i];
    float attenuation=1.0;
    if(light.meta.x==LIGHT_TYPE_DIRECTIONAL) {
        L=normalize(light.pos_intensity.xyz);
    } else {
        vec3 delta=light.pos_intensity.xyz-v_world_pos;
        float distance_to_light=length(delta);
        L=delta/max(distance_to_light,0.00001);
        float range=light.color_distance.w;
        float fade=clamp(1.0-distance_to_light/max(range,0.001),0.0,1.0);
        // A gentle range fade keeps the plastic's curved lighting readable.
        attenuation=range>0.001 ? fade : 1.0/(1.0+distance_to_light*distance_to_light);
        if(light.meta.x==LIGHT_TYPE_SPOT) {
            float outer=light.direction_angle.w;
            float inner=uintBitsToFloat(light.meta.y);
            attenuation*=smoothstep(outer,max(inner,outer+0.00001),
                dot(-L,normalize(light.direction_angle.xyz)));
        }
    }
    return max(light.pos_intensity.w,0.0)*attenuation;
}

void main() {
    vec3 N=normalize(v_normal)*(plastic.normal_sign<0.0 ? -1.0 : 1.0);
    vec3 view_position=(ubo.view*vec4(v_world_pos,1.0)).xyz;
    vec3 V=normalize(transpose(mat3(ubo.view))*(-view_position));
    // The common mesh pipeline is two-sided. Reject the unlit reverse surface
    // so the outer wall cannot hide the separately shaded inward-facing liner.
    if(dot(N,V)<=0.0) discard;
    vec3 irradiance=vec3(0.0);
    vec3 specular=vec3(0.0);
    float power=max(plastic.shininess,1.0);
    for(uint i=0u;i<min(g_lights.count,64u);i++) {
        vec3 L;
        float amount=light_influence(i,L);
        float ndotl=max(dot(N,L),0.0);
        if(ndotl>0.0 && amount>0.0) {
            vec3 radiance=g_lights.lights[i].color_distance.rgb*amount;
            irradiance+=radiance*ndotl;
            vec3 H=normalize(L+V);
            // Dielectric white highlight, with a Fresnel increase at grazing
            // angles. Broad smooth normal-based diffuse defines the tube shape.
            float fresnel=0.04+0.96*pow(1.0-clamp(dot(V,H),0.0,1.0),5.0);
            float highlight=pow(max(dot(N,H),0.0),power)*(power+8.0)/(8.0*3.14159265);
            specular+=radiance*ndotl*highlight*fresnel;
        }
    }
    float ambient=clamp(plastic.ambient_strength,0.0,1.0);
    vec3 diffuse=v_color.rgb*(ambient+(1.0-ambient)*(vec3(1.0)-exp(-irradiance*0.65)));
    float rim=pow(1.0-clamp(dot(N,V),0.0,1.0),2.5)*max(plastic.rim_strength,0.0);
    diffuse=min(v_color.rgb,diffuse+v_color.rgb*rim);
    // Scale the complete diffuse/ambient/rim and specular terms independently.
    // Specular remains additive and may exceed the hand-authored albedo.
    f_color=vec4(diffuse*max(plastic.diffuse_scale,0.0)
        +specular*max(plastic.specular_scale,0.0),v_color.a);
}
