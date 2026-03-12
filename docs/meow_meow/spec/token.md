# Tokens (v1)

This document defines:

1. The **lexical tokens** of Meow Meow Script (MMS).
2. The **component type shortforms** used in component expressions.

## 1) Lexical tokens

### Whitespace and comments

- Whitespace is ignored except as a separator.
- Line comment: `// ...` to end-of-line.
- Block comment: `/* ... */` (not nestable).

### Identifiers

- Identifiers are case-sensitive.
- Practically: `[A-Za-z_][A-Za-z0-9_]*` (matching current tokenizer behavior).

### Literals

- String: `"..."` with escapes: `\"`, `\\`, `\n`, `\r`, `\t`.
- Number: parsed as `f64`.
- Keywords: `true`, `false`, `null`.

### Punctuation

`{ } ( ) [ ] , . = ;`

### Keywords

Reserved words (cannot be used as identifiers):

- `let`, `if`, `else`, `return`, `new`, `true`, `false`, `null`

### Source of truth

The canonical Rust definitions live in:

- [src/meow_meow/token.rs](src/meow_meow/token.rs)

## 2) Component type shortforms

Component expressions start with a component type identifier:

```txt
T { TXT { "hi" } }
```

For ergonomics, MMS supports **shortforms** (aliases) for component type names.
Evaluation expands shortforms into canonical component type names before resolving them via the host component registry.

### Core shortforms (requested)

| Short | Canonical |
|------:|-----------|
| `I` | `Input` |
| `T` | `Transform` |
| `TF` | `TransformFilter` |
| `R` | `Renderable` |
| `C` | `Color` |
| `RC` | `Raycast` |
| `RCB` | `Raycastable` |
| `A` | `Animation` |
| `KF` | `Keyframe` |
| `AC` | `Action` |
| `BG` | `Background` |
| `OV` | `Overlay` |
| `OP` | `Opacity` |
| `BGC` | `BackgroundColor` |
| `TXT` | `Text` |
| `TXTR` | `Texture` |
| `C3D` | `Camera3D` |
| `C2D` | `Camera2D` |
| `PL` | `PointLight` |
| `DL` | `DirectionalLight` |
| `AL` | `AmbientLight` |
| `ED` | `Editor` |
| `GZM` | `Gizmo` |

### Proposed additions (based on current engine components)

These exist in `src/engine/ecs/component/` today and are likely useful in scripts.

| Short | Canonical |
|------:|-----------|
| `GLTF` | `GLTF` |
| `UV` | `UV` |
| `EM` | `Emissive` |
| `CK` | `Clock` |
| `PTR` | `Pointer` |
| `COL` | `Collision` |
| `COLS` | `CollisionShape` |
| `GVT` | `Gravity` |
| `KIN` | `KineticResponse` |
| `LQ` | `LightQuantization` |
| `TC` | `TransparentCutout` |
| `TS` | `TextShadow` |
| `SM` | `SkinnedMesh` |
| `ITM` | `InputTransformMode` |
| `MESH` | `Mesh` |
| `SRU` | `SignalRouteUpward` |
| `NOTE` | `MusicNote` |
| `TFILT` | `TextureFiltering` |

Audio (optional):

| Short | Canonical |
|------:|-----------|
| `AOUT` | `AudioOutput` |
| `AOSC` | `AudioOscillator` |
| `AG` | `AudioGain` |
| `AMIX` | `AudioMix` |
| `ALIM` | `AudioLimiter` |
| `ABUF` | `AudioBufferSize` |
| `ALPF` | `AudioLowPassFilter` |
| `AHPF` | `AudioHighPassFilter` |
| `ABPF` | `AudioBandPassFilter` |

XR (optional):

| Short | Canonical |
|------:|-----------|
| `XR` | `OpenXR` |
| `CXR` | `CameraXR` |
| `CCTL` | `ControllerXR` |

### Source of truth

The canonical mapping is a curated list in:

- [src/meow_meow/token.rs](src/meow_meow/token.rs)

### Collision rules

- Shortforms are case-sensitive.
- If an identifier is both a shortform and a canonical component name, **the canonical name wins** (recommended), or the host registry can resolve after expansion.
- If two shortforms collide, the mapping must be edited (no implicit shadowing).
