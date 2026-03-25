# ₍˄·͈༝·͈˄₎ MMS Module / Import System — Design Analysis

Phase 9 sketch. Implementation deferred — see §0 for the current design direction.

---

## 0. Current direction (pre-implementation note)

> **Not implementing yet.** Capturing design intent while it's fresh.

### What we're fairly sure about

- **`export` keyword exists.** `export let foo = 256` and `export fn make_grid(n) { }` make
  named things retrievable from a file. Any value type — number, function, CE — can be exported.
- **Root CEs are implicitly positionally exported.** Every top-level emit in a file gets a
  numeric index (0, 1, 2, ...) automatically, with no `export` keyword needed.
- **The selector/query system and the named-export retrieval system should be unified,
  not two separate mechanisms.** Getting a named export and querying a CE subtree by
  selector are the same operation — you're retrieving things from a file by some key.

### What is open

- **`import` keyword may not exist.** If the retrieval operation is unified with querying,
  a separate `import` keyword that creates bindings is potentially redundant or at odds with
  the query model. The verb for loading a file and retrieving things from it hasn't been decided.
- **If `import` does exist**, it should BE the query mechanism — not a separate binding form
  that you then query on top of. There shouldn't be two different mental models for
  "get a named value from a file" and "search a file's CE tree by selector."
- **Unification example sketch** (syntax not decided):
  ```mms
  // named export — same mechanism as positional/selector:
  load("parts.mms").pi             // export let pi = 3.14
  load("parts.mms")[0]             // first root CE
  load("parts.mms").query("T")     // all T-type CEs in the file
  load("parts.mms")[0].query("[name=foo] T")  // selector within first root CE
  ```
  The key property: one object, one access model. Named exports are just entries in
  that object reachable by string key, same as positional entries are reachable by
  integer key, same as selector queries return arrays of entries.

The rest of this doc is the design exploration that led here. Treat it as background
and options, not a finalized spec.

---

---

## 1. The dual nature of a `.mms` file

A `.mms` file is simultaneously:

| Role | What it exposes |
|------|----------------|
| **Script** | Named bindings and functions — `export let cube = ...`, `export fn make_grid(n) { }` |
| **Database** | All emitted component expressions in emission order — queryable by index or selector |

This is a natural consequence of MMS's emission model: every free-standing component
expression in a file implicitly calls `emit(ce)`. When you import a file you can ask
"give me the thing it would have emitted at position N" without actually spawning it —
or search within one of those trees with a selector.

The editor writes `.mms` files as scene descriptions. Those files can later be treated
as queryable datasets: "find all transforms nested under something named 'torso'."

---

## 2. Existing engine selector system

The engine already has a `[name='...']` selector used for live component queries:

```rust
// World::find_component / find_all_components
world.find_component(root, "[name='J_Bip_L_Hand']")
world.find_all_components(root, "[name='spine']")
```

**Format:** `[name='value']` or `[name="value"]` — attribute selector, exact string match.
**Traversal:** DFS from a root `ComponentId`, returns matching `ComponentId`(s).
**Used by:** `avatar_control_system`, `bone_mapping_system` for skeleton bone lookup.

The MMS import selector system extends this same format and semantics to the **static
CE AST** — no live world required. The selector grammar is a superset.

---

## 3. Export syntax

### 3.1 Named exports

```mms
export let red_cube = R.cube() { C.rgba(1, 0, 0, 1) }
export fn make_grid(n, color) { ... }
export let pi = 3.14159
```

`export` is a prefix modifier on `let` and `fn`. The distinction between `let` and
`export let` is purely visibility — evaluation is identical.

### 3.2 Implicit positional export

Every free-standing emit in the file is collected in emission order into an implicit
**emission sequence**. Always exported, regardless of whether any `export let` exists.

```mms
// parts.mms
T.position(0, 0, 0) { R.cube() { C.rgba(1, 0, 0, 1) } }   // index 0
T.position(1, 0, 0) { R.cube() { C.rgba(0, 1, 0, 1) } }   // index 1
T.position(2, 0, 0) { R.sphere() {} }                       // index 2
```

### 3.3 Named CE annotation

For the selector system to match by name, a CE needs a name. The mechanism is a
`name = "..."` body item — already valid MMS syntax (`Named` body item):

```mms
// editor-written file
T.position(0, 1, 0) {
    name = "torso"
    R.cube() { }
}
```

This is the CE equivalent of `ComponentNode.name` in the live world. The engine uses
`ComponentNode.name` for `[name='...']` queries; the import selector uses the
`Named { name: "name", value: String("torso") }` body item for the same purpose.

---

## 4. Import syntax

### 4.1 Named import (destructuring)

```mms
import { make_cube, pi } from "parts.mms"
```

Binds named exports into scope. The file evaluates once (cached). No emits reach the world.

### 4.2 Namespace import

```mms
import parts from "parts.mms"
```

Binds the whole module as `parts`. Named exports via `parts.make_cube(...)`,
emission sequence via `parts[n]` and `parts.query(selector)`.

### 4.3 Inline selector import

The selector can be applied directly in the import statement, returning only the
matching CE(s) rather than the whole module:

```mms
// Import the first emitted CE from parts.mms, then find transforms
// nested under something named "foo" within that CE's subtree.
import "[name=foo] T" from "parts.mms"[0]

// Without the [0] — search across all emitted CEs in the file.
import "[name=foo] T" from "parts.mms"
```

**`"parts.mms"[0]`** — `[0]` is an index into the file's emission sequence. It selects
the first top-level CE as the root to search within. Omitting it searches across all
emitted CEs (the whole file is the search space).

The result is bound to a local variable (destructured or namespace):

```mms
import transforms from "[name=foo] T" from "parts.mms"[0]
// transforms is Value::Array of matching ComponentExpr values
```

> **Syntax note:** `import X from selector from file` is a bit awkward. Alternative
> spellings to consider: `import (selector) from file`, or make the selector a method
> call on the module: `import parts from "parts.mms"; let t = parts[0].query("[name=foo] T")`.
> The method-call form is less novel syntax but requires two lines. ❓ open.

### 4.4 Side-effect import

```mms
import "scene.mms"
```

Evaluates the file; emits go to the world. Named exports discarded. The only form
that actually spawns components.

---

## 5. Selector grammar

The CE selector uses the **unified cat-engine component query language** defined in
[`docs/spec/component-query-selectors.md`](../../spec/component-query-selectors.md).
The same grammar applies to live world queries and MMS CE-tree queries — only the root
differs.

### 5.1 Summary of the selector grammar

```
selector   := compound (combinator compound)*
combinator := WS+     // descendant
            | '>'     // direct child
compound   := simple+
simple     := TYPE_IDENT                    // 'T', 'R', 'C', 'GLTF' — component type
            | '.' LOWER_IDENT              // '.transform', '.renderable' — type alias
            | '[' 'name' '=' quoted ']'    // [name='foo'] — name attribute match
            | TYPE_IDENT '.' LOWER_IDENT   // 'T.position' — type + constructor
```

Type selectors use the MMS uppercase convention (`T`, `R`, `C`); the dot-prefix
lowercase aliases (`.transform`, `.renderable`, `.color`) map to those same types for
ergonomic use in selector strings.

### 5.2 The root specifier

The canonical query API uses a `ComponentQuery` struct where the root and the selector
are separate fields. In MMS contexts, the root is a file + optional emission index:

```
load("scene.mms")[0].query("[name='torso'] T")
//                          ^^^^^^^^^^^^^^ selector — same grammar as world queries
// root = first CE emitted by scene.mms
```

See §17 of the selector spec for the full root-encoding options and the open design
question around string-embedded roots vs method-call form.

### 5.3 MMS-specific extension: constructor selector

In the live world, component type is all you need for a type selector. In static CE
trees, you also have the **constructor method name** — the `.position(...)` in
`T.position(0, 1, 0) { }`. The `TYPE.constructor` compound form (`T.position`,
`R.cube`) is specific to MMS CE queries and has no live-world equivalent.

---

## 6. Query API on a module value

When imported as namespace (`import parts from "..."`), the module value supports:

```mms
parts[0]                         // first emitted CE (ComponentExpr value)
parts[2]                         // third emitted CE
parts.query("[name=foo] T")      // all matching CEs — Value::Array
parts[0].query("[name=foo] T")   // search within first CE's subtree only
parts.query("[name=foo] T").first()   // first match
```

`parts[n]` returns a `Value::ComponentExpr`. It can be re-emitted anywhere:

```mms
let torso = parts[0].query("[name=torso]").first()
T.position(0, 2, 0) { torso }
```

The module value type:

```
Value::Module {
    named:    HashMap<String, Value>,   // export let / export fn bindings
    sequence: Vec<ComponentExpr>,       // emission-order CE list
}
```

---

## 7. Import semantics — sandboxed evaluation

Importing is always **pure / sandboxed**. The imported file evaluates in an isolated
emit context where `emit()` calls populate `sequence` rather than the engine queue.
Nothing is spawned until the caller explicitly re-emits a CE.

```mms
import parts from "parts.mms"   // evaluates parts.mms, nothing spawned

parts[0]                         // now this emits — one CE spawns
```

**Alignment with existing engine pattern:** This is exactly analogous to how the engine
currently uses `SpawnComponentTree` — the CE is described first, then explicitly sent
as an intent. The module system just makes that description reusable and queryable.

A file is evaluated at most once per session; repeated imports return the cached
`Value::Module`.

---

## 8. Editor integration

The editor writes `.mms` files as scene descriptions. The selector system makes those
files machine-readable datasets:

```mms
// avatar.mms (editor-written)
GLTF.new("rei.glb") {
    name = "avatar"
    T.position(0, 1, 0) {
        name = "spine"
        T.position(0, 0.5, 0) {
            name = "head"
        }
    }
}
```

```mms
// script.mms (user-written)
let head = (import "[name=avatar] [name=head]" from "avatar.mms"[0]).first()
// head is the CE for the head transform — can be re-emitted, mutated (Phase 7), etc.
```

The `name = "..."` body item is the bridge between the editor's named scene graph and
the import selector system.

---

## 9. Module resolution

```mms
import parts from "parts.mms"           // relative to current file
import parts from "assets/parts.mms"    // relative to project root (TBD)
import parts from "@std/math.mms"       // stdlib prefix (future)
```

Circular imports: detected and rejected. Caching: one evaluation per session.

---

## 10. Open questions

| Question | Impact |
|----------|--------|
| Inline selector import syntax (`import X from sel from file` vs method call) | Parser ergonomics |
| `[name=foo]` vs `[name='foo']` — require quotes? (engine requires quotes; MMS could relax) | Consistency with existing selector format |
| `.lowercase` alias list — what aliases ship in Phase 9 vs later? | Spec completeness |
| Descendant vs child default when no combinator: `"[name=foo] T"` — space = descendant? | CSS convention says yes |
| `*` wildcard — needed in Phase 9 or defer? | Scope |
| Are queried CEs clones or references? (pre-Phase-6: clone; Phase-6+: ❓) | Value semantics |
| `ce.query(sel)` on a plain `ComponentExpr` value outside of a module — allowed? | Reflection API surface |
| Can a module re-export from another? (`export { foo } from "..."`) | Composition ergonomics |
| Circular import detection — parse time or eval time? | Implementation complexity |
| `@std/` stdlib — what ships? | Ecosystem scope |
