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

fn edit_manifest(root: &Path, edit: impl FnOnce(&mut Value)) {
    let path = root.join("manifest.jsonc");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    edit(&mut manifest);
    fs::write(path, manifest.to_string()).unwrap();
}

fn runtime_fixture() -> tempfile::TempDir {
    let dir = fixture();
    fs::write(dir.path().join("font.bin"), b"typed resolution only").unwrap();
    fs::write(
        dir.path().join("fonts.jsonc"),
        r#"[{"id":"font.common","path":"font.bin"}]"#,
    )
    .unwrap();
    fs::write(dir.path().join("screen.jsonc"), json!({"screenSchema":1,"root":{"type":"screen","id":"main","children":[
        {"type":"text","bind":{"localized":"next"}},
        {"type":"button","id":"next","bind":{"localized":"next"},"action":{"do":"state.set","key":"language","value":"id"}},
        {"type":"choiceList","id":"choices"}
    ]}}).to_string()).unwrap();
    edit_manifest(dir.path(), |m| {
        m["requires"]["capabilities"] = json!([
            "dialogue.yarn/1",
            "localization.catalog/1",
            "localization.text/1",
            "assets.catalog/1",
            "ui.screens/1",
            "rules.events/1",
            "rules.commands/1",
            "rules.rhai/1"
        ]);
        m["localization"]["stateKey"] = json!("language");
        m["localization"]["font"] = json!("font.common");
        m["state"] = json!({"language":{"scope":"settings","default":"en"}});
        m["catalogs"] = json!({"fonts":["fonts.jsonc"]});
        m["screens"] = json!(["screen.jsonc"]);
        m["initialScreen"] = json!("main");
    });
    dir
}

#[test]
fn runtime_locale_uses_canonical_settings_and_rejects_unknown_values() {
    let dir = runtime_fixture();
    fs::write(
        dir.path().join("id.jsonc"),
        r#"{"localizationSchema":1,"ui":{"next":"Lanjut"}}"#,
    )
    .unwrap();
    let story = Story::load(dir.path()).unwrap();
    let catalogs = story.localization().unwrap();
    let mut state = story.initial_state().unwrap();
    assert_eq!(catalogs.language(&state).unwrap(), "en");
    state
        .set("language", weftora_api::Value::String("id".into()))
        .unwrap();
    assert_eq!(catalogs.ui_text(&state, "next").unwrap(), "Lanjut");
    state
        .set("language", weftora_api::Value::String("unknown".into()))
        .unwrap();
    assert!(
        catalogs
            .language(&state)
            .unwrap_err()
            .contains("supported language")
    );
}

#[test]
fn runtime_schema_requires_settings_supported_defaults_and_typed_font() {
    for (pointer, value, expected) in [
        (
            "/localization/stateKey",
            Value::Null,
            "requires localization.stateKey",
        ),
        ("/localization/font", json!("missing"), "typed font"),
        (
            "/state/language/scope",
            json!("persistent"),
            "Settings string",
        ),
        (
            "/state/language/default",
            json!("xx"),
            "supported language default",
        ),
        ("/state/language/default", json!(1), "Settings string"),
        (
            "/requires/capabilities",
            json!(["localization.catalog/1", "assets.catalog/1", "ui.screens/1"]),
            "requires localization.text/1",
        ),
    ] {
        let dir = runtime_fixture();
        edit_manifest(dir.path(), |m| *m.pointer_mut(pointer).unwrap() = value);
        assert!(
            errors(dir.path()).contains(expected),
            "{}",
            errors(dir.path())
        );
    }
    let dir = runtime_fixture();
    edit_manifest(dir.path(), |m| m["initialState"] = json!({"language":"xx"}));
    assert!(errors(dir.path()).contains("initialState must name a supported language"));
}

#[test]
fn localized_bindings_and_authored_language_actions_validate_before_activation() {
    for (field, value, expected) in [
        (
            "bind",
            json!({"localized":"missing"}),
            "UI key present in every language",
        ),
        (
            "bind",
            json!({"localized":"next","state":"language"}),
            "did not match",
        ),
        (
            "action",
            json!({"do":"state.set","key":"language","value":"xx"}),
            "supported language",
        ),
    ] {
        let dir = runtime_fixture();
        let path = dir.path().join("screen.jsonc");
        let mut screen: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        screen["root"]["children"][1][field] = value;
        fs::write(path, screen.to_string()).unwrap();
        assert!(
            errors(dir.path()).contains(expected),
            "{}",
            errors(dir.path())
        );
    }
}

#[test]
fn rules_and_scripts_cannot_observe_locale_settings() {
    for (trigger, condition, expected) in [
        ("state.changed", Value::Null, "may not drive state.changed"),
        (
            "state.changed.language",
            Value::Null,
            "may not drive state.changed",
        ),
        (
            "test",
            json!({"op":"compare","operator":"==","left":{"state":"language"},"right":{"literal":"en"}}),
            "conditions may not read",
        ),
    ] {
        let dir = runtime_fixture();
        edit_manifest(dir.path(), |m| m["events"] = json!(["events.jsonc"]));
        fs::write(dir.path().join("events.jsonc"), json!({"rulesSchema":1,"events":[{"id":"test","trigger":trigger,"condition":condition,"actions":[]}]}).to_string()).unwrap();
        assert!(
            errors(dir.path()).contains(expected),
            "{}",
            errors(dir.path())
        );
    }
    let dir = runtime_fixture();
    edit_manifest(dir.path(), |m| m["events"] = json!(["events.jsonc"]));
    fs::write(dir.path().join("script.rhai"), "fn run(state) {}").unwrap();
    fs::write(dir.path().join("events.jsonc"), json!({"rulesSchema":1,"events":[],"commands":[{"id":"test","script":{"file":"script.rhai","function":"run","reads":["language"]}}]}).to_string()).unwrap();
    assert!(
        errors(dir.path()).contains("scripts may not read localization Settings"),
        "{}",
        errors(dir.path())
    );
}

#[test]
fn catalog_only_defaults_to_base_and_runtime_capability_requires_configuration() {
    let dir = fixture();
    let story = Story::load(dir.path()).unwrap();
    assert_eq!(
        story
            .localization()
            .unwrap()
            .language(&story.initial_state().unwrap())
            .unwrap(),
        "en"
    );
    let dir = runtime_fixture();
    edit_manifest(dir.path(), |m| m["localization"] = Value::Null);
    assert!(errors(dir.path()).contains("requires localization"));
    let dir = runtime_fixture();
    edit_manifest(dir.path(), |m| {
        m["requires"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .retain(|c| c != "localization.catalog/1");
    });
    assert!(errors(dir.path()).contains("requires localization.catalog/1"));
}

#[test]
fn rules_cannot_write_reset_or_remove_locale() {
    let dir = runtime_fixture();
    edit_manifest(dir.path(), |m| m["events"] = json!(["events.jsonc"]));
    for action in [
        json!({"do":"state.set","key":"language","value":"id"}),
        json!({"do":"state.reset","key":"language"}),
        json!({"do":"state.remove","key":"language"}),
    ] {
        fs::write(dir.path().join("events.jsonc"), json!({"rulesSchema":1,"events":[{"id":"language","trigger":"select.language","actions":[action]}]}).to_string()).unwrap();
        assert!(
            errors(dir.path()).contains("rules may not write localization Settings"),
            "{}",
            errors(dir.path())
        );
    }
}

#[test]
fn scripts_cannot_write_locale() {
    let dir = runtime_fixture();
    edit_manifest(dir.path(), |m| m["events"] = json!(["events.jsonc"]));
    fs::write(dir.path().join("script.rhai"), "fn run(state) {}").unwrap();
    fs::write(dir.path().join("events.jsonc"), json!({"rulesSchema":1,"events":[],"commands":[{"id":"test","script":{"file":"script.rhai","function":"run","writes":["language"]}}]}).to_string()).unwrap();
    assert!(
        errors(dir.path()).contains("scripts may not write localization Settings"),
        "{}",
        errors(dir.path())
    );
}
