use super::*;
use bevy::ecs::system::RunSystemOnce;
use serde_json::json;
use std::fs;
use weftora_api::Value;
use weftora_story::ui::UiAction;

const SCRIPT: &str = "title: Start\n---\n<<declare $n = 0>>\n<<declare $name = \"Ada\">>\n<<declare $eligible = true>>\n<<checkpoint begin>>\n<<set $n = $n + 1>>\nHello {$name} {$n} #line:hello\n-> Go {$name} <<if $eligible>> #line:go\n    <<set $n = $n + 10>>\n    Done {$n} #line:done\n-> Stay #line:stay\n    Rest #line:rest\n===\n";

fn fixture() -> (tempfile::TempDir, App) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("main.yarn"), SCRIPT).unwrap();
    fs::write(
        root.join("font.ttf"),
        b"typed model fixture; decoding tested separately",
    )
    .unwrap();
    fs::write(
        root.join("fonts.jsonc"),
        r#"[{"id":"font.common","path":"font.ttf"}]"#,
    )
    .unwrap();
    fs::write(root.join("manifest.jsonc"), json!({
        "manifestSchema":1,"id":"test.localization-runtime","title":"Test","storyVersion":"0.1.0","saveSchema":1,
        "entrypoint":"Start","dialogue":["main.yarn"],
        "requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["dialogue.yarn/1","localization.catalog/1","localization.text/1","assets.catalog/1","ui.screens/1","save/1","rules.events/1"]},
        "localization":{"baseLanguage":"en","stateKey":"locale","font":"font.common","languages":{"en":"en.jsonc","id":"id.jsonc","ja":"ja.jsonc"}},
        "state":{"locale":{"default":"en","scope":"settings"},"n":{"default":0.0},"hits":{"default":0}},
        "events":["rules.jsonc"],
        "catalogs":{"fonts":["fonts.jsonc"]},"screens":["ui.jsonc"],"initialScreen":"main"
    }).to_string()).unwrap();
    fs::write(root.join("rules.jsonc"), json!({"rulesSchema":1,"events":[{"id":"count","trigger":"state.changed.n","actions":[{"do":"state.add","key":"hits","amount":1}]}]}).to_string()).unwrap();
    fs::write(root.join("ui.jsonc"), json!({"screenSchema":1,"root":{"type":"screen","id":"main","children":[
        {"type":"button","id":"language","bind":{"localized":"title"},"action":{"do":"state.set","key":"locale","value":"ja"}},
        {"type":"text","bind":{"dialogue":"text"}},
        {"type":"choiceList","id":"choices"}
    ]}}).to_string()).unwrap();
    for (language, title, dialogue) in [
        ("en", "English", json!({})),
        (
            "id",
            "Indonesia",
            json!({"line:hello":"Halo {0} {1}","line:go":"Pergi {0}","line:stay":"Tunggu","line:done":"Selesai {0}","line:rest":"Istirahat"}),
        ),
        (
            "ja",
            "日本語",
            json!({"line:hello":"こんにちは {0} {1}","line:go":"行く {0}","line:stay":"待つ","line:done":"完了 {0}","line:rest":"休む"}),
        ),
    ] {
        fs::write(
            root.join(format!("{language}.jsonc")),
            json!({"localizationSchema":1,"ui":{"title":title},"dialogue":dialogue}).to_string(),
        )
        .unwrap();
    }
    let story = Story::load(root).unwrap();
    let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
    assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
    let mut app = crate::tests::app_for_debug(story, compiled, true);
    app.world_mut().non_send_mut::<Player>().save_store =
        Some(Ok(weftora_save::storage::Store::new(root.join("data"))));
    (temp, app)
}

fn switch(app: &mut App, language: &str) {
    let player = &mut *app.world_mut().non_send_mut::<Player>();
    player
        .presenter
        .source_mut()
        .ui_action(&UiAction::SetState {
            key: "locale".into(),
            value: Value::String(language.into()),
        })
        .unwrap();
    player.presenter.refresh_localization();
    player.dirty = true;
}

fn saved_bytes(player: &Player) -> Vec<u8> {
    let context = weftora_yarn::save_context(&player.story, &player.compiled).unwrap();
    weftora_save::SavedGame::new(&context, player.presenter.source().save_point().unwrap())
        .unwrap()
        .to_bytes()
        .unwrap()
}

#[test]
fn switching_a_line_preserves_capture_progress_trace_and_current_settings_on_load() {
    let (_temp, mut app) = fixture();
    let (bytes, trace, revision) = {
        let p = app.world().non_send::<Player>();
        assert!(!p.presenter.source().rule_trace().unwrap().rows.is_empty());
        assert!(matches!(p.presenter.screen(),Screen::Line{text,..} if text=="Hello Ada 1"));
        (
            saved_bytes(p),
            format!("{:?}", p.presenter.source().rule_trace()),
            p.presenter.revision(),
        )
    };
    for (language, text) in [
        ("id", "Halo Ada 1"),
        ("ja", "こんにちは Ada 1"),
        ("en", "Hello Ada 1"),
    ] {
        switch(&mut app, language);
        let p = app.world().non_send::<Player>();
        assert!(matches!(p.presenter.screen(),Screen::Line{text:actual,..} if actual==text));
        assert_eq!(p.state.read().unwrap().get("n"), Ok(&Value::Float(1.0)));
        assert_eq!(saved_bytes(p), bytes);
        assert_eq!(format!("{:?}", p.presenter.source().rule_trace()), trace);
        assert_eq!(p.presenter.revision(), revision);
    }
    app.world().non_send::<Player>().write_save().unwrap();
    switch(&mut app, "ja");
    let candidate = app.world().non_send::<Player>().prepare_load().unwrap();
    assert_eq!(
        candidate.state.read().unwrap().get("locale"),
        Ok(&Value::String("ja".into()))
    );
    assert!(
        matches!(candidate.presenter.screen(),Screen::Line{text,..} if text=="こんにちは Ada 1")
    );
    assert_eq!(
        candidate.state.read().unwrap().get("n"),
        Ok(&Value::Float(1.0))
    );
    let p = &mut *app.world_mut().non_send_mut::<Player>();
    p.state
        .write()
        .unwrap()
        .set("name", Value::String("Grace".into()))
        .unwrap();
    switch(&mut app, "id");
    assert!(
        matches!(app.world().non_send::<Player>().presenter.screen(),Screen::Line{text,..} if text=="Halo Ada 1"),
        "captured substitutions must not be reevaluated"
    );
}

#[test]
fn offered_choices_keep_availability_ids_and_target_after_language_change() {
    let (_temp, mut app) = fixture();
    app.world_mut().non_send_mut::<Player>().presenter.advance();
    let choices = match app.world().non_send::<Player>().presenter.screen() {
        Screen::Choices(v) => v.clone(),
        s => panic!("{s:?}"),
    };
    let state = app.world().non_send::<Player>().state.clone();
    state
        .write()
        .unwrap()
        .set("eligible", Value::Bool(false))
        .unwrap();
    state
        .write()
        .unwrap()
        .set("name", Value::String("Grace".into()))
        .unwrap();
    for (language, text) in [("ja", "行く Ada"), ("id", "Pergi Ada"), ("en", "Go Ada")] {
        switch(&mut app, language);
        let p = app.world().non_send::<Player>();
        let Screen::Choices(actual) = p.presenter.screen() else {
            panic!("expected choices")
        };
        assert_eq!(
            actual.iter().map(|c| c.index).collect::<Vec<_>>(),
            choices.iter().map(|c| c.index).collect::<Vec<_>>()
        );
        assert_eq!(actual[0].text, text);
    }
    app.world_mut().non_send_mut::<Player>().presenter.select(0);
    assert_eq!(state.read().unwrap().get("n"), Ok(&Value::Float(11.0)));
}

#[test]
fn localized_labels_and_choices_update_without_replacing_entities_or_focus() {
    let (_temp, mut app) = fixture();
    // Model screen construction uses the common default font; actual font decoding is separate.
    app.world_mut()
        .resource_mut::<Ui>()
        .reset(&Story::load(_temp.path()).unwrap());
    app.add_systems(
        Update,
        (
            declarative::sync_layers,
            declarative::sync_choices,
            declarative::restore_focus,
            declarative::update_bindings,
        )
            .chain()
            .after(drive),
    );
    app.update();
    app.world_mut().non_send_mut::<Player>().presenter.advance();
    app.update();
    let entities: Vec<Entity> = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<Button>>()
            .iter(world)
            .collect()
    };
    let focused = *entities.last().unwrap();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(focused, bevy::input_focus::FocusCause::Navigated);
    app.world_mut()
        .resource_mut::<Intents>()
        .push(Intent::Action {
            layer: Some("main".into()),
            action: UiAction::SetState {
                key: "locale".into(),
                value: Value::String("ja".into()),
            },
        });
    app.update();
    assert_eq!(
        app.world()
            .non_send::<Player>()
            .state
            .read()
            .unwrap()
            .get("locale"),
        Ok(&Value::String("ja".into()))
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(focused));
    for entity in entities {
        assert!(app.world().get::<Button>(entity).is_some());
    }
    let world = app.world_mut();
    let texts: Vec<String> = world
        .query::<&Text>()
        .iter(world)
        .map(|t| t.0.clone())
        .collect();
    assert!(texts.contains(&"日本語".into()));
    assert!(texts.contains(&"1. 行く Ada".into()));
    let ticks = |app: &mut App| {
        let world = app.world_mut();
        world
            .query_filtered::<(Entity, Ref<Text>), With<declarative::ChoiceText>>()
            .iter(world)
            .map(|(entity, text)| (entity, text.last_changed()))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let before = ticks(&mut app);
    assert!(!before.is_empty());
    app.update();
    assert_eq!(
        ticks(&mut app),
        before,
        "unchanged choices must not trigger text layout again"
    );
}

#[test]
fn unknown_locale_edits_and_invalid_load_candidates_preserve_live_state_and_files() {
    let (_temp, mut app) = fixture();
    app.world().non_send::<Player>().write_save().unwrap();
    let p = &mut *app.world_mut().non_send_mut::<Player>();
    let path = p.save_store.as_ref().unwrap().as_ref().unwrap().path();
    let file = fs::read(&path).unwrap();
    let before = p.state.read().unwrap().clone();
    assert!(crate::debug_tools::apply(p, "locale", r#""missing""#).is_err());
    assert_eq!(*p.state.read().unwrap(), before);
    let screen = p.presenter.screen().clone();
    p.state
        .write()
        .unwrap()
        .set("locale", Value::String("missing".into()))
        .unwrap();
    assert!(p.prepare_load().is_err());
    assert_eq!(p.presenter.screen(), &screen);
    assert_eq!(fs::read(path).unwrap(), file);
}

#[test]
fn bundled_font_registers_covers_fixture_glyphs_and_is_the_default_ui_font() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/localization-demo");
    let story = Story::load(&root).unwrap();
    let sample = story
        .localization()
        .unwrap()
        .languages
        .values()
        .flat_map(|c| c.ui.values().chain(c.dialogue.values()))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
    let mut app = crate::tests::app_for(story, compiled);
    app.init_resource::<Assets<Font>>()
        .init_resource::<bevy::text::FontCx>();
    app.world_mut().run_system_once(declarative::setup).unwrap();
    let handle = app
        .world()
        .resource::<Ui>()
        .default_font
        .clone()
        .expect("configured common font");
    let data = app
        .world()
        .resource::<Assets<Font>>()
        .get(&handle)
        .unwrap()
        .data
        .clone();
    let registered = bevy::text::FontCx::default()
        .collection
        .register_fonts(data.clone(), None);
    let faces: Vec<_> = registered
        .into_iter()
        .flat_map(|(_, faces)| faces)
        .collect();
    assert!(!faces.is_empty());
    for c in sample.chars().filter(|c| !c.is_control()) {
        assert!(
            faces.iter().any(|face| face
                .charmap_index()
                .charmap(data.as_ref())
                .unwrap()
                .map(c)
                .is_some()),
            "missing glyph {c}"
        );
    }
    app.world_mut()
        .run_system_once(declarative::sync_layers)
        .unwrap();
    let world = app.world_mut();
    for font in world.query::<&TextFont>().iter(world) {
        assert!(matches!(&font.font,bevy::text::FontSource::Handle(actual) if actual==&handle));
    }
    let mut fonts = bevy::text::FontCx::default();
    fonts.collection.register_fonts(data, None);
    fonts.set_sans_serif_family("Noto Sans JP").unwrap();
    fonts.set_system_ui_family("Noto Sans JP").unwrap();
    let mut layouts = bevy::text::LayoutCx::default();
    let text = "東京都に行く";
    let mut layout = layouts
        .0
        .ranged_builder(&mut fonts.context, text, 1.0, true)
        .build(text);
    layout.break_all_lines(None);
    let mut words = 0;
    for line in layout.lines() {
        for run in line.runs() {
            words += run
                .clusters()
                .filter(|cluster| cluster.is_word_boundary())
                .count();
        }
    }
    assert!(
        words > 1,
        "Japanese text needs dictionary word boundaries, found {words}"
    );
}

#[test]
fn fallback_panel_retranslates_existing_line_and_choice_entities() {
    let (temp, _app) = fixture();
    let path = temp.path().join("manifest.jsonc");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest.as_object_mut().unwrap().remove("screens");
    manifest.as_object_mut().unwrap().remove("initialScreen");
    fs::write(path, manifest.to_string()).unwrap();
    let story = Story::load(temp.path()).unwrap();
    let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
    let mut app = crate::tests::app_for(story, compiled);
    app.world_mut()
        .spawn((Panel(None), Visibility::Inherited, Node::default()));
    app.add_systems(Update, refresh_panel);
    app.update();
    let line = {
        let world = app.world_mut();
        world
            .query::<(Entity, &PanelBinding)>()
            .iter(world)
            .find(|(_, b)| matches!(b, PanelBinding::Text))
            .unwrap()
            .0
    };
    switch(&mut app, "ja");
    app.update();
    assert_eq!(app.world().get::<Text>(line).unwrap().0, "こんにちは Ada 1");
    app.world_mut().non_send_mut::<Player>().presenter.advance();
    app.world_mut().non_send_mut::<Player>().dirty = true;
    app.update();
    let button = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<Button>>()
            .iter(world)
            .next()
            .unwrap()
    };
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(button, bevy::input_focus::FocusCause::Navigated);
    switch(&mut app, "id");
    app.update();
    assert!(app.world().get::<Button>(button).is_some());
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));
    let world = app.world_mut();
    assert!(
        world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == "1. Pergi Ada")
    );
}
