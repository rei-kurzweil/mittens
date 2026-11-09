import { halo_material } from "../materials/halo.mms"

// Attach this root to the head. The basis tilts local Y halfway toward the back (-Z).
// Ring meshes remain rigid children of joint transforms; no weights or skinning.
export fn concentric_halo(label, options) {
    if options==null {options={}}
    let opacity=0.70
    let gap=0.10
    let clearance=0.26
    let band_width=0.034
    let position=[0,0,-0.10]
    let rotation=[-0.7853981634,0,0]
    let response_scale=0.25
    let drag_force=0.55
    if options["opacity"]!=null {opacity=Math.clamp(options.opacity,0.0,1.0)}
    if options["ring_gap"]!=null {gap=options.ring_gap}
    if options["head_clearance"]!=null {clearance=options.head_clearance}
    if options["band_width"]!=null {band_width=options.band_width}
    if options["position"]!=null {position=options.position}
    if options["rotation"]!=null {rotation=options.rotation}
    if options["response_scale"]!=null {response_scale=options.response_scale}
    if response_scale<0.01 {response_scale=0.01}
    if options["drag_force"]!=null {drag_force=Math.clamp(options.drag_force,0.0,1.0)}
    let material=halo_material(options)
    let specs=[
        { radius=0.22 color=[1.0,0.16,0.22] stiffness=4.5 },
        { radius=0.32 color=[0.96,0.57,0.84] stiffness=3.5 },
    ]
    let joint_root=T {name=label+"_joint_root" Joint {}}
    let parent=joint_root
    for index in range(2) {
        let height=gap
        if index==0 {height=clearance}
        let spec=specs[index]
        let joint=T.position(0,height,0) {
            name=label+"_ring_"+index
            Joint {}
            // Four profile sides form a rectangular section. Y scaling makes
            // it a thin extruded annulus without changing radial band width.
            T.scale(1,0.4,1) {
                R.torus(spec.radius,band_width/Math.sqrt(2.0),64,4) {
                    name=label+"_mesh_"+index
                    material
                    C.rgba(spec.color[0],spec.color[1],spec.color[2],opacity)
                }
            }
        }
        parent.attach(joint)
        parent=joint
    }
    let rig=Armature.joints(joint_root).capture_rest_pose() {
        name=label+"_rig"
        SecondaryMotion {
            name=label+"_secondary_motion"
            SpringBone.new(label+"_spring").virtual_end_length_ratio(1.0)
                .allow_stretch(options["allow_stretch"]!=false)
                .world_collision(false).avatar_collision(false)
                .enabled(options["motion_enabled"]!=false) {
                name=label+"_spring"
                // The anchored root's spring moves the first ring; subsequent
                // joints move the next ring and the virtual tip.
                SpringJoint.new("[name='"+label+"_joint_root']")
                    .stiffness(6.0*response_scale).drag_force(drag_force)
                    .gravity(0.0,[0,-1,0])
                for index in range(2) {
                    SpringJoint.new("[name='"+label+"_ring_"+index+"']")
                        .stiffness(specs[index].stiffness*response_scale).drag_force(drag_force)
                        .gravity(0.0,[0,-1,0])
                }
            }
        }
    }
    return T.position(position[0],position[1],position[2])
        .rotation(rotation[0],rotation[1],rotation[2]) {
        name=label
        Raycastable.disabled()
        Selectable.off()
        joint_root
        rig
    }
}
