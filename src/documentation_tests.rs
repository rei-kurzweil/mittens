use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::engine::ecs::{CommandQueue, RxWorld, World};
use crate::engine::graphics::RenderAssets;
use crate::scripting::component_registry::SUPPORTED_COMPONENT_NAMES;
use crate::scripting::parser::MeowMeowParser;
use crate::scripting::runner::MeowMeowRunner;
use crate::scripting::tokenizer::MeowMeowTokenizer;

const COMPONENT_GUIDE: &str = "docs/how_to/guide/components.md";
const SIGNAL_GUIDE: &str = "docs/how_to/guide/signals.md";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    fs::read_to_string(root().join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn rust_files_below(path: &Path, output: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_files_below(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

fn words_after(source: &str, needle: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut rest = source;
    while let Some(index) = rest.find(needle) {
        rest = &rest[index + needle.len()..];
        let name: String = rest
            .chars()
            .skip_while(|character| character.is_whitespace())
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            result.push(name);
        }
    }
    result
}

fn marker_attributes(guide: &str, marker: &str) -> Vec<BTreeMap<String, String>> {
    guide
        .lines()
        .filter(|line| line.contains(marker))
        .map(|line| {
            let mut attributes = BTreeMap::new();
            for field in line.split_ascii_whitespace() {
                if let Some((key, value)) = field.split_once('=') {
                    attributes.insert(
                        key.to_string(),
                        value
                            .trim_matches(|character| character == '"' || character == '>')
                            .to_string(),
                    );
                }
            }
            attributes
        })
        .collect()
}

fn assert_exactly_once(
    expected: impl IntoIterator<Item = String>,
    documented: Vec<String>,
    label: &str,
) {
    let mut counts = BTreeMap::<String, usize>::new();
    for name in documented {
        *counts.entry(name).or_default() += 1;
    }
    let expected: BTreeSet<_> = expected.into_iter().collect();
    let actual: BTreeSet<_> = counts.keys().cloned().collect();
    assert_eq!(actual, expected, "{label} catalog differs from source");
    let duplicates: Vec<_> = counts
        .into_iter()
        .filter(|(_, count)| *count != 1)
        .collect();
    assert!(
        duplicates.is_empty(),
        "duplicate {label} entries: {duplicates:?}"
    );
}

fn enum_variants(source: &str, enum_name: &str) -> Vec<String> {
    let uncommented = source
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n");
    let enum_start = uncommented
        .find(&format!("pub enum {enum_name}"))
        .unwrap_or_else(|| panic!("missing enum {enum_name}"));
    let body_start = uncommented[enum_start..].find('{').expect("enum body") + enum_start;
    let mut depth = 1usize;
    let mut segment_start = body_start + 1;
    let mut variants = Vec::new();
    for (offset, character) in uncommented[body_start + 1..].char_indices() {
        let index = body_start + 1 + offset;
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let tail = uncommented[segment_start..index].trim();
                    if let Some(name) = tail
                        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .find(|part| !part.is_empty())
                    {
                        variants.push(name.to_string());
                    }
                    break;
                }
            }
            ',' if depth == 1 => {
                let segment = uncommented[segment_start..index].trim();
                if let Some(name) = segment
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .find(|part| !part.is_empty())
                {
                    variants.push(name.to_string());
                }
                segment_start = index + 1;
            }
            _ => {}
        }
    }
    variants
}

#[test]
fn component_catalog_covers_concrete_implementations_and_mms_names() {
    let mut files = Vec::new();
    rust_files_below(&root().join("src/engine/ecs/component"), &mut files);
    rust_files_below(&root().join("src/engine/ecs/system"), &mut files);
    let implementations = files.into_iter().flat_map(|path| {
        words_after(
            &fs::read_to_string(path).expect("component source"),
            "impl Component for",
        )
    });
    let markers = marker_attributes(&read(COMPONENT_GUIDE), "catalog:component");
    assert_exactly_once(
        implementations,
        markers
            .iter()
            .map(|entry| entry["source"].clone())
            .collect(),
        "component",
    );

    let documented_names: BTreeSet<_> = markers
        .iter()
        .flat_map(|entry| {
            entry
                .get("names")
                .into_iter()
                .flat_map(|names| names.split(','))
        })
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect();
    let supported: BTreeSet<_> = SUPPORTED_COMPONENT_NAMES
        .iter()
        .map(|name| name.to_string())
        .collect();
    assert_eq!(
        documented_names, supported,
        "every registered MMS name must map to a catalog entry"
    );
}

#[test]
fn signal_catalog_covers_both_signal_enums() {
    let source = read("src/engine/ecs/rx/signal.rs");
    let markers = marker_attributes(&read(SIGNAL_GUIDE), "catalog:signal");
    for (enum_name, kind) in [("EventSignal", "event"), ("IntentValue", "intent")] {
        assert_exactly_once(
            enum_variants(&source, enum_name),
            markers
                .iter()
                .filter(|entry| entry["kind"] == kind)
                .map(|entry| entry["source"].clone())
                .collect(),
            kind,
        );
    }
}

fn annotated_mms_fences(guide: &str) -> Vec<(&str, String)> {
    let mut examples = Vec::new();
    let mut annotation = None;
    let mut body = String::new();
    for line in guide.lines() {
        if let Some(info) = line.strip_prefix("```mms ") {
            assert!(
                matches!(info, "parse-only" | "runnable"),
                "unknown MMS fence annotation: {info}"
            );
            annotation = Some(info);
            body.clear();
        } else if line == "```" && annotation.is_some() {
            examples.push((annotation.take().unwrap(), body.clone()));
        } else if annotation.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }
    assert!(annotation.is_none(), "unterminated annotated MMS fence");
    examples
}

#[test]
fn guide_mms_examples_tokenize_parse_and_runnable_examples_evaluate() {
    for relative in [COMPONENT_GUIDE, SIGNAL_GUIDE] {
        for (annotation, source) in annotated_mms_fences(&read(relative)) {
            let tokens = MeowMeowTokenizer::new(&source)
                .tokenize()
                .unwrap_or_else(|error| {
                    panic!("tokenize {relative} example:\n{source}\n{error:?}")
                });
            MeowMeowParser::new(tokens)
                .parse_program()
                .unwrap_or_else(|error| panic!("parse {relative} example:\n{source}\n{error:?}"));
            if annotation == "runnable" {
                let mut world = World::default();
                let mut rx = RxWorld::default();
                let mut assets = RenderAssets::new();
                let mut emit = CommandQueue::new();
                let output = MeowMeowRunner::eval_with_world_and_assets(
                    &source,
                    &mut world,
                    &mut rx,
                    &mut assets,
                    &mut emit,
                );
                assert!(
                    output.errors.is_empty(),
                    "evaluate {relative} example:\n{source}\n{:?}",
                    output.errors
                );
            }
        }
    }
}

#[test]
fn catalog_local_links_exist() {
    for relative in [COMPONENT_GUIDE, SIGNAL_GUIDE] {
        let guide_path = root().join(relative);
        let guide = fs::read_to_string(&guide_path).expect("guide");
        for target in guide
            .split("](")
            .skip(1)
            .filter_map(|tail| tail.split(')').next())
        {
            let target = target.split('#').next().unwrap_or(target);
            if target.is_empty() || target.contains("://") || target.starts_with('#') {
                continue;
            }
            let resolved = guide_path.parent().unwrap().join(target);
            assert!(
                resolved.exists(),
                "broken local link `{target}` in {relative}"
            );
        }
    }
}

#[test]
fn event_exposure_classifications_match_mms_conversion() {
    let markers = marker_attributes(&read(SIGNAL_GUIDE), "catalog:signal");
    let actual: BTreeMap<_, _> = markers
        .iter()
        .filter(|entry| entry["kind"] == "event")
        .map(|entry| (entry["source"].clone(), entry["mms"].clone()))
        .collect();
    let payload = [
        "FrameTick",
        "GltfInitialized",
        "HttpRequest",
        "HttpResponse",
        "HttpError",
    ];
    let partial = [
        "DataEvent",
        "XrButtonDown",
        "XrButtonUp",
        "XrButtonChanged",
        "XrAxisChanged",
        "TextInputChanged",
    ];
    assert_eq!(actual["LayoutRootSizeAvailable"], "unavailable");

    // Keep these labels coupled to both sides of the MMS bridge. A new `on(...)`
    // spelling or payload conversion must be accompanied by a guide change.
    let evaluator = read("src/scripting/world_evaluator.rs");
    let handler_parser = evaluator
        .split("fn parse_signal_kind")
        .nth(1)
        .expect("parse_signal_kind")
        .split("fn is_truthy")
        .next()
        .expect("parse_signal_kind body");
    let runner = read("src/scripting/runner.rs");
    let payload_converter = runner
        .split("fn event_arg_value")
        .nth(1)
        .expect("event_arg_value")
        .split("impl MeowMeowRunner")
        .next()
        .expect("event_arg_value body");

    let handler_name = |name: &str| match name {
        "GltfInitialized" => "GLTFInitialized".to_string(),
        other => other.to_string(),
    };

    for name in payload {
        assert_eq!(actual[name], "observable-payload", "{name}");
        assert!(
            handler_parser.contains(&format!("\"{}\" => Ok(", handler_name(name))),
            "{name} is not accepted by on(...)"
        );
        assert!(
            payload_converter.contains(&format!("EventSignal::{name}")),
            "{name} has no MMS payload conversion"
        );
    }
    for name in partial {
        assert_eq!(actual[name], "observable-partial-payload", "{name}");
        assert!(
            handler_parser.contains(&format!("\"{}\" => Ok(", handler_name(name))),
            "{name} is not accepted by on(...)"
        );
        assert!(
            payload_converter.contains(&format!("EventSignal::{name}")),
            "{name} has no MMS payload conversion"
        );
    }
    for (name, exposure) in actual {
        if name == "LayoutRootSizeAvailable" {
            assert!(!handler_parser.contains(&format!("\"{}\" => Ok(", handler_name(&name))));
            continue;
        }
        assert!(
            handler_parser.contains(&format!("\"{}\" => Ok(", handler_name(&name))),
            "{name} is not accepted by on(...)"
        );
        if !payload.contains(&name.as_str()) && !partial.contains(&name.as_str()) {
            assert!(
                !payload_converter.contains(&format!("EventSignal::{name}")),
                "{name} no longer converts to null"
            );
        }
        if name != "LayoutRootSizeAvailable"
            && !payload.contains(&name.as_str())
            && !partial.contains(&name.as_str())
        {
            assert_eq!(exposure, "observable-null", "{name}");
        }
    }
}
