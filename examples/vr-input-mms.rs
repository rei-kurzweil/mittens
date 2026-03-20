use cat_engine::meow_meow::ast::expression::{ComponentBodyItem, Expression};
use cat_engine::meow_meow::ast::statement::Statement;
use cat_engine::meow_meow::parser::MeowMeowParser;
use cat_engine::meow_meow::tokenizer::MeowMeowTokenizer;

fn main() {
    let src = include_str!("vr-input.mms");

    let tokens = MeowMeowTokenizer::new(src).tokenize().expect("tokenize vr-input.mms");
    let program = MeowMeowParser::new(tokens).parse_program().expect("parse vr-input.mms");

    println!("vr-input.mms parsed ok — {} top-level statements", program.len());
    for (i, stmt) in program.iter().enumerate() {
        match stmt {
            Statement::Expression(Expression::Component(c)) => {
                let head = c
                    .constructor
                    .as_ref()
                    .map(|hc| format!(".{}({})", hc.method.0, hc.args.len()))
                    .unwrap_or_default();
                let child_count = c
                    .body
                    .iter()
                    .filter(|b| matches!(b, ComponentBodyItem::Child(_)))
                    .count();
                println!(
                    "  [{i}] {}{head}  ({} body items, {child_count} children)",
                    c.component_type.0,
                    c.body.len(),
                );
            }
            Statement::Expression(_) => println!("  [{i}] <expr>"),
            Statement::Assignment(a) => println!("  [{i}] let {} = ...", a.name.0),
            Statement::Return(_) => println!("  [{i}] return"),
            Statement::If(_) => println!("  [{i}] if"),
            Statement::Block(_) => println!("  [{i}] block"),
        }
    }

    // TODO: evaluate program → SpawnComponentTree intents → live scene
    // This requires the SpawnComponentTree IntentValue variant and the component
    // registry executor (phase 1 checklist steps 4-6).
    println!("\n(scene spawning not yet implemented — parser wiring complete)");
}
