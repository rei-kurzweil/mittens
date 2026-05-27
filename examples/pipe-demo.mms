// pipe-demo.mms
// Demonstrates the |> forward pipe operator.
//
// Features exercised (all require implementation):
//   |>  forward pipe operator        -- parser + eval, no type system needed
//   print(value)                     -- evaluator builtin, EvalResponse::Print
//   assert(cond, msg)                -- evaluator builtin, fails loudly on bad output
//   name = "id" in CE body           -- component registry: sets ComponentNode name
//   component.set_rgba(r,g,b,a)      -- Phase 7 mutation method on ColorComponent
//   component.set_position(x,y,z)    -- Phase 7 mutation method on TransformComponent
//   query("selector", handler)       -- Phase 6+ HostCall

// ── pure function definitions ─────────────────────────────────────────────────

fn double(x) { return x * 2 }
fn halve(x)  { return x / 2 }
fn square(x) { return x * x }
fn negate(x) { return 0 - x }

fn clamp01(x) {
    if x < 0.0 { return 0.0 }
    if x > 1.0 { return 1.0 }
    return x
}

// rough perceptual gamma curve (pow ~2.2, approximated as square)
fn gamma_encode(x) { return x * x }
fn gamma_decode(x) { return x + (x - x * x) * 0.5 }  // crude sqrt approx

// ── section 1: basic pipe chain ───────────────────────────────────────────────
//
// expr |> f  desugars to  f(expr)
// a |> f |> g  is left-associative: g(f(a))

let a = 2.0 |> double           // 4.0
print("double(2) = " + a)
assert(a == 4.0, "double(2) should be 4.0")

let b = 3.0 |> double |> double  // 12.0
print("double(double(3)) = " + b)
assert(b == 12.0, "double chain should be 12.0")

let c = 0.3 |> double |> clamp01  // 0.6, already in range
print("clamp01(double(0.3)) = " + c)
assert(c == 0.6, "should be 0.6")

let d = 5.0 |> double |> clamp01  // 10.0 clamped to 1.0
print("clamp01(double(5)) = " + d)
assert(d == 1.0, "should clamp to 1.0")

// pipe into an inline function literal
let e = 7.0 |> fn(x) { return x * 3 }   // 21.0
print("inline fn pipe: 7 * 3 = " + e)
assert(e == 21.0, "inline fn should give 21.0")

// ── section 2: pipe to build a colour gradient scene ──────────────────────────
//
// Spawn 8 cubes in a row. Each cube's brightness is computed as:
//   fraction → gamma_encode → used as greyscale colour
// All via forward pipe — no intermediate let bindings needed.

BGC.rgba(0.08, 0.08, 0.10, 1.0)
AL.rgb(0.6, 0.6, 0.6)

for i in range(8) {
    let frac = i / 7           // 0.0 .. 1.0
    let bright = frac |> gamma_encode |> clamp01
    T.position(i - 3.5, 0, -3) {
        R.cube() { C.rgba(bright, bright, bright, 1.0) }
    }
}

print("gradient strip spawned (8 cubes)")

// second strip: same but inverted — shows negate working in a pipe chain
for i in range(8) {
    let frac   = i / 7
    let bright = frac |> gamma_encode |> negate |> fn(x) { return x + 1.0 } |> clamp01
    T.position(i - 3.5, -1.2, -3) {
        R.cube() { C.rgba(bright, bright, bright, 1.0) }
    }
}

print("inverted strip spawned")

// ── section 3: pipe chain ending at a component query HostCall ───────────────
//
// Build a "target" cube, then drive its colour with a value computed via pipe.
// The final step is a query dispatch via -> — a HostCall that crosses into the engine.
//
// -> is the query/dispatch operator: "selector" -> method(args)
// desugars to: query("selector", fn(r) { r.method(args) })

T.position(0, 1.5, -3) {
    name = "target_t"
    R.sphere() {
        name = "target_r"
        C.rgba(1.0, 0.0, 0.0, 1.0)  // starts red
    }
}

// compute a colour via pipe — purely functional, no engine involvement yet
let intensity = 0.4 |> double |> gamma_encode |> clamp01  // 0.64
let tint_r = intensity
let tint_g = intensity |> halve                            // 0.32
let tint_b = intensity |> double |> clamp01               // 1.0 (clamped)

print("computed tint: r=" + tint_r + " g=" + tint_g + " b=" + tint_b)

// now apply the computed colour to the spawned component
// this is the HostCall sink — crosses the script/engine boundary
"#target_r C" -> set_rgba(tint_r, tint_g, tint_b, 1.0)

print("target cube colour set via -> query dispatch  [HostCall]")

// equivalent explicit form:
// query("#target_r C", fn(c) {
//     if !c { return }
//     c.set_rgba(tint_r, tint_g, tint_b, 1.0)
// })

// ── section 4: pipe as composition — functions returning functions ─────────────
//
// Standard forward pipe: a |> f |> g  where f and g are named functions.
// f can itself return a function — the result is piped onward.
// Demonstrates that pipe works with any callable, not just simple transforms.

fn make_adder(n) {
    return fn(x) { return x + n }
}

let add5 = make_adder(5)
let result = 10.0 |> add5 |> double   // (10 + 5) * 2 = 30
print("(10 + 5) * 2 via pipe = " + result)
assert(result == 30.0, "should be 30.0")

print("pipe-demo: all assertions passed")
