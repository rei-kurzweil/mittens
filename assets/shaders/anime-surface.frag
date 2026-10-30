#version 450
// Custom opaque anime-style surface with colored ambient/direct light.
layout(location=0) in vec3 v_world_pos;
layout(location=1) in vec3 v_normal;
layout(location=2) in vec2 v_uv;
layout(location=3) in vec4 v_color;
layout(location=0) out vec4 f_color;
layout(set=0,binding=0,std140) uniform CameraUBO {
    mat4 view; mat4 proj; mat3 camera2d; vec2 viewport; vec2 _pad0;
    vec3 ambient_light; uint renderer_flags;
} camera;
struct Light { vec4 pos_intensity; vec4 color_distance; vec4 direction_angle; uvec4 meta; };
layout(set=0,binding=1,std430) readonly buffer LightsSSBO {
    uint count; uint _pad0; uint _pad1; uint _pad2; Light lights[64];
} scene;
layout(set=1,binding=0,std140) uniform AnimeInputs {
    vec3 shade_color; float shade_strength;
    float shade_threshold; float lit_threshold;
    vec3 rim_color; float rim_strength; float rim_power;
} anime;
layout(set=1,binding=1) uniform sampler2D base_tex;
void main() {
    vec3 N=normalize(v_normal);
    vec3 illumination=max(camera.ambient_light,vec3(0));
    float facing=0.0;
    for(uint i=0u;i<min(scene.count,64u);i++) {
        Light light=scene.lights[i];
        vec3 delta=light.meta.x==2u ? light.pos_intensity.xyz : light.pos_intensity.xyz-v_world_pos;
        float distance_to_light=length(delta);
        if(distance_to_light<=0.00001) continue;
        vec3 L=delta/distance_to_light;
        float attenuation=1.0;
        if(light.meta.x!=2u) {
            float range=light.color_distance.w;
            float fade=clamp(1.0-distance_to_light/max(range,0.001),0.0,1.0);
            attenuation=range>0.001 ? fade*fade : 1.0/(1.0+distance_to_light*distance_to_light);
            if(light.meta.x==3u) {
                float outer=light.direction_angle.w;
                float inner=uintBitsToFloat(light.meta.y);
                attenuation*=smoothstep(outer,max(inner,outer+0.00001),dot(-L,normalize(light.direction_angle.xyz)));
            }
        }
        // Match the anime family's facing gate, rather than cosine shading.
        if(dot(N,L)>0.0) {
            float amount=max(light.pos_intensity.w,0.0)*attenuation;
            facing+=amount;
            illumination+=max(light.color_distance.rgb,vec3(0))*amount;
        }
    }
    vec3 base=texture(base_tex,v_uv).rgb*v_color.rgb;
    vec3 shade=mix(vec3(1),clamp(anime.shade_color,vec3(0),vec3(1)),clamp(anime.shade_strength,0.0,1.0));
    float ramp=anime.lit_threshold<=anime.shade_threshold ? step(anime.lit_threshold,facing)
        : smoothstep(anime.shade_threshold,anime.lit_threshold,facing);
    vec3 color=base*mix(shade,vec3(1),ramp)*clamp(illumination,vec3(0),vec3(1));
    vec3 view_pos=(camera.view*vec4(v_world_pos,1)).xyz;
    vec3 view_normal=normalize(mat3(camera.view)*N);
    float rim=pow(1.0-max(dot(view_normal,normalize(-view_pos)),0.0),max(anime.rim_power,0.01))*clamp(anime.rim_strength,0.0,1.0);
    f_color=vec4(min(base,color+clamp(anime.rim_color,vec3(0),vec3(1))*rim),1.0);
}
