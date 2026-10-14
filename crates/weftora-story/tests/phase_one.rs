use std::fs;
use weftora_api::Value;
use weftora_core::Scope;
use weftora_story::{MANIFEST_FILE, Story};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("main.yarn"),
        "title: Start\n---\nHello.\n===\n",
    )
    .unwrap();
    fs::write(dir.path().join(MANIFEST_FILE), r#"{
      "manifestSchema":1,"id":"test.story","storyVersion":"0.1.0","title":"Test",
      "requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["dialogue.yarn/1"]},
      "entrypoint":"Start","dialogue":["main.yarn"],
      "state":{"world.day":{"default":1,"scope":"persistent"},"ui.language":{"default":"en","scope":"settings"}}
    }"#).unwrap();
    dir
}
fn change(dir: &tempfile::TempDir, from: &str, to: &str) {
    let text = fs::read_to_string(dir.path().join(MANIFEST_FILE)).unwrap();
    assert!(text.contains(from));
    fs::write(dir.path().join(MANIFEST_FILE), text.replace(from, to)).unwrap();
}
#[test]
fn v1_defaults_scopes_and_rejected_compatibility() {
    let dir = fixture();
    let story = Story::load(dir.path()).unwrap();
    let state = story.initial_state().unwrap();
    assert_eq!(state.get("world.day"), Ok(&Value::Int(1)));
    assert_eq!(state.scope("ui.language"), Ok(Scope::Settings));
    for (from, to, message) in [
        (
            "\"manifestSchema\":1",
            "\"manifestSchema\":99",
            "unsupported manifestSchema 99",
        ),
        (
            ">=0.1.0 <0.2.0",
            ">=0.2.0 <0.3.0",
            "engine provides `0.1.0`",
        ),
        ("dialogue.yarn/1", "ui.basic/1", "ui.basic/1"),
        (
            "\"storyVersion\":\"0.1.0\"",
            "\"storyVersion\":\"oops\"",
            "storyVersion",
        ),
    ] {
        let dir = fixture();
        change(&dir, from, to);
        let diagnostics = Story::load(dir.path()).unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.message.contains(message)),
            "{diagnostics:?}"
        );
    }
    let dir = fixture();
    change(
        &dir,
        "\"state\":",
        "\"initialState\":{\"world.day\":true},\"state\":",
    );
    assert!(
        Story::load(dir.path())
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("expected int"))
    );
}

#[test]
fn catalog_duplicate_missing_ids_and_duplicate_json_keys_report_source() {
    let dir = fixture();
    change(
        &dir,
        "\"entrypoint\":",
        "\"catalogs\":{\"data\":[\"items.jsonc\"]},\"entrypoint\":",
    );
    fs::write(
        dir.path().join("items.jsonc"),
        "[{\"id\":\"item.apple\",\"price\":2}]",
    )
    .unwrap();
    let story = Story::load(dir.path()).unwrap();
    assert_eq!(
        story
            .catalog("data")
            .unwrap()
            .get("item.apple", "items.jsonc", "/0/id")
            .unwrap()["price"],
        2
    );
    let error = story
        .catalog("data")
        .unwrap()
        .get("item.missing", "items.jsonc", "/1/id")
        .unwrap_err();
    assert!(error.message.contains("/1/id"));
    for (text, message) in [
        ("[{\"id\":\"a\"},{\"id\":\"a\"}]", "/1/id: duplicate"),
        ("[{\"price\":2}]", "/0/id: missing"),
        (
            "[{\"id\":\"a\",\"id\":\"b\"}]",
            "duplicate ID or field `id`",
        ),
    ] {
        fs::write(dir.path().join("items.jsonc"), text).unwrap();
        let diagnostics = Story::load(dir.path()).unwrap_err();
        assert_eq!(diagnostics[0].file, "items.jsonc");
        assert!(diagnostics[0].message.contains(message), "{diagnostics:?}");
        if message.contains("field") {
            assert_eq!(diagnostics[0].line, Some(1));
        }
    }
    let dir = fixture();
    change(
        &dir,
        "\"dialogue\":[\"main.yarn\"]",
        "\"dialogue\":[\"main.yarn\",\"main.yarn\"]",
    );
    assert!(
        Story::load(dir.path())
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("duplicate source"))
    );
    let dir = fixture();
    change(
        &dir,
        "\"entrypoint\":",
        "\"assets\":{\n\"bg\":\"main.yarn\",\n\"bg\":\"main.yarn\"},\"entrypoint\":",
    );
    let diagnostics = Story::load(dir.path()).unwrap_err();
    assert_eq!(diagnostics[0].file, MANIFEST_FILE);
    assert_eq!(diagnostics[0].line, Some(6));
    assert!(diagnostics[0].message.contains("duplicate"));
}

#[test]
fn oversized_file_is_rejected_without_reading_it() {
    let dir = fixture();
    let file = fs::File::create(dir.path().join("unused.bin")).unwrap();
    file.set_len(weftora_story::paths::MAX_FILE_BYTES + 1)
        .unwrap();
    assert!(
        weftora_story::paths::read_text(dir.path().join("unused.bin"))
            .unwrap_err()
            .contains("file size")
    );
    assert!(
        Story::load(dir.path()).unwrap_err()[0]
            .message
            .contains("file size")
    );
}

#[cfg(windows)]
#[test]
fn windows_junction_escape_and_case_alias_are_rejected_before_activation() {
    let dir = fixture();
    let outside = tempfile::tempdir().unwrap();
    let junction = dir.path().join("escape");
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "junction fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = Story::load(dir.path());
    fs::remove_dir(&junction).unwrap(); // Remove the junction, never the external target.
    assert!(result.unwrap_err()[0].message.contains("junction"));
    let dir = fixture();
    change(&dir, "main.yarn", "MAIN.YARN");
    assert!(
        Story::load(dir.path())
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("differs in case"))
    );
}

#[cfg(windows)]
#[test]
fn windows_symlink_escape_is_rejected_before_activation() {
    let dir = fixture();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("external");
    fs::write(&target, "secret").unwrap();
    // GitHub's Windows runner is an administrator; a missing symlink privilege must fail this fixture.
    std::os::windows::fs::symlink_file(&target, dir.path().join("escape")).unwrap();
    assert!(
        Story::load(dir.path()).unwrap_err()[0]
            .message
            .contains("symlink")
    );
}

#[test]
fn unsupported_schema_wins_over_future_fields_and_missing_legacy_fields() {
    for text in [
        r#"{"manifestSchema":2,"saveSchema":1}"#,
        r#"{"manifestSchema":99,"newDialogueFormat":{}}"#,
    ] {
        let dir = fixture();
        fs::write(dir.path().join(MANIFEST_FILE), text).unwrap();
        let errors = Story::load(dir.path()).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("unsupported manifestSchema"));
        assert!(
            !errors[0].message.contains("unknown field")
                && !errors[0].message.contains("missing field")
        );
    }
}

#[test]
fn declaration_errors_do_not_hide_override_type_mismatches() {
    let dir = fixture();
    change(
        &dir,
        "\"state\":{",
        "\"initialState\":{\"world.day\":true},\"state\":{\"Bad\":{\"default\":1},",
    );
    let errors = Story::load(dir.path()).unwrap_err();
    assert!(errors.iter().any(|d| d.message.contains("Bad")));
    assert!(
        errors
            .iter()
            .any(|d| d.message.contains("world.day") && d.message.contains("expected int"))
    );
}

#[test]
fn duplicate_sources_and_noncanonical_separators_are_rejected() {
    let dir = fixture();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::rename(
        dir.path().join("main.yarn"),
        dir.path().join("sub/main.yarn"),
    )
    .unwrap();
    change(
        &dir,
        "\"dialogue\":[\"main.yarn\"]",
        "\"dialogue\":[\"sub/main.yarn\",\"sub//main.yarn\"]",
    );
    assert!(
        Story::load(dir.path())
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("invalid Windows path segment"))
    );
    let dir = fixture();
    fs::write(dir.path().join("items.jsonc"), "[]").unwrap();
    change(
        &dir,
        "\"entrypoint\":",
        "\"catalogs\":{\"data\":[\"items.jsonc\",\"items.jsonc\"]},\"entrypoint\":",
    );
    assert!(
        Story::load(dir.path())
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("duplicate catalog source"))
    );
    let dir = fixture();
    fs::write(dir.path().join("first.jsonc"), "[{\"id\":\"item.apple\"}]").unwrap();
    fs::write(dir.path().join("second.jsonc"), "[{\"id\":\"item.apple\"}]").unwrap();
    change(
        &dir,
        "\"entrypoint\":",
        "\"catalogs\":{\"data\":[\"first.jsonc\",\"second.jsonc\"]},\"entrypoint\":",
    );
    let errors = Story::load(dir.path()).unwrap_err();
    assert_eq!(errors[0].file, "second.jsonc");
    assert!(errors[0].message.contains("first.jsonc:/0/id"));
}

#[test]
fn whole_loader_enforces_the_real_file_count_boundary() {
    let dir = fixture();
    for index in 2..weftora_story::paths::MAX_FILES {
        fs::write(dir.path().join(format!("f{index:04}")), "").unwrap();
    }
    assert!(Story::load(dir.path()).is_ok());
    fs::write(dir.path().join("one-too-many"), "").unwrap();
    assert!(
        Story::load(dir.path()).unwrap_err()[0]
            .message
            .contains("file count exceeds 4096")
    );
}

#[test]
fn whole_loader_enforces_the_real_total_size_boundary() {
    let dir = fixture();
    let initial = fs::metadata(dir.path().join(MANIFEST_FILE)).unwrap().len()
        + fs::metadata(dir.path().join("main.yarn")).unwrap().len();
    let mut remaining = weftora_story::paths::MAX_TOTAL_BYTES - initial;
    let mut index = 0;
    while remaining > 0 {
        let length = remaining.min(weftora_story::paths::MAX_FILE_BYTES);
        fs::File::create(dir.path().join(format!("f{index:02}")))
            .unwrap()
            .set_len(length)
            .unwrap();
        remaining -= length;
        index += 1;
    }
    assert!(Story::load(dir.path()).is_ok());
    fs::write(dir.path().join("one-too-many"), "x").unwrap();
    assert!(
        Story::load(dir.path()).unwrap_err()[0]
            .message
            .contains("total size")
    );
}
