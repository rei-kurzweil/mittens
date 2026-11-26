//! Interpreter micro-benchmark: cost of calling and creating closures as the
//! number and kind of bindings visible at definition grows. Today every
//! closure snapshots all visible bindings and every call clones that
//! snapshot, so cost should scale with scope size; after capture-by-reference
//! it should be flat. See `docs/benchmarks/mms-interpreter.md`.
//!
//! cargo test --release -p meow-meow-script --test closure_call_bench -- --ignored --nocapture
use meow_meow_script::{Hostless, Runtime};
use std::time::Instant;

const ITERATIONS: usize = 20_000;

enum Scope {
    Numbers(usize),
    Arrays { count: usize, len: usize },
    Functions(usize),
}

impl Scope {
    fn label(&self) -> String {
        match self {
            Scope::Numbers(count) => format!("{count} numbers"),
            Scope::Arrays { count, len } => format!("{count} arrays x {len}"),
            Scope::Functions(count) => format!("{count} functions"),
        }
    }

    fn source(&self) -> String {
        let mut out = String::new();
        match *self {
            Scope::Numbers(count) => {
                for i in 0..count {
                    out.push_str(&format!("let g{i} = {i}.0\n"));
                }
            }
            Scope::Arrays { count, len } => {
                let items = (0..len).map(|i| format!("{i}.0")).collect::<Vec<_>>().join(", ");
                for i in 0..count {
                    out.push_str(&format!("let g{i} = [{items}]\n"));
                }
            }
            Scope::Functions(count) => {
                for i in 0..count {
                    out.push_str(&format!("fn g{i}(x) {{ return x + {i}.0 }}\n"));
                }
            }
        }
        out
    }
}

/// Microseconds per loop iteration for `body`, after `setup` defines `scope`.
fn time_loop(scope: &Scope, setup: &str, body: &str) -> f64 {
    let mut session = Runtime::standard().session(Hostless);
    session.eval(&scope.source()).unwrap();
    session.eval(setup).unwrap();
    let source = format!("for i in range({ITERATIONS}) {{ {body} }}");
    let started = Instant::now();
    session.eval(&source).unwrap();
    started.elapsed().as_secs_f64() * 1e6 / ITERATIONS as f64
}

#[test]
#[ignore = "benchmark; run with --release --ignored --nocapture"]
fn closure_call_bench() {
    let scopes = [
        Scope::Numbers(0),
        Scope::Numbers(16),
        Scope::Numbers(128),
        Scope::Numbers(1024),
        Scope::Arrays { count: 4, len: 64 },
        Scope::Arrays { count: 32, len: 64 },
        Scope::Functions(16),
        Scope::Functions(128),
    ];
    let cases = [
        (
            "empty loop",
            "let acc = { total = 0.0 }",
            "acc.total = acc.total + i",
        ),
        (
            "named fn call",
            "fn add(a, b) { return a + b }\nlet acc = { total = 0.0 }",
            "acc.total = add(acc.total, i)",
        ),
        (
            "table fn call",
            "let acc = { total = 0.0 }\nlet api = { add = fn(v) { acc.total = acc.total + v } }",
            "api.add(i)",
        ),
        (
            "closure create",
            "let acc = { total = 0.0 }",
            "let f = fn(x) { return x + i }",
        ),
    ];

    println!("\nus/iteration, {ITERATIONS} iterations");
    print!("{:<20}", "scope");
    for (name, _, _) in &cases {
        print!("{name:>16}");
    }
    println!();
    for scope in &scopes {
        print!("{:<20}", scope.label());
        for (_, setup, body) in &cases {
            print!("{:>16.3}", time_loop(scope, setup, body));
        }
        println!();
    }
}
