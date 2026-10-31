// Notes for examples/material-shader-flowchart.mms. Desktop/Vulkano, 2026-10-31.
// Proposed names are displayed strings; this module does not register components.
export fn material_shader_notes() {
    return {
        material_base = {
            title = "MaterialBase"
            status = "EXISTS / renderer contract"
            summary = "Geometry interface and available renderer resources."
            detail = "The base describes the interface a material program is allowed to use. The current choices are the ordinary surface, extracted emissive surface, and planar reflective surface. Reflection adds renderer-owned capture resources; extracted emission changes the extraction contract."
            contract = "A base is shared, read-only configuration. It is not a material instance and does not contain per-object touch state. Current custom definitions inherit the engine's static surface vertex stage."
            example = "MaterialBase()\nMaterialBase.extracted_emissive()\nMaterialBase.planar_reflective()"
            source = "src/engine/graphics/materials.rs: MaterialBaseKind / MaterialBase\nsrc/engine/ecs/component/material.rs"
        }
        material_definition = {
            title = "MaterialDefinition"
            status = "EXISTS / shader and input schema"
            summary = "Base + fragment shader + typed input names."
            detail = "A custom definition references a base, a GLSL fragment file, and a record of named input types. The host accepts f32, vec2, vec3 and vec4 fields. Built-in anime()/toon() constructors instead reference shared engine definitions with their defaults and static/deformed programs."
            contract = "Custom shader preparation happens once per definition. The current API has .fragment(...), not an authored .vertex(...) API. Custom definitions currently register deformed_program = None."
            example = "import { f32, vec3 } from \"types\"\nlet definition = MaterialDefinition(MaterialBase())\n    .fragment(\"assets/shaders/plastic.frag\")\n    .inputs({ gain: f32 tint: vec3 })\n// Illustrative schema: it must match the actual fragment file."
            source = "src/engine/graphics/materials.rs: MaterialDefinition / register_custom_definition\nsrc/engine/graphics/custom_material.rs"
        }
        material_instance = {
            title = "Material"
            status = "EXISTS / shared parameter values"
            summary = "Values follow the definition's typed schema."
            detail = "Material(definition).inputs(...) creates an instance with named values. Attaching the same Material to several renderables shares its identity and values. Separate Material objects can use the same definition with independent values. set_input updates the instance revision."
            contract = "The type record declares the value schema. It does not request an arbitrary collection of Vulkan descriptor bindings. Custom GLSL reflection supplies the actual byte offsets used when packing these values."
            example = "let material = Material(definition).inputs({\n    gain = 1.0\n    tint = [0.7, 0.8, 1.0]\n})\nmaterial.set_input(\"gain\", 0.8)"
            source = "src/engine/graphics/materials.rs: MaterialInstance / upsert_source\nsrc/scripting/runtime_config.rs"
        }
        renderable = {
            title = "Renderable"
            status = "EXISTS / geometry consumer"
            summary = "Mesh + placement + resolved material identity."
            detail = "A renderable supplies mesh geometry, placement and standard instance inputs. The common material resolver finds its nearest declaration or its imported GLTF material scope. The renderer retains material identity alongside the mesh and instance data."
            contract = "Appearance values can be shared while transforms and geometry state belong to individual render instances. A proposed Squishable receiver belongs on this geometry side. Changing a shared material value must not accidentally share a dent between objects."
            example = "R.cube() { material }\n// Attaching the same material shares its values.\n// Squishable below is a proposal, not a live constructor."
            source = "src/engine/ecs/system/material_system.rs: resolve_surface / program\nsrc/engine/ecs/system/renderable_system.rs"
        }
        schema_values = {
            title = "Typed schema + live values"
            status = "EXISTS / MMS host to renderer"
            summary = "Field names / types, followed by instance values."
            detail = "The host consumes the definition's type record and validates instance values against it. Logical values are stored as scalar slots. The shader's declared uniform block has its own GPU alignment and padding, so logical slots are not treated as raw byte offsets."
            contract = "Only supported numeric field types are accepted. The authored fragment uniform block must match the schema by field name and exact type. Matrices, nested records, contact arrays and custom vertex parameters are not part of the current custom material input API."
            example = "Schema: { gain: f32 tint: vec3 }\nValues: { gain = 1.0 tint = [0.7, 0.8, 1.0] }"
            source = "src/engine/graphics/materials.rs: MaterialParameterSchema\nsrc/scripting/runtime_config.rs"
        }
        reflection = {
            title = "Compile / reflect / pack"
            status = "EXISTS / custom fragment preparation"
            summary = "Validate GLSL fields and use reflected byte offsets."
            detail = "The custom fragment shader is compiled to SPIR-V. Reflection validates supported descriptors, field names and numeric types, then records the uniform block's physical offsets. Material values are copied into those offsets, including required zeroed padding."
            contract = "Schema validation and descriptor compatibility are separate from geometry selection. Recompilation is not required for an ordinary value edit. The pipeline and material snapshots are retained and reused."
            example = "layout(set = 1, binding = 0) uniform Inputs {\n    float gain;\n    vec3 tint;\n} material;"
            source = "src/engine/graphics/custom_material.rs: compile_source / reflect / pack\nsrc/engine/graphics/vulkano_custom_materials.rs"
        }
        descriptors = {
            title = "Material descriptor set"
            status = "EXISTS / graphics set 1"
            summary = "Numeric uniform buffer + base texture binding."
            detail = "The engine uses a supported material descriptor layout: set 1 binding 0 holds numeric parameters, and binding 1 supplies the base texture/sampler. Set 0 holds camera and renderer resources. Set 2 holds object/rig resources, including the deformation cache."
            contract = "The MMS schema determines values and packing within the material buffer, not arbitrary descriptor layout creation. On desktop the material numeric binding is currently visible to the fragment stage. Retained descriptor snapshots preserve older recorded revisions."
            example = "set 0: camera / lights / renderer captures\nset 1: material numbers / base texture\nset 2: object lighting / deformation cache"
            source = "src/engine/graphics/pipeline_descriptor_set_layouts.rs\nsrc/engine/graphics/vulkano_material_instances.rs\nsrc/engine/graphics/vulkano_custom_materials.rs"
        }
        custom_pipeline = {
            title = "Custom pipeline today"
            status = "EXISTS / static geometry only"
            summary = "Engine vertex stage paired with your fragment stage."
            detail = "prepare_custom_programs clones the ordinary surface pipeline template and replaces stages[1], the fragment shader. The inherited vertex stage is toon-mesh.vert. Its name is historical: it transforms geometry and produces surface varyings; the custom fragment shader supplies the appearance."
            contract = "Custom definitions currently have no deformed_program. MaterialSystem::program reports an unavailable cached-deformation program for that geometry. The proposed custom cache bridge is not implemented."
            example = "Current custom pair:\n  toon-mesh.vert (engine)\n  your-material.frag (authored)"
            source = "src/engine/graphics/vulkano_custom_materials.rs: prepare_custom_programs\nsrc/engine/graphics/materials.rs: register_custom_definition\nsrc/engine/ecs/system/material_system.rs: program"
        }
        mesh = {
            title = "Static mesh + instance"
            status = "EXISTS / graphics inputs"
            summary = "Positions / normals / UVs, model and camera."
            detail = "The mesh stream supplies positions, normals and UVs. The instance stream supplies placement, color and other standard inputs. Camera data comes from the renderer's global resources."
            contract = "A material definition is not the owner of the mesh, bone pose or a touch contact. Mesh instances may share immutable geometry while keeping independent placement and deformation output."
            example = "Mesh vertices + instance model matrix\n+ camera view/projection\n+ color / opacity / UV inputs"
            source = "assets/shaders/toon-mesh.vert\nsrc/engine/graphics/visual_world.rs: VisualInstance"
        }
        surface_vertex = {
            title = "Surface vertex shader"
            status = "EXISTS / engine-owned vertex stage"
            summary = "Mesh to clip position and interpolated surface inputs."
            detail = "toon-mesh.vert transforms the static vertex into world and clip space, transforms its normal, and forwards UVs, instance color and standard inputs. Rasterization interpolates the ordinary surface outputs for the fragment stage."
            contract = "This stage does not execute the authored custom material shading. Its output interface must agree with the fragment shader. A cached vertex variant can provide the same interface from already-deformed positions and normals."
            example = "position -> model -> view -> projection\noutputs: world position / normal / UV / color"
            source = "assets/shaders/toon-mesh.vert\nassets/shaders/cached-skinned-toon-mesh.vert"
        }
        fragment = {
            title = "Material fragment shader"
            status = "EXISTS / authored appearance"
            summary = "Surface inputs + material values + renderer resources."
            detail = "Your .frag program reads interpolated surface inputs and the supported material/renderer bindings, then computes fragment output. This is where custom color, lighting, textures and effects are authored in the generic material path."
            contract = "The fragment stage consumes geometry; it cannot move the mesh silhouette. The same compatible fragment interface could be supplied by a static or cached-deformation vertex stage. That custom cached pairing still needs implementation."
            example = "v_world_pos / v_normal / v_uv / v_color\n+ material uniform values\n+ supported textures / camera / lights"
            source = "src/engine/graphics/custom_material.rs\nassets/shaders/plastic.frag\nassets/shaders/pool-water.frag"
        }
        output = {
            title = "Render target"
            status = "EXISTS / draw and view output"
            summary = "Depth / blend tests and the selected view's attachments."
            detail = "Rasterized fragment output participates in the selected pipeline's depth and blend behavior. A render target may be the window, an XR eye, or a renderer capture. Bases such as planar reflective or extracted emissive request specific resource/pass contracts."
            contract = "A graphics pipeline joins vertex and fragment stages with descriptor layouts and render state. It is retained per program/state, not rebuilt because a material number or a touch contact changed."
            example = "Compatible vertex + fragment + layouts + render state\n-> color/depth attachments for the current view"
            source = "src/engine/graphics/vulkano_renderer.rs\nsrc/engine/graphics/vulkano_cbb.rs"
        }
        deformation_inputs = {
            title = "Morphs + bone pose"
            status = "EXISTS / optional deformation inputs"
            summary = "Base mesh, active morph deltas and skin matrices."
            detail = "The current compute path adds active morph deltas before skinning. Bones, morph factors and output allocations have dirty tracking. Static meshes normally use the direct vertex path; morph-only meshes use identity skinning."
            contract = "Morphs and skinning are independent inputs to the existing deformation computation. The proposed squish operation would not require authored morph targets. A static squishable mesh would need admission to this cached path too."
            example = "base mesh -> optional morphs -> optional skinning"
            source = "assets/shaders/mesh-deformation.comp\nsrc/engine/graphics/deformation.rs\nsrc/engine/ecs/system/skinned_mesh_system.rs"
        }
        compute = {
            title = "Deformation compute"
            status = "EXISTS / morphs then skinning"
            summary = "Compute positions/normals once for a dirty generation."
            detail = "mesh-deformation.comp operates per vertex, applies morphs followed by skinning, and writes mesh-local positions with packed normals to a retained GPU cache. Multiple views reuse the same generation."
            contract = "Current compute contains morphing and skinning only. The gold squish-operation card is a proposed addition after skinning and before output. It must update normals and preserve batched dispatch and unchanged-generation reuse."
            example = "Current: morphs -> skinning -> cache\nProposed: morphs -> skinning -> squish -> cache"
            source = "assets/shaders/mesh-deformation.comp\nsrc/engine/graphics/vulkano_renderer.rs: record_dirty_deformations"
        }
        cached_vertex = {
            title = "Cached surface vertex"
            status = "EXISTS / used by built-in deformation programs"
            summary = "Read cached geometry; emit the same surface interface."
            detail = "cached-skinned-toon-mesh.vert reads the instance's cached mesh-local position/normal using i_deformed_base and gl_VertexIndex. It then applies model/view/projection and emits the same surface inputs as the direct static vertex shader."
            contract = "Despite the skinned/toon filename, the vertex stage is a geometry reader. Built-in Toon/Anime definitions already have cached programs. Custom fragment definitions currently lack this pairing."
            example = "set 2 binding 1: cached positions / packed normals\n-> world position / normal / UV / color"
            source = "assets/shaders/cached-skinned-toon-mesh.vert\nsrc/engine/graphics/materials.rs: built-in deformed_program entries"
        }
        custom_bridge = {
            title = "Custom cached pairing"
            status = "PLANNED / missing integration"
            planned = true
            summary = "Pair cached vertex output with the same custom fragment."
            detail = "Prepare a compatible cached-geometry pipeline for custom definitions, retaining the same fragment shader, parameter schema and Material identity. Geometry selects static versus cached input while the base retains its resource contract."
            contract = "This is not implemented. We must validate varying compatibility and every supported view/pass. No new SquishableSurface base or separately authored squish fragment shader is proposed. Android's new renderer is a separate static custom-surface slice."
            example = "Proposed custom pair:\n  cached surface vertex (engine)\n  the same your-material.frag\n  the same Material parameter values"
            source = "Integration points: materials.rs / vulkano_custom_materials.rs\nmaterial_system.rs / renderable_system.rs"
        }
        collidable = {
            title = "Collidable + Zone"
            status = "EXISTS / candidate contact geometry"
            summary = "Enabled sphere, capsule or box with a current pose."
            detail = "Zone describes invisible shape geometry and placement; Collidable opts a zone into collision behavior. These existing shapes can supply a proposed touch-deformation receiver with contact geometry, including tracked fingertip and palm probes."
            contract = "There is no render-mesh deformation connection today. Broad-phase bounds should only select candidates; actual shape tests determine whether vertices are affected. The current static-contact solver's movement response is a separate consumer."
            example = "Zone.sphere(0.02) { Collidable.static() {} }\n// Existing shape syntax; deformation consumption is proposed."
            source = "src/engine/ecs/component/zone.rs\nsrc/engine/ecs/component/collidable.rs\nsrc/engine/ecs/system/collision_geometry.rs"
        }
        squishable = {
            title = "Squishable receiver"
            status = "PLANNED / geometry opt-in"
            planned = true
            summary = "Per-renderable limits, painted weights and contact list."
            detail = "Mark selected receiving geometry as squishable. A shared system would gather nearby enabled contact shapes and publish a bounded per-instance contact list. A capacity of up to 16 is proposed; the active count would usually be zero or one."
            contract = "Squishable is not a live MMS component yet. Touch state belongs to the receiving instance, independent of shared Material values. Painted vertex weights are separate from joint weights and need an explicit mesh import/authoring contract."
            example = "Proposed data, not executable MMS:\n  enabled / maximum indentation\n  optional painted vertex weights\n  contact offset + active count"
            source = "Proposal from the climbing/material discussion / 2026-10-31\nExisting foundations: Zone, Collidable, mesh deformation cache"
        }
        squish_compute = {
            title = "Touch deformation step"
            status = "PLANNED / after skinning, before cache output"
            planned = true
            summary = "Contact shapes + weights produce local dents and normals."
            detail = "For each affected vertex, compare the animated surface with active touch shapes in a consistent coordinate frame. Apply smooth local deformation, weighted and capped by the receiver's limits. Correct normals along with positions."
            contract = "This would extend the existing compute operation rather than add a squish-specific surface vertex shader. Static meshes would need cached-path support. Bounds, release behavior and CPU collision/raycast proxies need explicit handling; GPU deformation does not update them automatically."
            example = "morphs -> skinning -> TOUCH SQUISH -> output cache\nNo authored morph target is needed for touch squishing."
            source = "Proposed extension: assets/shaders/mesh-deformation.comp\nGPU contact data + per-instance dirtiness remain to implement"
        }
    }
}
