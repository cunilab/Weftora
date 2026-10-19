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
