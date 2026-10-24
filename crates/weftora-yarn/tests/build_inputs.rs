#[allow(dead_code)]
#[path = "../build.rs"]
mod engine_build;

#[test]
fn embedded_player_font_participates_in_engine_tool_identity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let sources = engine_build::engine_sources(root);
    assert!(sources.contains(&std::path::PathBuf::from(
        "crates/weftora-render-bevy/assets/FiraSans-Regular.ttf"
    )));
}
