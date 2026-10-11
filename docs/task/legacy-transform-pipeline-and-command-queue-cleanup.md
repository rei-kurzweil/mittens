# Legacy transform-pipeline and command-queue cleanup

Status: deferred; inventory started, no production rename or removal authorized by this task yet.

Direction update: [transform operator API cleanup](transform-operator-api-cleanup.md)
now owns the authored API and transform-naming direction. Use **transform
operators**, rather than renaming every public concept from pipeline to stream.
The command-queue/signal-staging work here remains separate. Existing source
names in the inventory below remain accurate until an implementation migration.

Related:

- [Transform component accessors: Mittens engine API](../draft/transform-component-accessors-engine-api.md)
- [Transform pipeline cleanup checklist](transform-pipeline-cleanup-checklist.md)
- [Event signal pipelines](../draft/event-signal-pipelines.md)

## Goal

Audit and eventually remove misleading legacy terminology left by two engine architecture changes:

- authored/runtime **transform pipeline** terminology was replaced by **transform stream**;
- the old command execution model was replaced by event/intent signals, while a type still named
  `CommandQueue` remains at the engine boundary.

This is cleanup work, not a prerequisite for the current transform-accessor or VTuber slide-deck
work. Do not widen those tasks to perform these renames opportunistically.

## Important finding: `CommandQueue` is not currently dead code

`src/engine/ecs/command_queue.rs` already describes the type as a legacy-named transitional
facade. Its current payload is `Vec<Signal>`, it implements `SignalEmitter`, and it drains events
and intents into `RxWorld` at explicit drain points.

Therefore the audit must distinguish:

1. obsolete command vocabulary;
2. a still-required local signal staging/buffering role;
3. call sites that could emit directly into `RxWorld`;
4. call sites where direct emission would create conflicting borrows or an unsafe self-reference
   inside `Universe`.

Do not delete `CommandQueue` or mechanically replace every use with `RxWorld`. First decide whether
the surviving role should be named something like `SignalQueue`, `SignalStagingBuffer`, or
`LocalSignalEmitter`, and prove that its explicit drain semantics are still necessary.

## Inventory snapshot

### Transform terminology

Active `TransformStreamSystem` code still contains internal identifiers including:

- `TransformPipelineInput`;
- `TransformPipelineVec3Op`;
- `TransformPipelineQuatOp`;
- `TransformPipelineStage` and `TransformPipelineStageKey`;
- `TransformPipelinePlan`;
- `TransformPipelineChannels`;
- `[TransformPipeline]` diagnostic labels.

There are also module names such as `transform_pipeline` and `transform_pipeline_map`, comments in
the gizmo system, scripting test fixtures, and historical design documents. Some historical docs
already carry notes explaining that their examples predate the authored API cleanup.

### Command terminology

`CommandQueue` remains widely used by:

- `Universe` as `universe.command_queue`;
- component-tree initialization;
- `SystemWorld::process_signals` and `process_commands` drain points;
- intent executors that need a local emitter while systems are mutably borrowed;
- the MMS evaluator and example bootstrap path;
- unit and integration tests;
- examples and documentation.

The breadth of usage means this should be a deliberate API migration with compatibility strategy,
not a drive-by symbol rename.

## Workstream A: legacy transform names → transform operator terminology

- [ ] Classify every `TransformPipeline*` Rust identifier as active runtime vocabulary,
  serialized/authored compatibility surface, or historical text.
- [ ] Choose active internal names under the operator cleanup's settled schema;
  do not mechanically replace every `TransformPipeline*` with `TransformStream*`.
- [ ] Decide whether `TransformPipelineChannels` should instead become the shared `TransformTrs`
  value proposed by the transform-accessor work.
- [ ] Rename active diagnostic prefixes to the settled transform-operator vocabulary.
- [ ] Audit `transform_pipeline` and `transform_pipeline_map` module/component names for MMS asset,
  serialization, query-selector, or persisted-world compatibility before changing them.
- [ ] Preserve historical documents as historical where rewriting them would erase useful design
  context; add clear status notes and links to current terminology instead.
- [ ] Update active comments, tests, and documentation after code/API names settle.

## Workstream B: command queue → signal staging

- [ ] Document the exact invariants currently provided by `CommandQueue`: ordering, same-tick
  draining, event deferral, timed intents, recursion limits, and borrow separation.
- [ ] Classify call sites into direct `RxWorld` emission versus local staging required by Rust
  borrowing or execution-phase boundaries.
- [ ] Decide whether the staging abstraction remains a concrete queue, becomes a scoped emitter,
  or can be eliminated in some paths.
- [ ] Choose terminology that describes the surviving behavior. Candidate names are
  `SignalQueue`, `SignalStagingBuffer`, and `LocalSignalEmitter`.
- [ ] Decide whether `Universe.command_queue` needs a compatibility alias/deprecation period or can
  migrate atomically inside the repository.
- [ ] Rename `process_commands` only after separating its remaining responsibilities: queue flush,
  signal dispatch/execution, follow-up flush, audio graph rebuild, and decode completion drain.
- [ ] Update evaluator parameters and examples so they speak in signal/intent terminology.
- [ ] Update comments and docs that still describe intent signals as commands.
- [ ] Remove the old name only when no behavior depends on a command-specific abstraction.

## Safety and compatibility checks

- [ ] Preserve FIFO ordering for signals emitted through the staging layer.
- [ ] Preserve the rule that events emitted during handler processing are deferred while intents
  can execute later in the same drain cycle.
- [ ] Preserve timed-intent promotion behavior.
- [ ] Preserve the `100_000` signal recursion/work cap or replace it with a separately documented
  equivalent.
- [ ] Avoid raw pointers from an emitter stored in `Universe` into the sibling `SystemWorld.rx`;
  moving `Universe` must remain safe.
- [ ] Audit public Rust API, MMS constructor names, serialized type tags, selectors, saved scenes,
  and asset files before renaming compatibility-sensitive identifiers.
- [ ] Run focused signal-ordering, transform-stream, scripting, and representative example tests
  after each migration slice.

## Suggested sequence when this task is resumed

1. Finish the current transform-accessor slice without unrelated renames.
2. Extract shared `TransformTrs`; this may naturally retire `TransformPipelineChannels`.
3. Perform the remaining internal transform terminology rename as a mechanical, test-backed slice.
4. Specify the signal-staging abstraction and its invariants.
5. Rename or replace `CommandQueue` separately, with no transform changes in the same patch.
6. Clean active documentation, then annotate rather than rewrite intentionally historical docs.

## Exit criteria

- Active transform-operator implementation and diagnostics no longer use accidental pipeline/stream
  terminology; any retained usage is explicitly justified for compatibility or history.
- No active API describes a `Vec<Signal>` staging facade as a command queue.
- Signal ordering, deferral, timing, and borrow-safety invariants have regression coverage.
- Historical documents clearly distinguish old architecture from current behavior.

