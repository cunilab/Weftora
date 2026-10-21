use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use weftora_story::Story;

fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/localization-demo")
}

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_story(dir.path());
    dir
}

fn copy_story(root: &Path) {
    fs::create_dir_all(root.join("locales")).unwrap();
    for file in [
        "manifest.jsonc",
        "main.yarn",
        "locales/en.jsonc",
        "locales/id.jsonc",
        "locales/ja.jsonc",
    ] {
        fs::copy(source().join(file), root.join(file)).unwrap();
    }
}

fn edit(root: &Path, file: &str, pointer: &str, value: Value) {
    let path = root.join(file);
    let mut document: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    *document.pointer_mut(pointer).unwrap() = value;
    fs::write(path, document.to_string()).unwrap();
}

fn check(root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_weftora"))
        .arg("check")
        .arg(root)
        .output()
        .unwrap()
}

#[test]
fn three_language_catalogs_validate_and_base_dialogue_still_runs() {
    let out = check(&source());
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let story = Story::load(&source()).unwrap();
    let catalogs = story.localization().unwrap();
    assert_eq!(
        catalogs
            .languages
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["en", "id", "ja"]
    );
    assert_eq!(catalogs.languages["ja"].ui["continue"], "続ける");
    for choice in ["0", "1"] {
        let out = Command::new(env!("CARGO_BIN_EXE_weftora"))
            .args(["run"])
            .arg(source())
            .args(["--headless", "--choices", choice])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let events: Vec<Value> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(
            events
                .iter()
                .any(|e| e["text"] == "Hello, Mira. You have 7 coins."),
            "{events:?}"
        );
        assert_eq!(events.last().unwrap()["event"], "end");
    }
}

#[test]
fn invalid_catalogs_refuse_check_run_and_export_before_activation() {
    let cases = [
        (
            "manifest.jsonc",
            "/requires/capabilities",
            json!(["dialogue.yarn/1"]),
            "localization requires localization.catalog/1",
        ),
        (
            "manifest.jsonc",
            "/localization/baseLanguage",
            json!("fr"),
            "baseLanguage",
        ),
        (
            "manifest.jsonc",
            "/localization/languages",
            json!({}),
            "1..=8",
        ),
        (
            "manifest.jsonc",
            "/localization/languages",
            json!({"en":"locales/en.jsonc","ID":"locales/id.jsonc","ja":"locales/ja.jsonc"}),
            "canonical BCP 47",
        ),
        (
            "manifest.jsonc",
            "/localization/languages",
            json!({"en":"locales/en.jsonc","invalid_tag!":"locales/id.jsonc","ja":"locales/ja.jsonc"}),
            "canonical BCP 47",
        ),
        (
            "manifest.jsonc",
            "/localization/languages/id",
            json!("../outside.jsonc"),
            "path must not contain `.` or `..` segments",
        ),
        (
            "manifest.jsonc",
            "/localization/languages/id",
            json!("locales/en.jsonc"),
            "duplicate catalog path",
        ),
        (
            "locales/en.jsonc",
            "/dialogue",
            json!({"line:greeting":"Duplicate English"}),
            "base language dialogue must be empty",
        ),
        (
            "locales/id.jsonc",
            "/localizationSchema",
            json!(2),
            "unsupported localizationSchema 2",
        ),
        (
            "locales/id.jsonc",
            "/ui",
            json!({"language":"Bahasa"}),
            "/ui/continue: missing translation key",
        ),
        (
            "locales/ja.jsonc",
            "/ui",
            json!({"language":"言語","continue":"続ける","extra":"余分"}),
            "/ui/extra: unknown translation key",
        ),
        (
            "locales/id.jsonc",
            "/dialogue",
            json!({}),
            "/dialogue/line:visit: missing translation key",
        ),
        (
            "locales/id.jsonc",
            "/dialogue",
            json!({"line:unknown":"Unknown"}),
            "/dialogue/line:unknown: unknown translation key",
        ),
        (
            "locales/id.jsonc",
            "/dialogue/line:greeting",
            json!("Hai {0}"),
            "placeholders must match",
        ),
        (
            "locales/id.jsonc",
            "/dialogue/line:greeting",
            json!("Hai {0} {1} {2}"),
            "placeholders must match",
        ),
        (
            "locales/id.jsonc",
            "/dialogue/line:greeting",
            json!("Hai {00} {1}"),
            "placeholders must match",
        ),
        (
            "locales/id.jsonc",
            "/dialogue/line:greeting",
            json!("Hai {0} {1} {999999999999999999999999999999999}"),
            "placeholders must match",
        ),
        (
            "locales/id.jsonc",
            "/ui/language",
            json!("x".repeat(4097)),
            "4096 UTF-8 bytes",
        ),
        (
            "locales/ja.jsonc",
            "/ui/language",
            json!("あ".repeat(1366)),
            "4096 UTF-8 bytes",
        ),
        (
            "locales/id.jsonc",
            "/ui/language",
            json!("bad\u{0000}text"),
            "without controls",
        ),
    ];
    for (file, pointer, value, expected) in cases {
        let dir = fixture();
        edit(dir.path(), file, pointer, value);
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().join("export");
        for verb in ["check", "run", "export"] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_weftora"));
            command.arg(verb).arg(dir.path());
            if verb == "run" {
                command.arg("--headless");
            }
            if verb == "export" {
                command.arg("--out").arg(&destination);
            }
            let out = command.output().unwrap();
            let message = String::from_utf8_lossy(&out.stderr);
            assert!(!out.status.success(), "{file}{pointer} / {verb}");
            assert!(
                message.contains(expected),
                "{file}{pointer} / {verb}: {message}"
            );
            assert!(out.stdout.is_empty(), "invalid Story must not activate");
            assert!(
                !destination.exists(),
                "invalid Story must not create export output"
            );
        }
    }
}

#[test]
fn canonical_regional_and_script_language_identifiers_are_supported() {
    for language in ["ja-JP", "zh-Hant"] {
        let dir = fixture();
        edit(
            dir.path(),
            "manifest.jsonc",
            "/localization/languages",
            json!({
                "en":"locales/en.jsonc", "id":"locales/id.jsonc", language:"locales/ja.jsonc"
            }),
        );
        let out = check(dir.path());
        assert!(
            out.status.success(),
            "{language}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn artifact_validation_checks_translations_without_the_compiler() {
    let dir = fixture();
    let story = Story::load(dir.path()).unwrap();
    let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
    let bytes = compiled.to_artifact_bytes().unwrap();
    let mut artifact = weftora_yarn::CompiledDialogue::from_artifact_bytes(&bytes).unwrap();
    artifact
        .verify_sources(&story.dialogue_sources().unwrap())
        .unwrap();
    assert!(weftora_yarn::check_story(&story, &artifact).is_empty());
    edit(
        dir.path(),
        "locales/ja.jsonc",
        "/dialogue/line:greeting",
        json!("{1}枚、{0}さん、{1}枚。"),
    );
    let reordered = Story::load(dir.path()).unwrap();
    assert!(weftora_yarn::check_story(&reordered, &artifact).is_empty());
    edit(
        dir.path(),
        "locales/ja.jsonc",
        "/dialogue/line:greeting",
        json!("{0}さん。"),
    );
    let missing = Story::load(dir.path()).unwrap();
    assert!(
        weftora_yarn::check_story(&missing, &artifact)
            .iter()
            .any(|e| e.to_string().contains("placeholders must match"))
    );
    // Translation edits do not change the compiled English artifact or source hash.
    artifact
        .verify_sources(&missing.dialogue_sources().unwrap())
        .unwrap();
    assert_eq!(artifact.to_artifact_bytes().unwrap(), bytes);
}

#[test]
fn yarn_ids_preserve_case_and_accept_long_explicit_and_generated_tags() {
    let dir = fixture();
    fs::write(
        dir.path().join("main.yarn"),
        format!(
            "title: Start\n---\nHello. #line:Greeting\nImplicit.\nLong. #line:{}\n===\n",
            "a".repeat(70)
        ),
    )
    .unwrap();
    let story = Story::load(dir.path()).unwrap();
    let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
    let bytes = compiled.to_artifact_bytes().unwrap();
    let artifact: Value = serde_json::from_slice(&bytes).unwrap();
    let strings = artifact["body"]["strings"].as_object().unwrap();
    assert!(strings.contains_key("line:Greeting"));
    // The pinned compiler generates decimal CRC32 IDs, independent of node-name casing.
    assert!(strings.keys().any(|key| {
        key.strip_prefix("line:")
            .is_some_and(|id| id.bytes().all(|b| b.is_ascii_digit()))
    }));
    let translated: serde_json::Map<String, Value> = strings
        .keys()
        .map(|key| (key.clone(), json!("翻訳")))
        .collect();
    for file in ["locales/id.jsonc", "locales/ja.jsonc"] {
        edit(dir.path(), file, "/dialogue", json!(translated));
    }
    let out = check(dir.path());
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let story = Story::load(dir.path()).unwrap();
    let artifact = weftora_yarn::CompiledDialogue::from_artifact_bytes(&bytes).unwrap();
    assert!(weftora_yarn::check_story(&story, &artifact).is_empty());
}

#[test]
fn existing_catalog_outside_story_root_is_rejected_by_path_policy() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("story");
    copy_story(&root);
    fs::copy(
        root.join("locales/id.jsonc"),
        dir.path().join("outside.jsonc"),
    )
    .unwrap();
    edit(
        &root,
        "manifest.jsonc",
        "/localization/languages/id",
        json!("../outside.jsonc"),
    );
    let out = check(&root);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("path must not contain `.` or `..` segments")
    );
    assert!(out.stdout.is_empty());
}
