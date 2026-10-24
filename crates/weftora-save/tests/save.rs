use serde_json::json;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use weftora_api::Value;
use weftora_core::{Scope, State, StateDefinition};
use weftora_save::{
    Context, Error, MAX_SAVE_BYTES, MAX_STRING_BYTES, SavedGame, Snapshot, storage::Store,
};

fn context() -> Context {
    Context {
        story_id: "dev.weftora.save-test".into(),
        story_version: "0.1.0".into(),
        save_schema: 1,
        writer_engine_api: "0.1.0".into(),
        writer_engine_release: "0.0.1".into(),
        checkpoints: ["scene.intro".into()].into(),
    }
}
fn template() -> State {
    let mut state = State::new();
    state.set("flag", Value::Bool(false)).unwrap();
    state.set("removed", Value::Int(1)).unwrap();
    state
        .declare(
            "session",
            StateDefinition {
                default: Value::Int(0),
                scope: Scope::Session,
            },
        )
        .unwrap();
    state
        .declare(
            "volume",
            StateDefinition {
                default: Value::Float(1.0),
                scope: Scope::Settings,
            },
        )
        .unwrap();
    state.drain_changes();
    state
}
fn game(context: &Context, flag: bool) -> SavedGame {
    SavedGame::new(
        context,
        Snapshot {
            checkpoint: "scene.intro".into(),
            state: BTreeMap::from([
                ("flag".into(), Some(Value::Bool(flag))),
                ("removed".into(), None),
            ]),
            visits: [("Intro".into(), 2)].into(),
        },
    )
    .unwrap()
}

#[test]
fn deterministic_roundtrip_preserves_removal_resets_session_and_keeps_settings() {
    let ctx = context();
    let original = game(&ctx, true);
    let bytes = original.to_bytes().unwrap();
    assert_eq!(original, SavedGame::from_bytes(&bytes, &ctx).unwrap());
    assert_eq!(
        bytes,
        SavedGame::from_bytes(&bytes, &ctx)
            .unwrap()
            .to_bytes()
            .unwrap()
    );
    let mut current = template();
    current.set("session", Value::Int(99)).unwrap();
    current.set("volume", Value::Float(0.25)).unwrap();
    let before = current.clone();
    let mut fresh = template();
    fresh
        .set("new_default", Value::String("new".into()))
        .unwrap();
    let mut candidate = original.prepare_state(&ctx, &fresh, &current).unwrap();
    assert_eq!(candidate.get("flag"), Ok(&Value::Bool(true)));
    assert!(!candidate.exists("removed"));
    assert_eq!(candidate.get("session"), Ok(&Value::Int(0)));
    assert_eq!(candidate.get("volume"), Ok(&Value::Float(0.25)));
    assert_eq!(
        candidate.get("new_default"),
        Ok(&Value::String("new".into()))
    );
    assert!(candidate.drain_changes().is_empty());
    assert_eq!(current, before);
}

#[test]
fn older_writer_api_and_release_numbers_allow_compatible_story_patches() {
    let mut ctx = context();
    let bytes = game(&ctx, true).to_bytes().unwrap();
    ctx.story_version = "0.2.0".into();
    ctx.writer_engine_api = "0.2.0".into();
    ctx.writer_engine_release = "1.0.0".into();
    assert!(SavedGame::from_bytes(&bytes, &ctx).is_ok());
}

#[test]
fn identity_schema_and_checkpoint_failures_leave_the_existing_file_and_live_state_untouched() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context();
    let store = Store::new(root.path().into());
    store.write(&game(&ctx, true)).unwrap();
    let before = fs::read(store.path()).unwrap();
    let live = template();
    let live_before = live.clone();
    for case in ["story", "upgrade", "downgrade", "checkpoint"] {
        let mut other = context();
        match case {
            "story" => other.story_id = "other.story".into(),
            "upgrade" => other.save_schema = 2,
            "downgrade" => {
                let mut newer = context();
                newer.save_schema = 2;
                store.write(&game(&newer, true)).unwrap();
                let files: Vec<_> = ["save.json", "save.bak"]
                    .map(|name| fs::read(root.path().join(name)).unwrap())
                    .into();
                assert!(matches!(
                    store.load(&ctx),
                    Err(Error::StoryDowngrade { from: 2, to: 1 })
                ));
                assert_eq!(
                    files,
                    ["save.json", "save.bak"].map(|name| fs::read(root.path().join(name)).unwrap())
                );
                assert_eq!(live, live_before);
                store.write(&game(&ctx, true)).unwrap();
                continue;
            }
            "checkpoint" => other.checkpoints = ["scene.other".into()].into(),
            _ => unreachable!(),
        }
        assert!(store.load(&other).is_err());
        assert_eq!(fs::read(store.path()).unwrap(), before);
        assert_eq!(live, live_before);
    }
}

#[test]
fn invalid_saved_keys_types_scopes_and_removed_declarations_refuse_candidate_activation() {
    let ctx = context();
    let live = template();
    for (key, value) in [
        ("unknown", Some(Value::Bool(true))),
        ("flag", Some(Value::String("wrong type".into()))),
        ("session", Some(Value::Int(2))),
        ("volume", Some(Value::Float(0.2))),
    ] {
        let save = SavedGame::new(
            &ctx,
            Snapshot {
                checkpoint: "scene.intro".into(),
                state: [(key.into(), value)].into(),
                visits: BTreeMap::new(),
            },
        )
        .unwrap();
        let before = live.clone();
        assert!(save.prepare_state(&ctx, &template(), &live).is_err());
        assert_eq!(live, before);
    }
    let mut changed = State::new();
    changed.set("flag", Value::Bool(false)).unwrap();
    assert!(
        game(&ctx, true)
            .prepare_state(&ctx, &changed, &live)
            .is_err()
    );
}

#[test]
fn malformed_truncated_unknown_format_and_checksum_corruption_are_rejected() {
    let ctx = context();
    let bytes = game(&ctx, true).to_bytes().unwrap();
    let mut changed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    changed["body"]["state"]["flag"] = json!(false);
    for bad in [
        b"not json".to_vec(),
        bytes[..bytes.len() / 2].to_vec(),
        br#"{"saveFormat":2,"futureBody":null}"#.to_vec(),
        serde_json::to_vec(&changed).unwrap(),
    ] {
        assert!(SavedGame::from_bytes(&bad, &ctx).is_err());
    }
    assert!(matches!(
        SavedGame::from_bytes(br#"{"saveFormat":999}"#, &ctx),
        Err(Error::UnsupportedFormat(999))
    ));
}

#[test]
fn duplicate_and_unknown_fields_are_rejected_even_with_a_valid_checksum() {
    use sha2::{Digest, Sha256};
    let ctx = context();
    let bytes = game(&ctx, true).to_bytes().unwrap();
    let root: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for body in [
        root["body"]
            .to_string()
            .replace("\"flag\":true", "\"flag\":true,\"flag\":false"),
        root["body"]
            .to_string()
            .replacen('{', "{\"unknown\":true,", 1),
    ] {
        let mut digest = Sha256::new();
        digest.update(b"weftora-save-format-1\0");
        digest.update(body.as_bytes());
        let hash = format!("{:x}", digest.finalize());
        let encoded = format!("{{\"saveFormat\":1,\"bodySha256\":\"{hash}\",\"body\":{body}}}");
        assert!(SavedGame::from_bytes(encoded.as_bytes(), &ctx).is_err());
    }
}

#[test]
fn oversized_reads_and_writes_and_invalid_values_fail_before_replacing_the_slot() {
    let ctx = context();
    let root = tempfile::tempdir().unwrap();
    let store = Store::new(root.path().into());
    store.write(&game(&ctx, true)).unwrap();
    let before = fs::read(store.path()).unwrap();
    for value in [
        Value::Float(f64::NAN),
        Value::String("x".repeat(MAX_STRING_BYTES + 1)),
    ] {
        assert!(
            SavedGame::new(
                &ctx,
                Snapshot {
                    checkpoint: "scene.intro".into(),
                    state: [("text".into(), Some(value))].into(),
                    visits: BTreeMap::new()
                }
            )
            .is_err()
        );
        assert_eq!(fs::read(store.path()).unwrap(), before);
    }
    let large = SavedGame::new(
        &ctx,
        Snapshot {
            checkpoint: "scene.intro".into(),
            state: (0..600)
                .map(|n| {
                    (
                        format!("key_{n}"),
                        Some(Value::String("x".repeat(MAX_STRING_BYTES))),
                    )
                })
                .collect(),
            visits: BTreeMap::new(),
        },
    )
    .unwrap();
    assert!(matches!(
        store.write(&large),
        Err(Error::TooLarge {
            resource: "save",
            ..
        })
    ));
    assert_eq!(fs::read(store.path()).unwrap(), before);
    let mut previous_file = File::create(store.path()).unwrap();
    previous_file.set_len(MAX_SAVE_BYTES as u64 + 1).unwrap();
    previous_file.write_all(b"nonzero prefix").unwrap();
    previous_file.seek(SeekFrom::End(-4)).unwrap();
    previous_file.write_all(b"tail").unwrap();
    drop(previous_file);
    let previous_bytes = fs::read(store.path()).unwrap();

    assert!(matches!(
        store.load(&ctx),
        Err(Error::TooLarge {
            resource: "save",
            ..
        })
    ));
    // Oversized current-format data can still be preserved as a streamed backup.
    store.write(&game(&ctx, false)).unwrap();
    assert_eq!(fs::read(store.backup_path()).unwrap(), previous_bytes);
    File::create(store.path())
        .unwrap()
        .set_len(weftora_save::storage::MAX_BACKUP_BYTES + 1)
        .unwrap();
    let huge_before = fs::metadata(store.path()).unwrap().len();
    assert!(matches!(
        store.write(&game(&ctx, true)),
        Err(Error::TooLarge { .. })
    ));
    assert_eq!(fs::metadata(store.path()).unwrap().len(), huge_before);
}

#[test]
fn replacing_the_slot_retains_exact_previous_bytes_and_loading_never_falls_back() {
    let ctx = context();
    let root = tempfile::tempdir().unwrap();
    let store = Store::new(root.path().into());
    store.write(&game(&ctx, false)).unwrap();
    let previous = fs::read(store.path()).unwrap();
    store.write(&game(&ctx, true)).unwrap();
    assert_eq!(fs::read(store.backup_path()).unwrap(), previous);
    fs::write(store.path(), b"corrupt current slot").unwrap();
    assert!(store.load(&ctx).is_err());
    assert_eq!(fs::read(store.path()).unwrap(), b"corrupt current slot");
    assert_eq!(fs::read(store.backup_path()).unwrap(), previous);
}

#[test]
fn backup_failure_and_concurrent_writer_refusal_preserve_the_current_slot() {
    let ctx = context();
    let root = tempfile::tempdir().unwrap();
    let store = Store::new(root.path().into());
    store.write(&game(&ctx, false)).unwrap();
    let previous = fs::read(store.path()).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.path().join("save.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(matches!(store.write(&game(&ctx, true)), Err(Error::Busy)));
    assert_eq!(fs::read(store.path()).unwrap(), previous);
    drop(lock);
    fs::create_dir(store.backup_path()).unwrap();
    assert!(store.write(&game(&ctx, true)).is_err());
    assert_eq!(fs::read(store.path()).unwrap(), previous);
    assert!(root.path().read_dir().unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".tmp")
    }));
}

#[cfg(unix)]
#[test]
fn slot_backup_and_lock_links_are_rejected_without_touching_the_target() {
    let ctx = context();
    for name in ["save.json", "save.bak", "save.lock"] {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("original");
        fs::write(&target, b"untouched").unwrap();
        let dir = root.path().join("saves");
        fs::create_dir(&dir).unwrap();
        std::os::unix::fs::symlink(&target, dir.join(name)).unwrap();
        let store = Store::new(dir);
        assert!(store.write(&game(&ctx, true)).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"untouched");
    }
}

#[test]
fn missing_slot_has_a_distinct_error_and_does_not_create_files() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::new(root.path().join("not_created"));
    assert!(matches!(store.load(&context()), Err(Error::Missing)));
    assert!(!root.path().join("not_created").exists());
}

#[test]
fn finite_float_bits_and_serialized_bytes_round_trip_exactly() {
    let mut values = vec![
        -0.0,
        0.1 + 0.2,
        5e-324,
        2.2250738585072014e-308,
        f64::MAX,
        1e23,
    ];
    let mut bits = 42u64;
    for _ in 0..500 {
        bits = bits
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let value = f64::from_bits(bits);
        if value.is_finite() {
            values.push(value);
        }
    }
    let ctx = context();
    for value in values {
        let original = SavedGame::new(
            &ctx,
            Snapshot {
                checkpoint: "scene.intro".into(),
                state: [("number".into(), Some(Value::Float(value)))].into(),
                visits: BTreeMap::new(),
            },
        )
        .unwrap();
        let bytes = original.to_bytes().unwrap();
        let loaded = SavedGame::from_bytes(&bytes, &ctx).unwrap();
        assert_eq!(
            loaded.state(),
            original.state(),
            "float bits {:x}",
            value.to_bits()
        );
        assert_eq!(
            bytes,
            loaded.to_bytes().unwrap(),
            "float bits {:x}",
            value.to_bits()
        );
    }
}

#[cfg(unix)]
#[test]
fn fifo_slot_is_rejected_without_opening() {
    if let Some(directory) = std::env::var_os("WEFTORA_FIFO_CHILD") {
        let store = Store::new(directory.into());
        assert!(matches!(store.load(&context()), Err(Error::Invalid(_))));
        assert!(matches!(
            store.write(&game(&context(), true)),
            Err(Error::Invalid(_))
        ));
        return;
    }
    let root = tempfile::tempdir().unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(root.path().join("save.json"))
            .status()
            .unwrap()
            .success()
    );
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "fifo_slot_is_rejected_without_opening",
            "--nocapture",
        ])
        .env("WEFTORA_FIFO_CHILD", root.path())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "{status}");
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("save operation blocked on a FIFO");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn collection_saves_are_deterministic_preserve_types_bits_and_removal() {
    let ctx = context();
    let mut state = State::new();
    state
        .set(
            "items",
            Value::List(vec![
                Value::String("key".into()),
                Value::Int(2),
                Value::Float(2.0),
            ]),
        )
        .unwrap();
    state
        .set(
            "schedule",
            Value::Map(BTreeMap::from([
                ("z".into(), Value::Float(-0.0)),
                ("a".into(), Value::Bool(true)),
            ])),
        )
        .unwrap();
    state.set("removed", Value::List(vec![])).unwrap();
    state.remove("removed").unwrap();
    let entries = state.snapshot_entries(Scope::Persistent);
    let game = SavedGame::new(
        &ctx,
        Snapshot {
            checkpoint: "scene.intro".into(),
            state: entries.clone(),
            visits: BTreeMap::new(),
        },
    )
    .unwrap();
    let bytes = game.to_bytes().unwrap();
    let loaded = SavedGame::from_bytes(&bytes, &ctx).unwrap();
    assert_eq!(loaded.to_bytes().unwrap(), bytes);
    for (key, value) in &entries {
        match value {
            Some(value) => assert!(value.same_bits(loaded.state()[key].as_ref().unwrap())),
            None => assert_eq!(loaded.state()[key], None),
        }
    }
    let mut fresh = State::new();
    fresh.set("items", Value::List(vec![])).unwrap();
    fresh.set("schedule", Value::Map(BTreeMap::new())).unwrap();
    fresh.set("removed", Value::List(vec![])).unwrap();
    let candidate = loaded.prepare_state(&ctx, &fresh, &fresh).unwrap();
    assert!(candidate.matches_snapshot(Scope::Persistent, &entries));
    assert!(candidate.pending_changes().is_empty());
    let invalid = Snapshot {
        checkpoint: "scene.intro".into(),
        state: BTreeMap::from([(
            "bad".into(),
            Some(Value::List(vec![Value::Float(f64::NAN)])),
        )]),
        visits: BTreeMap::new(),
    };
    assert!(SavedGame::new(&ctx, invalid).is_err());
}

#[test]
fn newer_engine_api_refuses_by_semver_precedence_and_preserves_files_and_live_state() {
    for (writer, reader, accepts) in [
        ("0.1.1", "0.1.0", false),
        ("0.2.0", "0.1.0", false),
        ("1.0.0", "0.1.0", false),
        ("0.1.0", "0.1.1", true),
        ("0.1.0+writer", "0.1.0+reader", true),
        ("0.1.0-beta.2", "0.1.0-beta.1", false),
        ("0.1.0-beta.1", "0.1.0-beta.2", true),
        ("0.1.0", "0.1.0-rc.1", false),
        ("0.1.0-rc.1", "0.1.0", true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let store = Store::new(root.path().into());
        let mut producer = context();
        producer.writer_engine_api = writer.into();
        let game = game(&producer, true);
        store.write(&game).unwrap();
        store.write(&game).unwrap();
        let before: Vec<_> = ["save.json", "save.bak"]
            .map(|name| fs::read(root.path().join(name)).unwrap())
            .into();
        let mut consumer = context();
        consumer.writer_engine_api = reader.into();
        let live = template();
        let old_live = live.clone();
        if accepts {
            assert!(store.load(&consumer).is_ok(), "{writer} -> {reader}");
        } else {
            let error = store.load(&consumer).unwrap_err();
            assert!(matches!(error, Error::NewerEngineApi { .. }));
            let message = error.to_string();
            assert!(
                message.contains(writer)
                    && message.contains(reader)
                    && message.contains("or newer")
            );
            assert!(game.prepare_state(&consumer, &template(), &live).is_err());
        }
        assert_eq!(live, old_live);
        assert_eq!(
            before,
            ["save.json", "save.bak"].map(|name| fs::read(root.path().join(name)).unwrap())
        );
    }
}

#[test]
fn compatible_release_number_rollbacks_do_not_reject_saves() {
    let mut writer = context();
    writer.story_version = "99.0.0".into();
    writer.writer_engine_release = "99.0.0".into();
    let game = game(&writer, true);
    let reader = context();
    assert!(SavedGame::from_bytes(&game.to_bytes().unwrap(), &reader).is_ok());
    assert!(
        game.prepare_state(&reader, &template(), &template())
            .is_ok()
    );
}
