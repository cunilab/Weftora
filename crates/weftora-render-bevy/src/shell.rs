//! Player navigation stays outside authored Story screens and checkpoint eligibility.
use bevy::ecs::system::SystemParam;
use bevy::input_focus::{
    FocusCause, InputFocus,
    tab_navigation::{TabGroup, TabIndex},
};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};
use bevy::window::WindowCloseRequested;
use weftora_story::ui::UiAction;

use crate::{
    Player, Screen,
    declarative::{Ui, UiRoot},
    media::Media,
    theme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Menu,
    Return,
    NewGame,
    Save,
    Load,
    MainMenu,
    Quit,
    Confirm,
    Cancel,
    Controls,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Title,
    Playing,
    Menu,
    Controls,
}

#[derive(Resource)]
pub(super) struct Shell {
    page: Page,
    confirm: Option<Action>,
    actions: Vec<Action>,
    controls_return: Page,
    return_focus: Option<Entity>,
    load_reason: Option<String>,
    refresh_load: bool,
    refresh_save: bool,
    save_reason: Option<String>,
    settled: bool,
    return_to_menu_button: bool,
}
impl Default for Shell {
    fn default() -> Self {
        Self::new(false)
    }
}
impl Shell {
    pub fn new(playing: bool) -> Self {
        Self {
            page: if playing { Page::Playing } else { Page::Title },
            confirm: None,
            actions: Vec::new(),
            controls_return: Page::Title,
            return_focus: None,
            load_reason: None,
            refresh_load: true,
            refresh_save: true,
            save_reason: None,
            settled: false,
            return_to_menu_button: false,
        }
    }
    pub fn is_playing(&self) -> bool {
        self.page == Page::Playing && self.confirm.is_none()
    }
    pub fn push(&mut self, action: Action) {
        if self.actions.len() < 16 {
            self.actions.push(action);
        }
    }
    pub fn invalidate_load(&mut self) {
        self.refresh_load = true;
        self.refresh_save = true;
    }
    fn open(&mut self, focus: &mut InputFocus) {
        self.return_focus = focus.get();
        focus.clear();
        self.page = Page::Menu;
        self.refresh_load = true;
        self.refresh_save = true;
    }
    fn resume(&mut self, focus: &mut InputFocus) {
        self.page = Page::Playing;
        self.confirm = None;
        focus.clear();
        if let Some(entity) = self.return_focus.take() {
            focus.set(entity, FocusCause::Auto);
        }
    }
    pub fn loaded(&mut self, focus: &mut InputFocus) {
        self.return_focus = None;
        self.return_to_menu_button = false;
        self.resume(focus);
    }
    pub fn confirm_request(&mut self, request: UiAction, focus: &mut InputFocus) {
        if self.is_playing() {
            self.open(focus);
        }
        self.confirm = Some(if matches!(request, UiAction::Save) {
            Action::Save
        } else {
            Action::Load
        });
        focus.clear();
    }
}
pub(super) fn playing(shell: Res<Shell>) -> bool {
    shell.is_playing()
}

#[cfg(test)]
pub(super) fn accept_and_return(app: &mut App) {
    if app.world().resource::<Shell>().confirm.is_some() {
        app.world_mut()
            .resource_mut::<Shell>()
            .push(Action::Confirm);
        app.update();
    }
    if app.world().resource::<Shell>().page == Page::Menu {
        app.world_mut().resource_mut::<Shell>().push(Action::Return);
        app.update();
    }
}

pub(super) struct Route {
    pub blocked: bool,
    pub request: Option<UiAction>,
    pub confirmed: bool,
}

#[derive(SystemParam)]
pub(super) struct Input<'w, 's> {
    pub shell: ResMut<'w, Shell>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    buttons: Query<'w, 's, &'static MenuButton>,
    close: MessageReader<'w, 's, WindowCloseRequested>,
}
impl Input<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub fn route(
        &mut self,
        player: &mut Player,
        ui: &mut Ui,
        media: &mut Media,
        keys: &ButtonInput<KeyCode>,
        inspector: &mut crate::debug_tools::Inspector,
        focus: &mut InputFocus,
        exit: &mut MessageWriter<AppExit>,
    ) -> Route {
        let mut captured = inspector.captured;
        let was_blocked = !self.shell.is_playing();
        let mut consumed = false;
        let mut request = None;
        let mut confirmed = false;
        // OS close requests are never swallowed by the developer drawer.
        if self.close.read().count() > 0 {
            inspector.open = false;
            inspector.captured = false;
            captured = false;
            self.shell.actions.clear();
            if self.shell.is_playing() {
                self.shell.open(focus);
            }
            self.shell.confirm = Some(Action::Quit);
            focus.clear();
            consumed = true;
        } else if !captured
            && (keys.just_pressed(KeyCode::Escape) || self.mouse.just_pressed(MouseButton::Right))
        {
            self.shell.actions.clear();
            if self.shell.confirm.take().is_some() {
                focus.clear();
            } else if self.shell.page == Page::Controls {
                self.shell.page = self.shell.controls_return;
                focus.clear();
            } else if self.shell.is_playing() && ui.modal() {
                let _ = ui.navigate(&UiAction::Close, &player.story, None);
            } else if self.shell.is_playing() {
                self.shell.open(focus);
            } else if self.shell.page == Page::Menu {
                self.shell.resume(focus);
            }
            consumed = true;
        }
        let actions = std::mem::take(&mut self.shell.actions);
        if !captured
            && !consumed
            && let Some(action) = actions.into_iter().next()
        {
            consumed = true;
            if self.shell.confirm.is_some() {
                if action == Action::Cancel {
                    self.shell.confirm = None;
                    focus.clear();
                } else if action == Action::Confirm {
                    let operation = self.shell.confirm.take().expect("confirmation exists");
                    match operation {
                        Action::Save | Action::Load => {
                            request = Some(if operation == Action::Save {
                                UiAction::Save
                            } else {
                                UiAction::Load
                            });
                            confirmed = true;
                        }
                        Action::Quit => {
                            exit.write(AppExit::Success);
                        }
                        Action::MainMenu => {
                            match player.fresh_candidate(false).and_then(|candidate| {
                                media.restore(&candidate.actions, &player.story)?;
                                Ok(candidate)
                            }) {
                                Ok(candidate) => {
                                    player.state = candidate.state;
                                    player.presenter = candidate.presenter;
                                    player.pending.clear();
                                    player.save_status.clear();
                                    player.dirty = true;
                                    ui.reset(&player.story);
                                    self.shell.page = Page::Title;
                                    self.shell.return_focus = None;
                                    self.shell.refresh_load = true;
                                }
                                Err(error) => {
                                    player.save_status =
                                        format!("Couldn't return to title: {error}");
                                }
                            }
                        }
                        _ => {}
                    }
                    focus.clear();
                }
            } else {
                match action {
                    Action::Menu if self.shell.is_playing() => self.shell.open(focus),
                    Action::Return if self.shell.page == Page::Menu => self.shell.resume(focus),
                    Action::Controls if matches!(self.shell.page, Page::Title | Page::Menu) => {
                        self.shell.controls_return = self.shell.page;
                        self.shell.page = Page::Controls;
                        focus.clear();
                    }
                    Action::Return if self.shell.page == Page::Controls => {
                        self.shell.page = self.shell.controls_return;
                        focus.clear();
                    }
                    Action::NewGame
                        if self.shell.page == Page::Title && player.startup_error.is_none() =>
                    {
                        match player.fresh_candidate(true).and_then(|candidate| {
                            media.restore(&candidate.actions, &player.story)?;
                            Ok(candidate)
                        }) {
                            Ok(candidate) => {
                                player.state = candidate.state;
                                player.presenter = candidate.presenter;
                                player.pending.clear();
                                player.save_status.clear();
                                player.dirty = true;
                                ui.reset(&player.story);
                                self.shell.loaded(focus);
                            }
                            Err(error) => {
                                player.save_status = format!("Couldn't start: {error}");
                            }
                        }
                    }
                    Action::Save if self.shell.page == Page::Menu => {
                        self.shell.confirm_request(UiAction::Save, focus)
                    }
                    Action::Load if matches!(self.shell.page, Page::Menu | Page::Title) => {
                        self.shell.confirm_request(UiAction::Load, focus)
                    }
                    Action::MainMenu | Action::Quit if !self.shell.is_playing() => {
                        self.shell.confirm = Some(action);
                        focus.clear();
                    }
                    _ => {}
                }
            }
        }
        if self.shell.page == Page::Menu
            && self
                .shell
                .return_focus
                .is_some_and(|e| self.buttons.get(e).is_ok())
        {
            self.shell.return_focus = None;
            self.shell.return_to_menu_button = true;
        }
        Route {
            blocked: was_blocked || consumed || !self.shell.is_playing(),
            request,
            confirmed,
        }
    }
}

#[derive(Component)]
pub(super) struct Root;
#[derive(Component)]
pub(super) struct MenuButton(Action);
#[derive(Component)]
pub(super) struct ButtonLabel(Action);
#[derive(Component)]
pub(super) struct Description;
#[derive(Resource, Default)]
pub(super) struct Built(Option<View>);
#[derive(Clone, PartialEq)]
struct View {
    startup_error: Option<String>,
    page: Page,
    confirm: Option<Action>,
    save_reason: Option<String>,
    load_reason: Option<String>,
}

pub(super) fn save_available(
    player: &Player,
    ui: &Ui,
    presentation: &crate::media::Presentation,
) -> Result<(), String> {
    if !player.pending.is_empty() {
        return Err("Wait for presentation commands to finish.".into());
    }
    if !ui.at_initial_base(&player.story) {
        return Err("close overlays and return to the initial screen before saving".into());
    }
    if matches!(player.presenter.screen(), Screen::Error(_)) {
        return Err("Save is unavailable after a Story error.".into());
    }
    if !presentation.views.settled(presentation.now)
        || !presentation.audio.settled(presentation.now)
    {
        return Err("Wait for the scene transition to finish.".into());
    }
    player.save_game().map(|_| ())
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    mut commands: Commands,
    player: NonSend<Player>,
    mut shell: ResMut<Shell>,
    ui: Res<Ui>,
    presentation: Res<crate::media::Presentation>,
    mut built: Local<Built>,
    roots: Query<Entity, With<Root>>,
    buttons: Query<(Entity, &MenuButton, Has<bevy::ui::InteractionDisabled>)>,
    mut story_roots: Query<&mut Visibility, (With<UiRoot>, Without<Root>)>,
    mut focus: ResMut<InputFocus>,
    inspector: Res<crate::debug_tools::Inspector>,
    mut descriptions: Query<(&mut Text, &mut Node), With<Description>>,
    mut labels: Query<(&ButtonLabel, &mut TextColor)>,
) {
    for mut visibility in &mut story_roots {
        *visibility = if shell.is_playing() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if matches!(player.presenter.screen(), Screen::Error(_)) {
        shell.load_reason = Some("save/load is unavailable after a Story error".into());
    } else if shell.refresh_load && !shell.is_playing() {
        shell.load_reason = player.load_available().err();
        shell.refresh_load = false;
    }
    let settled = presentation.views.settled(presentation.now)
        && presentation.audio.settled(presentation.now);
    if matches!(player.presenter.screen(), Screen::Error(_)) {
        shell.save_reason = Some("Save is unavailable after a Story error.".into());
    } else if shell.page == Page::Menu
        && (shell.refresh_save || shell.settled != settled || inspector.open)
    {
        shell.save_reason = save_available(&player, &ui, &presentation).err();
        shell.refresh_save = false;
    }
    shell.settled = settled;
    let view = View {
        startup_error: player.startup_error.clone(),
        page: shell.page,
        confirm: shell.confirm,
        save_reason: shell.save_reason.clone(),
        load_reason: shell.load_reason.clone(),
    };
    let same_page = built
        .0
        .as_ref()
        .is_some_and(|old| old.page == view.page && old.confirm == view.confirm);
    if same_page {
        if built.0.as_ref() != Some(&view) && view.confirm.is_none() {
            for (entity, button, _) in &buttons {
                if enabled(&view, button.0) {
                    commands
                        .entity(entity)
                        .remove::<bevy::ui::InteractionDisabled>()
                        .insert(TabIndex(0));
                } else {
                    commands
                        .entity(entity)
                        .insert(bevy::ui::InteractionDisabled)
                        .remove::<TabIndex>();
                }
            }
            for (button, mut color) in &mut labels {
                color.0 = if enabled(&view, button.0) {
                    theme::TEXT
                } else {
                    theme::MUTED
                };
            }
            for (mut text, mut node) in &mut descriptions {
                if !matches!(view.page, Page::Title | Page::Menu) {
                    continue;
                }
                text.0 = details(&view);
                node.display = if text.0.is_empty() {
                    Display::None
                } else {
                    Display::Flex
                };
            }
        }
        // Empty focus cannot enter a modal tab group, including after a background click.
        if !shell.is_playing()
            && !inspector.open
            && focus.get().is_none_or(|e| {
                !buttons
                    .get(e)
                    .is_ok_and(|(_, button, _)| enabled(&view, button.0))
            })
            && let Some((entity, _, _)) = buttons
                .iter()
                .filter(|(_, button, _)| enabled(&view, button.0))
                .min_by_key(|(_, button, _)| match button.0 {
                    Action::Cancel => 0,
                    Action::Return => 1,
                    Action::NewGame => 2,
                    Action::Confirm => 4,
                    _ => 3,
                })
        {
            focus.set(entity, FocusCause::Auto);
        }
        built.0 = Some(view);
        return;
    }
    let old_action = focus
        .get()
        .and_then(|e| buttons.get(e).ok())
        .map(|(_, b, _)| b.0);
    for root in &roots {
        commands.entity(root).despawn();
    }
    let root = commands
        .spawn((
            Root,
            GlobalZIndex(200000),
            TabGroup {
                order: 50,
                modal: !shell.is_playing(),
            },
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(24)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(if shell.is_playing() {
                Color::NONE
            } else {
                theme::PANEL
            }),
        ))
        .id();
    if shell.is_playing() {
        commands.entity(root).insert((
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                right: px(24),
                top: px(16),
                ..default()
            },
        ));
    }
    let mut first = None;
    let mut preferred = None;
    let mut add = |action, title: &str, enabled: bool| {
        let button = commands
            .spawn((
                Button,
                MenuButton(action),
                Hovered::default(),
                theme::button_node(),
                BackgroundColor(theme::BUTTON),
                crate::declarative::ButtonBase(theme::BUTTON),
                BorderColor::all(theme::BORDER),
                Outline::default(),
                observe(move |_: On<Activate>, mut shell: ResMut<Shell>| {
                    shell.push(action);
                }),
            ))
            .id();
        if enabled {
            commands.entity(button).insert(TabIndex(0));
            first.get_or_insert(button);
            if old_action == Some(action) {
                preferred = Some(button);
            }
        } else {
            commands
                .entity(button)
                .insert(bevy::ui::InteractionDisabled);
        }
        let text = commands
            .spawn(crate::label_with_font(
                title,
                22.0,
                if enabled { theme::TEXT } else { theme::MUTED },
                ui.default_font.clone(),
            ))
            .id();
        commands.entity(text).insert(ButtonLabel(action));
        commands.entity(button).add_child(text);
        commands.entity(root).add_child(button);
    };
    let (heading, description) = if let Some(operation) = view.confirm {
        // Cancel is first and focused by default, so an extra Enter cannot discard progress.
        add(Action::Cancel, "Cancel", true);
        add(
            Action::Confirm,
            match operation {
                Action::Save => "Save checkpoint",
                Action::Load => "Load save",
                Action::MainMenu => "Return to title",
                _ => "Quit game",
            },
            true,
        );
        ("Are you sure?".to_string(), match operation {
            Action::Save => "Save this checkpoint? This replaces the single existing save, if any. Progress after this checkpoint is not included.",
            Action::Load => "Load the saved checkpoint? Any unsaved progress in the current game will be lost.",
            Action::MainMenu => "Return to the title? Unsaved progress will be lost.",
            _ => "Quit the game? Unsaved progress will be lost.",
        }.to_string())
    } else {
        match view.page {
            Page::Playing => {
                add(Action::Menu, "Menu (Esc)", true);
                (String::new(), String::new())
            }
            Page::Title => {
                add(Action::NewGame, "New Game", view.startup_error.is_none());
                add(Action::Load, "Load checkpoint", view.load_reason.is_none());
                add(Action::Controls, "Controls", true);
                add(Action::Quit, "Quit", true);
                (player.story.manifest().title.clone(), details(&view))
            }
            Page::Menu => {
                add(Action::Return, "Return to game", true);
                add(Action::Save, "Save checkpoint", view.save_reason.is_none());
                add(Action::Load, "Load checkpoint", view.load_reason.is_none());
                add(Action::Controls, "Controls", true);
                add(Action::MainMenu, "Main Menu", true);
                add(Action::Quit, "Quit", true);
                (
                    "Game Menu".into(),
                    [
                        view.save_reason
                            .as_ref()
                            .map(|r| format!("Save unavailable: {r}")),
                        view.load_reason
                            .as_ref()
                            .map(|r| format!("Load unavailable: {r}")),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("\n"),
                )
            }
            Page::Controls => {
                add(Action::Return, "Back", true);
                ("Controls".into(), "Click / Space / Enter — continue dialogue\n1–9 — choose an option\nTab / Shift+Tab — navigate controls\nEscape / right-click — game menu or back\nF5 — save checkpoint   F9 — load checkpoint\nSaving is available only at safe checkpoints after transitions settle.".into())
            }
        }
    };
    if !heading.is_empty() {
        let heading = commands
            .spawn((
                crate::label_with_font(heading, 38.0, theme::ACCENT, ui.default_font.clone()),
                Node::default(),
            ))
            .id();
        commands.entity(root).insert_children(0, &[heading]);
    }
    if !shell.is_playing() {
        let description_display = if description.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        let description = commands
            .spawn((
                crate::label_with_font(description, 18.0, theme::MUTED, ui.default_font.clone()),
                Description,
                Node {
                    display: description_display,
                    max_width: px(660),
                    ..default()
                },
            ))
            .id();
        commands.entity(root).insert_children(1, &[description]);
    }
    if !shell.is_playing() && !inspector.open {
        focus.clear();
        if let Some(entity) = preferred.or(first) {
            focus.set(entity, FocusCause::Auto);
        }
    }
    if shell.is_playing() && shell.return_to_menu_button {
        shell.return_to_menu_button = false;
        if let Some(entity) = first {
            focus.set(entity, FocusCause::Auto);
        }
    }
    built.0 = Some(view);
}

fn enabled(view: &View, action: Action) -> bool {
    match action {
        Action::NewGame => view.startup_error.is_none(),
        Action::Save => view.save_reason.is_none(),
        Action::Load => view.load_reason.is_none(),
        _ => true,
    }
}
fn details(view: &View) -> String {
    if let Some(error) = &view.startup_error {
        return format!("Story could not start: {error}");
    }
    match view.page {
        Page::Title => view
            .load_reason
            .as_ref()
            .map(|r| format!("Load unavailable: {r}"))
            .unwrap_or_else(|| "Your single checkpoint save is ready to load.".into()),
        Page::Menu => [
            view.save_reason
                .as_ref()
                .map(|r| format!("Save unavailable: {r}")),
            view.load_reason
                .as_ref()
                .map(|r| format!("Load unavailable: {r}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declarative::{Intent, Intents};
    use std::sync::{Arc, RwLock};

    fn app(title: bool) -> (tempfile::TempDir, App) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("story");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("manifest.jsonc"),
            r#"{
            "manifestSchema":1,"id":"test.shell","title":"Shell Test","storyVersion":"0.1.0",
            "saveSchema":1,"entrypoint":"Start","dialogue":["main.yarn"],
            "requires":{"engineApi":">=0.1.0 <0.2.0","capabilities":["dialogue.yarn/1","save/1"]},
            "state":{"n":{"default":0.0},"settings.audio.master":{"default":1.0,"scope":"settings"}}
        }"#,
        )
        .unwrap();
        std::fs::write(root.join("main.yarn"), "title: Start\n---\n<<checkpoint begin>>\n<<set $n = $n + 1>>\nFirst {$n}\n-> Left\n    Left\n-> Right\n    Right\n===\n").unwrap();
        let story = weftora_story::Story::load(&root).unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        let mut app = crate::tests::app_for(story.clone(), compiled.clone());
        if title {
            let state = Arc::new(RwLock::new(story.initial_state().unwrap()));
            let player = crate::prepare_player(story, compiled, state, false, None).unwrap();
            app.insert_non_send(player)
                .insert_resource(Shell::default());
        }
        app.world_mut().non_send_mut::<Player>().save_store = Some(Ok(
            weftora_save::storage::Store::new(temp.path().join("saves")),
        ));
        app.update();
        (temp, app)
    }
    fn act(app: &mut App, action: Action) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut().resource_mut::<Shell>().push(action);
        app.update();
    }
    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
    }
    fn display(app: &App) -> Screen {
        app.world().non_send::<Player>().presenter.screen().clone()
    }

    #[test]
    fn title_does_not_execute_story_and_new_game_retains_settings() {
        let (_temp, mut app) = app(true);
        let player = app.world().non_send::<Player>();
        assert!(player.pending.is_empty());
        assert_eq!(
            player.state.read().unwrap().get("n"),
            Ok(&weftora_api::Value::Float(0.0))
        );
        player
            .state
            .write()
            .unwrap()
            .set("settings.audio.master", weftora_api::Value::Float(0.4))
            .unwrap();
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action: UiAction::Advance,
            });
        app.update();
        assert!(matches!(display(&app), Screen::End));
        act(&mut app, Action::NewGame);
        assert!(matches!(display(&app), Screen::Line { text, .. } if text == "First 1"));
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .state
                .read()
                .unwrap()
                .get("settings.audio.master"),
            Ok(&weftora_api::Value::Float(0.4))
        );
    }

    #[test]
    fn menu_blocks_progress_and_restores_choice_focus_and_checkpoint() {
        let (_temp, mut app) = app(false);
        let bytes = app
            .world()
            .non_send::<Player>()
            .save_game()
            .unwrap()
            .to_bytes()
            .unwrap();
        let focus = app.world_mut().spawn(Button).id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(focus, FocusCause::Navigated);
        let before = display(&app);
        press(&mut app, KeyCode::Escape);
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action: UiAction::Advance,
            });
        press(&mut app, KeyCode::Space);
        assert_eq!(display(&app), before);
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .save_game()
                .unwrap()
                .to_bytes()
                .unwrap(),
            bytes
        );
        act(&mut app, Action::Return);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(focus));
        app.world_mut().resource_mut::<InputFocus>().clear();
        press(&mut app, KeyCode::Space);
        assert!(matches!(display(&app), Screen::Choices(_)));
        let before = display(&app);
        let context = app
            .world()
            .non_send::<Player>()
            .presenter
            .decision_context()
            .unwrap()
            .1
            .to_owned();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        press(&mut app, KeyCode::Digit1);
        assert!(!app.world().resource::<Shell>().is_playing());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        act(&mut app, Action::Return);
        assert_eq!(display(&app), before);
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .presenter
                .decision_context()
                .unwrap()
                .1,
            context
        );
    }

    #[test]
    fn every_save_load_route_confirms_and_cancel_preserves_session_and_slot() {
        let (temp, mut app) = app(false);
        for action in [UiAction::Save, UiAction::Load] {
            if matches!(action, UiAction::Load) {
                app.world().non_send::<Player>().write_save().unwrap();
            }
            let before = display(&app);
            let bytes = std::fs::read(temp.path().join("saves/save.json")).ok();
            app.world_mut()
                .resource_mut::<Intents>()
                .push(Intent::Action {
                    layer: None,
                    action,
                });
            app.update();
            assert!(app.world().resource::<Shell>().confirm.is_some());
            act(&mut app, Action::Cancel);
            assert_eq!(display(&app), before);
            assert_eq!(
                std::fs::read(temp.path().join("saves/save.json")).ok(),
                bytes
            );
            act(&mut app, Action::Return);
        }
        press(&mut app, KeyCode::F5);
        assert_eq!(app.world().resource::<Shell>().confirm, Some(Action::Save));
        act(&mut app, Action::Confirm);
        assert_eq!(app.world().non_send::<Player>().save_status, "Saved.");
        act(&mut app, Action::Return);
        press(&mut app, KeyCode::F9);
        assert_eq!(app.world().resource::<Shell>().confirm, Some(Action::Load));
        // A changed/corrupt slot must fail before replacing anything live.
        std::fs::write(temp.path().join("saves/save.json"), b"broken").unwrap();
        let before = display(&app);
        act(&mut app, Action::Confirm);
        assert_eq!(display(&app), before);
        assert!(
            app.world()
                .non_send::<Player>()
                .save_status
                .starts_with("Couldn't load:")
        );
    }

    #[test]
    fn window_close_and_main_menu_require_confirmation_and_escape_never_quits() {
        let (_temp, mut app) = app(false);
        let window = app.world_mut().spawn_empty().id();
        app.world_mut()
            .write_message(WindowCloseRequested { window });
        app.update();
        assert_eq!(app.world().resource::<Shell>().confirm, Some(Action::Quit));
        press(&mut app, KeyCode::Escape);
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        act(&mut app, Action::MainMenu);
        assert_eq!(
            app.world().resource::<Shell>().confirm,
            Some(Action::MainMenu)
        );
        act(&mut app, Action::Cancel);
        assert!(matches!(display(&app), Screen::Line { .. }));
        act(&mut app, Action::MainMenu);
        act(&mut app, Action::Confirm);
        assert_eq!(app.world().resource::<Shell>().page, Page::Title);
        assert!(matches!(display(&app), Screen::End));
        act(&mut app, Action::Quit);
        act(&mut app, Action::Confirm);
        assert_eq!(app.world().resource::<Messages<AppExit>>().len(), 1);
    }

    #[test]
    fn controls_return_to_their_origin_even_when_game_had_no_keyboard_focus() {
        let (_temp, mut app) = app(false);
        press(&mut app, KeyCode::Escape);
        act(&mut app, Action::Controls);
        act(&mut app, Action::Return);
        assert_eq!(app.world().resource::<Shell>().page, Page::Menu);
        let (_temp, mut app) = self::app(true);
        act(&mut app, Action::Controls);
        press(&mut app, KeyCode::Escape);
        assert_eq!(app.world().resource::<Shell>().page, Page::Title);
    }

    #[test]
    fn rendered_shell_blocks_story_roots_and_disabled_save_has_no_tab_target() {
        let (_temp, mut app) = app(false);
        let root = app.world_mut().spawn((UiRoot, Visibility::Inherited)).id();
        app.add_systems(Update, refresh.after(crate::drive));
        press(&mut app, KeyCode::Space); // Progress disarms checkpoint.
        press(&mut app, KeyCode::Escape);
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Hidden
        );
        let world = app.world_mut();
        let mut query = world.query::<(
            &MenuButton,
            Has<bevy::ui::InteractionDisabled>,
            Has<TabIndex>,
        )>();
        assert!(
            query
                .iter(world)
                .any(|(button, disabled, tab)| button.0 == Action::Save && disabled && !tab)
        );
        act(&mut app, Action::Return);
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn background_focus_loss_recovers_on_every_menu_page_and_tab_stays_inside() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::input_focus::tab_navigation::{NavAction, TabNavigation};
        let (_temp, mut app) = app(true);
        app.add_systems(Update, refresh.after(crate::drive));
        for action in [
            None,
            Some(Action::Controls),
            Some(Action::Return),
            Some(Action::Quit),
        ] {
            if let Some(action) = action {
                act(&mut app, action);
            } else {
                app.update();
            }
            app.world_mut().resource_mut::<InputFocus>().clear();
            app.update();
            let focus = app
                .world()
                .resource::<InputFocus>()
                .get()
                .expect("recover focus after a background click");
            assert!(app.world().get::<MenuButton>(focus).is_some());
            if app.world().resource::<Shell>().confirm.is_some() {
                assert_eq!(
                    app.world().get::<MenuButton>(focus).unwrap().0,
                    Action::Cancel
                );
            }
            // Headless App has no visibility propagation plugin; emulate its visible result.
            for mut visible in app
                .world_mut()
                .query::<&mut InheritedVisibility>()
                .iter_mut(app.world_mut())
            {
                *visible = InheritedVisibility::VISIBLE;
            }
            let next = app
                .world_mut()
                .run_system_once(|navigation: TabNavigation, focus: Res<InputFocus>| {
                    navigation.navigate(&focus, NavAction::Next).unwrap()
                })
                .unwrap();
            assert!(app.world().get::<MenuButton>(next).is_some());
        }
    }

    #[test]
    fn runtime_error_disables_cached_load_and_close_request_exits_inspector_first() {
        let (_temp, mut app) = app(false);
        app.world().non_send::<Player>().write_save().unwrap();
        app.add_systems(Update, refresh.after(crate::drive));
        press(&mut app, KeyCode::Escape);
        assert!(app.world().resource::<Shell>().load_reason.is_none());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .non_send_mut::<Player>()
            .presenter
            .fail("runtime failure".into());
        app.update();
        assert!(
            app.world()
                .resource::<Shell>()
                .load_reason
                .as_ref()
                .unwrap()
                .contains("Story error")
        );
        let world = app.world_mut();
        let mut query = world.query::<(&MenuButton, Has<bevy::ui::InteractionDisabled>)>();
        assert!(
            query
                .iter(world)
                .any(|(button, disabled)| button.0 == Action::Load && disabled)
        );
        app.world_mut()
            .resource_mut::<crate::debug_tools::Inspector>()
            .open = true;
        let window = app.world_mut().spawn_empty().id();
        app.world_mut()
            .write_message(WindowCloseRequested { window });
        app.update();
        assert!(!app.world().resource::<crate::debug_tools::Inspector>().open);
        assert_eq!(app.world().resource::<Shell>().confirm, Some(Action::Quit));
    }

    #[test]
    fn menu_button_focus_is_restored_to_its_rebuilt_entity() {
        let (_temp, mut app) = app(false);
        app.add_systems(Update, refresh.after(crate::drive));
        app.update();
        let menu = {
            let world = app.world_mut();
            world
                .query::<(Entity, &MenuButton)>()
                .iter(world)
                .find(|(_, b)| b.0 == Action::Menu)
                .unwrap()
                .0
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(menu, FocusCause::Navigated);
        act(&mut app, Action::Menu);
        act(&mut app, Action::Return);
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_ne!(focus, menu);
        assert_eq!(
            app.world().get::<MenuButton>(focus).unwrap().0,
            Action::Menu
        );
    }

    #[test]
    fn save_availability_changes_in_place_without_dropping_menu_buttons_or_inspector_focus() {
        let (_temp, mut app) = app(false);
        app.add_systems(Update, refresh.after(crate::drive));
        press(&mut app, KeyCode::Escape);
        let save = {
            let world = app.world_mut();
            world
                .query::<(Entity, &MenuButton)>()
                .iter(world)
                .find(|(_, b)| b.0 == Action::Save)
                .unwrap()
                .0
        };
        let inspector_focus = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<crate::debug_tools::Inspector>()
            .open = true;
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(inspector_focus, FocusCause::Navigated);
        app.world()
            .non_send::<Player>()
            .state
            .write()
            .unwrap()
            .set("n", weftora_api::Value::Float(9.0))
            .unwrap();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
        assert!(app.world().get::<MenuButton>(save).is_some());
        assert!(
            app.world()
                .get::<bevy::ui::InteractionDisabled>(save)
                .is_some()
        );
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(inspector_focus)
        );
    }
    #[test]
    fn startup_asset_failure_cannot_be_bypassed_with_new_game() {
        let (_temp, mut app) = app(true);
        {
            let mut player = app.world_mut().non_send_mut::<Player>();
            player.startup_error = Some("invalid UI font".into());
            player.presenter.fail("invalid UI font".into());
        }
        app.add_systems(Update, refresh.after(crate::drive));
        act(&mut app, Action::NewGame);
        assert!(matches!(display(&app), Screen::Error(_)));
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .state
                .read()
                .unwrap()
                .get("n"),
            Ok(&weftora_api::Value::Float(0.0))
        );
        let world = app.world_mut();
        let mut buttons = world.query::<(&MenuButton, Has<bevy::ui::InteractionDisabled>)>();
        assert!(
            buttons
                .iter(world)
                .any(|(button, disabled)| button.0 == Action::NewGame && disabled)
        );
        act(&mut app, Action::Quit);
        act(&mut app, Action::Confirm);
        assert_eq!(app.world().resource::<Messages<AppExit>>().len(), 1);
    }
}
