/// mms-functions: test harness for MMS Phases 2–4 (arithmetic, if/else, functions).
///
/// Evaluates mms-functions.mms and asserts on the output without spinning up
/// a window or GPU. Run with:
///
///   cargo run --example mms-functions
use cat_engine::meow_meow::MeowMeowRunner;

fn main() {
    let output = MeowMeowRunner::eval(include_str!("mms-functions.mms"));

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

    assert_eq!(
        output.intents.len(),
        4,
        "expected 4 SpawnComponentTree intents (got {})",
        output.intents.len()
    );

    println!("ok — all assertions passed");
}
