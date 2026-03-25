/// mms-loops: test harness for MMS Phase 5 (for/in, range, break, continue).
///
/// Evaluates mms-loops.mms and asserts on the output without spinning up
/// a window or GPU. Run with:
///
///   cargo run --example mms-loops
use cat_engine::meow_meow::MeowMeowRunner;

fn main() {
    let output = MeowMeowRunner::eval(include_str!("mms-loops.mms"));

    if !output.errors.is_empty() {
        eprintln!("MMS errors:");
        for e in &output.errors {
            eprintln!("  {e}");
        }
        std::process::exit(1);
    }

    println!("intents: {}", output.intents.len());
    for (i, iv) in output.intents.iter().enumerate() {
        println!("  [{i}] {iv:?}");
    }

    // 4×4 grid = 16 cells, minus the 2×2 hole (4 cells skipped) = 12 intents
    assert_eq!(
        output.intents.len(),
        12,
        "expected 12 SpawnComponentTree intents (got {})",
        output.intents.len()
    );

    println!("ok — all assertions passed");
}
