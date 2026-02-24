Places we actually clone meshes

UV-bake clone for text glyphs (and any UV overrides): renderable_system.rs:81-113

This is the one you just hit: let mut mesh = render_assets.cpu_mesh(base_mesh)?.clone(); then render_assets.register_mesh(mesh).
GLTF import path clones mesh data when registering imported meshes: gltf_system.rs:328-334

render_assets.register_imported_mesh(..., m.mesh.clone())
Places we create new meshes (not clone, but still “new handle”)

Dynamic primitive creation helpers (these call MeshFactory::* and then register_mesh): renderable.rs:38-93

e.g. square_dynamic, cube_dynamic, etc.
Built-in mesh registration (creates meshes once at startup): render_assets.rs:49-83

MeshFactory::quad_2d() etc.
Where it is NOT

VisualWorld doesn’t clone CPU meshes; it only stores GPU handles/instance state and updates instance parameters.
RenderAssets stores the meshes and provides register_mesh, but it does not itself “clone”; cloning is done by callers (like RenderableSystem’s UV bake and GLTF system).