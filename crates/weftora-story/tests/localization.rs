use serde_json::{Value, json};
use std::{fs, path::Path};
use weftora_story::{
    Story,
    localization::{MAX_CATALOG_BYTES, MAX_ENTRIES},
};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("main.yarn"),
        "title: Start\n---\nHello. #line:hello\n===\n",
    )
    .unwrap();
    fs::write(dir.path().join("manifest.jsonc"), json!({
        "manifestSchema":1,"id":"test.localization","storyVersion":"0.1.0","title":"Test",
        "requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["dialogue.yarn/1","localization.catalog/1"]},
        "entrypoint":"Start","dialogue":["main.yarn"],
        "localization":{"baseLanguage":"en","languages":{"en":"en.jsonc","id":"id.jsonc"}}
    }).to_string()).unwrap();
    for language in ["en", "id"] {
        fs::write(
            dir.path().join(format!("{language}.jsonc")),
            r#"{"localizationSchema":1,"ui":{"next":"Continue"}}"#,
        )
        .unwrap();
    }
    dir
}

fn errors(root: &Path) -> String {
    Story::load(root)
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn catalog_trust_boundaries_are_strict_and_bounded() {
    for (text, expected) in [
        (r#"{"localizationSchema":1,"ui":{"next":"A","next":"B"}}"#.to_owned(), "duplicate"),
        (r#"{"localizationSchema":1,"ui":{},"unknown":true}"#.to_owned(), "unknown field"),
        (r#"{"localizationSchema":1,"ui":{"INVALID":"A"}}"#.to_owned(), "invalid localization key"),
        (r#"{"localizationSchema":1,"ui":{},"dialogue":{"hello":"A"}}"#.to_owned(), "invalid localization key"),
        (" ".repeat(MAX_CATALOG_BYTES + 1), "exceeds 1048576 bytes"),
        (json!({"localizationSchema":1,"ui":(0..=MAX_ENTRIES).map(|n| (format!("key{n}"), json!("x"))).collect::<serde_json::Map<_,_>>()} ).to_string(), "exceeds 4096 entries"),
    ] {
        let dir = fixture();
        fs::write(dir.path().join("id.jsonc"), text).unwrap();
        assert!(errors(dir.path()).contains(expected), "expected {expected}: {}", errors(dir.path()));
    }
    let dir = fixture();
    let text = r#"// JSONC stays supported
    {"localizationSchema":1,"ui":{"next":"Lanjut\n\tkan"},}
    "#;
    fs::write(dir.path().join("id.jsonc"), text).unwrap();
    let story = Story::load(dir.path()).unwrap();
    assert_eq!(
        story.localization().unwrap().languages["id"].ui["next"],
        "Lanjut\n\tkan"
    );
    fs::write(
        dir.path().join("id.jsonc"),
        json!({"localizationSchema":1,"ui":{"\u{001b}[31m":"x"}}).to_string(),
    )
    .unwrap();
    let message = errors(dir.path());
    assert!(message.contains("invalid localization key"));
    assert!(
        !message.contains('\u{001b}'),
        "invalid keys must not inject terminal escapes"
    );
}

#[test]
fn manifest_requires_configuration_and_v1_with_bounded_language_count() {
    for (pointer, value, expected) in [
        (
            "/localization",
            Value::Null,
            "localization.catalog/1 requires localization",
        ),
        (
            "/manifestSchema",
            json!(0),
            "localization need manifestSchema 1",
        ),
        (
            "/localization/languages",
            json!(
                (0..9)
                    .map(|n| (format!("x{n}"), json!(format!("x{n}.jsonc"))))
                    .collect::<serde_json::Map<_, _>>()
            ),
            "1..=8",
        ),
        (
            "/localization/languages",
            json!({"en":"en.jsonc","id":"EN.JSONC"}),
            "duplicate catalog path",
        ),
    ] {
        let dir = fixture();
        let path = dir.path().join("manifest.jsonc");
        let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        *manifest.pointer_mut(pointer).unwrap() = value;
        fs::write(path, manifest.to_string()).unwrap();
        assert!(
            errors(dir.path()).contains(expected),
            "{}",
            errors(dir.path())
        );
    }
}
