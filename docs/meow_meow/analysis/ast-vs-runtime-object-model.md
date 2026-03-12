# AST vs runtime object model

Meow Meow has (at least) two distinct “shapes” of data:

1. **AST (syntax)**: what the parser produces.
2. **Runtime objects (values/heap)**: what evaluation produces and manipulates.

This document sketches the split so we can keep using the parsed component tree as part of the AST while still having a coherent runtime memory model.

## AST (today)

The current parser/tokenizer live in `src/meow_meow/`.

- Tokenizer: produces `Token { kind, span }`.
- Parser: produces `Vec<Statement>`.
- Expressions include `Expression::Component(ComponentExpression)`.

The component expression AST intentionally mirrors the authoring model:

- `component_type`: identifier
- `parameters`: header `k=v`
- `positional`: sugary body expressions
- `calls`: method-like invocations
- `children`: nested component expressions

## Runtime object model (why it’s separate)

Even if we compile a component expression directly into engine components, the scripting language still needs a runtime value model for:

- evaluated literals (`"hi"`, `123`, `true`, `null`)
- arrays (`[1, 2, 3]`)
- later: objects/maps, instances, closures, modules, etc.

If/when we compile to bytecode and run a VM, we still need:

- **heap objects** (arrays, objects, strings, instances)
- **value representation** (tagged union / NaN-boxing / etc.)
- **host interop values** (engine component handles, asset handles)

So: the AST describes *what to do*, and the runtime object model describes *what exists* while doing it.

## Proposed layering (v1)

- AST: stay close to syntax, spans, and source mapping.
- Runtime: a small `Value` + `Heap` that evaluation can use.
- Host interface: adapters between runtime values and engine-side constructors.

In code, this starts as `src/meow_meow/object.rs`.

## Future note: preserving component-body order

Today the AST stores `positional`/`calls`/`children` separately.

If we want precise semantics (and later things like `await`, conditional children, etc.), the AST should likely move to something like:

- `body: Vec<ComponentBodyItem>` where `ComponentBodyItem` can be `Call`, `Child`, `Positional`, `Separator`.

That change is intentionally deferred until v1 evaluation semantics are nailed down.
