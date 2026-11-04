"""Generate the pickup spring with headless Blender.

blender --background --factory-startup --python assets/models/generate-suspension-spring.py

Blender uses Z up; glTF export converts the downward Z axis to downward Y.
The object origin is the upper attachment, and the lower attachment is 0.5 m down.
"""

import json
import math
from pathlib import Path

import bmesh
import bpy
from mathutils import Vector


HEIGHT = 0.50
COIL_RADIUS = 0.075
WIRE_RADIUS = 0.012
TURNS = 6
STEPS_PER_TURN = 24
WIRE_SIDES = 8
OUTPUT = Path(__file__).resolve().parent


def build_spring():
    steps = TURNS * STEPS_PER_TURN
    travel = HEIGHT - 2 * WIRE_RADIUS
    vertices = []
    faces = []
    for step in range(steps + 1):
        t = step / steps
        angle = math.tau * TURNS * t
        radial = Vector((math.cos(angle), math.sin(angle), 0))
        tangent = Vector((
            -COIL_RADIUS * math.tau * TURNS * math.sin(angle),
            COIL_RADIUS * math.tau * TURNS * math.cos(angle),
            -travel,
        )).normalized()
        cross_section = tangent.cross(radial).normalized()
        center = COIL_RADIUS * radial + Vector((0, 0, -WIRE_RADIUS - travel * t))
        for side in range(WIRE_SIDES):
            phi = math.tau * side / WIRE_SIDES
            point = center + WIRE_RADIUS * (
                math.cos(phi) * radial + math.sin(phi) * cross_section
            )
            vertices.append(tuple(point))
    for step in range(steps):
        for side in range(WIRE_SIDES):
            a = step * WIRE_SIDES + side
            b = step * WIRE_SIDES + (side + 1) % WIRE_SIDES
            faces.append((a, b, b + WIRE_SIDES, a + WIRE_SIDES))
    faces.append(tuple(reversed(range(WIRE_SIDES))))
    faces.append(tuple(steps * WIRE_SIDES + side for side in range(WIRE_SIDES)))

    mesh = bpy.data.meshes.new("suspension_spring_mesh")
    mesh.from_pydata(vertices, [], faces)
    mesh.update()
    topology = bmesh.new()
    topology.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(topology, faces=list(topology.faces))
    assert all(edge.is_manifold for edge in topology.edges), "spring must be watertight"
    assert topology.calc_volume(signed=True) > 0, "spring normals must face outward"
    topology.to_mesh(mesh)
    topology.free()
    for polygon in mesh.polygons:
        polygon.use_smooth = len(polygon.vertices) == 4

    spring = bpy.data.objects.new("suspension_spring", mesh)
    bpy.context.collection.objects.link(spring)
    material = bpy.data.materials.new("spring_steel")
    material.diffuse_color = (0.30, 0.34, 0.37, 1)
    material.use_nodes = True
    shader = material.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Base Color"].default_value = material.diffuse_color
    shader.inputs["Metallic"].default_value = 0.8
    shader.inputs["Roughness"].default_value = 0.32
    mesh.materials.append(material)
    spring["rest_length_m"] = HEIGHT
    spring["coil_radius_m"] = COIL_RADIUS
    spring["wire_radius_m"] = WIRE_RADIUS
    spring["turns"] = TURNS
    return spring


def point_at(obj, position):
    obj.rotation_euler = (Vector(position) - obj.location).to_track_quat("-Z", "Y").to_euler()


def setup_preview(scene):
    camera_data = bpy.data.cameras.new("spring_preview_camera")
    camera = bpy.data.objects.new("spring_preview_camera", camera_data)
    scene.collection.objects.link(camera)
    camera.location = (0.85, -1.4, 0.2)
    point_at(camera, (0, 0, -HEIGHT / 2))
    camera_data.type = "ORTHO"
    camera_data.ortho_scale = 0.66
    scene.camera = camera
    for name, position, energy, size in [
        ("key", (0.6, -0.7, 0.5), 70, 0.7),
        ("fill", (-0.5, -0.3, -0.1), 35, 0.5),
        ("rim", (0.2, 0.5, 0.1), 100, 0.6),
    ]:
        data = bpy.data.lights.new(name, "AREA")
        data.energy = energy
        data.shape = "DISK"
        data.size = size
        light = bpy.data.objects.new(name, data)
        scene.collection.objects.link(light)
        light.location = position
        point_at(light, (0, 0, -HEIGHT / 2))
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 32
    scene.cycles.use_denoising = True
    scene.render.resolution_x = 512
    scene.render.resolution_y = 640
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.world.color = (0.12, 0.12, 0.12)
    scene.view_settings.view_transform = "AgX"


def main():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    spring = build_spring()
    bpy.context.view_layer.objects.active = spring
    spring.select_set(True)
    glb = OUTPUT / "suspension-spring.glb"
    bpy.ops.export_scene.gltf(
        filepath=str(glb), export_format="GLB", use_selection=True,
        export_yup=True, export_extras=True, export_animations=False,
        export_cameras=False, export_lights=False,
    )
    setup_preview(scene)
    # Keep the spring selected for convenient editing when opening the .blend.
    bpy.context.view_layer.objects.active = spring
    bpy.ops.wm.save_as_mainfile(filepath=str(OUTPUT / "blender" / "suspension-spring.blend"))
    scene.render.filepath = str(OUTPUT / "suspension-spring.preview.png")
    bpy.ops.render.render(write_still=True)
    spring.data.calc_loop_triangles()
    print(json.dumps({
        "glb": str(glb), "bytes": glb.stat().st_size,
        "vertices": len(spring.data.vertices),
        "triangles": len(spring.data.loop_triangles),
        "height_m": HEIGHT, "outer_diameter_m": 2 * (COIL_RADIUS + WIRE_RADIUS),
    }))


if __name__ == "__main__":
    main()
