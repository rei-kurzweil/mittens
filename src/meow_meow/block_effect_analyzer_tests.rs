use crate::meow_meow::ast::{Expression, Statement};
use crate::meow_meow::block_effect_analyzer::{BlockEffectAnalyzer, EffectKind};
use crate::meow_meow::parser::MeowMeowParser;
use crate::meow_meow::tokenizer::MeowMeowTokenizer;

fn parse(src: &str) -> Vec<Statement> {
    let tokens = MeowMeowTokenizer::new(src).tokenize().expect("tokenize ok");
    MeowMeowParser::new(tokens)
        .parse_program()
        .expect("parse ok")
}

#[test]
fn analyze_keyframe_block_marks_music_note_audio() {
    let prog = parse("Keyframe.at(0.0) { MusicNote.e(4, 0.25, lead) }");
    let Statement::Expression(Expression::Component(ce)) = &prog[0] else {
        panic!("expected keyframe component expression");
    };
    let analysis = BlockEffectAnalyzer::analyze_keyframe_block(&ce.body);
    assert!(analysis.contains_audio_effects);
    assert_eq!(analysis.statements.len(), 1);
    assert_eq!(analysis.statements[0].effect_kind, EffectKind::Audio);
}
