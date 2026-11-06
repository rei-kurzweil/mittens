#version 450
// Standard Surface vertex interface; no Grid pipeline or lighting UBO required.
layout(location=0) in vec3 v_world_pos;
layout(location=1) in vec3 v_normal;
layout(location=2) in vec2 v_uv;
layout(location=3) in vec4 v_color;
layout(location=0) out vec4 f_color;
layout(set=1,binding=0,std140) uniform GridInputs {
    float spacing;
    float world_space;
    vec2 local_extent;
    float minor_width;
    float major_width;
    float axis_width;
    float fade_start;
    float fade_end;
} grid;

float axis_alpha(float coordinate, float width) {
    return 1.0-smoothstep(0.0,max(fwidth(coordinate),1e-6)*max(width,0.01),abs(coordinate));
}
float line_alpha(vec2 position, float spacing, float width) {
    vec2 cells=position/max(spacing,1e-5);
    vec2 deriv=max(fwidth(cells),vec2(1e-6));
    vec2 distance=abs(fract(cells-0.5)-0.5)/(deriv*max(width,0.01));
    return 1.0-min(min(distance.x,distance.y),1.0);
}
void main() {
    vec2 position=(v_uv-0.5)*max(abs(grid.local_extent),vec2(1e-5));
    if(grid.world_space>0.5) {
        // Match the existing dominant-plane selection, including its tie order.
        vec3 alignment=abs(normalize(v_normal));
        int first=alignment.x<=alignment.y && alignment.x<=alignment.z ? 0
            : (alignment.y<=alignment.z ? 1 : 2);
        int second;
        if(first==0) second=alignment.y<=alignment.z ? 1 : 2;
        else if(first==1) second=alignment.x<=alignment.z ? 0 : 2;
        else second=alignment.x<=alignment.y ? 0 : 1;
        position=vec2(v_world_pos[first],v_world_pos[second]);
    }
    float spacing=max(abs(grid.spacing),1e-5);
    float minor=line_alpha(position,spacing,grid.minor_width);
    float major=line_alpha(position,spacing*8.0,grid.major_width);
    float axis=max(axis_alpha(position.x,grid.axis_width),axis_alpha(position.y,grid.axis_width*1.5));
    float fade=1.0-smoothstep(grid.fade_start,max(grid.fade_end,grid.fade_start+1e-4),length(position));
    float alpha=max(max(minor*0.45,major*0.75),axis)*v_color.a*fade;
    if(alpha<=0.001) discard;
    vec3 albedo=mix(v_color.rgb,vec3(minor*0.45),0.5);
    f_color=vec4(albedo,alpha);
}
