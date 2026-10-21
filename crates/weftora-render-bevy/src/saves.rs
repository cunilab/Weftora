//! Save candidates remain isolated until dialogue, bindings and media all validate.
use crate::{Action, Player, Screen, declarative, presenter::Presenter};
use std::sync::{Arc, RwLock};
use weftora_core::SharedState;
use weftora_save::{SavedGame, storage::Store};
use weftora_yarn::DialogueRunner;

pub(super) struct Candidate {
    pub state: SharedState,
    pub presenter: Presenter<DialogueRunner>,
    pub actions: Vec<Action>,
}

pub(super) fn store(story: &weftora_story::Story) -> Option<Result<Store, String>> {
    story.manifest().save_schema.map(|_| {
        weftora_save::directory::resolve(&story.manifest().id, story.root(), None)
            .map(Store::new)
            .map_err(|error| error.to_string())
    })
}

impl Player {
    fn store(&self) -> Result<&Store, String> {
        self.save_store
            .as_ref()
            .ok_or("Story does not support saves")?
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(super) fn write_save(&self) -> Result<(), String> {
        let store = self.store()?;
        let context = weftora_yarn::save_context(&self.story, &self.compiled)
            .map_err(|error| error.to_string())?;
        let snapshot = self
            .presenter
            .source()
            .save_point()
            .map_err(|error| error.to_string())?;
        let game = SavedGame::new(&context, snapshot).map_err(|error| error.to_string())?;
        {
            let state = self
                .state
                .read()
                .map_err(|_| "canonical state lock poisoned")?;
            declarative::validate_display(&self.story, &state, self.presenter.screen())?;
            if weftora_yarn::media_enabled(&self.story) {
                weftora_core::presentation::volumes(&state)?;
            }
        }
        store.write(&game).map_err(|error| error.to_string())
    }

    pub(super) fn prepare_load(&self) -> Result<Candidate, String> {
        let context = weftora_yarn::save_context(&self.story, &self.compiled)
            .map_err(|error| error.to_string())?;
        let game = self
            .store()?
            .load(&context)
            .map_err(|error| error.to_string())?;
        let state = Arc::new(RwLock::new(
            self.story.initial_state().map_err(|e| e.to_string())?,
        ));
        // Initialize Yarn defaults before restoring removals; never fill them again afterward.
        let mut runner = DialogueRunner::for_story(&self.compiled, state.clone(), &self.story);
        let prepared = game
            .prepare_state(
                &context,
                &*state.read().map_err(|_| "candidate state lock poisoned")?,
                &*self
                    .state
                    .read()
                    .map_err(|_| "canonical state lock poisoned")?,
            )
            .map_err(|error| error.to_string())?;
        *state.write().map_err(|_| "candidate state lock poisoned")? = prepared;
        runner
            .restore_visits(game.visits())
            .map_err(|error| error.to_string())?;
        let node = &self.compiled.checkpoints()[game.checkpoint()];
        let (mut presenter, actions) = Presenter::new(runner, node);
        if let Screen::Error(error) = presenter.screen() {
            return Err(error.clone());
        }
        let checkpoint = presenter
            .source()
            .save_point()
            .map_err(|error| error.to_string())?;
        if checkpoint.checkpoint != game.checkpoint() {
            return Err("restored display is not at the saved checkpoint".into());
        }
        {
            let mut candidate = state.write().map_err(|_| "candidate state lock poisoned")?;
            declarative::validate_display(&self.story, &candidate, presenter.screen())?;
            if weftora_yarn::media_enabled(&self.story) {
                weftora_core::presentation::volumes(&candidate)?;
            }
            candidate.drain_changes();
        }
        if self.presenter.source().rule_trace().is_some() {
            presenter.source_mut().enable_rule_trace();
        }
        Ok(Candidate {
            state,
            presenter,
            actions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "story-tests")]
    mod story_matrix {
        use super::*;
        include!("story_matrix.rs");
    }
    use crate::{
        declarative::{Intent, Intents, Ui},
        media::Presentation,
    };
    use bevy::prelude::*;
    use std::time::Duration;
    use weftora_api::Value;
    use weftora_core::presentation::{Channel, Slot};

    const SCRIPT: &str = "title: Start\n---\n<<declare $n = 0>>\n<<checkpoint scene.start>>\n<<background bg.day>>\n<<music audio.music 0>>\n<<set $n = $n + 1>>\nFirst {$n}\n-> Left\n    <<set $n = $n + 10>>\n-> Right\n    <<set $n = $n + 20>>\nFinal {$n}\n===\n";

    fn fixture(script: &str) -> (tempfile::TempDir, App) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/presentation-demo");
        for entry in std::fs::read_dir(example.join("assets")).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), root.join("assets").join(entry.file_name())).unwrap();
        }
        let mut manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(example.join("manifest.jsonc")).unwrap())
                .unwrap();
        manifest["id"] = "test.player-save".into();
        manifest["saveSchema"] = 1.into();
        manifest["screens"] = serde_json::json!(["ui.jsonc", "settings.jsonc"]);
        manifest["requires"]["capabilities"]
            .as_array_mut()
            .unwrap()
            .push("save/1".into());
        manifest["state"]["ui.session"] = serde_json::json!({"scope":"session","default":false});
        manifest["state"]["note"] = serde_json::json!({"scope":"persistent","default":"short"});
        manifest["state"]["hidden"] = serde_json::json!({"scope":"persistent","default":"short"});
        std::fs::write(root.join("manifest.jsonc"), manifest.to_string()).unwrap();
        std::fs::write(root.join("main.yarn"), script).unwrap();
        std::fs::write(root.join("ui.jsonc"), r#"{"screenSchema":1,"root":{"type":"screen","id":"dialogue","children":[{"type":"text","bind":{"dialogue":"text"}},{"type":"text","bind":{"state":"note"}},{"type":"choiceList","id":"choices"}]}}"#).unwrap();
        std::fs::write(root.join("settings.jsonc"), r#"{"screenSchema":1,"root":{"type":"overlay","id":"settings","children":[{"type":"text","bind":{"state":"hidden"}},{"type":"button","id":"close","text":"Close","action":{"do":"screen.close"}}]}}"#).unwrap();
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        app.world_mut().non_send_mut::<Player>().save_store =
            Some(Ok(Store::new(temp.path().join("data"))));
        let player_story = app.world().non_send::<Player>();
        let initial = player_story
            .story
            .manifest()
            .initial_screen
            .clone()
            .unwrap();
        app.world_mut()
            .resource_mut::<Ui>()
            .navigate(
                &weftora_story::ui::UiAction::Open { screen: initial },
                &weftora_story::Story::load(&root).unwrap(),
                None,
            )
            .unwrap();
        app.init_resource::<Assets<crate::media::LoopAudio>>();
        app.add_systems(Startup, crate::setup);
        app.add_systems(
            Update,
            (crate::media::sync_views, crate::media::sync_audio)
                .chain()
                .after(crate::drive),
        );
        app.update();
        (temp, app)
    }
    fn request(app: &mut App, action: weftora_story::ui::UiAction) {
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action,
            });
        app.update();
    }
    fn set(app: &App, key: &str, value: Value) {
        app.world()
            .non_send::<Player>()
            .state
            .write()
            .unwrap()
            .set(key, value)
            .unwrap();
    }

    #[test]
    fn ui_commands_and_state_writes_settle_rules_and_require_a_new_checkpoint_for_save() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir(&root).unwrap();
        let example =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/commands-demo");
        for file in [
            "manifest.jsonc",
            "main.yarn",
            "events.jsonc",
            "screen.jsonc",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        app.world_mut().non_send_mut::<Player>().save_store =
            Some(Ok(Store::new(temp.path().join("data"))));
        app.world_mut()
            .resource_mut::<Ui>()
            .navigate(
                &weftora_story::ui::UiAction::Open {
                    screen: "dialogue".into(),
                },
                &weftora_story::Story::load(&root).unwrap(),
                None,
            )
            .unwrap();
        request(&mut app, weftora_story::ui::UiAction::Save);
        let path = temp.path().join("data/save.json");
        let good = std::fs::read(&path).unwrap();
        let points = |app: &App| {
            app.world()
                .non_send::<Player>()
                .state
                .read()
                .unwrap()
                .get("points")
                .unwrap()
                .clone()
        };
        assert_eq!(points(&app), Value::Int(2));
        // Rejected origin cannot run a command or invalidate the display's save point.
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: Some("missing".into()),
                action: weftora_story::ui::UiAction::CallCommand {
                    command: "award".into(),
                    args: vec![Value::Int(99)],
                },
            });
        app.update();
        assert_eq!(points(&app), Value::Int(2));
        assert!(
            app.world()
                .non_send::<Player>()
                .presenter
                .source()
                .save_point()
                .is_ok()
        );
        request(
            &mut app,
            weftora_story::ui::UiAction::CallCommand {
                command: "award".into(),
                args: vec![Value::Int(3)],
            },
        );
        assert_eq!(points(&app), Value::Int(5));
        request(&mut app, weftora_story::ui::UiAction::Save);
        assert_ne!(app.world().non_send::<Player>().save_status, "Saved.");
        assert_eq!(std::fs::read(&path).unwrap(), good);
        request(&mut app, weftora_story::ui::UiAction::Load);
        assert_eq!(points(&app), Value::Int(2));
        request(
            &mut app,
            weftora_story::ui::UiAction::SetState {
                key: "input".into(),
                value: Value::Bool(true),
            },
        );
        assert_eq!(points(&app), Value::Int(12));
        request(&mut app, weftora_story::ui::UiAction::Save);
        assert_eq!(std::fs::read(&path).unwrap(), good);
        request(&mut app, weftora_story::ui::UiAction::Advance);
        assert_eq!(points(&app), Value::Int(13));
        request(&mut app, weftora_story::ui::UiAction::Save);
        assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
        request(&mut app, weftora_story::ui::UiAction::Load);
        assert_eq!(points(&app), Value::Int(13));
        assert!(
            matches!(app.world().non_send::<Player>().presenter.screen(), Screen::Line { text, .. } if text == "Next points 13")
        );
        // User-paced inputs each receive a fresh budget; they must not exhaust the VM loop cap.
        let mut player = app.world_mut().non_send_mut::<Player>();
        for _ in 0..5000 {
            player
                .presenter
                .source_mut()
                .ui_action(&weftora_story::ui::UiAction::CallCommand {
                    command: "award".into(),
                    args: vec![Value::Int(1)],
                })
                .unwrap();
        }
        assert_eq!(
            player.state.read().unwrap().get("points"),
            Ok(&Value::Int(5013))
        );
    }

    #[test]
    fn collection_restore_candidate_does_not_publish_on_failure() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir(&root).unwrap();
        let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/collections-demo");
        for file in [
            "manifest.jsonc",
            "main.yarn",
            "events.jsonc",
            "screen.jsonc",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        let mut player = app.world_mut().non_send_mut::<Player>();
        player.save_store = Some(Ok(Store::new(temp.path().join("data"))));
        player.write_save().unwrap();
        let path = temp.path().join("data/save.json");
        let bytes = std::fs::read(&path).unwrap();
        player
            .presenter
            .source_mut()
            .ui_action(&weftora_story::ui::UiAction::SetState {
                key: "items".into(),
                value: Value::List(vec![]),
            })
            .unwrap();
        assert!(player.presenter.source().save_point().is_err());
        let candidate = player.prepare_load().unwrap();
        assert_eq!(
            candidate.state.read().unwrap().get("items"),
            Ok(&Value::List(vec![
                Value::String("key".into()),
                Value::String("ticket".into())
            ]))
        );
        assert_eq!(
            player.state.read().unwrap().get("items"),
            Ok(&Value::List(vec![]))
        );
        let mut rules: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("events.jsonc")).unwrap()).unwrap();
        rules["commands"][0]["actions"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"do":"state.add","key":"item_count","amount":i64::MAX}));
        std::fs::write(root.join("events.jsonc"), rules.to_string()).unwrap();
        player.story = weftora_story::Story::load(&root).unwrap();
        let before = player.state.read().unwrap().clone();
        let error = match player.prepare_load() {
            Ok(_) => panic!("candidate activated"),
            Err(error) => error,
        };
        assert!(error.contains("overflow"), "{error}");
        assert_eq!(*player.state.read().unwrap(), before);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn script_buttons_events_and_failed_restore_use_isolated_state() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir(&root).unwrap();
        let example =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/scripts-demo");
        for file in [
            "manifest.jsonc",
            "main.yarn",
            "events.jsonc",
            "screen.jsonc",
            "mechanics.rhai",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        let mut player = app.world_mut().non_send_mut::<Player>();
        player.save_store = Some(Ok(Store::new(temp.path().join("data"))));
        player.write_save().unwrap();
        let path = temp.path().join("data/save.json");
        let bytes = std::fs::read(&path).unwrap();
        player
            .presenter
            .source_mut()
            .ui_action(&weftora_story::ui::UiAction::CallCommand {
                command: "unpack".into(),
                args: vec![],
            })
            .unwrap();
        player
            .presenter
            .source_mut()
            .ui_action(&weftora_story::ui::UiAction::CallCommand {
                command: "pack".into(),
                args: vec![Value::Int(1)],
            })
            .unwrap();
        assert_eq!(
            player.state.read().unwrap().get("item_count"),
            Ok(&Value::Int(1))
        );
        player
            .presenter
            .source_mut()
            .ui_action(&weftora_story::ui::UiAction::SetState {
                key: "request".into(),
                value: Value::Bool(true),
            })
            .unwrap();
        assert_eq!(
            player.state.read().unwrap().get("item_count"),
            Ok(&Value::Int(2))
        );
        assert!(player.presenter.source().save_point().is_err());
        let candidate = player.prepare_load().unwrap();
        assert_eq!(
            candidate.state.read().unwrap().get("item_count"),
            Ok(&Value::Int(2))
        );
        let source = std::fs::read_to_string(root.join("mechanics.rhai")).unwrap();
        std::fs::write(
            root.join("mechanics.rhai"),
            source.replace(
                "state.emit(\"kit.packed\", true);",
                "state.add(\"item_count\", 9223372036854775807);",
            ),
        )
        .unwrap();
        player.story = weftora_story::Story::load(&root).unwrap();
        let before = player.state.read().unwrap().clone();
        let error = match player.prepare_load() {
            Ok(_) => panic!("failed script activated"),
            Err(e) => e,
        };
        assert!(error.contains("overflow"), "{error}");
        assert_eq!(*player.state.read().unwrap(), before);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn random_load_candidates_never_advance_the_live_sequence() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir(&root).unwrap();
        let example =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/random-demo");
        for file in [
            "manifest.jsonc",
            "main.yarn",
            "events.jsonc",
            "screen.jsonc",
            "roll.rhai",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        let mut player = app.world_mut().non_send_mut::<Player>();
        player.save_store = Some(Ok(Store::new(temp.path().join("data"))));
        player.write_save().unwrap();
        let path = temp.path().join("data/save.json");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            player.state.read().unwrap().get("rng"),
            Ok(&Value::Int(-7046029254386353131))
        );
        player
            .presenter
            .source_mut()
            .ui_action(&weftora_story::ui::UiAction::CallCommand {
                command: "roll".into(),
                args: vec![],
            })
            .unwrap();
        assert_eq!(
            player.state.read().unwrap().get("rng"),
            Ok(&Value::Int(4354685564936845354))
        );
        assert!(player.write_save().is_err());
        let before = player.state.read().unwrap().clone();
        let candidate = player.prepare_load().unwrap();
        assert_eq!(
            candidate.state.read().unwrap().get("rng"),
            Ok(&Value::Int(-7046029254386353131))
        );
        assert_eq!(*player.state.read().unwrap(), before);
        let source = std::fs::read_to_string(root.join("roll.rhai")).unwrap();
        std::fs::write(
            root.join("roll.rhai"),
            source.replace("let rolls =", "state.get(\"denied\"); let rolls ="),
        )
        .unwrap();
        player.story = weftora_story::Story::load(&root).unwrap();
        let screen = player.presenter.screen().clone();
        let error = match player.prepare_load() {
            Ok(_) => panic!("failed random replay activated"),
            Err(e) => e,
        };
        assert!(error.contains("roll.rhai:3"), "{error}");
        assert_eq!(*player.state.read().unwrap(), before);
        assert_eq!(player.presenter.screen(), &screen);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        player
            .presenter
            .source_mut()
            .ui_action(&weftora_story::ui::UiAction::CallCommand {
                command: "roll".into(),
                args: vec![],
            })
            .unwrap();
        assert_eq!(
            player.state.read().unwrap().get("rng"),
            Ok(&Value::Int(-2691343689449507777))
        );
        assert_eq!(
            player.state.read().unwrap().get("rolls"),
            Ok(&Value::List(vec![
                Value::Int(2),
                Value::Int(1),
                Value::Int(2)
            ]))
        );
    }

    #[test]
    fn collection_query_displays_read_ui_mutations_and_failed_loads_stay_isolated() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir(&root).unwrap();
        let example =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/queries-demo");
        for file in [
            "manifest.jsonc",
            "main.yarn",
            "events.jsonc",
            "screen.jsonc",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        let mut player = app.world_mut().non_send_mut::<Player>();
        player.save_store = Some(Ok(Store::new(temp.path().join("data"))));
        player.write_save().unwrap();
        let path = temp.path().join("data/save.json");
        let bytes = std::fs::read(&path).unwrap();
        player
            .presenter
            .source_mut()
            .ui_action(&weftora_story::ui::UiAction::CallCommand {
                command: "unpack".into(),
                args: vec![],
            })
            .unwrap();
        assert!(player.write_save().is_err());
        player.presenter.advance();
        assert!(
            matches!(player.presenter.screen(), Screen::Line{text,..} if text=="Now 0, key false, schedule 0, morning false")
        );
        let candidate = player.prepare_load().unwrap();
        assert!(
            matches!(candidate.presenter.screen(),Screen::Line{text,..} if text=="Packed 2 items")
        );
        assert_eq!(
            player.state.read().unwrap().get("items"),
            Ok(&Value::List(vec![]))
        );
        let mut rules: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("events.jsonc")).unwrap()).unwrap();
        rules["commands"][0]["actions"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"do":"state.remove","key":"items"}));
        std::fs::write(root.join("events.jsonc"), rules.to_string()).unwrap();
        player.story = weftora_story::Story::load(&root).unwrap();
        let before = player.state.read().unwrap().clone();
        let screen = player.presenter.screen().clone();
        let error = match player.prepare_load() {
            Ok(_) => panic!("failed query activated"),
            Err(e) => e,
        };
        assert!(
            error.contains("main.yarn") && error.contains("state_list_len"),
            "{error}"
        );
        assert_eq!(*player.state.read().unwrap(), before);
        assert_eq!(player.presenter.screen(), &screen);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn rule_restore_uses_a_candidate_and_failure_preserves_live_state_and_slot() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir(&root).unwrap();
        let example =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rules-demo");
        for file in ["manifest.jsonc", "main.yarn", "events.jsonc"] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        let mut player = app.world_mut().non_send_mut::<Player>();
        player.save_store = Some(Ok(Store::new(temp.path().join("data"))));
        assert_eq!(
            player.state.read().unwrap().get("points"),
            Ok(&Value::Int(10))
        );
        player.write_save().unwrap();
        player.write_save().unwrap();
        let bytes = std::fs::read(temp.path().join("data/save.json")).unwrap();
        player
            .state
            .write()
            .unwrap()
            .set("points", Value::Int(99))
            .unwrap();
        let candidate = player.prepare_load().unwrap();
        assert_eq!(
            candidate.state.read().unwrap().get("points"),
            Ok(&Value::Int(10))
        );
        assert_eq!(
            player.state.read().unwrap().get("points"),
            Ok(&Value::Int(99))
        );
        let mut rules: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("events.jsonc")).unwrap()).unwrap();
        rules["events"][0]["actions"] = serde_json::json!([
            {"do":"state.set","key":"points","value":20},
            {"do":"state.add","key":"points","amount":i64::MAX}
        ]);
        std::fs::write(root.join("events.jsonc"), rules.to_string()).unwrap();
        player.story = weftora_story::Story::load(&root).unwrap();
        let before = player.state.read().unwrap().clone();
        let error = match player.prepare_load() {
            Ok(_) => panic!("invalid candidate activated"),
            Err(error) => error,
        };
        assert!(
            error.contains("events.jsonc") && error.contains("overflow"),
            "{error}"
        );
        assert_eq!(*player.state.read().unwrap(), before);
        assert_eq!(
            std::fs::read(temp.path().join("data/save.json")).unwrap(),
            bytes
        );
        assert_eq!(
            std::fs::read(temp.path().join("data/save.bak")).unwrap(),
            bytes
        );
    }

    #[test]
    fn player_load_keeps_settings_resets_session_and_rebuilds_audio_with_new_tokens() {
        let (temp, mut app) = fixture(SCRIPT);
        request(&mut app, weftora_story::ui::UiAction::Save);
        assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
        let bytes = std::fs::read(temp.path().join("data/save.json")).unwrap();
        let old_state = app.world().non_send::<Player>().state.clone();
        let old_token = app.world().resource::<Presentation>().audio.cues[0].token;
        set(&app, "ui.session", Value::Bool(true));
        set(&app, "settings.audio.master", Value::Float(0.25));
        set(&app, "n", Value::Float(99.0));
        request(
            &mut app,
            weftora_story::ui::UiAction::Open {
                screen: "settings".into(),
            },
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert!(
            app.world()
                .resource::<Ui>()
                .at_initial_base(&app.world().non_send::<Player>().story)
        );
        let player = app.world().non_send::<Player>();
        assert_eq!(player.save_status, "Loaded.");
        assert!(!Arc::ptr_eq(&old_state, &player.state));
        let state = player.state.read().unwrap();
        assert_eq!(state.get("n"), Ok(&Value::Float(1.0)));
        assert_eq!(state.get("ui.session"), Ok(&Value::Bool(false)));
        assert_eq!(state.get("settings.audio.master"), Ok(&Value::Float(0.25)));
        assert!(matches!(player.presenter.screen(), Screen::Line { text, .. } if text=="First 1"));
        assert!(player.dirty); // New and old presenters both have revision 1.
        drop(state);
        let presentation = app.world().resource::<Presentation>();
        assert_eq!(presentation.audio.cues.len(), 1);
        assert_eq!(presentation.audio.cues[0].channel, Channel::Music);
        assert_ne!(presentation.audio.cues[0].token, old_token);
        assert_eq!(
            presentation.views.samples(presentation.now)[0].slot,
            Slot::Background
        );
        let mut query = app.world_mut().query::<&crate::media::CueToken>();
        let tokens: Vec<_> = query.iter(app.world()).map(|token| token.0).collect();
        assert_eq!(tokens.len(), 1);
        assert!(!tokens.contains(&old_token));
        request(&mut app, weftora_story::ui::UiAction::Save);
        assert_eq!(
            std::fs::read(temp.path().join("data/save.json")).unwrap(),
            bytes
        );
        request(&mut app, weftora_story::ui::UiAction::Advance);
        assert!(matches!(
            app.world().non_send::<Player>().presenter.screen(),
            Screen::Choices(_)
        ));
    }

    #[test]
    fn failed_load_is_nonfatal_and_preserves_live_state_view_audio_and_slot() {
        let (temp, mut app) = fixture(SCRIPT);
        request(&mut app, weftora_story::ui::UiAction::Save);
        let path = temp.path().join("data/save.json");
        let good = std::fs::read(&path).unwrap();
        let state = app.world().non_send::<Player>().state.clone();
        let before = state.read().unwrap().clone();
        let screen = app.world().non_send::<Player>().presenter.screen().clone();
        let token = app.world().resource::<Presentation>().audio.cues[0].token;
        let samples = app
            .world()
            .resource::<Presentation>()
            .views
            .samples(Duration::ZERO);
        std::fs::write(&path, b"broken").unwrap();
        request(&mut app, weftora_story::ui::UiAction::Load);
        assert!(
            app.world()
                .non_send::<Player>()
                .save_status
                .contains("corrupt")
        );
        assert!(Arc::ptr_eq(&state, &app.world().non_send::<Player>().state));
        assert_eq!(*state.read().unwrap(), before);
        assert_eq!(app.world().non_send::<Player>().presenter.screen(), &screen);
        assert_eq!(
            app.world().resource::<Presentation>().audio.cues[0].token,
            token
        );
        assert_eq!(
            app.world()
                .resource::<Presentation>()
                .views
                .samples(Duration::ZERO),
            samples
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        std::fs::write(&path, &good).unwrap();
        let player = app.world().non_send::<Player>();
        let context = weftora_yarn::save_context(&player.story, &player.compiled).unwrap();
        let mut snapshot = player.presenter.source().save_point().unwrap();
        snapshot
            .state
            .insert("note".into(), Some(Value::String("x".repeat(4097))));
        let oversized = SavedGame::new(&context, snapshot)
            .unwrap()
            .to_bytes()
            .unwrap();
        std::fs::write(&path, &oversized).unwrap();
        request(&mut app, weftora_story::ui::UiAction::Load);
        assert!(
            app.world()
                .non_send::<Player>()
                .save_status
                .contains("bound text")
        );
        assert!(Arc::ptr_eq(&state, &app.world().non_send::<Player>().state));
        assert_eq!(
            app.world().resource::<Presentation>().audio.cues[0].token,
            token
        );
        assert_eq!(std::fs::read(&path).unwrap(), oversized);
        // Fail during candidate asset decoding after dialogue/binding validation.
        std::fs::write(&path, &good).unwrap();
        let replacement = SCRIPT.replace("bg.day", "bg.night");
        let compiled = weftora_yarn_compiler::compile(&[weftora_story::DialogueSource {
            file: "main.yarn".into(),
            contents: replacement,
        }])
        .unwrap();
        app.world_mut().non_send_mut::<Player>().compiled = compiled;
        std::fs::write(temp.path().join("story/assets/night.png"), b"not a png").unwrap();
        request(&mut app, weftora_story::ui::UiAction::Load);
        assert!(
            app.world()
                .non_send::<Player>()
                .save_status
                .contains("asset")
        );
        assert!(Arc::ptr_eq(&state, &app.world().non_send::<Player>().state));
        assert_eq!(
            app.world().resource::<Presentation>().audio.cues[0].token,
            token
        );
        assert_eq!(
            app.world()
                .resource::<Presentation>()
                .views
                .samples(Duration::ZERO),
            samples
        );
        assert_eq!(std::fs::read(&path).unwrap(), good);
        request(&mut app, weftora_story::ui::UiAction::Advance);
        assert!(matches!(
            app.world().non_send::<Player>().presenter.screen(),
            Screen::Choices(_)
        ));
    }

    #[test]
    fn save_refuses_transition_overlay_changed_state_and_same_frame_progress() {
        let (temp, mut app) =
            fixture(&SCRIPT.replace("<<background bg.day>>", "<<show background bg.day 1000>>"));
        request(&mut app, weftora_story::ui::UiAction::Save);
        assert!(
            app.world()
                .non_send::<Player>()
                .save_status
                .contains("transition")
        );
        assert!(!temp.path().join("data/save.json").exists());
        app.world_mut().resource_mut::<Presentation>().now = Duration::from_secs(2);
        request(&mut app, weftora_story::ui::UiAction::Save);
        assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
        request(
            &mut app,
            weftora_story::ui::UiAction::Open {
                screen: "settings".into(),
            },
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F5);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert!(
            app.world()
                .non_send::<Player>()
                .save_status
                .contains("close overlays")
        );
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: Some("settings".into()),
                action: weftora_story::ui::UiAction::Close,
            });
        app.update();
        set(&app, "n", Value::Float(2.0));
        request(&mut app, weftora_story::ui::UiAction::Save);
        assert!(
            app.world()
                .non_send::<Player>()
                .save_status
                .contains("persistent state changed")
        );
        set(&app, "n", Value::Float(1.0));
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action: weftora_story::ui::UiAction::Load,
            });
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action: weftora_story::ui::UiAction::Advance,
            });
        app.update();
        assert!(
            app.world()
                .non_send::<Player>()
                .save_status
                .contains("wait until")
        );
        assert!(matches!(
            app.world().non_send::<Player>().presenter.screen(),
            Screen::Choices(_)
        ));
    }
    #[test]
    fn f5_f9_with_space_do_not_advance_and_load_forces_panel_refresh() {
        let (_temp, mut app) = fixture(SCRIPT);
        app.add_systems(
            Update,
            (crate::refresh_panel, crate::refresh_save_status).after(crate::drive),
        );
        // Skip initial media commands before the first keyboard save.
        app.update();
        for key in [KeyCode::F5, KeyCode::Space] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
        set(&app, "n", Value::Float(99.0));
        for key in [KeyCode::F9, KeyCode::Space] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
        }
        app.update();
        assert_eq!(app.world().non_send::<Player>().save_status, "Loaded.");
        assert!(
            matches!(app.world().non_send::<Player>().presenter.screen(), Screen::Line { text, .. } if text=="First 1")
        );
        assert!(!app.world().non_send::<Player>().dirty);
        let mut query = app
            .world_mut()
            .query_filtered::<&Text, With<crate::SaveStatus>>();
        assert_eq!(query.single(app.world()).unwrap().0, "Loaded.");
    }

    #[test]
    fn ui_save_actions_require_manifest_save_support() {
        let (temp, _app) = fixture(SCRIPT);
        let root = temp.path().join("story");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("manifest.jsonc")).unwrap())
                .unwrap();
        manifest.as_object_mut().unwrap().remove("saveSchema");
        std::fs::write(root.join("manifest.jsonc"), manifest.to_string()).unwrap();
        let path = root.join("ui.jsonc");
        let mut ui: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        ui["root"]["children"].as_array_mut().unwrap().push(
            serde_json::json!({"type":"button","id":"save","text":"Save","action":{"do":"save"}}),
        );
        std::fs::write(path, ui.to_string()).unwrap();
        let errors = weftora_story::Story::load(&root).err().unwrap();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("save/load requires saveSchema"))
        );
    }
    #[test]
    fn save_refuses_unloadable_hidden_binding_without_replacing_good_slot() {
        let (temp, mut app) = fixture(SCRIPT);
        request(&mut app, weftora_story::ui::UiAction::Save);
        let path = temp.path().join("data/save.json");
        let good = std::fs::read(&path).unwrap();
        set(&app, "hidden", Value::String("x".repeat(4097)));
        {
            let player = &mut *app.world_mut().non_send_mut::<Player>();
            let runner = DialogueRunner::new(&player.compiled, player.state.clone());
            let (presenter, actions) = Presenter::new(runner, "Start");
            player.presenter = presenter;
            player.pending = actions;
            assert!(player.presenter.source().save_point().is_ok());
        }
        app.update();
        request(&mut app, weftora_story::ui::UiAction::Save);
        assert!(
            app.world()
                .non_send::<Player>()
                .save_status
                .contains("bound text")
        );
        assert_eq!(std::fs::read(&path).unwrap(), good);
        assert!(!path.with_file_name("save.bak").exists());
    }
    #[cfg(all(test, feature = "story-tests"))]
    #[test]
    fn story_a_every_checkpoint_reconstructs_the_same_settled_view_audio_and_state() {
        fn settle(app: &mut App) {
            app.world_mut().resource_mut::<Presentation>().now += Duration::from_secs(2);
            app.update();
        }
        fn audio(app: &App) -> Vec<(Channel, String, f32)> {
            let presentation = app.world().resource::<Presentation>();
            let mut cues: Vec<_> = presentation
                .audio
                .cues
                .iter()
                .map(|cue| {
                    (
                        cue.channel,
                        cue.asset.clone(),
                        cue.envelope.sample(presentation.now),
                    )
                })
                .collect();
            cues.sort_by(|a, b| a.1.cmp(&b.1));
            cues
        }
        for (target, branch) in [
            ("scene.menu", 0),
            ("scene.arrival", 0),
            ("scene.decision", 0),
            ("scene.repair", 0),
            ("scene.roof", 1),
            ("scene.dawn", 0),
            ("scene.dawn", 1),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let root =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/story-a");
            let story = weftora_story::Story::load(&root).unwrap();
            let compiled =
                weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
            let mut app = crate::tests::app_for(story.clone(), compiled);
            app.world_mut().non_send_mut::<Player>().save_store =
                Some(Ok(Store::new(directory.path().to_path_buf())));
            app.world_mut()
                .resource_mut::<Ui>()
                .navigate(
                    &weftora_story::ui::UiAction::Open {
                        screen: "dialogue".into(),
                    },
                    &story,
                    None,
                )
                .unwrap();
            app.init_resource::<Assets<crate::media::LoopAudio>>()
                .add_systems(
                    Update,
                    (crate::media::sync_views, crate::media::sync_audio)
                        .chain()
                        .after(crate::drive),
                );
            app.update();
            let mut reached = false;
            for _ in 0..100 {
                let player = app.world().non_send::<Player>();
                if player
                    .presenter
                    .source()
                    .save_point()
                    .is_ok_and(|point| point.checkpoint == target)
                {
                    reached = true;
                    break;
                }
                let display = player.presenter.screen().clone();
                match display {
                    Screen::Line { .. } => request(&mut app, weftora_story::ui::UiAction::Advance),
                    Screen::Choices(_) => {
                        let position = if player.presenter.source().save_point().unwrap().checkpoint
                            == "scene.decision"
                        {
                            branch
                        } else {
                            0
                        };
                        app.world_mut()
                            .resource_mut::<Intents>()
                            .push(Intent::Select {
                                layer: None,
                                position,
                            });
                        app.update();
                    }
                    other => panic!("did not reach {target}: {other:?}"),
                }
            }
            assert!(reached, "{target}");
            settle(&mut app);
            let player = app.world().non_send::<Player>();
            let state = player
                .state
                .read()
                .unwrap()
                .snapshot(weftora_core::Scope::Persistent);
            let display = player.presenter.screen().clone();
            let presentation = app.world().resource::<Presentation>();
            let views = presentation.views.samples(presentation.now);
            let cues = audio(&app);
            request(&mut app, weftora_story::ui::UiAction::Save);
            assert_eq!(
                app.world().non_send::<Player>().save_status,
                "Saved.",
                "{target}"
            );
            match display {
                Screen::Choices(_) => {
                    app.world_mut()
                        .resource_mut::<Intents>()
                        .push(Intent::Select {
                            layer: None,
                            position: 0,
                        });
                    app.update();
                }
                _ => {
                    request(&mut app, weftora_story::ui::UiAction::Advance);
                    request(&mut app, weftora_story::ui::UiAction::Advance);
                }
            }
            request(&mut app, weftora_story::ui::UiAction::Load);
            assert_eq!(
                app.world().non_send::<Player>().save_status,
                "Loaded.",
                "{target}"
            );
            settle(&mut app);
            let player = app.world().non_send::<Player>();
            assert_eq!(player.presenter.screen(), &display, "{target}");
            assert_eq!(
                player
                    .state
                    .read()
                    .unwrap()
                    .snapshot(weftora_core::Scope::Persistent),
                state,
                "{target}"
            );
            let presentation = app.world().resource::<Presentation>();
            assert_eq!(
                presentation.views.samples(presentation.now),
                views,
                "{target}"
            );
            assert_eq!(audio(&app), cues, "{target}");
        }
    }
    #[cfg(all(test, feature = "story-tests"))]
    #[test]
    fn life_sim_checkpoints_restore_the_day_without_repeating_costs_or_pay() {
        let directory = tempfile::tempdir().unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/life-sim");
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story.clone(), compiled);
        app.world_mut().non_send_mut::<Player>().save_store =
            Some(Ok(Store::new(directory.path().to_path_buf())));
        app.world_mut().resource_mut::<Ui>().reset(&story);
        app.update();
        for target in [
            "day.home",
            "day.market",
            "day.shift",
            "day.return",
            "day.end",
        ] {
            let mut reached = false;
            for _ in 0..40 {
                let player = app.world().non_send::<Player>();
                if player
                    .presenter
                    .source()
                    .save_point()
                    .is_ok_and(|point| point.checkpoint == target)
                {
                    reached = true;
                    break;
                }
                match player.presenter.screen().clone() {
                    Screen::Line { .. } => request(&mut app, weftora_story::ui::UiAction::Advance),
                    Screen::Choices(_) => {
                        app.world_mut()
                            .resource_mut::<Intents>()
                            .push(Intent::Select {
                                layer: None,
                                position: 0,
                            });
                        app.update();
                    }
                    other => panic!("did not reach {target}: {other:?}"),
                }
            }
            assert!(reached, "{target}");
            let player = app.world().non_send::<Player>();
            let snapshot = player
                .state
                .read()
                .unwrap()
                .snapshot(weftora_core::Scope::Persistent);
            let display = player.presenter.screen().clone();
            request(&mut app, weftora_story::ui::UiAction::Save);
            assert_eq!(
                app.world().non_send::<Player>().save_status,
                "Saved.",
                "{target}"
            );
            let slot = directory.path().join("save.json");
            let bytes = std::fs::read(&slot).unwrap();
            if target == "day.home" {
                request(
                    &mut app,
                    weftora_story::ui::UiAction::Open {
                        screen: "journal".into(),
                    },
                );
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::F5);
                app.update();
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .clear();
                assert!(
                    app.world()
                        .non_send::<Player>()
                        .save_status
                        .contains("close overlays")
                );
                assert_eq!(std::fs::read(&slot).unwrap(), bytes);
                app.world_mut()
                    .resource_mut::<Intents>()
                    .push(Intent::Action {
                        layer: Some("journal".into()),
                        action: weftora_story::ui::UiAction::Close,
                    });
                app.update();
                request(&mut app, weftora_story::ui::UiAction::Save);
                assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
                request(
                    &mut app,
                    weftora_story::ui::UiAction::Open {
                        screen: "journal".into(),
                    },
                );
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::F9);
                app.update();
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .clear();
                assert_eq!(app.world().non_send::<Player>().save_status, "Loaded.");
                assert!(app.world().resource::<Ui>().at_initial_base(&story));
            }
            // Spend resources before loading, so a no-op load cannot pass the state comparison.
            request(&mut app, weftora_story::ui::UiAction::Advance);
            if target != "day.end" {
                app.world_mut()
                    .resource_mut::<Intents>()
                    .push(Intent::Select {
                        layer: None,
                        position: 0,
                    });
                app.update();
                assert_ne!(
                    app.world()
                        .non_send::<Player>()
                        .state
                        .read()
                        .unwrap()
                        .snapshot(weftora_core::Scope::Persistent),
                    snapshot
                );
            }
            for _ in 0..if target == "day.shift" { 2 } else { 1 } {
                request(&mut app, weftora_story::ui::UiAction::Load);
                let player = app.world().non_send::<Player>();
                assert_eq!(player.save_status, "Loaded.", "{target}");
                assert_eq!(player.presenter.screen(), &display);
                assert_eq!(
                    player
                        .state
                        .read()
                        .unwrap()
                        .snapshot(weftora_core::Scope::Persistent),
                    snapshot
                );
            }
            request(&mut app, weftora_story::ui::UiAction::Save);
            assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
            assert_eq!(std::fs::read(&slot).unwrap(), bytes);
            assert_eq!(
                std::fs::read(directory.path().join("save.bak")).unwrap(),
                bytes
            );
            if target != "day.end" {
                request(&mut app, weftora_story::ui::UiAction::Advance);
            }
        }
        let player = app.world().non_send::<Player>();
        let state = player.state.read().unwrap();
        assert_eq!(state.get("coins"), Ok(&Value::Int(2)));
        assert_eq!(state.get("energy"), Ok(&Value::Int(3)));
        assert_eq!(state.get("hour"), Ok(&Value::Int(14)));
        assert_eq!(state.get("rng"), Ok(&Value::Int(-7046029254386353131)));
    }
}
