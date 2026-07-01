# BlockEffectAnalyzer

## Goal

Add an opt-in post-parse analysis pass for captured MMS blocks so runtime
systems such as `Keyframe.at(...) { ... }` can know, ahead of execution time,
which parts of a block are audio-producing and which are visual / non-audio.

This is meant to support:

- audio lookahead for imperative keyframe blocks
- future `audio_only` / `visual_only` evaluation modes
- lower runtime overhead than rediscovering effect kinds on every evaluation

## Why This Exists

The current runtime already stores a parsed `BlockStatement` inside
`CapturedBlock` for `Keyframe` callbacks. That means parsing is not the
problem. The missing piece is effect classification.

Today, the animation system can do lookahead scheduling for:

- legacy `ActionComponent` audio
- legacy `MusicNoteComponent` keyframe children

but not for imperative callback-authored audio inside:

```mms
Keyframe.at(0.0) {
    MusicNote.e(4, 0.25, lead)
}
```

The block is available as AST already, but the runtime does not have a stored
analysis telling it which statements/calls are safe and relevant for:

- audio-only lookahead evaluation
- visual-only due-frame evaluation

## Scope Boundary

This should **not** be part of the main parser.

The parser should remain purely syntactic:

- tokenize source
- build AST
- stop

`BlockEffectAnalyzer` is a semantic post-parse pass that runs only when a
specific construct asks for it.

The first intended user is:

- `Keyframe.at(...) { ... }`

Other block forms should not pay for this work unless they explicitly opt in.

## Where Analysis Lives

The analysis should live on `CapturedBlock`, not on `BlockStatement`.

Good:

```rust
pub struct CapturedBlock {
    pub body: BlockStatement,
    pub captured_env: Arc<HashMap<String, Value>>,
    pub analysis: Option<BlockEffectAnalysis>,
}
```

Bad:

```rust
pub struct BlockStatement {
    pub statements: Vec<Statement>,
    pub analysis: Option<BlockEffectAnalysis>,
}
```

Rationale:

- `BlockStatement` is generic parser AST
- most blocks in the language are ordinary control-flow/function/component-body
  blocks with no special runtime scheduling needs
- `CapturedBlock` already means "this block has special runtime meaning after
  parsing"
- `Keyframe` is the correct first consumer of that wrapper-level metadata

## Proposed Components

### 1. `BlockEffectAnalyzer`

An opt-in semantic pass:

```rust
pub struct BlockEffectAnalyzer;
```

Example entry point:

```rust
impl BlockEffectAnalyzer {
    pub fn analyze_keyframe_block(body: &BlockStatement) -> BlockEffectAnalysis;
}
```

This is intentionally scoped to the use case instead of pretending all blocks
 need one universal policy immediately.

### 2. `BlockEffectAnalysis`

Stores block-level and statement-level effect metadata.

Possible shape:

```rust
pub struct BlockEffectAnalysis {
    pub contains_audio_effects: bool,
    pub contains_visual_effects: bool,
    pub contains_unknown_effects: bool,
    pub statements: Vec<StatementEffectSummary>,
}
```

### 3. `StatementEffectSummary`

Per-statement classification result.

Possible shape:

```rust
pub struct StatementEffectSummary {
    pub effect_kind: EffectKind,
}
```

### 4. `EffectKind`

Minimal initial categories:

```rust
pub enum EffectKind {
    None,
    Audio,
    Visual,
    Mixed,
    Unknown,
}
```

`Unknown` is important. The first version should be conservative rather than
incorrectly claiming a statement is audio-only or visual-only when it is not.

## Initial Classification Rules

The first version does not need to solve all semantic cases perfectly.

It only needs to be good enough to support imperative keyframe audio lookahead
without breaking correctness.

Likely initial rules:

- `MusicNote.<pitch>(...)` call:
  - `Audio`
- live-handle visual mutation such as `transform.set_position(...)`,
  `emissive.set_intensity(...)`, `text.set_text(...)`:
  - `Visual`
- branches / loops:
  - summarize from children
- plain arithmetic / local reassignment / literals:
  - `None`
- function calls not known to be pure or classified:
  - `Unknown`
- statements containing both audio and visual effects:
  - `Mixed`

## Control Flow

`BlockEffectAnalyzer` does not replace evaluation.

Example:

```mms
if should_play {
    MusicNote.e(4, 0.25, lead)
} else {
    cube.set_position(1, 0, 0)
}
```

The analyzer can classify the branches:

- then branch contains audio
- else branch contains visual

but runtime evaluation still decides which branch is taken.

So the analyzer is for:

- effect classification
- filtering/planning

not:

- constant folding every runtime condition
- replacing the interpreter

## Runtime Use

The intended runtime split is:

- lookahead phase:
  - evaluate captured keyframe blocks in `audio_only` mode
- due-frame phase:
  - evaluate captured keyframe blocks in `visual_only` mode

`BlockEffectAnalysis` should let those evaluators skip obviously irrelevant
statements/calls rather than reclassifying them on every run.

## Why This Should Happen Before Runtime Modes Get Complicated

Without stored analysis, the evaluator must repeatedly determine:

- is this statement audio?
- is this statement visual?
- should this call be suppressed in this pass?

That makes the evaluator own both:

- execution
- effect classification policy

which is the wrong separation of concerns.

The better split is:

- parser builds syntax
- `BlockEffectAnalyzer` classifies effect intent once
- runtime evaluators execute according to that stored plan

## Integration Point

This pass should run when constructing `CapturedBlock` for a runtime-special
block owner such as `Keyframe`.

Roughly:

1. Parse source into AST.
2. Materialize `Keyframe.at(...) { ... }`.
3. Build `CapturedBlock`.
4. Run `BlockEffectAnalyzer::analyze_keyframe_block(...)`.
5. Store the result in `CapturedBlock.analysis`.

That keeps the analysis isolated and opt-in.

## Non-Goals

- analyzing every block in the language by default
- putting effect metadata directly on parser AST nodes
- solving arbitrary user-defined function purity/effect inference in v1
- replacing runtime evaluation with static execution

## First Implementation Target

The first success criterion should be narrow:

- `Keyframe.at(...) { MusicNote... }` participates in the same 100 ms audio
  lookahead scheduling model as legacy keyframe audio
- non-audio visual side effects inside the same keyframe block still occur on
  the due frame, not in lookahead
- the analysis is stored on `CapturedBlock`, not `BlockStatement`

## Open Questions

1. Should `BlockEffectAnalysis` classify only statements, or also individual
   subexpressions/calls?
2. How should `Unknown` interact with lookahead?
   - safest answer: skip unknown effects in lookahead
3. Should user-defined functions called from keyframe blocks be inlined for
   analysis, or treated as `Unknown` in v1?
4. Should `Mixed` statements be split during analysis, or simply routed to the
   stricter runtime path?

## Recommendation

Implement `BlockEffectAnalyzer` as a narrowly scoped semantic pass for
`CapturedBlock` owned by `Keyframe`.

Do not put this on the parser and do not put effect metadata on
`BlockStatement`.

This gives the animation system a clean path to:

- `audio_only` keyframe lookahead evaluation
- `visual_only` due-frame evaluation
- future built-in-table and host-call effect classification beyond
  `MusicNote`
