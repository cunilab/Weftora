mod support;

use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, process::Command, time::Instant};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_weftora"))
}

fn inventory(root: &Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.file_name().unwrap().into(), fs::read(path).unwrap())
        })
        .collect()
}

#[test]
fn tool_identity_is_machine_readable_and_preserves_version_output() {
    let output = cli().arg("--tool-identity").output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let identity: weftora_yarn::ToolIdentity = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(identity, weftora_yarn::ToolIdentity::current("weftora"));
    assert_eq!(identity.build_id.len(), 64);
    assert!(
        identity
            .build_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
    let version = cli().arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .starts_with(&format!("weftora {} (sha256 ", env!("CARGO_PKG_VERSION")))
    );
}

#[test]
fn export_refuses_every_mismatched_identity_and_bad_response_without_writes() {
    let temp = tempfile::tempdir().unwrap();
    let player = support::player(temp.path());
    let story = temp.path().join("source ü");
    assert!(cli().arg("new").arg(&story).status().unwrap().success());
    let source = inventory(&story);
    let original_player = fs::read(&player).unwrap();
    let identity =
        serde_json::to_value(weftora_yarn::ToolIdentity::current("weftora-player")).unwrap();
    let mut responses = Vec::new();
    for (field, wrong) in [
        ("identityFormat", json!(999)),
        ("tool", json!("weftora")),
        ("engineRelease", json!("99.0.0")),
        ("buildId", json!("0".repeat(64))),
        ("target", json!("other-target")),
        ("profile", json!("other-profile")),
        ("dialogueArtifactFormat", json!(999)),
        ("compilerId", json!("other-compiler")),
        ("compilerVersion", json!("99.0.0")),
    ] {
        let mut bad = identity.clone();
        assert_ne!(bad[field], wrong, "fixture must actually change {field}");
        bad[field] = wrong;
        responses.push(bad.to_string());
    }
    let mut unknown = identity.clone();
    unknown["unexpected"] = Value::Bool(true);
    responses.push(unknown.to_string());
    let mut missing = identity;
    missing.as_object_mut().unwrap().remove("buildId");
    responses.extend([
        missing.to_string(),
        "not JSON".into(),
        "{} {}".into(),
        "x".repeat(9000),
    ]);
    let empty = temp.path().join("existing-empty");
    fs::create_dir(&empty).unwrap();
    let absent_root = temp.path().join("absent");
    let nested = absent_root.join("nested/game");
    for response in responses {
        for output in [&empty, &nested] {
            let result = cli()
                .env("WEFTORA_PLAYER", &player)
                .env("WEFTORA_TEST_IDENTITY", &response)
                .arg("export")
                .arg(&story)
                .arg("--out")
                .arg(output)
                .output()
                .unwrap();
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            let message = String::from_utf8_lossy(&result.stderr);
            assert!(
                message.contains("player identity") && message.contains("same build"),
                "{message}"
            );
            assert!(empty.is_dir());
            assert_eq!(fs::read_dir(&empty).unwrap().count(), 0);
            assert!(!absent_root.exists());
            assert_eq!(inventory(&story), source);
            assert_eq!(fs::read(&player).unwrap(), original_player);
        }
    }
    // The matching responder still exports after all refusals; there is no production bypass.
    let result = cli()
        .env("WEFTORA_PLAYER", &player)
        .arg("export")
        .arg(&story)
        .arg("--out")
        .arg(&empty)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(inventory(&story), source);
}

#[test]
fn failed_and_hung_player_queries_are_bounded_and_create_no_output() {
    let temp = tempfile::tempdir().unwrap();
    let player = support::player(temp.path());
    let story = temp.path().join("story");
    assert!(cli().arg("new").arg(&story).status().unwrap().success());
    let before = inventory(&story);
    for mode in ["failed", "oversized", "hang"] {
        let output = temp.path().join(mode);
        let started = Instant::now();
        let result = cli()
            .env("WEFTORA_PLAYER", &player)
            .env("WEFTORA_TEST_PLAYER_MODE", mode)
            .arg("export")
            .arg(&story)
            .arg("--out")
            .arg(&output)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        let message = String::from_utf8_lossy(&result.stderr);
        assert!(
            message.contains("player identity") && message.contains("same build"),
            "{message}"
        );
        if mode == "hang" {
            assert!(message.contains("timed out"), "{message}");
        }
        assert!(started.elapsed().as_secs() < 30);
        assert!(!output.exists());
        assert_eq!(inventory(&story), before);
    }
    fs::write(temp.path().join("not-executable"), b"old or invalid player").unwrap();
    for path in [
        temp.path().join("not-executable"),
        temp.path().join("missing"),
    ] {
        let output = temp.path().join("game");
        let result = cli()
            .env("WEFTORA_PLAYER", path)
            .arg("export")
            .arg(&story)
            .arg("--out")
            .arg(&output)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
    }
    assert_eq!(inventory(&story), before);
}
