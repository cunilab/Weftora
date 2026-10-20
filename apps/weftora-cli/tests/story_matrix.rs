mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/story_matrix.rs"
    ));
}
use fixture::{manifest, replace_yarn, script};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cli(verb: &str, root: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_weftora"))
        .arg(verb)
        .arg(root)
        .args(flags)
        .output()
        .unwrap()
}
fn success(output: Output) -> Vec<Value> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn refused(output: Output, expected: &str) {
    let error = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected {expected}: {error}"
    );
    assert!(
        output.stdout.is_empty(),
        "candidate output was published: {error}"
    );
    assert!(error.contains(expected), "expected {expected}: {error}");
    assert!(!error.contains("panicked at"), "{error}");
}
fn save(root: &Path, saves: &Path, checkpoint: &str) {
    success(cli(
        "run",
        root,
        &[
            "--headless",
            "--save-at",
            checkpoint,
            "--save-dir",
            saves.to_str().unwrap(),
        ],
    ));
}

#[test]
fn story_matrix_rejects_invalid_packages_and_loads_without_publishing_or_replacing_files() {
    for (name, checkpoint) in fixture::STORIES {
        for (case, expected) in PACKAGE_CASES {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("story");
            fixture::copy_story(name, &root);
            package_case(&root, checkpoint, case);
            refused(cli("check", &root, &[]), expected);
            refused(cli("run", &root, &["--headless"]), expected);
            let out = temp.path().join("export");
            refused(
                cli("export", &root, &["--out", out.to_str().unwrap()]),
                expected,
            );
            assert!(
                !out.exists(),
                "failed export created a partial package: {name}/{case}"
            );
            fs::create_dir(&out).unwrap();
            fs::write(out.join("keep.txt"), b"existing output").unwrap();
            refused(
                cli("export", &root, &["--out", out.to_str().unwrap()]),
                expected,
            );
            assert_eq!(fs::read(out.join("keep.txt")).unwrap(), b"existing output");
            assert_eq!(fs::read_dir(&out).unwrap().count(), 1);
            eprintln!("PASS {name}/package/{case}: check, run and export refused before output");
        }
        for (case, expected) in fixture::SAVE_CASES {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("story");
            let saves = temp.path().join("data");
            fixture::copy_story(name, &root);
            save(&root, &saves, checkpoint);
            save(&root, &saves, checkpoint);
            let slot = saves.join("save.json");
            match case {
                "corrupt-save" => fs::write(&slot, b"not JSON").unwrap(),
                "save-format" => {
                    let mut value: Value =
                        serde_json::from_slice(&fs::read(&slot).unwrap()).unwrap();
                    value["saveFormat"] = 999.into();
                    fs::write(&slot, value.to_string()).unwrap();
                }
                "cross-story" => {
                    let other = fixture::STORIES
                        .into_iter()
                        .find(|(other, _)| *other != name)
                        .unwrap();
                    let source = temp.path().join("other");
                    let source_saves = temp.path().join("other-data");
                    fixture::copy_story(other.0, &source);
                    save(&source, &source_saves, other.1);
                    fs::copy(source_saves.join("save.json"), &slot).unwrap();
                }
                "save-schema" => fixture::manifest(&root, |m| m["saveSchema"] = 2.into()),
                "removed-checkpoint" => fixture::replace_yarn(
                    &root,
                    &format!("<<checkpoint {checkpoint}>>"),
                    "<<checkpoint matrix.replaced>>",
                ),
                "script-runtime" => {
                    fixture::script(&root, checkpoint, false);
                    // Static validation must succeed; this row must exercise runtime failure.
                    assert!(cli("check", &root, &[]).status.success());
                    refused(cli("run", &root, &["--headless"]), expected);
                }
                _ => unreachable!(),
            }
            let before = fixture::save_files(&saves);
            refused(
                cli(
                    "run",
                    &root,
                    &[
                        "--headless",
                        "--load",
                        "--save-dir",
                        saves.to_str().unwrap(),
                    ],
                ),
                expected,
            );
            assert_eq!(fixture::save_files(&saves), before, "{name}/{case}");
            eprintln!("PASS {name}/load/{case}: no output; slot and backup unchanged");
        }
    }
}

#[test]
fn story_matrix_compatible_revisions_keep_old_saves_and_enforce_version_boundaries() {
    use weftora_story::compatibility::{self, CAPABILITIES};
    for (name, checkpoint) in fixture::COMPAT_STORIES {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        let saves = temp.path().join("data");
        fixture::copy_story(name, &root);
        fixture::compatible_base(&root, name);
        let story = weftora_story::Story::load(&root).unwrap();
        let requirements = story.manifest().requires.as_ref().unwrap();
        for api in ["0.1.0", "0.1.1"] {
            compatibility::check(requirements, api, CAPABILITIES).unwrap();
        }
        for api in ["0.2.0", "0.1.1-beta.1"] {
            let errors = compatibility::check(requirements, api, CAPABILITIES)
                .unwrap_err()
                .join("\n");
            assert!(errors.contains(api) && errors.contains(&requirements.engine_api));
            assert!(errors.contains("prerelease comparator"));
        }
        let mut prerelease = requirements.clone();
        prerelease.engine_api = ">=0.1.1-beta.1 <0.2.0".into();
        compatibility::check(&prerelease, "0.1.1-beta.2", CAPABILITIES).unwrap();
        eprintln!(
            "PASS {name}/compatibility/api-policy: unchanged requirements accept compatible API; outside range and unnamed prerelease refuse"
        );
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        let bytes = compiled.to_artifact_bytes().unwrap();
        assert!(weftora_yarn::CompiledDialogue::from_artifact_bytes(&bytes).is_ok());
        let mut artifact: Value = serde_json::from_slice(&bytes).unwrap();
        artifact["dialogueArtifactFormat"] = 999.into();
        let error = match weftora_yarn::CompiledDialogue::from_artifact_bytes(
            artifact.to_string().as_bytes(),
        ) {
            Ok(_) => panic!("future dialogue format accepted"),
            Err(error) => error,
        };
        assert!(
            error.contains("unsupported dialogueArtifactFormat 999"),
            "{error}"
        );
        eprintln!(
            "PASS {name}/compatibility/dialogue-format: valid artifact accepts; future format refuses specifically"
        );
        let full_choices = if name == "story-a" { "0,0" } else { "0,0,0,0" };
        let baseline = success(cli(
            "run",
            &root,
            &["--headless", "--choices", full_choices],
        ));
        for _ in 0..2 {
            success(cli(
                "run",
                &root,
                &[
                    "--headless",
                    "--choices",
                    "0,0",
                    "--save-at",
                    checkpoint,
                    "--save-dir",
                    saves.to_str().unwrap(),
                ],
            ));
        }
        let slot = saves.join("save.json");
        let mut context = weftora_yarn::save_context(&story, &compiled).unwrap();
        let game =
            weftora_save::SavedGame::from_bytes(&fs::read(&slot).unwrap(), &context).unwrap();
        // Model an earlier Engine patch with the same API, without pretending to run another binary.
        context.writer_engine_release = "0.0.0".into();
        let snapshot = weftora_save::Snapshot {
            checkpoint: game.checkpoint().into(),
            state: game.state().clone(),
            visits: game.visits().clone(),
        };
        let older = weftora_save::SavedGame::new(&context, snapshot)
            .unwrap()
            .to_bytes()
            .unwrap();
        for file in ["save.json", "save.bak"] {
            fs::write(saves.join(file), &older).unwrap();
            let recorded: Value =
                serde_json::from_slice(&fs::read(saves.join(file)).unwrap()).unwrap();
            assert_eq!(recorded["body"]["writerEngineRelease"], "0.0.0");
            assert_eq!(recorded["body"]["writerEngineApi"], "0.1.0");
        }
        let files = fixture::save_files(&saves);
        let old_image = fs::read(root.join("matrix.png")).unwrap();
        fixture::compatible_patch(&root, name);
        assert_ne!(fs::read(root.join("matrix.png")).unwrap(), old_image);
        let patched = weftora_story::Story::load(&root).unwrap();
        assert_eq!(patched.manifest().save_schema, story.manifest().save_schema);
        let patched_compiled =
            weftora_yarn_compiler::compile(&patched.dialogue_sources().unwrap()).unwrap();
        assert_eq!(patched_compiled.checkpoints(), compiled.checkpoints());
        assert_ne!(patched_compiled.to_artifact_bytes().unwrap(), bytes);
        let mut flags = vec![
            "--headless",
            "--load",
            "--save-dir",
            saves.to_str().unwrap(),
        ];
        if name == "life-sim" {
            flags.extend(["--choices", "0,0"]);
        }
        let loaded = success(cli("run", &root, &flags));
        assert!(loaded.iter().any(|event| {
            event["event"] == "line"
                && event["text"]
                    .as_str()
                    .is_some_and(|text| text.contains("Compatibility patch is active."))
        }));
        let mut expected = baseline.last().unwrap()["state"].clone();
        expected["matrix_added"] = 23.into();
        assert_eq!(loaded.last().unwrap()["event"], "end");
        assert_eq!(loaded.last().unwrap()["state"], expected);
        assert_eq!(fixture::save_files(&saves), files);
        eprintln!(
            "PASS {name}/compatibility/text-asset-patch: old checkpoint completes with patched text, changed image bytes and new-field default; saves unchanged"
        );
        eprintln!(
            "PASS {name}/compatibility/writer-version: earlier Engine release diagnostics do not reject same-API save"
        );
    }
}

const PACKAGE_CASES: [(&str, &str); 11] = [
    ("malformed-manifest", "EOF"),
    ("manifest-schema", "unsupported manifestSchema 999"),
    ("api-range", ">=99.0.0"),
    ("capability", "matrix.unsupported/1"),
    ("missing-dialogue", "missing.yarn"),
    (
        "path-traversal",
        "path must not contain `.` or `..` segments",
    ),
    ("missing-entrypoint", "MatrixMissing"),
    ("missing-screen", "matrix-missing"),
    ("missing-binding", "matrix.missing"),
    ("unknown-command", "matrix_unknown"),
    ("script-syntax", "Expecting name of a variable"),
];

fn package_case(root: &Path, checkpoint: &str, case: &str) {
    match case {
        "malformed-manifest" => fs::write(root.join("manifest.jsonc"), "{").unwrap(),
        "manifest-schema" => manifest(root, |m| m["manifestSchema"] = json!(999)),
        "api-range" => manifest(root, |m| m["requires"]["engineApi"] = json!(">=99.0.0")),
        "capability" => manifest(root, |m| {
            m["requires"]["capabilities"]
                .as_array_mut()
                .unwrap()
                .push(json!("matrix.unsupported/1"))
        }),
        "missing-dialogue" => manifest(root, |m| m["dialogue"] = json!(["missing.yarn"])),
        "path-traversal" => {
            fs::copy(
                root.join("main.yarn"),
                root.parent().unwrap().join("outside.yarn"),
            )
            .unwrap();
            manifest(root, |m| m["dialogue"] = json!(["../outside.yarn"]));
        }
        "missing-entrypoint" => manifest(root, |m| m["entrypoint"] = json!("MatrixMissing")),
        "missing-screen" => manifest(root, |m| m["initialScreen"] = json!("matrix-missing")),
        "missing-binding" => {
            let m: Value =
                serde_json::from_slice(&fs::read(root.join("manifest.jsonc")).unwrap()).unwrap();
            let path = root.join(m["screens"][0].as_str().unwrap());
            let mut screen: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            screen["root"]["children"]
                .as_array_mut()
                .unwrap()
                .push(json!({"type":"text","bind":{"state":"matrix.missing"}}));
            fs::write(path, screen.to_string()).unwrap();
        }
        "unknown-command" => {
            let tag = format!("<<checkpoint {checkpoint}>>");
            replace_yarn(root, &tag, &format!("{tag}\n<<matrix_unknown>>"));
        }
        "script-syntax" => script(root, checkpoint, true),
        _ => panic!("unknown package case {case}"),
    }
}
