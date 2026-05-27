# ✦ MMS Module Resolution — Design Draft

> **Status: draft / thinking.** Nothing here is implemented yet.
> Current state lives in `resolve_import_path` in `evaluator.rs` (12 lines, relative-path-only).
> This doc is for designing what comes next.

---

## The problem with v1

Right now `resolve_import_path` does one thing:

```
"cat.mms"   →   {dir of importing file}/cat.mms
```

That's it. No extension inference, no stdlib, no fallback roots. You always need the full filename including `.mms`.

The user wants to be able to write:

```mms
import { "simplex" } from "noise"
```

where `"noise"` resolves to a built-in stdlib module — no extension, no relative path, no file on disk at all. And user files should shadow it if they have a `noise.mms` nearby.

---

## Import path anatomy

An import string can be one of several shapes. The resolver needs to classify it first:

| Shape | Example | Kind |
|---|---|---|
| Relative with extension | `"./cat.mms"` | Explicit relative |
| Relative without extension | `"./cat"` | Relative, needs extension probe |
| Bare name | `"noise"` | Could be relative OR stdlib |
| Bare name with slash | `"noise/simplex"` | Submodule path |
| Absolute path | `"/home/rei/scenes/foo.mms"` | Absolute |

The key ambiguity is **bare names** — `"noise"` could be a file called `noise.mms` next to the importing file, or it could be the stdlib `noise` module.

---

## Resolution order (proposed)

Check roots in order, return the first hit:

```
1.  Relative to the importing file's directory    (user file wins)
2.  Relative to a project search path             (user workspace, optional)
3.  Built-in stdlib module registry               (Rust-side, fallback)
```

**Why user first?** A user who writes `noise.mms` in their project should always get their own file, even if it happens to share a name with a stdlib module. This matches how most module systems work (Node.js, Python, Rust's `extern crate` shadowing).

### Step 1 — relative resolution

For each candidate root directory (starting with the importing file's directory):

```
"noise"
  → try  {dir}/noise          (exact — unusual but allowed)
  → try  {dir}/noise.mms      (add .mms)
  → try  {dir}/noise/mod.mms  (directory module)
```

If any exists on disk, use it. Stop checking further roots.

### Step 2 — project search path

An optional list of additional directories to check (like `PYTHONPATH` or `NODE_PATH`). Could be configured via a `mms.toml` or passed programmatically by the engine host. Low priority for v1 but the slot should exist in the data model.

### Step 3 — stdlib registry

A Rust-side `HashMap<&str, StdlibModule>` keyed by the bare name. If no file was found in steps 1–2, check if the bare name (without any extension) is a known stdlib module. Submodule paths like `"noise/simplex"` could be either a file inside a stdlib directory or a logical submodule — TBD.

Built-in modules are not files on disk. They're evaluated differently (see below).

---

## Extension inference

When a path has **no `.mms` extension** and isn't an absolute path, the resolver probes in order:

```
1. exact path as given       (covers extensionless files, unusual)
2. path + ".mms"             (standard)
3. path + "/mod.mms"         (directory module, a la Python __init__.py / Rust mod.rs)
```

If a path already ends in `.mms`, skip probing — use as-is.

> **Open question:** should we support `.mms.toml` sidecars for metadata? Probably not v1. Skip.

---

## Stdlib module design

**Decision: write all stdlib in MMS.**

MMS will eventually be transpiled to Rust (and potentially JavaScript/WASM) so that scene descriptions can be compiled to fast native code rather than interpreted at runtime. If stdlib modules were written in Rust, they would be unreachable from the transpiler — the transpiler works on MMS ASTs, not on opaque Rust closures. Writing stdlib in MMS means the transpiler gets the full AST for free and can emit optimised native versions of `simplex`, `lerp`, etc. alongside user code.

This also means stdlib is inspectable, forkable, and writable by users without touching the engine. A user who wants a tweaked `lerp` can copy `math.mms` locally and their version will shadow the stdlib (resolution order guarantees this).

For functions that genuinely require native primitives that MMS cannot express yet — true random number generation, calling into a C noise library, OS-level I/O — MMS will need a **native binding** mechanism (a `native fn` declaration or similar) that the transpiler knows to lower to a platform call. That mechanism is out of scope for now; functions that need it will be stubbed or deferred until the binding system exists.

### Implementation: bundled `.mms` files

Stdlib modules are `.mms` source files that live in `src/meow_meow/stdlib/` and are embedded into the binary at compile time via `include_str!`. The resolver maps a bare name to embedded source:

```
"noise"   →  include_str!("stdlib/noise.mms")
"math"    →  include_str!("stdlib/math.mms")
"color"   →  include_str!("stdlib/color.mms")
"easing"  →  include_str!("stdlib/easing.mms")
```

When step 3 of resolution hits, the evaluator calls `eval_as_module(source, Some("<std:noise>"))` on the embedded source string exactly like a file module. The `Value::Module` it returns is unpacked by the normal `Statement::Import` machinery — no special casing anywhere else.

The `StdlibModule` type is just:

```rust
struct StdlibModule {
    source: &'static str,   // embedded MMS source
    sentinel_path: &'static str, // e.g. "<std:noise>", used in error messages
}

static STDLIB: &[(&str, StdlibModule)] = &[
    ("noise",  StdlibModule { source: include_str!("stdlib/noise.mms"),  sentinel_path: "<std:noise>"  }),
    ("math",   StdlibModule { source: include_str!("stdlib/math.mms"),   sentinel_path: "<std:math>"   }),
    ("color",  StdlibModule { source: include_str!("stdlib/color.mms"),  sentinel_path: "<std:color>"  }),
    ("easing", StdlibModule { source: include_str!("stdlib/easing.mms"), sentinel_path: "<std:easing>" }),
];
```

---

## Stdlib module catalogue (planned)

| Module | Contents | Notes |
|---|---|---|
| `"noise"` | `simplex(x,y)`, `simplex3(x,y,z)`, `perlin(x,y)`, `worley(x,y)` | Needs native binding for actual noise; stub with approximation until then |
| `"math"` | `sin`, `cos`, `tan`, `sqrt`, `abs`, `floor`, `ceil`, `pow`, `clamp`, `lerp`, `map` | Wrappable in MMS via native bindings for trig/sqrt; `lerp`/`clamp`/`map` are pure MMS |
| `"color"` | `hsv(h,s,v)` → rgba array, `mix(a,b,t)`, `temperature(k)` | Pure MMS |
| `"easing"` | `ease_in`, `ease_out`, `ease_in_out`, cubic/back/elastic variants | Pure MMS — polynomial math only |
| `"random"` | `seed(n)`, `rand()`, `rand_range(lo,hi)` | Needs native binding for PRNG state |

**Pure MMS** modules (`color`, `easing`, most of `math`) can be written immediately. Modules that need native primitives (`noise`, `random`, trig in `math`) will need stubs or wait for the native binding mechanism.

> **Open question:** should `"math"` exist as an import, or are basic ops (`sin`, `cos`, etc.) global builtins exposed via a prelude? Could do both — stdlib module for explicit import, prelude for convenience. Defer until the language has a prelude concept.

---

## The `source_path` problem with stdlib

Currently `EvalContext::source_path` is the filesystem path of the file being evaluated. Stdlib modules have no filesystem path. Two consequences:

1. **Relative imports inside stdlib** — a stdlib module that tries `import "helper"` would resolve relative to... nothing. Either we give stdlib modules a virtual path (e.g. `std://noise`), or we forbid imports inside stdlib modules, or we give them a special root.

2. **Error messages** — `source_path: None` makes errors harder to locate. For native modules this doesn't matter (there's no source). For bundled `.mms` stdlib modules we'd want a path like `<std:noise>` in error messages.

Proposed: give each stdlib module a sentinel path string like `"<std:noise>"`. The resolver ignores it for relative imports (falls through to stdlib check), and it shows up in error messages for attribution.

---

## Implementation sketch

```
fn resolve_import(path: &str, ctx: &EvalContext) -> ResolvedModule

enum ResolvedModule {
    File(PathBuf),          // read from disk and eval_as_module
    Stdlib(StdlibModule),   // call stdlib constructor
}

fn resolve_import(path: &str, source_path: Option<&str>) -> Option<ResolvedModule> {
    let bare = strip_extension(path);  // remove .mms if present

    // Step 1: relative to importer
    if let Some(src) = source_path {
        let dir = Path::new(src).parent().unwrap_or(Path::new("."));
        for candidate in probe_extensions(dir.join(path)) {
            if candidate.exists() { return Some(ResolvedModule::File(candidate)); }
        }
    }

    // Step 2: project search paths (none in v1, reserved)

    // Step 3: stdlib
    if let Some(m) = STDLIB.get(bare) {
        return Some(ResolvedModule::Stdlib(m.clone()));
    }

    None  // → "module not found" error
}

fn probe_extensions(base: PathBuf) -> Vec<PathBuf> {
    // if base already ends in .mms, return as-is
    // otherwise: [base, base.with_extension("mms"), base.join("mod.mms")]
}
```

---

## Open questions / decisions needed

1. **Bare name ambiguity** — should `import { x } from "noise"` always check the stdlib, or only if no local `noise.mms` exists? (Proposed: local always wins, stdlib is last resort.)

2. **`"noise/simplex"` submodule paths** — support them or not? Could resolve as `noise/simplex.mms` in the stdlib dir, or as a logical sub-export of the `noise` module. Defer until there's a concrete use case.

3. **Math builtins vs `"math"` module** — expose `sin`/`cos`/etc. as top-level builtins (simpler for quick scripts) or require `import { sin } from "math"` (cleaner, more explicit)? Could do both via a prelude mechanism.

4. **Stdlib source location** — if we go with bundled `.mms` files, where do they live in the repo? `src/meow_meow/stdlib/`? Should they be checked into git as `.mms` files and embedded at compile time, or generated?

5. **`mod.mms` for directories** — is the directory-module probe (`noise/mod.mms`) worth the complexity? It's familiar from Rust/Python but adds a resolution step. Could defer until someone actually organises stdlib into subdirs.

6. **Error on ambiguity or silent precedence?** — if a user file `noise.mms` shadows the stdlib `noise`, should the resolver warn? Probably just silently shadow (Python/Node behaviour), but a debug log might be useful.

---

## What needs to change in the code

For reference, current state in `evaluator.rs`:

```rust
// Current (12 lines, relative-only):
fn resolve_import_path(path: &str, source_path: Option<&str>) -> String {
    if let Some(src) = source_path {
        if let Some(parent) = std::path::Path::new(src).parent() {
            return parent.join(path).to_string_lossy().into_owned();
        }
    }
    path.to_string()
}
```

When implementing, this becomes a richer function returning `ResolvedModule` instead of a `String`. The `Statement::Import` arm in `eval_stmt` branches on the result — file → `eval_as_module`, stdlib → call the native constructor directly.

Also: `EvalContext::source_path` stays as-is; stdlib modules just get a sentinel string. The `StmtEffect::ImportBindings` path is unchanged — both file and stdlib modules produce `Value::Module` and get unpacked the same way.
