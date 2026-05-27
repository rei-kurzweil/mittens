# ＼(≧▽≦)／ MMS Procedural Clouds — Feature Gap Audit

> **Goal:** understand what additions MMS needs to express `spawn_cloud_ring`
> (`examples/example_util/mod.rs:52`) natively, without a Rust helper.

---

## What `spawn_cloud_ring` Actually Does

The Rust helper builds two nested loops:

```
for i in 0..cloud_count          // cluster ring
    seed_i  = seed ^ (i * 0x9e3779b9)
    jitter  = (rand(seed_i ^ A) - 0.5) * step * angle_jitter
    angle   = i * step + jitter
    cx, cz  = radius * cos(angle), radius * sin(angle)
    cy      = center_y  OR  center_y * high_y_multiplier   (probabilistic)

    spawn: TransformComponent.position(cx, cy, cz)          → child of bg_root
        for puff_i in 0..puffs_per_cloud
            seed = seed_i ^ (puff_i * 1103515245)
            ox, oy, oz = rand-offset  ×  (9, 4, 9)
            sx, sy, sz = base * per-axis stretch             base ∈ [0.7, 3.5]
            r, g, b    = 0.70–0.80 + rand * 0.10–0.12  (cold-blue-white palette)

            spawn: T.position(ox,oy,oz).scale(sx,sy,sz)     → child of center_tx
                R.cube()                                     → child of T
                C.rgba(r,g,b,1)                              → child of R
```

All arithmetic is tightly chained; no spawning without math.

---

## ᓚᘏᗢ Current MMS Capabilities Audit

### Expressions — what exists today

| Feature | AST node | Evaluator support |
|---------|----------|-------------------|
| Number literals | `Expression::Number(f64)` | ✅ full |
| String / Bool / Null | ✅ | ✅ |
| Array literal | `Expression::Array` | ✅ |
| Identifier lookup | `Expression::Identifier` | ✅ (ComponentExpr only) |
| Free function call | `Expression::Call` | ✅ (only `emit()` dispatched; others silently dropped) |
| Component expression | `Expression::Component` | ✅ |
| **Binary operators** (`+`, `*`, …) | ❌ not in AST | ❌ |
| **Unary minus** | ❌ | ❌ |

### Statements — what exists today

| Feature | AST node | Evaluator support |
|---------|----------|-------------------|
| `let x = expr` | `Statement::Assignment` | ✅ (ComponentExpr or primitive) |
| `if cond { } else { }` | `Statement::If` | ⚠️ parsed, **ignored** in evaluator v1 |
| Block `{ }` | `Statement::Block` | ⚠️ parsed, **ignored** |
| `return expr` | `Statement::Return` | ⚠️ parsed, **ignored** |
| **`for` / `repeat` loop** | ❌ not in AST | ❌ |
| **`while` loop** | ❌ | ❌ |

### Built-in functions — what exists today

None beyond `emit()`. The evaluator has no function dispatch table.
`Expression::Call` nodes other than `emit` fall through to `StmtEffect::None`.

---

## (=^･ω･^=) What Needs to be Added

Grouped by layer (easiest → biggest):

### 1. Arithmetic / math expression evaluation

The AST needs binary operators and the evaluator needs to compute them at runtime.

**Required for clouds specifically:**

| Operation | Used for |
|-----------|----------|
| `+`, `-`, `*`, `/` | offset/scale math everywhere |
| Unary `-` | symmetric ranges (`-0.5 * step`) |

**Proposed AST addition:**

```rust
// in ast/expression.rs
pub enum Expression {
    // existing …
    BinOp(Box<Expression>, BinOpKind, Box<Expression>),
    UnaryMinus(Box<Expression>),
}

pub enum BinOpKind { Add, Sub, Mul, Div, Rem }
```

No precedence surprises are needed beyond standard math rules (PEMDAS).
The parser already handles a Pratt / precedence-climbing extension naturally here.

The evaluator needs a `eval_expr(expr, env) -> Result<MmsValue, String>` that
returns a numeric `MmsValue` — which means `StoredValue::Primitive` needs to
carry an `f64` properly (not just a debug string).

### 2. Built-in math functions

**Required for clouds:**

| Function | Signature | Use |
|----------|-----------|-----|
| `sin(x)` | f64 → f64 | angle → z position |
| `cos(x)` | f64 → f64 | angle → x position |

**Strongly recommended alongside these (keep math complete):**

`abs`, `floor`, `ceil`, `round`, `sqrt`, `pow(x,y)`, `min(a,b)`, `max(a,b)`,
`clamp(x,lo,hi)`, `lerp(a,b,t)`, `atan2(y,x)`, `log2`, `exp`

These would be dispatched inside a `call_builtin(name, args)` helper in the
evaluator, keyed on the function name string.

### 3. Random / noise functions

The Rust helper uses a fast hash-based PRNG (`hash_u32`, then `/u32::MAX`).
MMS needs something analogous.

**Minimum for clouds:**

| Function | Signature | Notes |
|----------|-----------|-------|
| `rand(seed)` | u32-ish → f64 in `[0,1)` | deterministic; same hash as Rust helper |

**Nice to have (and needed for more interesting clouds):**

| Function | Signature | Notes |
|----------|-----------|-------|
| `perlin(x, y)` | f64×f64 → f64 in `[-1,1]` | classic 2D Perlin / gradient noise |
| `perlin(x,y,z)` | f64³ → f64 | 3D variant |
| `simplex(x,y)` / `simplex(x,y,z)` | — | lower artefact alternative |
| `fbm(x,y,octaves)` | — | fractal Brownian motion over perlin |
| `value_noise(x,y)` | — | cheaper smooth noise |

For the first pass, only `rand(seed)` is strictly required. Perlin/simplex are
needed if we want position offsets that are spatially coherent rather than
purely stochastic.

**Seeding semantics:** MMS rand should mirror the Rust helper — same integer
hash, XOR seed mixing with distinct constants per independent dimension — so
ported scenes produce identical layouts.

### 4. `for` loop (integer range)

The outer and inner loops require count-driven iteration.

**Minimum form needed:**

```
for i in 0..cloud_count {
    // body
}
```

**AST addition:**

```rust
// in ast/statement.rs
pub struct ForRangeStatement {
    pub var: Ident,
    pub start: Expression,   // inclusive
    pub end: Expression,     // exclusive  (mirrors Rust)
    pub body: BlockStatement,
}
```

The evaluator needs:
- `eval_expr` returning numeric values (dependency on §1)
- A `for` arm in `eval_stmt` that loops and executes body statements,
  extending env with the loop variable each iteration

**Do not** design for `for x in array` or iterator chains yet — that is much
more complex and not needed here.

### 5. Variable binding of primitive values

Currently `let x = 1.5` parses and stores `StoredValue::Primitive("1.5")`
(a debug string), but that value is never read back usefully.

For loops and math to work, bindings need to hold real typed values:

```rust
enum StoredValue {
    ComponentExpr(Box<ComponentExpression>),
    Number(f64),
    Bool(bool),
    Str(String),
    Array(Vec<StoredValue>),
}
```

The evaluator's `capture_expr` and identifier lookup both need updating.

### 6. Spawning with computed positions/scales

`R.cube()` under a `T.position(ox,oy,oz).scale(sx,sy,sz)` works today — but
only with literal numbers. When the args are expressions (e.g., results of
arithmetic or `rand()`), the component registry methods need to receive
evaluated `f64` values rather than raw `Expression` nodes.

This requires that `eval_component_expr` call `eval_expr` on each argument
before passing to the registry builder, rather than pattern-matching on
`Expression::Number` directly.

The current `component_registry.rs` already expects `f64` from the expression
layer (it matches `Expression::Number(n)`), so the change is: replace that
pattern match with a call to `eval_expr(...)?` and coerce to `f64`.

---

## ≽^•⩊•^≼ Proposed MMS Syntax for Clouds

What the `.mms` file would look like once the above is implemented:

```
// clouds.mms  (illustrative — not yet valid MMS)

let cloud_count = 5;
let radius      = 26.0;
let center_y    = 2.0;
let puffs       = 28;
let seed        = 0xC10;
let step        = 6.283185307 / cloud_count;

for i in 0..cloud_count {
    let seed_i = seed ^ (i * 0x9E3779B9);
    let jitter = (rand(seed_i ^ 0xa53a9d2d) - 0.5) * step;
    let angle  = i * step + jitter;
    let cx     = radius * cos(angle);
    let cz     = radius * sin(angle);

    T.position(cx, center_y, cz) {
        for puff_i in 0..puffs {
            let s = seed_i ^ (puff_i * 1103515245);
            let ox = (rand(s ^ 0x68bc21eb) - 0.5) * 9.0;
            let oy = (rand(s ^ 0x02e5be93) - 0.5) * 4.0;
            let oz = (rand(s ^ 0xa1d34f2b) - 0.5) * 9.0;
            let base = 0.7 + rand(s ^ 0x9e3779b9) * 2.8;
            let sx = base * (0.7 + rand(s ^ 0x243f6a88) * 0.9);
            let sy = base * (0.6 + rand(s ^ 0x85a308d3) * 1.0);
            let sz = base * (0.7 + rand(s ^ 0x13198a2e) * 0.9);
            let t  = rand(s ^ 0x7f4a7c15);
            let r  = 0.70 + 0.10 * t;
            let g  = 0.72 + 0.10 * t;
            let b  = 0.80 + 0.12 * t;

            T.position(ox, oy, oz).scale(sx, sy, sz) {
                R.cube() {
                    C.rgba(r, g, b, 1.0)
                }
            }
        }
    }
}
```

Open question: should `for` bodies that contain `T { }` implicitly adopt the
outer T as parent, or does that require an explicit `parent =` annotation?
The current `SpawnComponentTree` intent always attaches to `parent: None`
(world root). Parent wiring inside loops will need a new intent form or a
stack-based "current parent" tracked by the evaluator.

---

## (◕ᴗ◕✿) Implementation Order

| Step | What | Scope |
|------|------|-------|
| 1 | Typed `StoredValue` (Number/Bool/Str/Array) + `eval_expr` returning `MmsValue` | `evaluator.rs`, `object.rs` |
| 2 | Binary ops + unary minus in AST + parser | `ast/expression.rs`, `parser.rs` |
| 3 | `eval_expr` arithmetic | `evaluator.rs` |
| 4 | Built-in math dispatch (`sin`, `cos`, `abs`, `min`, `max`, `clamp`, …) | `evaluator.rs` (new `builtins.rs`) |
| 5 | `rand(seed)` builtin (same hash as Rust helper) | `builtins.rs` |
| 6 | `for i in lo..hi` AST + parser + evaluator | `ast/statement.rs`, `parser.rs`, `evaluator.rs` |
| 7 | Arg evaluation in component registry (`eval_expr` instead of literal match) | `component_registry.rs` |
| 8 | Parent-stack for loop body spawning | `evaluator.rs`, possibly new `IntentValue` |
| 9 | Perlin/simplex noise builtins | `builtins.rs` |

Steps 1–5 together unlock math-heavy static scenes (no loops).
Steps 6–8 unlock procedural generation.
Step 9 is optional polish.

---

## Open Questions

- **Hex literals:** `0xC10`, `0x9E3779B9` — the tokenizer needs hex support for
  seed constants to be readable. Currently only decimal `f64` is tokenized.
- **Integer XOR / wrapping mul:** the hash uses `u32` wraparound. MMS uses `f64`
  internally; either expose a `xor_hash(a, b)` builtin or define integer types.
- **TAU constant:** should `tau` / `pi` be reserved identifiers or just
  encouraged to be spelled out as `6.283185307`?
- **Parent wiring in loops:** the biggest open design question (see §6 and the
  syntax sketch above). Options: implicit stack, explicit `attach_to(id)` call,
  or a `repeat(n) { }` block that stays inside the current component body.
