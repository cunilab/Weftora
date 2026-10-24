//! Settings outlive playthroughs without sharing the progress-save format.
use crate::Player;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
use weftora_api::Value;
use weftora_core::Scope;

const WRITE_FAILURE: &str = "Preferences were not saved. Changes still apply to this session. Retry when storage is available.";

pub(super) const VOLUMES: [&str; 5] = [
    "settings.audio.master",
    "settings.audio.music",
    "settings.audio.ambience",
    "settings.audio.effects",
    "settings.audio.voice",
];
pub(super) const VOLUME_LABELS: [&str; 5] = ["Master", "Music", "Ambience", "Effects", "Voice"];

#[derive(Default)]
pub(super) struct Preferences {
    directory: Option<PathBuf>,
    defaults: BTreeMap<String, Value>,
    overrides: BTreeMap<String, Value>,
    dirty: bool,
    last_attempt: weftora_save::Entries,
    next_write: Duration,
    pub blocked: bool,
    pub failed: bool,
    pub status: String,
}

impl Preferences {
    pub fn temporary() -> Self {
        Self {
            status: "Changes apply to this session only.".into(),
            ..Self::default()
        }
    }
    pub fn enabled(&self) -> bool {
        self.directory.is_some()
    }
}

/// Called only by native normal launch. Tests and --start-node never touch the user's profile.
pub(super) fn initialize(player: &mut Player) {
    let directory =
        weftora_save::directory::resolve(&player.story.manifest().id, player.story.root(), None)
            .map_err(|e| e.to_string());
    match directory {
        Ok(directory) => initialize_at(player, directory),
        Err(error) => {
            bevy::log::warn!("Preferences storage unavailable: {error}");
            player.preferences.status =
                "Preferences cannot be saved on this device. Changes apply to this session.".into();
        }
    }
}

fn initialize_at(player: &mut Player, directory: PathBuf) {
    let mut profile = Preferences {
        directory: Some(directory.clone()),
        ..Preferences::default()
    };
    let result = (|| {
        let mut state = player
            .state
            .write()
            .map_err(|_| "settings lock poisoned".to_owned())?;
        profile.defaults = state.snapshot(Scope::Settings);
        match weftora_save::preferences::load(&directory, &player.story.manifest().id) {
            Ok(values) => {
                let mut candidate = state.clone();
                let mut ignored = 0;
                profile.overrides = values.clone();
                for (key, value) in values {
                    // Only current manifest Settings can be restored; never apply profile data to progress.
                    if player
                        .story
                        .manifest()
                        .state
                        .get(&key)
                        .is_none_or(|d| d.scope != Scope::Settings)
                        || !candidate
                            .get(&key)
                            .is_ok_and(|current| current.type_name() == value.type_name())
                        || !valid_value(player, &key, &value)
                    {
                        ignored += 1;
                        continue;
                    }
                    profile.overrides.insert(key.clone(), value.clone());
                    candidate.set(&key, value).map_err(|e| e.to_string())?;
                }
                candidate.drain_changes();
                *state = candidate;
                profile.status = if ignored > 0 {
                    format!(
                        "Ignored {ignored} incompatible preferences; they are retained for other Story revisions."
                    )
                } else {
                    "Preferences restored. Changes are saved automatically.".into()
                };
            }
            Err(weftora_save::Error::Missing) => {
                profile.status = "Changes are saved automatically for this Story.".into()
            }
            Err(error) => {
                profile.blocked = true;
                bevy::log::warn!("Saved preferences could not be read: {error}");
                profile.status = "Saved preferences are unavailable. Changes are temporary until you reset saved preferences; the old file will be retained.".into();
            }
        }
        profile.last_attempt = state.snapshot_entries(Scope::Settings);
        Ok::<_, String>(())
    })();
    if let Err(error) = result {
        profile.blocked = true;
        profile.status = error;
    }
    player.preferences = profile;
}

fn valid_value(player: &Player, key: &str, value: &Value) -> bool {
    if VOLUMES.contains(&key)
        && !matches!(value, Value::Float(n) if n.is_finite() && (0.0..=1.0).contains(n))
    {
        return false;
    }
    if let Some(localization) = player.story.localization()
        && localization.definition.state_key.as_deref() == Some(key)
        && !matches!(value, Value::String(language) if localization.languages.contains_key(language))
    {
        return false;
    }
    true
}

impl Player {
    pub(super) fn volume_percent(&self, index: usize) -> Option<u8> {
        let key = *VOLUMES.get(index)?;
        if self.story.manifest().state.get(key)?.scope != Scope::Settings {
            return None;
        }
        let state = self.state.read().ok()?;
        match state.get(key).ok()? {
            Value::Float(n) if (0.0..=1.0).contains(n) => Some((n * 100.0).round() as u8),
            _ => None,
        }
    }
    pub(super) fn change_volume(&mut self, index: usize, increase: bool) -> Result<(), String> {
        let percent = self
            .volume_percent(index)
            .ok_or("This Story does not expose that volume control")?;
        let next = if increase {
            ((u16::from(percent) / 10 + 1) * 10).min(100) as u8
        } else {
            percent.saturating_sub(1) / 10 * 10
        };
        self.set_preference(VOLUMES[index], Value::Float(f64::from(next) / 100.0))
    }
    pub(super) fn change_language(&mut self, index: usize) -> Result<(), String> {
        let catalogs = self
            .story
            .localization()
            .ok_or("This Story does not expose language selection")?;
        let key = catalogs
            .definition
            .state_key
            .clone()
            .ok_or("This Story does not expose language selection")?;
        let language = catalogs
            .languages
            .keys()
            .nth(index)
            .cloned()
            .ok_or("Unknown language")?;
        self.set_preference(&key, Value::String(language))
    }
    fn set_preference(&mut self, key: &str, value: Value) -> Result<(), String> {
        if self
            .story
            .manifest()
            .state
            .get(key)
            .is_none_or(|d| d.scope != Scope::Settings)
            || !valid_value(self, key, &value)
        {
            return Err("Unsupported preference value".into());
        }
        let mut state = self.state.write().map_err(|_| "settings lock poisoned")?;
        state.set(key, value.clone()).map_err(|e| e.to_string())?;
        if self.preferences.enabled() {
            self.preferences.overrides.insert(key.to_owned(), value);
            self.preferences.dirty = true;
        }
        // Host preferences do not run Story rules, including while the runner is dormant.
        Ok(())
    }

    pub(super) fn persist_preferences(&mut self, now: Duration, force: bool) {
        let profile = &mut self.preferences;
        if profile.directory.is_none()
            || profile.blocked
            || (!force && profile.failed)
            || (!force && now < profile.next_write)
        {
            return;
        }
        if force {
            profile.failed = false;
        }
        let result = (|| {
            let state = self
                .state
                .read()
                .map_err(|_| "settings lock poisoned".to_owned())?;
            if !profile.dirty && state.matches_snapshot(Scope::Settings, &profile.last_attempt) {
                return Ok(None);
            }
            let snapshot = state.snapshot_entries(Scope::Settings);
            for (key, value) in &snapshot {
                if player_setting(&self.story, key) && profile.last_attempt.get(key) != Some(value)
                {
                    if let Some(value) = value {
                        profile.overrides.insert(key.clone(), value.clone());
                    } else {
                        profile.overrides.remove(key);
                    }
                    profile.dirty = true;
                }
            }
            profile.last_attempt = snapshot;
            if !profile.dirty {
                return Ok(None);
            }
            weftora_core::presentation::volumes(&state)?;
            if let Some(localization) = self.story.localization() {
                localization.language(&state)?;
            }
            weftora_save::preferences::write(
                profile.directory.as_ref().expect("checked directory"),
                &self.story.manifest().id,
                &profile.overrides,
            )
            .map_err(|e| e.to_string())?;
            Ok::<_, String>(Some(()))
        })();
        profile.next_write = now + Duration::from_millis(250);
        match result {
            Ok(Some(())) => {
                profile.dirty = false;
                if self.save_status == WRITE_FAILURE {
                    self.save_status.clear();
                }
                profile.status = "Changes are saved automatically.".into();
            }
            Ok(None) => {}
            Err(error) => {
                profile.failed = true;
                bevy::log::warn!("Preferences could not be saved: {error}");
                profile.status = WRITE_FAILURE.into();
            }
        }
    }
    pub(super) fn preferences_unsaved(&self) -> bool {
        self.preferences.enabled()
            && (self.preferences.dirty
                || self.state.read().map_or(true, |state| {
                    !state.matches_snapshot(Scope::Settings, &self.preferences.last_attempt)
                }))
    }
    pub(super) fn retry_preferences(&mut self, now: Duration) {
        self.preferences.failed = false;
        self.persist_preferences(now, true);
    }
    pub(super) fn reset_preferences(&mut self) -> Result<(), String> {
        let directory = self
            .preferences
            .directory
            .as_ref()
            .ok_or("Preferences cannot be saved in this preview")?;
        let mut state = self.state.write().map_err(|_| "settings lock poisoned")?;
        let mut candidate = state.clone();
        let defaults = self
            .preferences
            .defaults
            .iter()
            .map(|(key, value)| (key.clone(), Some(value.clone())))
            .collect();
        candidate
            .apply_snapshot(Scope::Settings, &defaults)
            .map_err(|e| e.to_string())?;
        let retained = weftora_save::preferences::replace(directory, &self.story.manifest().id)
            .map_err(|e| e.to_string())?;
        *state = candidate;
        self.preferences.last_attempt = state.snapshot_entries(Scope::Settings);
        self.preferences.overrides.clear();
        self.preferences.dirty = false;
        self.preferences.blocked = false;
        self.preferences.failed = false;
        if self.save_status == WRITE_FAILURE {
            self.save_status.clear();
        }
        self.preferences.status = if retained.is_some() {
            "Preferences reset. The previous file was retained separately.".into()
        } else {
            "Preferences reset to this Story's defaults.".into()
        };
        Ok(())
    }
}

fn player_setting(story: &weftora_story::Story, key: &str) -> bool {
    story
        .manifest()
        .state
        .get(key)
        .is_some_and(|d| d.scope == Scope::Settings)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use bevy::prelude::*;
    use serde_json::json;

    pub(crate) fn fixture() -> (tempfile::TempDir, App) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("main.yarn"), "title: Start\n---\n<<declare $n = 0>>\n<<checkpoint begin>>\n<<set $n = $n + 1>>\nHello {$n} #line:hello\n-> Left #line:left\n-> Right #line:right\n===\n").unwrap();
        std::fs::write(root.join("font.ttf"), b"model-only font fixture").unwrap();
        std::fs::write(
            root.join("fonts.jsonc"),
            r#"[{"id":"font.common","path":"font.ttf"}]"#,
        )
        .unwrap();
        std::fs::write(root.join("manifest.jsonc"), json!({
            "manifestSchema":1,"id":"test.preferences","title":"Preferences","storyVersion":"0.1.0","saveSchema":1,
            "entrypoint":"Start","dialogue":["main.yarn"],
            "requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["dialogue.yarn/1","save/1","localization.catalog/1","localization.text/1","assets.catalog/1"]},
            "localization":{"baseLanguage":"en","stateKey":"locale","font":"font.common","languages":{"en":"en.jsonc","id":"id.jsonc","ja":"ja.jsonc"}},
            "catalogs":{"fonts":["fonts.jsonc"]},
            "state":{"n":{"default":0.0},"temporary":{"default":false,"scope":"session"},"locale":{"default":"en","scope":"settings"},"settings.audio.master":{"default":0.5,"scope":"settings"},"settings.audio.music":{"default":0.7,"scope":"settings"}}
        }).to_string()).unwrap();
        for (language, dialogue) in [
            ("en", json!({})),
            (
                "id",
                json!({"line:hello":"Halo {0}","line:left":"Kiri","line:right":"Kanan"}),
            ),
            (
                "ja",
                json!({"line:hello":"こんにちは {0}","line:left":"左","line:right":"右"}),
            ),
        ] {
            std::fs::write(
                root.join(format!("{language}.jsonc")),
                json!({"localizationSchema":1,"ui":{},"dialogue":dialogue}).to_string(),
            )
            .unwrap();
        }
        let app = reopen(&root, &temp.path().join("data"));
        (temp, app)
    }

    fn reopen(root: &std::path::Path, data: &std::path::Path) -> App {
        let story = weftora_story::Story::load(root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        app.world_mut().non_send_mut::<Player>().save_store =
            Some(Ok(weftora_save::storage::Store::new(data.to_path_buf())));
        initialize_at(
            &mut app.world_mut().non_send_mut::<Player>(),
            data.to_path_buf(),
        );
        app.update();
        app
    }
    fn persist(app: &mut App) {
        app.world_mut()
            .non_send_mut::<Player>()
            .persist_preferences(Duration::from_secs(2), true);
    }

    #[test]
    fn restart_preserves_changed_preferences_untouched_defaults_follow_story_edits() {
        let (temp, mut app) = fixture();
        let data = temp.path().join("data");
        persist(&mut app);
        assert!(
            !data.join("preferences.json").exists(),
            "launch must not persist defaults"
        );
        app.world_mut()
            .non_send_mut::<Player>()
            .change_volume(1, true)
            .unwrap();
        persist(&mut app);
        let overrides = weftora_save::preferences::load(&data, "test.preferences").unwrap();
        assert_eq!(
            overrides,
            BTreeMap::from([("settings.audio.music".into(), Value::Float(0.8))])
        );
        let manifest = temp.path().join("story/manifest.jsonc");
        let mut document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
        document["state"]["settings.audio.master"]["default"] = 0.2.into();
        std::fs::write(manifest, document.to_string()).unwrap();
        let mut restarted = reopen(&temp.path().join("story"), &data);
        assert_eq!(
            restarted.world().non_send::<Player>().volume_percent(0),
            Some(20)
        );
        assert_eq!(
            restarted.world().non_send::<Player>().volume_percent(1),
            Some(80)
        );
        let bytes = std::fs::read(data.join("preferences.json")).unwrap();
        {
            let mut player = restarted.world_mut().non_send_mut::<Player>();
            player.write_save().unwrap();
            let fresh = player.fresh_candidate(true).unwrap();
            player.state = fresh.state;
            player.presenter = fresh.presenter;
            let loaded = player.prepare_load().unwrap();
            player.state = loaded.state;
            player.presenter = loaded.presenter;
        }
        persist(&mut restarted);
        assert_eq!(std::fs::read(data.join("preferences.json")).unwrap(), bytes);
        assert!(
            !data.join("preferences.bak").exists(),
            "load/new game must not rewrite unchanged preferences"
        );
    }

    #[test]
    fn corrupt_profiles_are_preserved_until_confirmed_reset_and_never_rotate_away_afterwards() {
        let (temp, _app) = fixture();
        let data = temp.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let corrupt = br#"{"preferencesFormat":99,"storyId":"test.preferences","values":{}}"#;
        std::fs::write(data.join("preferences.json"), corrupt).unwrap();
        let mut app = reopen(&temp.path().join("story"), &data);
        assert!(app.world().non_send::<Player>().preferences.blocked);
        app.world_mut()
            .non_send_mut::<Player>()
            .change_volume(0, true)
            .unwrap();
        persist(&mut app);
        assert_eq!(
            std::fs::read(data.join("preferences.json")).unwrap(),
            corrupt
        );
        app.world_mut()
            .non_send_mut::<Player>()
            .reset_preferences()
            .unwrap();
        assert_eq!(app.world().non_send::<Player>().volume_percent(0), Some(50));
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .state
                .read()
                .unwrap()
                .get("n"),
            Ok(&Value::Float(1.0))
        );
        for _ in 0..2 {
            app.world_mut()
                .non_send_mut::<Player>()
                .change_volume(0, true)
                .unwrap();
            persist(&mut app);
        }
        let retained: Vec<_> = std::fs::read_dir(&data)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("preferences.replaced-")
            })
            .collect();
        assert_eq!(retained.len(), 1);
        assert_eq!(std::fs::read(&retained[0]).unwrap(), corrupt);
    }

    #[test]
    fn profile_cannot_write_progress_session_or_invalid_audio_locale() {
        let (temp, _app) = fixture();
        let data = temp.path().join("data");
        weftora_save::preferences::write(
            &data,
            "test.preferences",
            &BTreeMap::from([
                ("n".into(), Value::Float(99.0)),
                ("temporary".into(), Value::Bool(true)),
                ("locale".into(), Value::String("unknown".into())),
                ("settings.audio.master".into(), Value::Float(2.0)),
                ("settings.audio.music".into(), Value::Float(0.4)),
                ("removed".into(), Value::Bool(true)),
            ]),
        )
        .unwrap();
        let mut app = reopen(&temp.path().join("story"), &data);
        {
            let player = app.world().non_send::<Player>();
            let state = player.state.read().unwrap();
            assert_eq!(state.get("n"), Ok(&Value::Float(1.0)));
            assert_eq!(state.get("temporary"), Ok(&Value::Bool(false)));
            assert_eq!(state.get("locale"), Ok(&Value::String("en".into())));
            assert_eq!(player.volume_percent(0), Some(50));
            assert_eq!(player.volume_percent(1), Some(40));
            assert!(player.preferences.status.contains("Ignored 5"));
        }
        for _ in 0..2 {
            app.world_mut()
                .non_send_mut::<Player>()
                .change_volume(1, true)
                .unwrap();
            persist(&mut app);
        }
        let retained = weftora_save::preferences::load(&data, "test.preferences").unwrap();
        assert_eq!(retained["removed"], Value::Bool(true));
        assert_eq!(retained["locale"], Value::String("unknown".into()));
        app.world_mut()
            .non_send_mut::<Player>()
            .change_language(0)
            .unwrap();
        persist(&mut app);
        assert_eq!(
            weftora_save::preferences::load(&data, "test.preferences").unwrap()["locale"],
            Value::String("en".into())
        );
    }

    #[test]
    fn failure_does_not_spin_and_explicit_retry_saves_latest_values() {
        let (temp, mut app) = fixture();
        let data = temp.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(data.join("preferences.lock"))
            .unwrap();
        lock.lock().unwrap();
        app.world_mut()
            .non_send_mut::<Player>()
            .change_volume(0, true)
            .unwrap();
        persist(&mut app);
        assert!(app.world().non_send::<Player>().preferences.failed);
        let status = app.world().non_send::<Player>().preferences.status.clone();
        for _ in 0..2 {
            app.world_mut()
                .non_send_mut::<Player>()
                .change_volume(0, true)
                .unwrap();
            app.world_mut()
                .non_send_mut::<Player>()
                .persist_preferences(Duration::from_secs(4), false);
        }
        assert_eq!(app.world().non_send::<Player>().preferences.status, status);
        assert!(!data.join("preferences.json").exists());
        lock.unlock().unwrap();
        app.world_mut()
            .non_send_mut::<Player>()
            .retry_preferences(Duration::from_secs(3));
        assert!(!app.world().non_send::<Player>().preferences.failed);
        assert_eq!(
            weftora_save::preferences::load(&data, "test.preferences").unwrap()["settings.audio.master"],
            Value::Float(0.8)
        );
    }

    #[test]
    fn volume_grid_and_language_changes_are_exact_and_do_not_replay_dialogue() {
        let (temp, mut app) = fixture();
        {
            let mut player = app.world_mut().non_send_mut::<Player>();
            player
                .set_preference("settings.audio.master", Value::Float(0.25))
                .unwrap();
            player.change_volume(0, true).unwrap();
            assert_eq!(player.volume_percent(0), Some(30));
            player
                .set_preference("settings.audio.master", Value::Float(0.25))
                .unwrap();
            player.change_volume(0, false).unwrap();
            assert_eq!(player.volume_percent(0), Some(20));
            player
                .set_preference("settings.audio.master", Value::Float(0.0))
                .unwrap();
            for _ in 0..10 {
                player.change_volume(0, true).unwrap();
            }
            assert_eq!(
                player.state.read().unwrap().get("settings.audio.master"),
                Ok(&Value::Float(1.0))
            );
            let revision = player.presenter.revision();
            player.change_language(2).unwrap();
            player.presenter.refresh_localization();
            assert!(
                matches!(player.presenter.screen(), crate::Screen::Line { text, .. } if text == "こんにちは 1")
            );
            assert_eq!(player.presenter.revision(), revision);
            assert_eq!(
                player.state.read().unwrap().get("n"),
                Ok(&Value::Float(1.0))
            );
            player.persist_preferences(Duration::from_secs(2), true);
        }
        let restarted = reopen(&temp.path().join("story"), &temp.path().join("data"));
        assert_eq!(
            restarted
                .world()
                .non_send::<Player>()
                .state
                .read()
                .unwrap()
                .get("locale"),
            Ok(&Value::String("ja".into()))
        );
    }

    #[test]
    fn start_node_preview_never_uses_normal_preferences() {
        let (temp, app) = fixture();
        let data = temp.path().join("data");
        weftora_save::preferences::write(
            &data,
            "test.preferences",
            &BTreeMap::from([("settings.audio.master".into(), Value::Float(0.1))]),
        )
        .unwrap();
        let before = std::fs::read(data.join("preferences.json")).unwrap();
        let story = app.world().non_send::<Player>().story.clone();
        let compiled = app.world().non_send::<Player>().compiled.clone();
        let state = std::sync::Arc::new(std::sync::RwLock::new(story.initial_state().unwrap()));
        let mut preview =
            crate::prepare_player(story, compiled, state, true, Some("Start")).unwrap();
        assert!(!preview.preferences.enabled());
        assert_eq!(preview.volume_percent(0), Some(50));
        preview.change_volume(0, true).unwrap();
        preview.persist_preferences(Duration::from_secs(9), true);
        assert_eq!(
            std::fs::read(data.join("preferences.json")).unwrap(),
            before
        );
    }
}
