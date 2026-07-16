# MMS REPL navigation and cat unification

## Status

Design task.

No implementation yet.

## Goal

Make the MMS REPL navigation model coherent and filesystem-like.

The REPL should expose a small set of navigation-aware commands that behave
predictably when inspecting component trees and values:

- `ls` lists navigable children
- `cd 0` moves to child index `0` from the current listing
- `cd name` moves to a named child from the current listing
- `cd /path` moves to an absolute component path
- `pwd` prints the current location
- `cat` dumps the current component or value by default
- `cat 0` resolves index `0` from the current listing and dumps it
- `cat component_variable_name` evaluates and dumps that variable
- `cat query("#some_component > Text")` evaluates and dumps the query result

## Current confusion

The integrated MMS REPL currently mixes navigation and expression evaluation in
ways that make similar commands behave differently.

Known confusing behavior:

- `cd 0` resolves listing index `0`
- `cat 0` evaluates the numeric literal `0` and prints `0.0`
- `dump(cwd)` works as a short-term inspection workaround, but overlaps
  conceptually with `cat`

The user-facing expectation is that `cat` should inspect the thing named by the
operand in the current navigation context, while still allowing expression
evaluation where the operand is clearly not a navigation target.

## Proposed direction

Make the MMS REPL own navigation-aware commands instead of treating every command
operand as a raw MMS expression.

Commands that should become navigation-aware:

- `cat`
- `tree`
- maybe `type`

Suggested resolution order for `cat <operand>`:

1. Resolve an index from the current `ls` listing, such as `cat 0`.
2. Resolve a named child or component path from the current navigation context.
3. Fall back to MMS expression evaluation for variables, calls, and queries.

This keeps expression evaluation available for operands like:

```mms
component_variable_name
query("#some_component > Text")
```

while making navigation operands behave consistently with `ls` and `cd`.

## Dump relationship

Decide whether `dump()` remains as a lower-level function or becomes an alias for
`cat`.

Short-term, `dump(cwd)` is a valid workaround. Long-term, `cat` should be the
user-facing inspection command because it matches the navigation model and avoids
requiring users to know the internal `cwd` value shape.

## Crate-level goal

`crates/meow-meow-script` should eventually have its own REPL with the same
navigation semantics.

The current engine-integrated MMS REPL should not be the only place where
filesystem-like component navigation exists. The standalone crate REPL should
share the command model so `ls`, `cd`, `pwd`, `cat`, and related inspection
commands behave the same way across host contexts.

## Assumptions

- `dump(cwd)` is acceptable as a short-term workaround
- `cat` should become the user-facing inspection command
- implementation should happen separately after the desired REPL semantics are
  agreed
