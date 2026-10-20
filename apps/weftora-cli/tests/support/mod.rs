//! Identity-aware export plumbing fixture; never a substitute for actual-player validation.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn player(directory: &Path) -> PathBuf {
    let identity =
        serde_json::to_string(&weftora_yarn::ToolIdentity::current("weftora-player")).unwrap();
    let source = directory.join("identity-player.rs");
    fs::write(&source, format!(r#"
fn main() {{
    if std::env::args().nth(1).as_deref() != Some("--tool-identity") {{ std::process::exit(2); }}
    match std::env::var("WEFTORA_TEST_PLAYER_MODE").as_deref() {{
        Ok("hang") => std::thread::sleep(std::time::Duration::from_secs(60)),
        Ok("oversized") => println!("{{}}", "x".repeat(9000)),
        Ok("failed") => std::process::exit(7),
        _ => println!("{{}}", std::env::var("WEFTORA_TEST_IDENTITY").unwrap_or_else(|_| {identity:?}.into())),
    }}
}}
"#)).unwrap();
    let player = directory.join(format!("identity-player{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("--crate-name")
        .arg("identity_player")
        .arg(&source)
        .arg("-o")
        .arg(&player)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    player
}
