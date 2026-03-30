# ₍˄·͈༝·͈˄₎ MMS Module System — Spec

> **Status: implemented.** This describes the working v1 module system.
> See `docs/meow_meow/analysis/module-import-export.md` for the broader design exploration.

---

## Export

Use `export` to make a binding visible to other files. Bare `let`/`fn` are file-private.

```mms
// math.mms
export let pi = 3.14159265358979
export let tau = 6.28318530717959

export fn lerp(a, b, t) { return a + (b - a) * t }

let _scratch = 0   // file-private — not importable
```

Only `export let` and `export fn` at the root level are supported. Exports inside function bodies or if-branches are ignored (treated like `let`).

---

## Import

```mms
import { name }             from "file.mms"   // named — requires 'export'
import { name as alias }    from "file.mms"   // named with local alias
import { 0 as alias }       from "file.mms"   // positional CE by index, requires 'as'
```

Multiple items in one statement:

```mms
import { pi, lerp, tau as τ } from "math.mms"
```

Paths are resolved relative to the importing file's directory.

---

## Positional imports — root CE emits

Every free-standing component expression at the root of a file is indexed in emission order (0, 1, 2, ...), whether or not it uses `export`. This lets you import geometry files that have no bindings at all:

```mms
// parts.mms — no exports, just geometry
T.position(0, 0, 0) { R.cube() {} }   // index 0
T.position(1, 0, 0) { R.sphere() {} } // index 1
```

```mms
// scene.mms
import { 0 as cube, 1 as sphere } from "parts.mms"
cube
sphere
```

Positional imports always require an `as` alias — `import { 0 }` is a parse error.

---

## Module evaluation semantics

When a file is imported, it is evaluated in a **sandboxed context**: any CE emits go into the module's positional sequence rather than the engine's spawn queue. Nothing is actually spawned until the importing script re-emits the CE value.

```mms
import { 0 as my_cube } from "parts.mms"  // parts.mms is evaluated, nothing spawned
my_cube                                     // re-emit: THIS spawns
```

Named exports (functions, numbers, etc.) are available immediately after import and work like any local binding.

---

## File resolution

```
"math.mms"          → relative to the importing file's directory
"lib/math.mms"      → relative path segments supported
```

Absolute paths are also accepted (pass through unchanged). There is no stdlib prefix (`@std/`) in v1 — stdlib files are loaded by relative path or prelude.

---

## What is / isn't supported in v1

| Feature | Status |
|---------|--------|
| `export let` / `export fn` at root | ✅ |
| `import { name }` named import | ✅ |
| `import { name as alias }` | ✅ |
| `import { 0 as alias }` positional | ✅ |
| Mixed `{ name, 0 as alias }` list | ✅ |
| Relative path resolution | ✅ |
| Imports inside function bodies | ⚠️ relative path resolution unavailable (source_path = None) |
| `import parts from "..."` namespace import | ❌ not yet |
| Circular import detection | ❌ not yet (will stack overflow) |
| Module caching (eval once) | ❌ not yet (re-evaluated per import) |
| `@std/` stdlib prefix | ❌ not yet |
| Re-export (`export { x } from "..."`) | ❌ not yet |
