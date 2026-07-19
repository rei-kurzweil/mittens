# Cat Engine Components

This document provides a comprehensive inventory of all built-in components in cat-engine.

## Transforms & Scene Graph

- **TransformComponent**
  - Stores local TRS (translation, rotation, scale) and recomputes `matrix_world` for the scene graph.
- **TransformParentComponent**
  - Allows overriding or managing parent-child transform relationships.
- **TransformPipelineComponent**
  - Advanced transform derivation system supporting `Fork`, `Merge`, `SampleAncestor`, and `Drop`.
- **TransformPipelineMapComponent**
  - Mapping functions for rotation, scale, and translation within a pipeline.
- **TransformTemporalFilterComponent**
  - Smoothing filters for position and rotation (e.g., `QuatTemporalFilter`, `Vector3TemporalFilter`).

## Rendering

- **RenderableComponent**
  - Links a mesh and material for 3D/2D rendering.
- **MeshComponent**
  - Stores CPU-side geometry data before GPU upload.
- **ColorComponent**
  - Per-instance RGBA tint multiplier.
- **OpacityComponent**
  - Per-instance opacity. Determines if an object uses opaque or transparent render passes.
- **UVComponent**
  - Supplies UV coordinates and tiling/offset data for shaders.
- **TextureComponent**
  - Loads and caches textures by URI.
- **GLTFComponent**
  - Loads glTF 2.0 models and spawns appropriate child hierarchies.
- **SkinnedMeshComponent**
  - Enables skeletal animation by linking meshes to bones.
- **BackgroundComponent** / **BackgroundColorComponent**
  - specialized rendering for environment backgrounds or solid clear colors.
- **BloomComponent** / **BlurPassComponent**
  - Post-processing effects for glow and blurring.
- **EmissiveComponent**
  - Configures emissive/glow properties for materials.
- **TransparentCutoutComponent**
  - Enables alpha-testing (cutout) for transparent textures like foliage.
- **StencilClipComponent**
  - Provides stencil-based masking for UI or portals.

## Lighting

- **PointLightComponent**
  - Omnidirectional light source with range and intensity.
- **DirectionalLightComponent**
  - Infinite parallel light source (e.g., sun).
- **SpotLightComponent**
  - Local cone light with range, angle, edge softness, intensity, and transform-driven direction.
- **AmbientLightComponent**
  - Base global illumination color.
- **LightQuantizationComponent**
  - Controls toon-shading steps for cel-shaded visuals.

## Input & Control

- **InputComponent**
  - Primary sink for keyboard and mouse events.
- **InputTransformModeComponent**
  - Maps input events to movement/rotation (e.g., FPS controls, WASD).
- **PointerComponent**
  - Represents a mouse cursor or virtual pointer in 3D space.
- **TextInputComponent**
  - Handles text entry, cursor position, and selection in text fields.
- **GestureCoordTypeComponent**
  - Defines how drag interactions map to coordinates (WorldPlane, ScreenSpace, etc.).

## Camera

- **Camera2DComponent**
  - Orthographic camera for 2D UI and games.
- **Camera3DComponent**
  - Perspective camera for 3D environments.
- **CameraXRComponent**
  - Stereoscopic camera used in VR/AR contexts.

## XR (Extended Reality)

- **OpenXRComponent**
  - Integrates the OpenXR lifecycle into the engine.
- **InputXRComponent** / **ControllerXRComponent**
  - Manages VR controller poses, buttons, and haptics.

## Physics & Collision

- **CollisionComponent**
  - Marks a transform as a physical body (STATIC, KINEMATIC, RIGGED).
- **CollisionShapeComponent**
  - Defines cube, sphere, or upright `CapsuleY` geometry used for collision detection.
- **CollisionResponseComponent**
  - Automatically resolves overlaps by pushing the body out of collisions.
- **GravityComponent**
  - Defines a local gravity field that affects children in its subtree.
- **BoundsComponent** / **LayoutBoundsComponent**
  - Tracks axis-aligned bounding boxes for rendering or layout logic.
- **FitBoundsComponent**
  - Dynamically scales or positions a subtree to fit within specified bounds.

## UI & Layout

- **LayoutComponent**
  - Implements a flexbox-like layout system for nesting and alignment.
- **StyleComponent**
  - Stores CSS-like visual properties (margins, padding, display, justify).
- **TextComponent**
  - Renders dynamic text strings with support for fonts and alignment.
- **HtmlElementComponent**
  - Maps semantic HTML tags (e.g., `div`, `span`) to engine behaviors.
- **ScrollingComponent**
  - Enables scrollable regions with clipping and inertia.
- **OverlayComponent**
  - Forces a subtree to render on top of the main scene.

## Audio

- **AudioClipComponent**
  - References and plays audio files.
- **AudioOutputComponent** / **AudioMixComponent**
  - Nodes in the audio graph for output and sub-mixing.
- **AudioOscillatorComponent**
  - Generates synthesized waveforms (Sine, Square, Saw).
- **MusicContextComponent** / **MusicNoteComponent**
  - High-level musical orchestration (tempo, scales, note triggers).
- **AudioFilterComponents**
  - Real-time filters: `LowPass`, `HighPass`, `BandPass`, `Gain`, `Limiter`.

## Signals & Logic

- **ActionComponent**
  - Encapsulates discrete executable actions or script triggers.
- **RouterComponent** / **SignalObserverRouterComponent**
  - Manages event propagation and scoped signal handling.
- **DataComponent**
  - General-purpose key-value storage (numbers, strings, booleans).
- **ClockComponent**
  - Provides a centralized time source for animations and periodic events.
- **SerializeComponent**
  - Flags whether a component subtree should be saved/loaded.
- **ComponentRef**
  - A utility component that holds a reference to another `ComponentId`.

## Animation

- **AnimationComponent**
  - Animates properties over time using curves or keyframes.
- **KeyframeComponent**
  - Stores a sequence of timed values for complex animations.
- **IKChainComponent**
  - Implements Inverse Kinematics for procedural limb posing.

## Editor & Tools

- **EditorComponent**
  - General marker for components used exclusively by the editor.
- **GizmoComponent**
  - Visual handles for translating, rotating, and scaling objects.
- **SelectionComponent** / **SelectableComponent**
  - Manages the visual and logical state of selected objects.
- **GridComponent**
  - Renders a reference grid in the 3D viewport.

## Scripts (Planned)

- **MeowMeowComponent**
  - A planned component to encapsulate state and logic defined entirely within `.mms` scripts.
