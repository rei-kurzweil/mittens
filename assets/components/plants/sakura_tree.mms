// The mitten example's three-way branching tree, with Noto 🌸 at every leaf.
// Crossed, double-sided cutout cards keep blossoms visible from XR viewpoints
// without per-frame billboarding or sorted transparent layers.
fn blossom_cluster(material, seed) {
    return T.rotation(0.0, seed * 0.37, seed * 0.21) {
        name = "sakura_leaf_cluster"
        for plane in range(3) {
            let pitch = 0.0
            let yaw = plane * 1.04719755
            if plane == 2 { pitch = 1.04719755 }
            T.rotation(pitch, yaw, plane * 0.31).scale(0.42, 0.42, 0.42) {
                R.square() {
                    material
                }
            }
        }
    }
}

fn branch(depth, length, width, wood, blossoms, seed, flowers) {
    return T {
        if !flowers {
            R.cylinder(width / 2.0, length, 10) {
                wood
                C.rgba(0.34, 0.21, 0.12, 1.0)
            }
        }
        if depth > 0 {
            T.position(0.0, length, 0.0).rotation(0.48, 0.0, 0.43) {
                branch(depth - 1, length * 0.68, width * 0.70, wood, blossoms, seed * 3 + 1, flowers)
            }
            T.position(0.0, length, 0.0).rotation(-0.40, 2.10, -0.46) {
                branch(depth - 1, length * 0.68, width * 0.70, wood, blossoms, seed * 3 + 2, flowers)
            }
            T.position(0.0, length, 0.0).rotation(0.38, -2.10, 0.34) {
                branch(depth - 1, length * 0.68, width * 0.70, wood, blossoms, seed * 3 + 3, flowers)
            }
        } else if flowers {
            T.position(0.0, length, 0.0) { blossom_cluster(blossoms, seed) }
        }
    }
}

export fn sakura_tree(depth, length, width) {
    let definition = MaterialDefinition.anime()
    let wood = Material(definition).inputs({
        shade_color = [0.42, 0.35, 0.43]
        shade_strength = 0.55
        rim_strength = 0.15
    })
    let blossoms = Material(definition).inputs({
        shade_color = [0.75, 0.58, 0.70]
        shade_strength = 0.30
        rim_color = [1.0, 0.90, 0.95]
        rim_strength = 0.18
    })
    // Four levels / 27 leaf nodes by default at depth=3. Bound recursion for XR.
    if depth < 0 { depth = 0 }
    if depth > 4 { depth = 4 }
    return T {
        CombineMesh {
            name = "sakura_tree_branches"
            branch(depth, length, width, wood, blossoms, 1, false)
        }
        CombineMesh {
            name = "sakura_tree_blossoms"
            Texture.with_uri("assets/textures/cherry-blossoms/sakura-blossom-256.png")
            TextureFiltering.linear()
            TransparentCutout {}
            branch(depth, length, width, wood, blossoms, 1, true)
        }
        T.position(0.0, length / 2.0, 0.0) {
            Zone.capsule_y(width / 2.0, length / 2.0 - width / 2.0) {
                Collidable.static().friction(0.6) {}
            }
        }
    }
}

// Explicit custom-material variant for the native Android renderer. The caller
// supplies an opaque wood shader and a blossom shader that discards its texture
// mask; no legacy material or TransparentCutout component is introduced.
export fn sakura_tree_custom(depth, length, width, wood, blossoms) {
    depth = Math.clamp(depth, 0, 4)
    return T {
        CombineMesh {
            name = "sakura_tree_branches"
            wood
            branch(depth, length, width, wood, blossoms, 1, false)
        }
        CombineMesh {
            name = "sakura_tree_blossoms"
            blossoms
            Texture.with_uri("assets/textures/cherry-blossoms/sakura-blossom-256.png")
            TextureFiltering.linear()
            branch(depth, length, width, wood, blossoms, 1, true)
        }
    }
}
