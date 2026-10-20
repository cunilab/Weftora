mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/story_matrix.rs"
    ));
}

#[test]
fn story_matrix_compatible_patch_restores_old_checkpoint_with_new_text_image_and_defaults() {
    use std::fs;
    use weftora_story::ui::UiAction;
    for (name, checkpoint) in fixture::COMPAT_STORIES {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        let data = temp.path().join("data");
        fixture::copy_story(name, &root);
        fixture::compatible_base(&root, name);
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story.clone(), compiled.clone());
        app.world_mut().non_send_mut::<Player>().save_store = Some(Ok(Store::new(data.clone())));
        app.world_mut().resource_mut::<Ui>().reset(&story);
        app.update();
        for _ in 0..40 {
            let player = app.world().non_send::<Player>();
            if player
                .presenter
                .source()
                .save_point()
                .is_ok_and(|point| point.checkpoint == checkpoint)
            {
                break;
            }
            match player.presenter.screen() {
                Screen::Line { .. } => request(&mut app, UiAction::Advance),
                Screen::Choices(_) => {
                    app.world_mut()
                        .resource_mut::<Intents>()
                        .push(Intent::Select {
                            layer: None,
                            position: 0,
                        });
                    app.update();
                }
                other => panic!("did not reach {checkpoint}: {other:?}"),
            }
        }
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .presenter
                .source()
                .save_point()
                .unwrap()
                .checkpoint,
            checkpoint
        );
        app.world_mut().resource_mut::<Presentation>().now += Duration::from_secs(2);
        app.update();
        for _ in 0..2 {
            request(&mut app, UiAction::Save);
            assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
        }
        let mut expected = app
            .world()
            .non_send::<Player>()
            .state
            .read()
            .unwrap()
            .snapshot(weftora_core::Scope::Persistent);
        let old_pixels = crate::decode_image(
            "matrix.image",
            "png",
            &fs::read(root.join("matrix.png")).unwrap(),
        )
        .unwrap()
        .data;
        let files = fixture::save_files(&data);
        drop(app);
        fixture::compatible_patch(&root, name);
        let revised = weftora_story::Story::load(&root).unwrap();
        assert_eq!(revised.manifest().save_schema, story.manifest().save_schema);
        assert_eq!(revised.manifest().story_version, "0.1.1");
        let new_compiled =
            weftora_yarn_compiler::compile(&revised.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&revised, &new_compiled).is_empty());
        assert_eq!(new_compiled.checkpoints(), compiled.checkpoints());
        let new_pixels = crate::decode_image(
            "matrix.image",
            "png",
            &fs::read(root.join("matrix.png")).unwrap(),
        )
        .unwrap()
        .data;
        assert_ne!(new_pixels, old_pixels);
        // A fresh session has no old-revision media cache; runtime hot reload is not the contract.
        let mut app = crate::tests::app_for(revised.clone(), new_compiled);
        app.world_mut().non_send_mut::<Player>().save_store = Some(Ok(Store::new(data.clone())));
        app.world_mut().resource_mut::<Ui>().reset(&revised);
        app.update();
        set(&app, "matrix_probe", Value::Int(77));
        let old_state = app.world().non_send::<Player>().state.clone();
        let images_before: Vec<_> = app
            .world()
            .resource::<Assets<Image>>()
            .iter()
            .map(|(id, _)| id)
            .collect();
        request(&mut app, UiAction::Load);
        assert_eq!(app.world().non_send::<Player>().save_status, "Loaded.");
        app.world_mut().resource_mut::<Presentation>().now += Duration::from_secs(2);
        app.update();
        let player = app.world().non_send::<Player>();
        assert!(!Arc::ptr_eq(&old_state, &player.state));
        expected.insert("matrix_added".into(), Value::Int(23));
        assert_eq!(
            player
                .state
                .read()
                .unwrap()
                .snapshot(weftora_core::Scope::Persistent),
            expected
        );
        assert_eq!(
            player.presenter.source().save_point().unwrap().checkpoint,
            checkpoint
        );
        assert!(
            matches!(player.presenter.screen(), Screen::Line {text,..} if text.contains("Compatibility patch is active."))
        );
        let media = app.world().resource::<Presentation>();
        assert!(
            media
                .views
                .samples(media.now)
                .iter()
                .any(|view| view.asset == "matrix.image")
        );
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .iter()
                .any(|(id, image)| !images_before.contains(&id) && image.data == new_pixels)
        );
        assert_eq!(fixture::save_files(&data), files);
        eprintln!(
            "PASS {name}/compatibility/player-patch: old noninitial checkpoint restores patched text, newly decoded image pixels, new-field default and once-only mechanics; files unchanged"
        );
    }
}

#[test]
fn story_matrix_failed_loads_preserve_live_state_dialogue_ui_audio_and_files() {
    use std::fs;
    use weftora_story::ui::UiAction;
    for (name, checkpoint) in fixture::STORIES {
        for (case, expected) in fixture::SAVE_CASES.into_iter().chain([
            ("display-validation", "bound text exceeds 4096 bytes"),
            ("media-restore", "matrix.image"),
        ]) {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("story");
            fixture::copy_story(name, &root);
            let story = weftora_story::Story::load(&root).unwrap();
            let compiled =
                weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
            assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
            let original_compiled = compiled.clone();
            let mut app = crate::tests::app_for(story.clone(), compiled);
            app.world_mut().non_send_mut::<Player>().save_store =
                Some(Ok(Store::new(temp.path().join("data"))));
            app.world_mut().resource_mut::<Ui>().reset(&story);
            app.update();
            app.world_mut().resource_mut::<Presentation>().now += Duration::from_secs(2);
            app.update();
            request(&mut app, UiAction::Save);
            assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
            request(&mut app, UiAction::Save);
            assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
            let slot = temp.path().join("data/save.json");
            let good_slot = fs::read(&slot).unwrap();
            let point = app
                .world()
                .non_send::<Player>()
                .presenter
                .source()
                .save_point()
                .unwrap();
            assert_eq!(point.checkpoint, checkpoint);
            match case {
                "corrupt-save" => fs::write(&slot, b"not JSON").unwrap(),
                "save-format" => {
                    let mut value: serde_json::Value =
                        serde_json::from_slice(&fs::read(&slot).unwrap()).unwrap();
                    value["saveFormat"] = 999.into();
                    fs::write(&slot, value.to_string()).unwrap();
                }
                "cross-story" => {
                    let player = app.world().non_send::<Player>();
                    let mut context =
                        weftora_yarn::save_context(&player.story, &player.compiled).unwrap();
                    let other = fixture::STORIES
                        .into_iter()
                        .find(|(other, _)| *other != name)
                        .unwrap();
                    let other_root = temp.path().join("other");
                    fixture::copy_story(other.0, &other_root);
                    context.story_id = weftora_story::Story::load(&other_root)
                        .unwrap()
                        .manifest()
                        .id
                        .clone();
                    let game =
                        SavedGame::new(&context, player.presenter.source().save_point().unwrap())
                            .unwrap();
                    fs::write(&slot, game.to_bytes().unwrap()).unwrap();
                }
                "save-schema" => fixture::manifest(&root, |m| m["saveSchema"] = 2.into()),
                "removed-checkpoint" => fixture::replace_yarn(
                    &root,
                    &format!("<<checkpoint {checkpoint}>>"),
                    "<<checkpoint matrix.replaced>>",
                ),
                "script-runtime" => fixture::script(&root, checkpoint, false),
                "display-validation" => {
                    fixture::manifest(&root, |m| {
                        m["state"]["matrix_label"] = serde_json::json!({"default":"short"})
                    });
                    let path = root.join(story.manifest().screens[0].as_str());
                    let mut screen: serde_json::Value =
                        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    screen["root"]["children"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!({"type":"text","bind":{"state":"matrix_label"}}));
                    fs::write(path, screen.to_string()).unwrap();
                }
                "media-restore" => {
                    fixture::manifest(&root, |m| {
                        let caps = m["requires"]["capabilities"].as_array_mut().unwrap();
                        if !caps.contains(&serde_json::json!("presentation.media/1")) {
                            caps.push(serde_json::json!("presentation.media/1"));
                        }
                        m["assets"]["matrix.image"] = "matrix.png".into();
                    });
                    fs::write(root.join("matrix.png"), b"not a png").unwrap();
                    let tag = format!("<<checkpoint {checkpoint}>>");
                    fixture::replace_yarn(
                        &root,
                        &tag,
                        &format!("{tag}\n<<background matrix.image>>"),
                    );
                }
                _ => unreachable!(),
            }
            // Install the revised package metadata; the active runner still owns the live session.
            let revised = weftora_story::Story::load(&root).unwrap();
            let compiled =
                weftora_yarn_compiler::compile(&revised.dialogue_sources().unwrap()).unwrap();
            assert!(weftora_yarn::check_story(&revised, &compiled).is_empty());
            if case == "display-validation" {
                let context = weftora_yarn::save_context(&revised, &compiled).unwrap();
                let mut snapshot = point.clone();
                snapshot
                    .state
                    .insert("matrix_label".into(), Some(Value::String("x".repeat(4097))));
                fs::write(
                    &slot,
                    SavedGame::new(&context, snapshot)
                        .unwrap()
                        .to_bytes()
                        .unwrap(),
                )
                .unwrap();
            }
            {
                let mut player = app.world_mut().non_send_mut::<Player>();
                player.story = revised;
                player.compiled = compiled;
            }
            if case == "media-restore" {
                assert!(
                    app.world().non_send::<Player>().prepare_load().is_ok(),
                    "media failure must happen after candidate preparation"
                );
            }
            set(&app, "matrix_probe", Value::Int(77));
            // Keep a noninitial layer open: a failed load must not reset navigation.
            let overlay = if name == "story-a" {
                "volume"
            } else {
                "journal"
            };
            request(
                &mut app,
                UiAction::Open {
                    screen: overlay.into(),
                },
            );
            assert!(app.world().resource::<Ui>().accepts(&Some(overlay.into())));
            let player = app.world().non_send::<Player>();
            let live = player.state.clone();
            let before = live.read().unwrap().clone();
            let screen = player.presenter.screen().clone();
            let revision = player.presenter.revision();
            assert!(matches!(
                player.presenter.source().save_point(),
                Err(weftora_yarn::SaveRefusal::Changed)
            ));
            let media = app.world().resource::<Presentation>();
            let views = media.views.samples(media.now);
            let audio: Vec<_> = media
                .audio
                .cues
                .iter()
                .map(|cue| {
                    (
                        cue.channel,
                        cue.asset.clone(),
                        cue.token,
                        cue.envelope.sample(media.now),
                    )
                })
                .collect();
            if name == "story-a" {
                assert!(!audio.is_empty());
            }
            let files = fixture::save_files(&temp.path().join("data"));
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::F9);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            let player = app.world().non_send::<Player>();
            assert!(
                player.save_status.contains(expected),
                "{name}/{case}: {}",
                player.save_status
            );
            assert!(Arc::ptr_eq(&live, &player.state));
            assert_eq!(*player.state.read().unwrap(), before, "{name}/{case}");
            assert_eq!(player.presenter.screen(), &screen);
            assert_eq!(player.presenter.revision(), revision);
            assert!(matches!(
                player.presenter.source().save_point(),
                Err(weftora_yarn::SaveRefusal::Changed)
            ));
            assert!(app.world().resource::<Ui>().accepts(&Some(overlay.into())));
            let media = app.world().resource::<Presentation>();
            assert_eq!(media.views.samples(media.now), views);
            assert_eq!(
                media
                    .audio
                    .cues
                    .iter()
                    .map(|cue| (
                        cue.channel,
                        cue.asset.clone(),
                        cue.token,
                        cue.envelope.sample(media.now)
                    ))
                    .collect::<Vec<_>>(),
                audio
            );
            assert_eq!(fixture::save_files(&temp.path().join("data")), files);
            // A valid retry must replace the changed live state, rather than leaving loads poisoned.
            fs::write(&slot, good_slot).unwrap();
            {
                let mut player = app.world_mut().non_send_mut::<Player>();
                player.story = story.clone();
                player.compiled = original_compiled;
            }
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::F9);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            let player = app.world().non_send::<Player>();
            assert_eq!(player.save_status, "Loaded.");
            assert!(!Arc::ptr_eq(&live, &player.state));
            assert_eq!(
                player.state.read().unwrap().get("matrix_probe"),
                Ok(&Value::Int(0))
            );
            assert_eq!(player.presenter.source().save_point().unwrap(), point);
            assert!(app.world().resource::<Ui>().at_initial_base(&story));
            eprintln!(
                "PASS {name}/player/{case}: state, dialogue, overlay, media tokens and save files unchanged"
            );
        }
    }
}
