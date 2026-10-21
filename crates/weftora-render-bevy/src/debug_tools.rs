//! Opt-in developer state inspection; edits use the ordinary validated rule bridge.
use bevy::input::keyboard::KeyboardInput;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use weftora_api::Value;
use weftora_core::{Scope, State};
use weftora_story::ui::UiAction;

use crate::{Player, Screen, declarative};

const MAX_INPUT: usize = 16 * 1024;
const PAGE_SIZE: usize = 12;

#[derive(Resource, Default)]
pub(super) struct Inspector {
    enabled: bool,
    open: bool,
    pub captured: bool,
    selected: Option<String>,
    input: String,
    status: String,
    previous_focus: Option<Entity>,
}
impl Inspector {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..default()
        }
    }
    fn select(&mut self, player: &Player, movement: isize) {
        let Ok(state) = player.state.read() else {
            self.status = "State lock poisoned".into();
            return;
        };
        let keys = state_keys(&state);
        let position = self
            .selected
            .as_ref()
            .and_then(|key| keys.iter().position(|k| k == key))
            .unwrap_or(0);
        let position = position
            .saturating_add_signed(movement)
            .min(keys.len().saturating_sub(1));
        self.selected = keys.get(position).cloned();
        self.input = self
            .selected
            .as_ref()
            .and_then(|key| state.get(key).ok())
            .and_then(|value| serde_json::to_string(value).ok())
            .unwrap_or_default();
        self.status.clear();
        if self.input.len() > MAX_INPUT {
            self.input.clear();
            self.status =
                "Value exceeds the 16 KiB editor limit; enter a replacement JSON value.".into();
        }
    }
}

fn state_keys(state: &State) -> Vec<String> {
    let mut keys: Vec<_> = [Scope::Persistent, Scope::Session, Scope::Settings]
        .into_iter()
        .flat_map(|scope| state.scope_entries(scope).map(|(key, _)| key.to_string()))
        .collect();
    keys.sort();
    keys
}

fn apply(player: &mut Player, key: &str, input: &str) -> Result<(), String> {
    if matches!(player.presenter.screen(), Screen::Error(_)) {
        return Err("State editing is unavailable after a Story error".into());
    }
    if !player.pending.is_empty() {
        return Err("Wait for pending presentation commands".into());
    }
    if input.len() > MAX_INPUT {
        return Err("JSON value exceeds 16 KiB".into());
    }
    let value: Value = serde_json::from_str(input).map_err(|error| error.to_string())?;
    {
        let mut candidate = player
            .state
            .read()
            .map_err(|_| "State lock poisoned")?
            .clone();
        candidate.get(key).map_err(|error| error.to_string())?;
        candidate
            .set(key, value.clone())
            .map_err(|error| error.to_string())?;
        if let Some(error) = weftora_yarn::check_state(&player.compiled, &candidate).first() {
            return Err(error.to_string());
        }
        declarative::validate_display(&player.story, &candidate, player.presenter.screen())?;
        if weftora_yarn::media_enabled(&player.story) {
            weftora_core::presentation::volumes(&candidate)?;
        }
    }
    if let Err(error) = player
        .presenter
        .source_mut()
        .ui_action(&UiAction::SetState {
            key: key.into(),
            value,
        })
    {
        let message = error.to_string();
        player.presenter.fail(message.clone());
        return Err(message);
    }
    Ok(())
}

pub(super) fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mut events: MessageReader<KeyboardInput>,
    mut inspector: ResMut<Inspector>,
    mut player: NonSendMut<Player>,
    mut focus: ResMut<InputFocus>,
    entities: Query<()>,
) {
    inspector.captured = inspector.enabled && inspector.open;
    if !inspector.enabled {
        events.clear();
        return;
    }
    if keys.just_pressed(KeyCode::F12) || (inspector.open && keys.just_pressed(KeyCode::Escape)) {
        inspector.captured = true;
        inspector.open = !inspector.open;
        if inspector.open {
            inspector.previous_focus = focus.get();
            focus.clear();
            inspector.select(&player, 0);
        } else {
            focus.clear();
            if let Some(entity) = inspector
                .previous_focus
                .take()
                .filter(|entity| entities.contains(*entity))
            {
                focus.set(entity, FocusCause::Navigated);
            }
        }
        events.clear();
        return;
    }
    if !inspector.open {
        events.clear();
        return;
    }
    focus.clear();
    for (key, movement) in [
        (KeyCode::ArrowUp, -1),
        (KeyCode::ArrowDown, 1),
        (KeyCode::PageUp, -(PAGE_SIZE as isize)),
        (KeyCode::PageDown, PAGE_SIZE as isize),
    ] {
        if keys.just_pressed(key) {
            inspector.select(&player, movement);
        }
    }
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let select_all = control
        && keys.just_pressed(KeyCode::KeyA)
        && !keys.pressed(KeyCode::AltLeft)
        && !keys.pressed(KeyCode::AltRight);
    if select_all {
        inspector.input.clear();
    }
    for event in events.read().filter(|event| event.state.is_pressed()) {
        if event.key_code == KeyCode::Backspace {
            inspector.input.pop();
        } else if !(select_all && event.key_code == KeyCode::KeyA)
            && let Some(text) = &event.text
        {
            let text: String = text.chars().filter(|c| !c.is_control()).collect();
            if inspector.input.len() + text.len() <= MAX_INPUT {
                inspector.input.push_str(&text);
            } else {
                inspector.status = "JSON value exceeds 16 KiB".into();
            }
        }
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        inspector.status = match inspector.selected.as_deref() {
            Some(key) => match apply(&mut player, key, &inspector.input) {
                Ok(()) => {
                    "Applied. Persistent edits require a new checkpoint before saving.".into()
                }
                Err(message) => message,
            },
            None => "No state keys declared".into(),
        };
    }
}

#[derive(Component)]
pub(super) struct Overlay;
#[derive(Component)]
pub(super) struct Contents;

pub(super) fn setup(mut commands: Commands, inspector: Res<Inspector>) {
    if !inspector.enabled {
        return;
    }
    commands
        .spawn((
            Overlay,
            GlobalZIndex(1_000_000),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(24)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgb(0.02, 0.02, 0.04)),
        ))
        .with_children(|parent| {
            parent.spawn((Contents, crate::label("", 16.0, Color::WHITE)));
        });
}

pub(super) fn refresh(
    inspector: Res<Inspector>,
    player: NonSend<Player>,
    mut overlay: Query<&mut Node, With<Overlay>>,
    mut contents: Query<&mut Text, With<Contents>>,
) {
    if !inspector.enabled {
        return;
    }
    let (Ok(mut overlay), Ok(mut text)) = (overlay.single_mut(), contents.single_mut()) else {
        return;
    };
    overlay.display = if inspector.open {
        Display::Flex
    } else {
        Display::None
    };
    if !inspector.open {
        return;
    }
    let Ok(state) = player.state.read() else {
        text.0 = "State lock poisoned".into();
        return;
    };
    let keys = state_keys(&state);
    let selected = inspector
        .selected
        .as_ref()
        .and_then(|key| keys.iter().position(|k| k == key))
        .unwrap_or(0);
    let start = selected / PAGE_SIZE * PAGE_SIZE;
    let mut lines = vec!["DEVELOPER STATE INSPECTOR — F12 / Esc: close".to_string(),
        "Up/Down: select • PageUp/PageDown: page • Ctrl+A: replace • Backspace: delete • Enter: apply JSON".into(),
        "Story input is captured. Displayed dialogue remains unchanged until normal progression.".into(), String::new()];
    for (position, key) in keys.iter().enumerate().skip(start).take(PAGE_SIZE) {
        let value = state
            .get(key)
            .ok()
            .and_then(|value| serde_json::to_string(value).ok())
            .unwrap_or_else(|| "<removed>".into());
        lines.push(format!(
            "{} {} [{:?}] = {}",
            if position == selected { ">" } else { " " },
            key.chars().take(56).collect::<String>(),
            state.scope(key).unwrap_or_default(),
            value.chars().take(120).collect::<String>()
        ));
    }
    lines.push(format!(
        "\nKeys {}–{} of {}\nSelected: {}\n\nJSON: {}\n\n{}",
        if keys.is_empty() { 0 } else { start + 1 },
        (start + PAGE_SIZE).min(keys.len()),
        keys.len(),
        inspector.selected.as_deref().unwrap_or("<none>"),
        inspector
            .input
            .chars()
            .rev()
            .take(240)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>(),
        inspector.status
    ));
    text.0 = lines.join("\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declarative::{Intent, Intents};
    use std::sync::Arc;

    fn fixture() -> (tempfile::TempDir, App) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("manifest.jsonc"), r#"{"manifestSchema":1,"saveSchema":1,"id":"test.debug-tools","storyVersion":"0.1.0","title":"Debug","requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["dialogue.yarn/1","rules.events/1","save/1"]},"entrypoint":"Start","dialogue":["main.yarn"],"events":["events.jsonc"],"state":{"n":{"default":0},"derived":{"default":false},"temporary":{"default":0,"scope":"session"}}}"#).unwrap();
        std::fs::write(root.join("main.yarn"), "title: Start\n---\n<<declare $n = 0>>\n<<checkpoint debug.start>>\nFirst {$n}\nSecond {$n}\n===\n").unwrap();
        std::fs::write(root.join("events.jsonc"), r#"{"rulesSchema":1,"events":[{"id":"derive","trigger":"state.changed.n","actions":[{"do":"state.set","key":"derived","value":true}]}]}"#).unwrap();
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        app.world_mut().non_send_mut::<Player>().save_store = Some(Ok(
            weftora_save::storage::Store::new(temp.path().join("saves")),
        ));
        (temp, app)
    }
    fn tap(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
    }
    fn advance_intent(app: &mut App) {
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action: UiAction::Advance,
            });
    }

    #[test]
    fn overlay_setup_spawns_only_when_enabled_and_renders_after_opening() {
        for enabled in [false, true] {
            let (_temp, mut app) = fixture();
            app.world_mut().resource_mut::<Inspector>().enabled = enabled;
            app.add_systems(Startup, setup);
            app.add_systems(Update, refresh.after(crate::drive));
            app.update();
            let mut overlays = app.world_mut().query_filtered::<&Node, With<Overlay>>();
            assert_eq!(overlays.iter(app.world()).count(), usize::from(enabled));
            if enabled {
                tap(&mut app, KeyCode::F12);
                assert_eq!(overlays.single(app.world()).unwrap().display, Display::Flex);
                let mut text = app.world_mut().query_filtered::<&Text, With<Contents>>();
                assert!(
                    text.single(app.world())
                        .unwrap()
                        .0
                        .contains("DEVELOPER STATE INSPECTOR")
                );
            }
        }
    }

    #[test]
    fn inspector_is_disabled_by_default_and_does_not_capture_story_input() {
        let (_temp, mut app) = fixture();
        advance_intent(&mut app);
        tap(&mut app, KeyCode::F12);
        assert!(!app.world().resource::<Inspector>().captured);
        assert!(
            matches!(app.world().non_send::<Player>().presenter.screen(), Screen::Line { text, .. } if text == "Second 0")
        );
    }

    #[test]
    fn opening_open_and_closing_frames_discard_story_keyboard_and_pointer_intents() {
        let (_temp, mut app) = fixture();
        app.world_mut().resource_mut::<Inspector>().enabled = true;
        let revision = app.world().non_send::<Player>().presenter.revision();
        advance_intent(&mut app);
        tap(&mut app, KeyCode::F12);
        assert!(app.world().resource::<Inspector>().open);
        advance_intent(&mut app);
        tap(&mut app, KeyCode::F5);
        assert!(app.world().non_send::<Player>().save_status.is_empty());
        advance_intent(&mut app);
        tap(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<Inspector>().open);
        assert_eq!(
            app.world().non_send::<Player>().presenter.revision(),
            revision
        );
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        tap(&mut app, KeyCode::Enter);
        assert!(app.world().non_send::<Player>().presenter.revision() > revision);
    }

    #[test]
    fn inspector_does_not_discard_pending_presentation_actions() {
        let (_temp, mut app) = fixture();
        app.world_mut().resource_mut::<Inspector>().enabled = true;
        app.world_mut()
            .non_send_mut::<Player>()
            .pending
            .push(crate::Action::Background("missing.asset".into()));
        tap(&mut app, KeyCode::F12);
        assert!(matches!(
            app.world().non_send::<Player>().presenter.screen(),
            Screen::Error(_)
        ));
        assert!(app.world().non_send::<Player>().pending.is_empty());
    }

    #[test]
    fn invalid_edits_leave_state_runner_and_checkpoint_unchanged() {
        let (_temp, mut app) = fixture();
        let player = &mut *app.world_mut().non_send_mut::<Player>();
        let before = player.state.read().unwrap().clone();
        for (key, value) in [
            ("n", "false"),
            ("n", "{"),
            ("n", "16777217"),
            ("missing", "0"),
        ] {
            assert!(apply(player, key, value).is_err());
            assert_eq!(*player.state.read().unwrap(), before);
            assert!(player.presenter.source().save_point().is_ok());
        }
        assert!(apply(player, "n", &" ".repeat(MAX_INPUT + 1)).is_err());
        player.presenter.advance();
        assert!(
            matches!(player.presenter.screen(), Screen::Line { text, .. } if text == "Second 0")
        );
    }

    #[test]
    fn persistent_edits_settle_rules_disarm_save_and_preserve_existing_files() {
        let (temp, mut app) = fixture();
        let player = &mut *app.world_mut().non_send_mut::<Player>();
        player.write_save().unwrap();
        let path = temp.path().join("saves/save.json");
        let good = std::fs::read(&path).unwrap();
        let screen = player.presenter.screen().clone();
        apply(player, "n", "3").unwrap();
        assert_eq!(
            player.state.read().unwrap().get("derived"),
            Ok(&Value::Bool(true))
        );
        assert_eq!(player.presenter.screen(), &screen);
        assert!(player.write_save().is_err());
        assert_eq!(std::fs::read(path).unwrap(), good);
    }

    #[test]
    fn session_edits_keep_save_eligible_and_load_replaces_the_inspected_state() {
        let (_temp, mut app) = fixture();
        app.world_mut().resource_mut::<Inspector>().enabled = true;
        let old = {
            let player = &mut *app.world_mut().non_send_mut::<Player>();
            player.write_save().unwrap();
            apply(player, "temporary", "5").unwrap();
            assert!(player.presenter.source().save_point().is_ok());
            apply(player, "n", "3").unwrap();
            player.state.clone()
        };
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action: UiAction::Load,
            });
        app.update();
        assert_eq!(app.world().non_send::<Player>().save_status, "Loaded.");
        assert!(!Arc::ptr_eq(&old, &app.world().non_send::<Player>().state));
        app.world_mut().resource_mut::<Inspector>().selected = Some("n".into());
        tap(&mut app, KeyCode::F12);
        assert_eq!(app.world().resource::<Inspector>().input, "0");
        assert_eq!(old.read().unwrap().get("n"), Ok(&Value::Int(3)));
    }

    #[test]
    fn unicode_text_entry_backspace_and_oversized_input_stay_inside_the_inspector() {
        use bevy::input::{ButtonState, keyboard::Key};
        let (_temp, mut app) = fixture();
        app.world_mut().resource_mut::<Inspector>().enabled = true;
        tap(&mut app, KeyCode::F12);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut().resource_mut::<Inspector>().input.clear();
        // AltGr produces valid text while Windows reports Control+Alt held.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlRight);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::AltRight);
        let send = |app: &mut App, code, text: &str| {
            app.world_mut().write_message(KeyboardInput {
                key_code: code,
                logical_key: Key::Character(text.into()),
                state: ButtonState::Pressed,
                text: Some(text.into()),
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            app.update();
        };
        send(&mut app, KeyCode::KeyE, "é");
        assert_eq!(app.world().resource::<Inspector>().input, "é");
        send(&mut app, KeyCode::Backspace, "");
        send(&mut app, KeyCode::KeyE, &"é".repeat(MAX_INPUT));
        let inspector = app.world().resource::<Inspector>();
        assert!(inspector.input.is_empty());
        assert!(inspector.status.contains("16 KiB"));
        assert!(
            matches!(app.world().non_send::<Player>().presenter.screen(), Screen::Line { text, .. } if text == "First 0")
        );
    }

    #[test]
    fn removed_declarations_stay_visible_in_the_key_list() {
        let mut state = State::new();
        state.set("removed", Value::Bool(true)).unwrap();
        state.remove("removed").unwrap();
        assert_eq!(state_keys(&state), ["removed"]);
    }
}
