// query-demo.mms
// Demonstrates the MMS query system.
//
// Features exercised:
//   name = "id" in CE body           -- component registry: sets ComponentNode name
//   query("selector")                -- HostCall, single result (ComponentObject?)
//   query_all("selector")            -- HostCall, all results ([ComponentObject])
//   component.query("selector")      -- scoped subtree query
//   "selector" -> handler            -- -> query operator
//   "selector" -> method(args)       -- -> query method shorthand
//   scope -> "selector" -> handler   -- scoped query via -> chain
//   print(value)                     -- evaluator builtin
//   assert(cond, msg)                -- evaluator builtin
//
// The scene has:
//   - one "hero" cube (yellow)
//   - three "enemy" cubes (red)   — each named "enemy_0", "enemy_1", "enemy_2"
//   - a "panel" group with several children
//
// After building the scene, queries find and mutate components:
//   - hero turns blue  (found by name #hero)
//   - enemies turn green  (found by type/selector query_all)
//   - panel children queried from the panel component's scope

// ── scene setup ───────────────────────────────────────────────────────────────

BGC.rgba(0.05, 0.05, 0.08, 1.0)
AL.rgb(0.5, 0.5, 0.5)

// hero: a single yellow cube, named "hero"
T.position(0, 0, -3) {
    name = "hero"
    R.cube() {
        name = "hero_r"
        C.rgba(1.0, 1.0, 0.0, 1.0)     // yellow
    }
}

// enemies: three red cubes
for i in range(3) {
    T.position(i * 2 - 2, -1.5, -3) {
        name = "enemy_" + i
        R.cube() {
            name = "enemy_r_" + i
            C.rgba(1.0, 0.0, 0.0, 1.0)  // red
        }
    }
}

// panel: a parent transform with two child objects inside
// used to demonstrate scoped .query() from a ComponentObject handle
T.position(0, 2.5, -4) {
    name = "panel"
    T.position(-1, 0, 0) {
        name = "panel_left"
        R.sphere() { C.rgba(0.2, 0.8, 1.0, 1.0) }
    }
    T.position(1, 0, 0) {
        name = "panel_right"
        R.sphere() { C.rgba(1.0, 0.4, 0.2, 1.0) }
    }
}

print("scene built: 1 hero + 3 enemies + 1 panel with 2 children")

// ── query 1: single result by name ────────────────────────────────────────────
//
// query("#id") → ComponentObject?  (null if not found)

let hero = query("#hero")
if !hero {
    assert(false, "hero not found — expected a named T component")
}
print("query('#hero') found: " + hero)

// turn hero blue via its colour child
let hero_color = query("#hero_r C")
if !hero_color {
    assert(false, "hero_r C not found")
}
hero_color.set_rgba(0.0, 0.4, 1.0, 1.0)     // blue
print("hero turned blue")

// ── query 2: multiple results ──────────────────────────────────────────────────
//
// query_all("selector") → [ComponentObject]  (empty if none)

let enemy_colors = query_all("T[name^=enemy_] R C")   // all C children of enemy Rs
print("found " + enemy_colors + " enemy colour components")

for c in enemy_colors {
    c.set_rgba(0.0, 1.0, 0.2, 1.0)   // green
}
print("all enemies turned green")

// ── query 3: -> query operator ────────────────────────────────────────────────
//
// "selector" -> handler  is always a query dispatch — no ambiguity with |> pipe.
// Desugars: "selector" -> handler  →  query("selector", handler)

// method shorthand: the receiver is the implicit query result
"#hero_r C" -> set_rgba(1.0, 0.0, 1.0, 1.0)   // hero → magenta
print("hero turned magenta via -> method shorthand")

// full callback form
"#enemy_r_0 C" -> fn(c) {
    if !c { return }
    c.set_rgba(1.0, 0.5, 0.0, 1.0)   // first enemy → orange
}
print("enemy_0 turned orange via -> callback")

// ── query 4: scoped query on a ComponentObject ────────────────────────────────
//
// component.query("selector") restricts the search to the component's subtree.
// Equivalent to CSS:  #panel .R  but only searching inside the panel subtree.

let panel = query("#panel")
if !panel {
    assert(false, "panel not found")
}

// query within the panel's subtree — won't accidentally find hero or enemy Rs
let panel_spheres = panel.query_all("R")
print("panel contains " + panel_spheres + " renderables")
// expected: 2  (panel_left R and panel_right R)

for r in panel_spheres {
    r.set_rgba(0.9, 0.9, 0.2, 1.0)   // panel children → yellow
}
print("panel children turned yellow")

// ── query 5: scoped query via -> chain ────────────────────────────────────────
//
// scope -> "selector" -> handler
// The LHS ComponentObject scopes the search to its subtree.
// Desugars: scope.query("selector", handler)

panel -> "C" -> fn(c) {
    // called for each C descendant of panel
    c.set_rgba(0.4, 0.2, 0.9, 1.0)   // override to purple
}
print("panel colour components set to purple via scoped -> chain")

// ── query 6: query returning null — graceful handling ─────────────────────────

let missing = query("#definitely_not_a_thing")
if missing {
    assert(false, "should not have found a component named 'definitely_not_a_thing'")
} else {
    print("null result from missing selector handled correctly")
}

// method shorthand on a non-matching selector — handler is not called
"#also_missing C" -> set_rgba(1, 0, 0, 1)
print("no-match shorthand did not crash  (null guard applied implicitly)")

print("query-demo: all assertions passed")
