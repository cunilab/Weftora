use serde_json::{Value, json};
use std::{fs, path::Path};

pub const STORIES: [(&str, &str); 2] = [("story-a", "scene.menu"), ("life-sim", "day.home")];
pub const SAVE_CASES: [(&str, &str); 6] = [
    ("corrupt-save", "corrupt save"),
    ("save-format", "unsupported saveFormat 999"),
    ("cross-story", "save belongs to another Story"),
    ("save-schema", "no save migration from schema 1 to 2"),
    ("removed-checkpoint", "save checkpoint '"),
    ("script-runtime", "matrix runtime failure"),
];

pub fn save_files(directory: &Path) -> std::collections::BTreeMap<std::ffi::OsString, Vec<u8>> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect()
}

pub fn copy_story(name: &str, destination: &Path) {
    fn copy(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &destination.join(entry.file_name()));
            } else {
                assert!(entry.file_type().unwrap().is_file());
                fs::copy(entry.path(), destination.join(entry.file_name())).unwrap();
            }
        }
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    assert!(
        source.is_dir(),
        "cannot access Story root {}",
        source.display()
    );
    copy(&source, destination);
    manifest(destination, |value| {
        value["state"]["matrix_probe"] = json!({"default":0})
    });
}

pub fn manifest(root: &Path, edit: impl FnOnce(&mut Value)) {
    let path = root.join("manifest.jsonc");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    edit(&mut value);
    fs::write(path, value.to_string()).unwrap();
}

pub fn replace_yarn(root: &Path, before: &str, after: &str) {
    let path = root.join("main.yarn");
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(
        text.matches(before).count(),
        1,
        "ambiguous fixture edit: {before}"
    );
    fs::write(path, text.replacen(before, after, 1)).unwrap();
}

pub fn script(root: &Path, checkpoint: &str, syntax_error: bool) {
    manifest(root, |value| {
        let capabilities = value["requires"]["capabilities"].as_array_mut().unwrap();
        for capability in ["rules.events/1", "rules.commands/1", "rules.rhai/1"] {
            if !capabilities.contains(&json!(capability)) {
                capabilities.push(json!(capability));
            }
        }
        if value.get("events").is_none() {
            value["events"] = json!([]);
        }
        value["events"]
            .as_array_mut()
            .unwrap()
            .push(json!("matrix-rules.jsonc"));
    });
    fs::write(root.join("matrix-rules.jsonc"), json!({"rulesSchema":1,"commands":[{
        "id":"matrix_fail", "script":{"file":"matrix.rhai","function":"matrix_fail","writes":["matrix_probe"]}
    }]}).to_string()).unwrap();
    fs::write(root.join("matrix.rhai"), if syntax_error {
        "fn matrix_fail(state) { let = ; }"
    } else {
        "fn matrix_fail(state) { state.set(\"matrix_probe\", 99); throw \"matrix runtime failure\"; }"
    }).unwrap();
    let tag = format!("<<checkpoint {checkpoint}>>");
    replace_yarn(root, &tag, &format!("{tag}\n<<matrix_fail>>"));
}

pub const COMPAT_STORIES: [(&str, &str); 2] =
    [("story-a", "scene.repair"), ("life-sim", "day.shift")];

pub fn compatible_base(root: &Path, name: &str) {
    manifest(root, |m| {
        let caps = m["requires"]["capabilities"].as_array_mut().unwrap();
        if !caps.contains(&json!("presentation.media/1")) {
            caps.push(json!("presentation.media/1"));
        }
        m["assets"]["matrix.image"] = "matrix.png".into();
    });
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/presentation-demo/assets/day.png"),
        root.join("matrix.png"),
    )
    .unwrap();
    if name == "story-a" {
        replace_yarn(
            root,
            "<<background bg.night>>\n<<show left actor.alice 500>>\n<<show right actor.bob 500>>",
            "<<background matrix.image>>\n<<show left actor.alice 500>>\n<<show right actor.bob 500>>",
        );
    } else {
        replace_yarn(
            root,
            "<<checkpoint day.shift>>\n<<work_shift>>",
            "<<checkpoint day.shift>>\n<<background matrix.image>>\n<<work_shift>>",
        );
    }
}

pub fn compatible_patch(root: &Path, name: &str) {
    manifest(root, |m| {
        m["storyVersion"] = "0.1.1".into();
        m["state"]["matrix_added"] = json!({"default":23});
    });
    let before = if name == "story-a" {
        "Hold these two wires together. I will tell you when to let go."
    } else {
        "{$status} Earned {$earned} coins."
    };
    replace_yarn(
        root,
        before,
        &format!("Compatibility patch is active. {before}"),
    );
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/presentation-demo/assets/night.png"),
        root.join("matrix.png"),
    )
    .unwrap();
}
