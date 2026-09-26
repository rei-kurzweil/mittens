# Mittens and MMS: working philosophy

## Start with what people see and author

- Mittens is a hardware accelerated runtime and game engine with a hypermedia influence. It serves many uses, with character driven VTubing as a central one.
- Judge features by their visual result and shape language, as well as by how they feel to author in MMS and the Rust API.
- Design the MMS surface first: the component tree, names, and calls are what creators repeatedly read and change.

## Build from coherent pieces

- Prefer reusable, composable behavior over overlapping special cases.
- Let the implementation support an expressive authoring surface without making its convenience expensive at runtime.
- Treat the hierarchy and relationships between components as part of the language, not just implementation plumbing.

## Let the intended pose matter

- Physical motion is useful when a character moves, but it is not always the best resting image.
- When a part settles, an intentionally authored silhouette can be more expressive than perpetual simulation. The head bow ribbon is one example.
- The runtime should make room for both responsive motion and deliberate, stylized poses, including poses chosen relative to gravity.
