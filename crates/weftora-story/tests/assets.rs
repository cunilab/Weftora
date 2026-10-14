use std::fs;
use weftora_story::{AssetKind, Story};
fn package() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("main.yarn"),
        "title: Start\n---\nHello\n===\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("media.bin"),
        b"asset resolution fixture, not decode evidence",
    )
    .unwrap();
    fs::write(
        tmp.path().join("images.jsonc"),
        r#"[{"id":"bg.room","path":"media.bin"}]"#,
    )
    .unwrap();
    fs::write(
        tmp.path().join("audio.jsonc"),
        r#"[{"id":"sfx.chime","path":"media.bin"}]"#,
    )
    .unwrap();
    fs::write(
        tmp.path().join("fonts.jsonc"),
        r#"[{"id":"font.dialogue","path":"media.bin"}]"#,
    )
    .unwrap();
    fs::write(tmp.path().join("manifest.jsonc"), r#"{"manifestSchema":1,"requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["assets.catalog/1"]},"id":"dev.test.assets","storyVersion":"0.1.0","title":"Assets","entrypoint":"Start","dialogue":["main.yarn"],"assets":{"legacy":"media.bin"},"catalogs":{"images":["images.jsonc"],"audio":["audio.jsonc"],"fonts":["fonts.jsonc"]}}"#).unwrap();
    tmp
}
#[test]
fn typed_assets_resolve_and_legacy_stays_untyped() {
    let tmp = package();
    let story = Story::load(tmp.path()).unwrap();
    assert_eq!(story.asset_count(), 4);
    for (kind, id) in [
        (AssetKind::Image, "bg.room"),
        (AssetKind::Audio, "sfx.chime"),
        (AssetKind::Font, "font.dialogue"),
    ] {
        assert_eq!(
            story.asset_path_of(kind, id).unwrap(),
            tmp.path().join("media.bin").canonicalize().unwrap()
        );
        assert!(story.asset_path_of(kind, "legacy").is_ok());
    }
    let error = story
        .asset_path_of(AssetKind::Image, "sfx.chime")
        .unwrap_err();
    assert!(error.message.contains("belongs to audio, expected images"));
    let missing = story
        .asset_path_of(AssetKind::Font, "font.missing")
        .unwrap_err();
    assert_eq!(missing.file, "manifest.jsonc");
    assert!(
        missing
            .message
            .contains("missing catalog ID `font.missing`")
    );
    assert_eq!(story.asset_references().count(), 4);
}
#[test]
fn duplicates_across_kinds_and_legacy_report_both_sources() {
    for id in ["bg.room", "legacy"] {
        let tmp = package();
        fs::write(
            tmp.path().join("audio.jsonc"),
            format!(r#"[{{"id":"{id}","path":"media.bin"}}]"#),
        )
        .unwrap();
        let errors = Story::load(tmp.path()).unwrap_err();
        let error = errors
            .iter()
            .find(|e| e.message.contains("duplicate catalog ID"))
            .unwrap();
        assert!(error.message.contains("/0/id"));
        assert!(error.message.contains("first declared at"));
        if id == "legacy" {
            assert!(error.message.contains("manifest.jsonc:/assets/legacy"));
        } else {
            assert!(error.message.contains("audio.jsonc:/0/id"));
            assert_eq!(error.file, "images.jsonc");
        }
    }
}
#[test]
fn typed_paths_and_entry_schema_reject_bad_rows_without_partial_load() {
    for (row, message) in [
        (
            r#"{"id":"bad","path":"../escape"}"#,
            "path must not contain",
        ),
        (r#"{"id":"bad","path":"missing.png"}"#, "missing.png"),
        (
            r#"{"id":"bad","path":"media.bin","unknown":true}"#,
            "unknown field",
        ),
        (r#"{"id":"bad"}"#, "missing field `path`"),
        (r#"{"path":"media.bin"}"#, "missing string catalog ID"),
        (r#"{"id":"","path":"media.bin"}"#, "ID must not be empty"),
    ] {
        let tmp = package();
        fs::write(tmp.path().join("images.jsonc"), format!("[{row}]")).unwrap();
        let errors = Story::load(tmp.path()).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.file == "images.jsonc" && e.message.contains(message)),
            "{errors:?}"
        );
    }
}
#[test]
fn legacy_schema_zero_still_works_and_typed_catalogs_need_schema_one() {
    let tmp = package();
    let path = tmp.path().join("manifest.jsonc");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        text.replace("\"manifestSchema\":1", "\"manifestSchema\":0"),
    )
    .unwrap();
    assert!(
        Story::load(tmp.path())
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("need manifestSchema 1"))
    );
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["manifestSchema"] = 0.into();
    value.as_object_mut().unwrap().remove("requires");
    value.as_object_mut().unwrap().remove("catalogs");
    fs::write(path, value.to_string()).unwrap();
    assert!(
        Story::load(tmp.path())
            .unwrap()
            .asset_path_of(AssetKind::Font, "legacy")
            .is_ok()
    );
}
#[test]
fn media_read_is_bounded_even_after_inventory() {
    let tmp = package();
    let story = Story::load(tmp.path()).unwrap();
    let path = story.asset_path_of(AssetKind::Image, "bg.room").unwrap();
    fs::File::create(&path)
        .unwrap()
        .set_len(weftora_story::paths::MAX_FILE_BYTES + 1)
        .unwrap();
    assert!(
        weftora_story::paths::read_bytes(path)
            .unwrap_err()
            .contains("file size exceeds")
    );
}

#[test]
fn typed_assets_require_the_capability_and_unknown_categories_are_rejected() {
    let tmp = package();
    let manifest = tmp.path().join("manifest.jsonc");
    let original = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, original.replace("[\"assets.catalog/1\"]", "[]")).unwrap();
    assert!(Story::load(tmp.path()).unwrap_err().iter().any(|d| {
        d.message
            .contains("typed asset catalogs require `assets.catalog/1`")
    }));
    fs::write(&manifest, original.replace("\"images\":[", "\"unknown\":[")).unwrap();
    assert!(
        Story::load(tmp.path())
            .unwrap_err()
            .iter()
            .any(|d| d.message.contains("unsupported catalog category `unknown`"))
    );
}
