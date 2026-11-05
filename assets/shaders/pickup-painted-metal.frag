#version 450
// Painted metal with scene-light highlights, a bright rim and a view-space matcap.
layout(location=0) in vec3 v_world_pos;
layout(location=1) in vec3 v_normal;
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
layout(set=1,binding=0,std140) uniform PaintedMetal {
    float matcap_strength; float specular_strength; float shininess;
    float rim_strength; float rim_power; float exposure;
} metal;
layout(set=1,binding=1) uniform sampler2D matcap;

void main() {
    vec3 N=normalize(v_normal);
    vec3 view_position=(camera.view*vec4(v_world_pos,1.0)).xyz;
    vec3 Vview=normalize(-view_position);
    vec3 V=normalize(transpose(mat3(camera.view))*Vview);
    // The imported body is made of double-sided panels. Back-facing normals
    // otherwise produce a full-strength white rim across entire visible faces.
    if(dot(N,V)<0.0) N=-N;
    vec3 diffuse=max(camera.ambient_light,vec3(0.0));
    vec3 highlights=vec3(0.0);
    for(uint i=0u;i<min(scene.count,64u);i++) {
        Light light=scene.lights[i];
        vec3 delta=light.meta.x==2u ? light.pos_intensity.xyz : light.pos_intensity.xyz-v_world_pos;
        float distance_to_light=length(delta);
        if(distance_to_light<0.00001) continue;
        float range=light.color_distance.w;
        if(light.meta.x!=2u && range>0.001 && distance_to_light>=range) continue;
        vec3 L=delta/distance_to_light;
        float attenuation=1.0;
        if(light.meta.x!=2u) {
            float fade=max(0.0,1.0-distance_to_light/max(range,0.001));
            attenuation=range>0.001 ? fade*fade : 1.0/(1.0+distance_to_light*distance_to_light);
            if(light.meta.x==3u) {
                attenuation*=smoothstep(light.direction_angle.w,
                    max(uintBitsToFloat(light.meta.y),light.direction_angle.w+0.00001),
                    dot(-L,normalize(light.direction_angle.xyz)));
            }
        }
        float facing=max(dot(N,L),0.0);
        if(facing<=0.0) continue;
        vec3 radiance=max(light.color_distance.rgb,vec3(0.0))*max(light.pos_intensity.w,0.0)*attenuation;
        diffuse+=radiance*facing;
        float specular=pow(max(dot(N,normalize(V+L)),0.0),max(metal.shininess,1.0));
        highlights+=radiance*specular*metal.specular_strength;
    }
    vec3 Nview=normalize(mat3(camera.view)*N);
    vec2 matcap_uv=clamp(Nview.xy*vec2(0.5,-0.5)+0.5,vec2(0.001),vec2(0.999));
    vec3 reflection=texture(matcap,matcap_uv).rgb;
    float rim=pow(1.0-max(dot(Nview,Vview),0.0),max(metal.rim_power,0.01));
    vec3 paint=max(v_color.rgb,vec3(0.0));
    vec3 color=paint*diffuse + highlights
        + reflection*mix(vec3(0.55),paint,0.45)*metal.matcap_strength
        + vec3(0.90,0.94,1.0)*rim*metal.rim_strength;
    // Preserve highlight detail on the lit imported finish instead of clipping
    // the entire body white. Zero exposure keeps the older HDR preset unchanged.
    if(metal.exposure>0.0) color=vec3(1.0)-exp(-max(color,vec3(0.0))*metal.exposure);
    f_color=vec4(max(color,vec3(0.0)),v_color.a);
}
