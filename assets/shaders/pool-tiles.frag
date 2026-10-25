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


// RGB contains tangent-space normal; A contains grayscale albedo. This packed
// material image is generated with the separate inspectable source maps in Rust.
layout(set=1,binding=1) uniform sampler2D tile_surface;
layout(set=1,binding=0,std140) uniform TileInputs {
    float tile_size;
    float normal_strength;
    float specular_strength;
    float shininess;
    float rim_strength;
    float shade_threshold;
    float lit_threshold;
} tile;

// MeshSurfaceV1 world-space lighting. Return a surface-to-light direction and
// distance/cone-attenuated intensity; eligibility is tested by the material.
float light_influence(uint i, out vec3 L, out float specular_amount) {
    Light light=g_lights.lights[i];
    uint kind=light.meta.x;
    float attenuation=1.0;
    float specular_attenuation=1.0;
    if(kind==2u) { L=normalize(light.pos_intensity.xyz); }
    else {
        vec3 delta=light.pos_intensity.xyz-v_world_pos;
        float distance_to_light=length(delta);
        L=delta/max(distance_to_light,0.00001);
        float range=light.color_distance.w;
        float fade=clamp(1.0-distance_to_light/max(range,0.001),0.0,1.0);
        attenuation=range>0.001 ? fade*fade : 1.0/(1.0+distance_to_light*distance_to_light);
        // Specular keeps its normal/view dependence further into the room.
        // Linear range fade is gentler than the squared Anime diffuse fade.
        specular_attenuation=range>0.001 ? fade : 1.0/(1.0+distance_to_light);
        if(kind==3u) {
            float outer=light.direction_angle.w;
            float inner=uintBitsToFloat(light.meta.y);
            float cone=smoothstep(outer,max(inner,outer+0.00001),dot(-L,normalize(light.direction_angle.xyz)));
            attenuation*=cone;
            specular_attenuation*=cone;
        }
    }
    specular_amount=max(light.pos_intensity.w,0.0)*specular_attenuation;
    return max(light.pos_intensity.w,0.0)*attenuation;
}
vec3 view_direction() {
    vec3 p=(ubo.view*vec4(v_world_pos,1.0)).xyz;
    return normalize(transpose(mat3(ubo.view))*(-p));
}

void main() {
    vec3 N=normalize(v_normal);
    vec3 U,V; vec2 uv;
    vec3 axis=abs(N);
    if(axis.y>=axis.x && axis.y>=axis.z) {U=vec3(1,0,0);V=vec3(0,0,1);uv=v_world_pos.xz;}
    else if(axis.x>=axis.z) {U=vec3(0,0,1);V=vec3(0,1,0);uv=v_world_pos.zy;}
    else {U=vec3(1,0,0);V=vec3(0,1,0);uv=v_world_pos.xy;}
    vec4 surface=texture(tile_surface,uv/max(tile.tile_size,0.01));
    vec3 bump=surface.rgb*2.0-1.0;
    N=normalize(N*max(bump.z,0.01)+(U*bump.x+V*bump.y)*tile.normal_strength);
    vec3 eye=view_direction();
    float light_amount=0.0;
    vec3 specular=vec3(0);
    float ceramic=smoothstep(0.80,0.93,surface.a);
    for(uint i=0u;i<min(g_lights.count,64u);i++) {
        vec3 L; float specular_amount;
        float amount=light_influence(i,L,specular_amount);
        float ndotl=max(dot(N,L),0.0);
        if(ndotl>0.0) {
            light_amount+=amount;
            vec3 H=normalize(L+eye);
            float highlight=pow(max(dot(N,H),0.0),max(tile.shininess,1.0));
            specular+=g_lights.lights[i].color_distance.rgb*highlight*ndotl*specular_amount*tile.specular_strength*ceramic;
        }
    }
    vec3 albedo=v_color.rgb*surface.a;
    float ramp=smoothstep(tile.shade_threshold,
        max(tile.lit_threshold,tile.shade_threshold+0.00001),light_amount);
    vec3 color=albedo*mix(0.48,1.0,ramp);
    float rim=pow(1.0-max(dot(N,eye),0.0),4.0)*tile.rim_strength;
    color=min(albedo,color+vec3(rim));
    // Direct reflected light may exceed albedo; only the Anime base/rim is capped.
    f_color=vec4(color+specular,v_color.a);
}
