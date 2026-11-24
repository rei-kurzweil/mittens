# Accelerate the MMS runtime: bytecode and a stack + register VM

Status: proposed, 2026-11-24. Investigation; nothing started.

## Why

MMS now carries real game logic per frame and per event: NPC combat and
walking, vehicle patrols, weapon bursts, carry and damage reactions,
smoke updates. The evaluator (`crates/meow-meow-script/src/evaluator.rs`)
walks the AST directly:

- every variable read and write resolves a name through a stack of
  `HashMap<String, Value>` scopes;
- every call re-walks its body's AST nodes and re-matches builtin names
  (`"sin" | "cos" | ...` string matches in `call_builtin`);
- tables are string-keyed maps; component fields live in a side map keyed
  by handle;
- values are a large enum, cloned freely across scopes.

That is fine for scene construction, but costly for handlers that run every
frame (`FrameTick` updates, per-shot checks) and for heavy one-shot scripts
(the radio tower and building generators build hundreds of nodes).

`lowering.rs` is a placeholder pointing at the planned typed lowering
(`docs/analysis/mms-dimension-expression.md`,
`docs/task/mms-dimension-lowering-checklist.md`); this ticket is the
execution side of that direction.

## Proposal

1. **Resolve names at compile time.** A pass after parsing and the existing
   AST transforms assigns every local a slot index per function frame and
   every captured variable an upvalue index (closures capture cells, so loop
   variables captured by handlers get a fresh cell per iteration, which also
   fixes the "keyframe body sees the last loop index" class of bugs).
   Globals and imports resolve to module-slot indices.
2. **Compile to bytecode.** One `Chunk` per function: constants, a compact
   instruction stream, line table for errors. Register-style instructions for
   arithmetic and locals (`ADD r3, r1, r2`), an operand stack only for call
   arguments and expression temporaries that don't fit registers. Builtins
   (Math, Transform field reads, `len`, `range`) become opcodes or
   pre-resolved native function pointers, not string matches.
3. **Host calls stay at the boundary.** Component construction, component
   methods and `emit_data` keep going through the existing host interface
   (`HostRequest`/`HostResponse`), with operation IDs resolved once at compile
   time from the configured `RuntimeSpec` instead of per call.
4. **Values.** Small immediate values (numbers, bools, null, component
   handles) unboxed; strings interned; tables with a shape (fixed keys at
   construction get slots; `"" + index` keys fall back to a map); arrays as
   vectors. Avoid cloning on reads.
5. **Callbacks.** `on(...)` handlers become closures referencing chunks;
   dispatch pushes a frame instead of re-entering the tree walker. The
   audio/visual deferred phases stay a host-side gate, unchanged.
6. **Keep both paths during migration.** Run the bytecode VM behind a
   switch, with the tree walker as the reference; a differential test mode
   evaluates the same scripts both ways and compares host request streams.

## Native math, alongside (not part of the VM)

Scenes currently hand-write quaternion and transform math in MMS (rotating a
point into a vehicle's frame, composing an aim pitch and yaw, offsetting a
muzzle rotation). Native helpers would remove most of it, whatever the VM:

- `Math.min`, `Math.max`, `Math.pow`, `Math.lerp`, a seeded `Math.random`;
- quaternion helpers: `Quat.from_euler`, `Quat.mul`, `Quat.rotate(q, v)`,
  `Quat.yaw(q)`;
- transform methods: `t.local_to_world(point)`, `t.world_to_local(point)`,
  `t.direction(local_vector)` (e.g. `-Z` forward in world), and a runtime
  `t.look_at(world_point)` (today `looking_at` is only a constructor).

## Validation

- Differential runs of every example and test scene: identical host request
  streams and errors from both runtimes.
- Benchmarks: scene load time for the sakura scenes and the truck demo; per
  frame cost of FrameTick handlers (NPC wander, smoke update) before and
  after; allocations per handler call.
- Closures in loops capture per-iteration values (a regression test).
- Error messages keep source line and column.
