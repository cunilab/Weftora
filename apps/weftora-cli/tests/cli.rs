use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn weftora() -> Command {
    Command::new(env!("CARGO_BIN_EXE_weftora"))
}

fn hello() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello-story")
}

fn broken() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/broken")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn version_prints_hash() {
    let out = weftora().arg("--version").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.starts_with(&format!("weftora {} (sha256 ", env!("CARGO_PKG_VERSION"))),
        "{text}"
    );
    let hex = text
        .split("sha256 ")
        .nth(1)
        .unwrap()
        .trim_end_matches([')', '\n']);
    assert_eq!(hex.len(), 64);
}

#[test]
fn check_ok_on_hello_story() {
    let out = weftora().arg("check").arg(hello()).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("OK: dev.weftora.hello-story"), "{text}");
}

#[test]
fn check_fails_on_broken_story() {
    let out = weftora().arg("check").arg(broken()).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.contains("dialogue/main.yarn:3: `background` uses undeclared asset ID `bg.missing`"),
        "{err}"
    );
    assert!(
        err.contains("dialogue/main.yarn:4: unknown command `teleport`"),
        "{err}"
    );
}

#[test]
fn check_fails_on_missing_story() {
    let out = weftora()
        .arg("check")
        .arg("/nonexistent/story")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("manifest.json"));
}

#[test]
fn export_copies_player_and_story() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_player = tmp.path().join("fake-player");
    fs::write(&fake_player, b"not a real player").unwrap();
    let out_dir = tmp.path().join("export");

    let out = weftora()
        .env("WEFTORA_PLAYER", &fake_player)
        .arg("export")
        .arg(hello())
        .arg("--out")
        .arg(&out_dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));

    let player_name = format!("weftora-player{}", std::env::consts::EXE_SUFFIX);
    assert_eq!(
        fs::read(out_dir.join(player_name)).unwrap(),
        b"not a real player"
    );
    assert!(out_dir.join("story/manifest.json").is_file());
    assert!(out_dir.join("story/dialogue/intro.yarn").is_file());
    assert!(out_dir.join("story/assets/room.png").is_file());

    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(out_dir.join("weftora-export.json")).unwrap()).unwrap();
    assert_eq!(report["exportFormat"], 0);
    assert_eq!(report["story"]["id"], "dev.weftora.hello-story");
    assert_eq!(report["story"]["storyVersion"], "0.1.0");
    assert_eq!(report["cli"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["cli"]["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(report["player"]["sha256"].as_str().unwrap().len(), 64);
    let paths: Vec<&str> = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            "README.md",
            "assets/chime.wav",
            "assets/room.png",
            "dialogue/intro.yarn",
            "manifest.json"
        ]
    );

    // A second export into the now non-empty directory is refused.
    let again = weftora()
        .env("WEFTORA_PLAYER", &fake_player)
        .arg("export")
        .arg(hello())
        .arg("--out")
        .arg(&out_dir)
        .output()
        .unwrap();
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("not empty"));
}

#[test]
fn run_without_player_reports_clear_error() {
    let out = weftora()
        .env("WEFTORA_PLAYER", "/nonexistent/player")
        .arg("run")
        .arg(hello())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("player executable not found"));
}
