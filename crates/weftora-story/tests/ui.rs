use serde_json::{Value, json};
use std::fs;
use weftora_story::{Story, ui::ScreenDefinition};

fn document() -> Value {
    json!({"screenSchema":1,"root":{"type":"screen","id":"dialogue","children":[
        {"type":"text","bind":{"state":"curious"}},
        {"type":"choiceList","id":"choices"},
        {"type":"button","id":"next","text":"Next","action":{"do":"dialogue.advance"}}
    ]}})
}
fn package(doc: Value) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("main.yarn"),
        "title: Start\n---\nHello\n===\n",
    )
    .unwrap();
    fs::write(tmp.path().join("screen.jsonc"), doc.to_string()).unwrap();
    fs::write(tmp.path().join("manifest.jsonc"), json!({"manifestSchema":1,"id":"test.ui","storyVersion":"0.1.0","title":"UI","entrypoint":"Start","dialogue":["main.yarn"],"requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["ui.screens/1"]},"initialState":{"curious":false},"screens":["screen.jsonc"],"initialScreen":"dialogue"}).to_string()).unwrap();
    tmp
}
fn errors(doc: Value) -> String {
    Story::load(package(doc).path())
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn valid_screen_loads_and_state_validation_is_separate() {
    let tmp = package(document());
    let story = Story::load(tmp.path()).unwrap();
    let screen = story.screen("dialogue").unwrap();
    assert!(
        screen
            .state_errors(&story.initial_state().unwrap())
            .is_empty()
    );
    let mut doc = document();
    doc["root"]["children"][0]["bind"]["state"] = "unknown.key".into();
    let tmp = package(doc);
    let story = Story::load(tmp.path()).unwrap();
    let diagnostics = story
        .screen("dialogue")
        .unwrap()
        .state_errors(&story.initial_state().unwrap());
    assert_eq!(diagnostics[0].file, "screen.jsonc");
    assert!(
        diagnostics[0]
            .message
            .contains("/root/children/0/bind/state")
    );
}
#[test]
fn action_type_and_missing_key_report_author_pointer() {
    for (key, value, expected) in [
        ("curious", json!(1), "expects bool, got int"),
        ("missing", json!(true), "does not exist"),
    ] {
        let mut doc = document();
        doc["root"]["children"][2]["action"] = json!({"do":"state.set","key":key,"value":value});
        let tmp = package(doc);
        let story = Story::load(tmp.path()).unwrap();
        let diagnostics = story
            .screen("dialogue")
            .unwrap()
            .state_errors(&story.initial_state().unwrap());
        assert!(diagnostics[0].message.contains(expected));
        assert!(diagnostics[0].message.contains("/root/children/2/action/"));
    }
}
#[test]
fn malformed_widgets_and_cross_references_are_rejected() {
    for (pointer, value, expected) in [
        (
            "/root/children/2/id",
            json!("choices"),
            "duplicate widget ID",
        ),
        (
            "/root/children/0/bind",
            json!({"state":"curious","dialogue":"text"}),
            "did not match",
        ),
        (
            "/root/children/2/action",
            json!({"do":"screen.open","screen":"missing"}),
            "/root/children/2/action/screen",
        ),
        (
            "/root/children/2/action",
            json!({"do":"screen.close"}),
            "only valid in overlays",
        ),
        (
            "/root/children/0/font",
            json!("missing"),
            "/root/children/0/font",
        ),
        ("/root/children/0/size", json!(201), "within 6..=200"),
        ("/root/children/0/color", json!("red"), "#RRGGBB"),
    ] {
        let mut doc = document();
        // Optional fields need insertion rather than pointer_mut.
        let (parent, field) = pointer.rsplit_once('/').unwrap();
        doc.pointer_mut(parent).unwrap()[field] = value;
        assert!(
            errors(doc).contains(expected),
            "{pointer}: expected {expected}"
        );
    }
    let mut doc = document();
    doc["root"]["children"][0]["unknown"] = true.into();
    assert!(errors(doc).contains("unknown field"));
    let mut doc = document();
    doc["screenSchema"] = 99.into();
    doc["future"] = true.into();
    assert!(errors(doc).contains("unsupported screenSchema 99"));
}
#[test]
fn capability_initial_screen_and_path_are_checked() {
    for (field, value, expected) in [
        ("initialScreen", json!("missing"), "/initialScreen"),
        (
            "screens",
            json!(["../outside.jsonc"]),
            "path must not contain",
        ),
        (
            "screens",
            json!(["screen.jsonc", "screen.jsonc"]),
            "duplicate source",
        ),
        (
            "requires",
            json!({"engineApi":">=0.1.0 <0.2.0","capabilities":[]}),
            "ui.screens/1",
        ),
        ("manifestSchema", json!(0), "need manifestSchema 1"),
    ] {
        let tmp = package(document());
        let path = tmp.path().join("manifest.jsonc");
        let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        manifest[field] = value;
        fs::write(path, manifest.to_string()).unwrap();
        assert!(
            Story::load(tmp.path())
                .unwrap_err()
                .iter()
                .any(|e| e.message.contains(expected)),
            "{field}: {expected}"
        );
    }
}
#[test]
fn bounds_and_root_contracts_are_enforced() {
    let mut doc = document();
    doc["root"]["children"] = json!([]);
    assert!(errors(doc).contains("exactly one choiceList"));
    let mut doc = document();
    doc["root"]["type"] = "overlay".into();
    assert!(errors(doc).contains("overlays cannot contain choiceList"));
    let mut doc = document();
    doc["root"]["children"][0]["text"] = "x".repeat(4097).into();
    assert!(errors(doc).contains("4096 bytes"));
    let mut doc = document();
    doc["root"]["layout"] = json!({"width":"101%"});
    assert!(errors(doc).contains("within 0..=100"));
    let mut doc = document();
    doc["root"]["children"] = Value::Array((0..129).map(|_| json!({"type":"panel"})).collect());
    assert!(errors(doc).contains("128 direct children"));
    let mut root = json!({"type":"panel"});
    for _ in 0..18 {
        root = json!({"type":"panel","children":[root]});
    }
    let mut doc = document();
    doc["root"]["children"][0] = root;
    assert!(errors(doc).contains("16 nesting levels"));
}
#[test]
fn typed_image_required_even_if_legacy_id_exists() {
    let mut doc = document();
    doc["root"]["children"][0] = json!({"type":"image","image":"legacy"});
    let tmp = package(doc);
    fs::write(tmp.path().join("image.png"), b"resolution fixture").unwrap();
    let path = tmp.path().join("manifest.jsonc");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["assets"] = json!({"legacy":"image.png"});
    fs::write(path, manifest.to_string()).unwrap();
    assert!(
        Story::load(tmp.path())
            .unwrap_err()
            .iter()
            .any(|e| e.message.contains("typed images catalog asset"))
    );
}
#[test]
fn finite_lengths_include_direct_api_values() {
    for n in [f32::NAN, f32::INFINITY, -1.0, 8193.0] {
        assert!(weftora_story::ui::Length::Pixels(n).value().is_err());
    }
    let definition = ScreenDefinition {
        file: "screen.jsonc".into(),
        document: serde_json::from_value(document()).unwrap(),
    };
    assert!(definition.structure_errors().is_empty());
}

#[test]
fn stories_without_screens_keep_legacy_compatibility() {
    let tmp = package(document());
    let path = tmp.path().join("manifest.jsonc");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for field in ["screens", "initialScreen", "requires"] {
        manifest.as_object_mut().unwrap().remove(field);
    }
    manifest["manifestSchema"] = 0.into();
    fs::write(path, manifest.to_string()).unwrap();
    let story = Story::load(tmp.path()).unwrap();
    assert_eq!(story.screens().count(), 0);
    assert!(story.manifest().initial_screen.is_none());
}
