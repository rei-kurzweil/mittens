# ✦ AST transform visitor

## Problem

`transform.rs` currently has separate structs (`EmitLiftTransform`,
`QueryDesugarTransform`) each with their own `apply()` that walks the full AST
independently. Each new transform adds another full tree walk. Three transforms
= three passes over every node.

The transforms don't need separate passes. They should be **visitors** registered
on a single walker that traverses the tree once and dispatches to each visitor at
the matching node.

---

## Design

### Visitor trait

```rust
pub trait AstVisitor {
    /// Called on each `Statement` before its children are visited.
    /// Default: no-op (visitor is not interested in statements).
    fn visit_statement(&mut self, _stmt: &mut Statement) {}

    /// Called on each `Expression` before its children are visited.
    fn visit_expression(&mut self, _expr: &mut Expression) {}

    /// Called on each `ComponentExpression` before its children are visited.
    /// Note: `ComponentExpression` is also reachable via `visit_expression`
    /// (`Expression::Component`); this hook fires additionally for convenience.
    fn visit_component(&mut self, _ce: &mut ComponentExpression) {}
}
```

Each transform implements only the hooks it cares about:

| Transform | Hook used |
|---|---|
| `EmitLiftTransform` | `visit_statement` — rewrites `Statement::Expression(Component(_))` |
| `QueryDesugarTransform` | `visit_expression` — rewrites `BinOp { op: Query, .. }` |
| `GestureImpliedRaycastableTransform` | `visit_component` — injects `Raycastable` into `Renderable { }` |

### Walker

```rust
pub struct AstWalker<'a> {
    visitors: Vec<&'a mut dyn AstVisitor>,
}

impl<'a> AstWalker<'a> {
    pub fn new(visitors: Vec<&'a mut dyn AstVisitor>) -> Self {
        Self { visitors }
    }

    pub fn walk_stmts(&mut self, stmts: &mut Vec<Statement>) {
        for stmt in stmts.iter_mut() {
            self.walk_stmt(stmt);
        }
    }
}
```

The walker drives one traversal. Visitors mutate nodes in place during their hook.

### Dispatch order within a node

For each node the walker visits:

1. **Pre-order hooks**: fire each visitor's hook for the current node.
2. **Recurse**: walk children.

Pre-order lets a visitor rewrite a node (e.g. replace `BinOp { Query }` with a
`Call`) before the walker descends into it, so children of the rewritten node are
visited correctly.

If a visitor needs post-order (visit children first, then rewrite), it defers its
rewrite by setting a flag or accumulating work — but in practice the current
transforms don't need this.

---

## Rewriting existing transforms

### EmitLiftTransform

```rust
impl AstVisitor for EmitLiftTransform {
    fn visit_statement(&mut self, stmt: &mut Statement) {
        if let Statement::Expression(Expression::Component(_)) = stmt {
            let inner = std::mem::replace(
                if let Statement::Expression(e) = stmt { e } else { unreachable!() },
                Expression::Null,
            );
            *stmt = Statement::Expression(Expression::Call(CallExpression {
                callee: Box::new(Expression::Identifier(Ident("emit".into()))),
                args: vec![inner],
            }));
        }
    }
}
```

### QueryDesugarTransform

```rust
impl AstVisitor for QueryDesugarTransform {
    fn visit_expression(&mut self, expr: &mut Expression) {
        if let Expression::BinaryOp { op: BinOpKind::Query, lhs, rhs } = expr {
            let callee = /* selector_is_single logic */ ...;
            let sel  = std::mem::replace(lhs.as_mut(), Expression::Null);
            let handler = std::mem::replace(rhs.as_mut(), Expression::Null);
            *expr = Expression::Call(CallExpression {
                callee: Box::new(Expression::Identifier(Ident(callee.into()))),
                args: vec![sel, handler],
            });
        }
    }
}
```

### GestureImpliedRaycastableTransform

```rust
impl AstVisitor for GestureImpliedRaycastableTransform {
    fn visit_component(&mut self, ce: &mut ComponentExpression) {
        if ce.component_type.0 != "Renderable" { return; }

        let has_handler = ce.body.iter().any(|item| matches!(item,
            ComponentBodyItem::NamedAssignment { name, .. }
                if GESTURE_HANDLER_NAMES.contains(&name.0.as_str())
        ));
        if !has_handler { return; }

        let has_raycastable = ce.body.iter().any(|item| match item {
            ComponentBodyItem::Child(c) => c.component_type.0 == "Raycastable",
            ComponentBodyItem::Call(c)  => c.callee.0 == "Raycastable",
            _ => false,
        });
        if has_raycastable { return; }

        ce.body.insert(0, ComponentBodyItem::Child(ComponentExpression {
            component_type: Ident("Raycastable".into()),
            constructor: None,
            body: vec![],
        }));
    }
}
```

---

## Call site (runner)

```rust
let mut emit_lift  = EmitLiftTransform;
let mut query_desugar = QueryDesugarTransform;
let mut gesture_rc = GestureImpliedRaycastableTransform;

AstWalker::new(vec![
    &mut emit_lift,
    &mut query_desugar,
    &mut gesture_rc,
]).walk_stmts(&mut stmts);
```

One call, one pass, all transforms applied. Adding a new transform is one new
struct + one extra line in the visitor vec.

---

## Non-goals

- **Transform ordering guarantees between visitors at the same node**: visitors
  fire in vec order. If transform B's output needs to be seen by transform A, put
  B before A. Current transforms are independent so order doesn't matter today.
- **Stopping descent**: no `visit_*` returns a bool to skip children. If needed,
  add a `fn should_descend_component(&self, ce: &ComponentExpression) -> bool`
  hook later.
- **Multiple walkers for different traversal strategies**: out of scope. One
  pre-order walker covers all current needs.

---

## Implementation checklist

- [ ] Define `AstVisitor` trait in `transform.rs`
- [ ] Define `AstWalker` with `walk_stmts`, `walk_stmt`, `walk_expr`,
  `walk_component`, `walk_block` methods
- [ ] Rewrite `EmitLiftTransform` as `impl AstVisitor`
- [ ] Rewrite `QueryDesugarTransform` as `impl AstVisitor`
- [ ] Remove old `apply()` free functions; update call sites in runner
- [ ] Add `GestureImpliedRaycastableTransform` as `impl AstVisitor` (when gesture
  handler syntax lands)
