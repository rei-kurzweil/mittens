# ꩜ MMS Standard Library — draft

> **Status: draft / pre-design.** Nothing here is implemented.

## Core thesis: write the stdlib in MMS

Non-primitive standard library functions (math, noise, string utilities, etc.) should be
written in MMS script itself rather than as Rust evaluator built-ins. This means:

- The stdlib is **transpilable** — a future MMS→WASM or MMS→GLSL backend gets the stdlib
  for free without re-implementing it per target
- The stdlib is **readable and auditable** in MMS — users can read it, learn from it,
  and override individual functions
- The stdlib is **testable** with the same MMS test harness used for user scripts
- Rust built-ins stay minimal and honest about what truly requires the host runtime

## What stays as a Rust built-in

Only things that genuinely cannot be expressed in MMS itself:

| Built-in | Reason |
|----------|--------|
| `emit(ce)` | cat-engine intent — spawns component trees, requires the host runtime |
| `range(n)` / `range(start, end)` | array construction primitive; no way to construct arrays in MMS yet without it |
| Binary/unary operators (`+`, `-`, `*`, `/`, `==`, ...) | parser-level, not callable |

`range()` may eventually move to MMS once the language has array literals and a lower-level
iteration primitive (or simply stays as a built-in convenience since it's so fundamental).

## What belongs in MMS stdlib

### Math object

```mms
// meow_meow/stdlib/math.mms
let Math = {
    pi:  3.14159265358979,
    tau: 6.28318530717959,
    e:   2.71828182845904,

    fn abs(x)        { if x < 0.0 { return -x } return x }
    fn min(a, b)     { if a < b { return a } return b }
    fn max(a, b)     { if a > b { return a } return b }
    fn clamp(x,lo,hi){ return Math.min(Math.max(x, lo), hi) }
    fn lerp(a, b, t) { return a + (b - a) * t }
    fn sign(x)       { if x > 0.0 { return 1.0 } if x < 0.0 { return -1.0 } return 0.0 }

    // Trig — these need host built-ins (see below) or a Taylor series impl
    fn sin(x) { ... }
    fn cos(x) { ... }
    fn sqrt(x){ ... }
}
```

Trig and sqrt are the awkward ones: a pure MMS implementation would need a Taylor series
or table — possible but slow. A small set of **primitive numeric built-ins**
(`__sin`, `__cos`, `__sqrt`, `__floor`, `__ceil`, `__pow`) can be exposed as Rust built-ins
and wrapped by the MMS stdlib so callsites always use `Math.sin(x)`, not the raw primitive.

### Noise

```mms
// meow_meow/stdlib/noise.mms
fn perlin(x, y) { ... }        // pure MMS implementation
fn simplex(x, y, z) { ... }    // pure MMS implementation
```

A value-noise or gradient-noise implementation in MMS is straightforward given integer
hashing (which needs `__floor` and arithmetic). This is a good early candidate for a real
stdlib file.

### Array utilities

```mms
fn map(arr, f)    { ... }
fn filter(arr, f) { ... }
fn reduce(arr, f, init) { ... }
```

## Stdlib loading

The stdlib is evaluated before the user script. Two options:

- **Prelude**: concatenate `stdlib/*.mms` onto the front of every script before parsing.
  Simple; no import machinery needed right now.
- **Module**: `import { Math } from "std:math"` — requires the module system (Phase 9).
  Under the v1 module design, all top-level `let`/`fn` in a stdlib file are automatically
  exported by name — no `export` keyword needed.

  ```mms
  // std/math.mms
  pub let pi = 3.14159265358979
  pub fn lerp(a, b, t) { return a + (b - a) * t }
  let _scratch = 0  // file-private helpers use bare let
  ```

  ```mms
  // user script
  import { pi, lerp } from "std:math"
  ```

The prelude approach works today; module imports are cleaner long-term.

## Relationship to transpilation

When MMS is transpiled to another target, the compiler can either:
1. Include the stdlib source in the output (most portable)
2. Map `Math.sin` → target's native `sin` (optimisation, target-specific)

Because the stdlib is written in MMS, option 1 is always available as a fallback.
