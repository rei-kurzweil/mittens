# meow meow script

This folder is the start of **meow meow script**: an imperative, object-oriented-ish scripting language that also embeds a **declarative component tree/graph syntax** for cat-engine.

The intent is to eventually replace (or deprecate) JSON component graphs for authoring content, because scripts can express things that are awkward in pure data.

## Core idea: component expressions

A component expression has three kinds of nodes (plus one sugar bucket):

- **Parameters**: named attributes on the component header, e.g. `name="dialog-continue"`, `guid="..."`.
- **Function calls**: method-like calls that would be invoked during component creation in Rust, e.g. `with_occlusion_and_lighting()`.
- **Children**: nested component expressions.
- **Positional / sugary parameters**: unnamed values or flags inside the body, e.g. `TXT { "line1", "line2" }`, `gltf { "assets/button.gltf" }`, `QUAD_2D`.

Example:

```txt
Background {
    with_occlusion_and_lighting()
    T {
        TXT { "click to start" }
    }
}
```

## Current status

This is scaffolding only:

- `tokenizer.rs`: `MeowMeowTokenizer` produces a token stream.
- `parser.rs`: `MeowMeowParser` parses a minimal AST that already reflects the component model.
- `evaluator.rs`: `MeowMeowEvaluator` runs on a worker thread and currently just tokenizes/parses and returns a debug AST.

## Threading + communication

The evaluator is intended to run off the main engine thread.

Current prototype uses **two SPSC queues** via `rtrb`:

- request queue: main thread → evaluator (`EvalRequest`)
- response queue: evaluator → main thread (`EvalResponse`)

This matches the “engine loop owns the world; script worker proposes changes” direction.
Actual integration will likely evolve into “worker produces commands; main thread applies them through `CommandQueue` / `Universe` APIs”.
