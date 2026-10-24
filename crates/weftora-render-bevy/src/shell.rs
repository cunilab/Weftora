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
    Preferences,
    Volume { index: u8, increase: bool },
    Language(u8),
    RetryPreferences,
    ResetPreferences,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Title,
    Playing,
    Menu,
    Controls,
    Preferences,
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
    restore_action: Option<Action>,
    quit_preferences_warning: bool,
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
            restore_action: None,
            quit_preferences_warning: false,
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
    time: Res<'w, Time>,
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
            self.shell.quit_preferences_warning = false;
            focus.clear();
            consumed = true;
        } else if !captured
            && (keys.just_pressed(KeyCode::Escape) || self.mouse.just_pressed(MouseButton::Right))
        {
            self.shell.actions.clear();
            if let Some(operation) = self.shell.confirm.take() {
                if operation == Action::ResetPreferences {
                    self.shell.restore_action = Some(Action::ResetPreferences);
                }
                focus.clear();
            } else if matches!(self.shell.page, Page::Controls | Page::Preferences) {
                if self.shell.page == Page::Preferences {
                    player.persist_preferences(self.time.elapsed(), true);
                }
                self.shell.restore_action = Some(if self.shell.page == Page::Preferences {
                    Action::Preferences
                } else {
                    Action::Controls
                });
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
                    if self.shell.confirm == Some(Action::ResetPreferences) {
                        self.shell.restore_action = Some(Action::ResetPreferences);
                    }
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
                            player.persist_preferences(self.time.elapsed(), true);
                            if player.preferences_unsaved() && !self.shell.quit_preferences_warning
                            {
                                self.shell.confirm = Some(Action::Quit);
                                self.shell.quit_preferences_warning = true;
                            } else {
                                exit.write(AppExit::Success);
                            }
                        }
                        Action::ResetPreferences => {
                            if let Err(error) = player.reset_preferences() {
                                warn!("Could not reset preferences: {error}");
                                player.preferences.status = "Preferences could not be reset. The file was preserved; storage needs attention before retrying.".into();
                            }
                            self.shell.invalidate_load();
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
                    Action::Controls | Action::Preferences
                        if matches!(self.shell.page, Page::Title | Page::Menu) =>
                    {
                        self.shell.controls_return = self.shell.page;
                        self.shell.page = if action == Action::Preferences {
                            Page::Preferences
                        } else {
                            Page::Controls
                        };
                        focus.clear();
                    }
                    Action::Return
                        if matches!(self.shell.page, Page::Controls | Page::Preferences) =>
                    {
                        if self.shell.page == Page::Preferences {
                            player.persist_preferences(self.time.elapsed(), true);
                        }
                        self.shell.restore_action = Some(if self.shell.page == Page::Preferences {
                            Action::Preferences
                        } else {
                            Action::Controls
                        });
                        self.shell.page = self.shell.controls_return;
                        focus.clear();
                    }
                    Action::Volume { index, increase } if self.shell.page == Page::Preferences => {
                        if let Err(error) = player.change_volume(usize::from(index), increase) {
                            warn!("{error}");
                        }
                        self.shell.refresh_save = true;
                    }
                    Action::Language(index) if self.shell.page == Page::Preferences => {
                        if let Err(error) = player.change_language(usize::from(index)) {
                            warn!("{error}");
                        }
                        self.shell.refresh_save = true;
                    }
                    Action::RetryPreferences if self.shell.page == Page::Preferences => {
                        player.retry_preferences(self.time.elapsed())
                    }
                    Action::ResetPreferences
                        if self.shell.page == Page::Preferences && player.preferences.enabled() =>
                    {
                        self.shell.confirm = Some(Action::ResetPreferences);
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
                        self.shell.quit_preferences_warning = false;
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
#[derive(Component)]
pub(super) struct VolumeLabel(usize);
#[derive(Resource, Default)]
pub(super) struct Built(Option<View>);
#[derive(Clone, PartialEq)]
struct View {
    volumes: [Option<u8>; 5],
    languages: Vec<String>,
    language: Option<String>,
    preferences_status: String,
    preferences_enabled: bool,
    preferences_blocked: bool,
    preferences_failed: bool,
    quit_preferences_warning: bool,
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
    mut labels: Query<
        (&ButtonLabel, &mut TextColor, &mut Text),
        (Without<Description>, Without<VolumeLabel>),
    >,
    mut volume_labels: Query<
        (&VolumeLabel, &mut Text),
        (Without<Description>, Without<ButtonLabel>),
    >,
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
    let (languages, language) = if let Some(catalogs) = player.story.localization()
        && catalogs.definition.state_key.is_some()
        && catalogs.languages.len() > 1
    {
        let language = player
            .state
            .read()
            .ok()
            .and_then(|s| catalogs.language(&s).ok());
        (catalogs.languages.keys().cloned().collect(), language)
    } else {
        (Vec::new(), None)
    };
    let view = View {
        volumes: std::array::from_fn(|index| player.volume_percent(index)),
        languages,
        language,
        preferences_status: player.preferences.status.clone(),
        preferences_enabled: player.preferences.enabled(),
        preferences_blocked: player.preferences.blocked,
        preferences_failed: player.preferences.failed,
        quit_preferences_warning: shell.quit_preferences_warning,
        startup_error: player.startup_error.clone(),
        page: shell.page,
        confirm: shell.confirm,
        save_reason: shell.save_reason.clone(),
        load_reason: shell.load_reason.clone(),
    };
    let same_page = built.0.as_ref().is_some_and(|old| {
        old.page == view.page
            && old.confirm == view.confirm
            && old.quit_preferences_warning == view.quit_preferences_warning
            && (view.page != Page::Preferences || old.preferences_failed == view.preferences_failed)
    });
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
            for (button, mut color, mut text) in &mut labels {
                if let Action::Language(index) = button.0 {
                    text.0 = language_label(&view, usize::from(index));
                }
                color.0 = if enabled(&view, button.0) {
                    theme::TEXT
                } else {
                    theme::MUTED
                };
            }
            for (binding, mut text) in &mut volume_labels {
                if let Some(percent) = view.volumes[binding.0] {
                    text.0 = format!(
                        "{}  {percent}%",
                        crate::preferences::VOLUME_LABELS[binding.0]
                    );
                }
            }
            for (mut text, mut node) in &mut descriptions {
                if !matches!(view.page, Page::Title | Page::Menu | Page::Preferences) {
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
        let paired_volume =
            focus
                .get()
                .and_then(|e| buttons.get(e).ok())
                .and_then(|(_, button, _)| {
                    if let Action::Volume { index, increase } = button.0 {
                        Some(Action::Volume {
                            index,
                            increase: !increase,
                        })
                    } else {
                        None
                    }
                });
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
                .min_by_key(|(_, button, _)| {
                    if paired_volume == Some(button.0) {
                        0
                    } else {
                        match button.0 {
                            Action::Cancel => 1,
                            Action::Return => 2,
                            Action::NewGame => 3,
                            Action::Confirm => 5,
                            _ => 4,
                        }
                    }
                })
        {
            focus.set(entity, FocusCause::Auto);
        }
        built.0 = Some(view);
        return;
    }
    let old_action = shell.restore_action.take().or_else(|| {
        focus
            .get()
            .and_then(|e| buttons.get(e).ok())
            .map(|(_, b, _)| b.0)
    });
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
    let mut controls = Vec::new();
    let mut add = |action, title: &str, enabled: bool| {
        controls.push((action, title.to_owned(), enabled));
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
                Action::ResetPreferences => "Reset saved preferences",
                Action::Quit if view.quit_preferences_warning => "Quit without saving preferences",
                _ => "Quit game",
            },
            true,
        );
        ("Are you sure?".to_string(), match operation {
            Action::Save => "Save this checkpoint? This replaces the single existing save, if any. Progress after this checkpoint is not included.",
            Action::Load => "Load the saved checkpoint? Any unsaved progress in the current game will be lost.",
            Action::MainMenu => "Return to the title? Unsaved progress will be lost.",
            Action::ResetPreferences => "Reset preferences to this Story's defaults? Game progress is unchanged. Any previous preferences file will be kept separately.",
            Action::Quit if view.quit_preferences_warning => "Preferences could not be saved. Quit anyway? Those changes and any unsaved game progress will be lost.",
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
                add(Action::Preferences, "Preferences", true);
                add(Action::Controls, "Controls", true);
                add(Action::Quit, "Quit", true);
                (player.story.manifest().title.clone(), details(&view))
            }
            Page::Menu => {
                add(Action::Return, "Return to game", true);
                add(Action::Save, "Save checkpoint", view.save_reason.is_none());
                add(Action::Load, "Load checkpoint", view.load_reason.is_none());
                add(Action::Preferences, "Preferences", true);
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
            Page::Preferences => {
                add(Action::Return, "Back", true);
                if view.preferences_failed {
                    add(
                        Action::RetryPreferences,
                        "Retry saving",
                        view.preferences_enabled && !view.preferences_blocked,
                    );
                }
                if view.preferences_enabled {
                    add(Action::ResetPreferences, "Reset preferences", true);
                }
                ("Preferences".into(), details(&view))
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
        commands.entity(root).add_child(heading);
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
        commands.entity(root).add_child(description);
    }
    let mut first = None;
    let mut preferred = None;
    let mut spawned = Vec::new();
    if view.page == Page::Preferences && view.confirm.is_none() {
        for (index, percent) in view.volumes.iter().enumerate() {
            let Some(percent) = percent else {
                continue;
            };
            let row = commands
                .spawn(Node {
                    width: Val::Percent(100.0),
                    max_width: px(420),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: px(12),
                    ..default()
                })
                .id();
            commands.entity(root).add_child(row);
            let minus = Action::Volume {
                index: index as u8,
                increase: false,
            };
            let plus = Action::Volume {
                index: index as u8,
                increase: true,
            };
            spawned.push((
                spawn_button(&mut commands, &ui, row, minus, "−", enabled(&view, minus)),
                minus,
                enabled(&view, minus),
            ));
            let label = commands
                .spawn((
                    VolumeLabel(index),
                    crate::label_with_font(
                        format!("{}  {percent}%", crate::preferences::VOLUME_LABELS[index]),
                        20.0,
                        theme::TEXT,
                        ui.default_font.clone(),
                    ),
                ))
                .id();
            commands.entity(row).add_child(label);
            spawned.push((
                spawn_button(&mut commands, &ui, row, plus, "+", enabled(&view, plus)),
                plus,
                enabled(&view, plus),
            ));
        }
        if !view.languages.is_empty() {
            let row = commands
                .spawn(Node {
                    max_width: px(700),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    column_gap: px(8),
                    row_gap: px(8),
                    ..default()
                })
                .id();
            commands.entity(root).add_child(row);
            for index in 0..view.languages.len() {
                let action = Action::Language(index as u8);
                spawned.push((
                    spawn_button(
                        &mut commands,
                        &ui,
                        row,
                        action,
                        &language_label(&view, index),
                        enabled(&view, action),
                    ),
                    action,
                    enabled(&view, action),
                ));
            }
        }
    }
    let footer = if view.page == Page::Preferences && view.confirm.is_none() {
        let row = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: px(8),
                row_gap: px(8),
                margin: UiRect::top(px(8)),
                ..default()
            })
            .id();
        commands.entity(root).add_child(row);
        row
    } else {
        root
    };
    for (action, title, enabled) in controls {
        spawned.push((
            spawn_button(&mut commands, &ui, footer, action, &title, enabled),
            action,
            enabled,
        ));
    }
    for (entity, action, enabled) in spawned {
        if enabled {
            first.get_or_insert(entity);
            if old_action == Some(action) {
                preferred = Some(entity);
            }
        }
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

fn spawn_button(
    commands: &mut Commands,
    ui: &Ui,
    parent: Entity,
    action: Action,
    title: &str,
    enabled: bool,
) -> Entity {
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
    } else {
        commands
            .entity(button)
            .insert(bevy::ui::InteractionDisabled);
    }
    if let Action::Volume { index, increase } = action {
        let label = format!(
            "{} {} volume",
            if increase { "Increase" } else { "Decrease" },
            crate::preferences::VOLUME_LABELS[usize::from(index)]
        );
        commands.queue(move |world: &mut World| {
            if let Some(mut node) = world.get_mut::<bevy::a11y::AccessibilityNode>(button) {
                node.set_label(label);
            }
        });
    }
    let text = commands
        .spawn((
            ButtonLabel(action),
            crate::label_with_font(
                title,
                22.0,
                if enabled { theme::TEXT } else { theme::MUTED },
                ui.default_font.clone(),
            ),
        ))
        .id();
    commands.entity(button).add_child(text);
    commands.entity(parent).add_child(button);
    button
}
fn language_label(view: &View, index: usize) -> String {
    let language = &view.languages[index];
    let label = match language.as_str() {
        "en" => "English",
        "id" => "Bahasa Indonesia",
        "ja" => "日本語",
        _ => language,
    };
    format!(
        "{label}{}",
        if view.language.as_ref() == Some(language) {
            " (selected)"
        } else {
            ""
        }
    )
}

fn enabled(view: &View, action: Action) -> bool {
    match action {
        Action::NewGame => view.startup_error.is_none(),
        Action::Volume { index, increase } => view
            .volumes
            .get(usize::from(index))
            .and_then(|p| *p)
            .is_some_and(|p| if increase { p < 100 } else { p > 0 }),
        Action::Language(index) => view.languages.get(usize::from(index)).is_some(),
        Action::RetryPreferences => view.preferences_enabled && !view.preferences_blocked,
        Action::ResetPreferences => view.preferences_enabled,
        Action::Save => view.save_reason.is_none(),
        Action::Load => view.load_reason.is_none(),
        _ => true,
    }
}
fn details(view: &View) -> String {
    if let Some(error) = &view.startup_error {
        return format!("Story could not start: {error}");
    }
    let mut text = match view.page {
        Page::Preferences => {
            let controls = if view.volumes.iter().all(Option::is_none) && view.languages.is_empty()
            {
                "This Story does not expose audio or language controls. "
            } else {
                "Volume changes in 10% steps. "
            };
            format!("{controls}{}", view.preferences_status)
        }
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
    };
    if matches!(view.page, Page::Title | Page::Menu)
        && (view.preferences_blocked || view.preferences_failed)
    {
        text.push_str("\nSaved preferences need attention. Open Preferences to review them.");
    }
    text
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
    #[test]
    fn preference_buttons_update_in_place_and_back_restores_origin_focus() {
        let (temp, mut app) = crate::preferences::tests::fixture();
        app.add_systems(Update, refresh.after(crate::drive));
        press(&mut app, KeyCode::Escape);
        act(&mut app, Action::Preferences);
        assert_eq!(app.world().resource::<Shell>().page, Page::Preferences);
        let minus = Action::Volume {
            index: 0,
            increase: false,
        };
        let button = {
            let world = app.world_mut();
            world
                .query::<(Entity, &MenuButton)>()
                .iter(world)
                .find(|(_, b)| b.0 == minus)
                .unwrap()
                .0
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(button, FocusCause::Navigated);
        act(&mut app, minus);
        assert_eq!(app.world().non_send::<Player>().volume_percent(0), Some(40));
        assert!(app.world().get::<MenuButton>(button).is_some());
        assert_eq!(
            app.world()
                .get::<bevy::a11y::AccessibilityNode>(button)
                .unwrap()
                .label(),
            Some("Decrease Master volume")
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));
        let world = app.world_mut();
        assert!(
            world
                .query::<(&VolumeLabel, &Text)>()
                .iter(world)
                .any(|(label, text)| label.0 == 0 && text.0.contains("40%"))
        );
        act(&mut app, Action::Language(2));
        assert!(matches!(display(&app), Screen::Line { text, .. } if text == "こんにちは 1"));
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .state
                .read()
                .unwrap()
                .get("n"),
            Ok(&weftora_api::Value::Float(1.0))
        );
        act(&mut app, Action::Return);
        assert_eq!(app.world().resource::<Shell>().page, Page::Menu);
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<MenuButton>(focus).unwrap().0,
            Action::Preferences
        );
        assert_eq!(
            weftora_save::preferences::load(&temp.path().join("data"), "test.preferences").unwrap()
                ["locale"],
            weftora_api::Value::String("ja".into())
        );
        // Preferences opened from Title must return to Title without starting a Story.
        app.world_mut().resource_mut::<Shell>().page = Page::Title;
        app.update();
        act(&mut app, Action::Preferences);
        press(&mut app, KeyCode::Escape);
        assert_eq!(app.world().resource::<Shell>().page, Page::Title);
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<MenuButton>(focus).unwrap().0,
            Action::Preferences
        );
    }

    #[test]
    fn preference_bounds_and_confirmed_reset_leave_progress_unchanged() {
        let (temp, mut app) = crate::preferences::tests::fixture();
        app.add_systems(Update, refresh.after(crate::drive));
        press(&mut app, KeyCode::Escape);
        act(&mut app, Action::Preferences);
        let plus = Action::Volume {
            index: 0,
            increase: true,
        };
        let plus_button = {
            let world = app.world_mut();
            world
                .query::<(Entity, &MenuButton)>()
                .iter(world)
                .find(|(_, b)| b.0 == plus)
                .unwrap()
                .0
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(plus_button, FocusCause::Navigated);
        for _ in 0..5 {
            act(&mut app, plus);
        }
        assert_eq!(
            app.world().non_send::<Player>().volume_percent(0),
            Some(100)
        );
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<MenuButton>(focus).unwrap().0,
            Action::Volume {
                index: 0,
                increase: false
            }
        );
        let world = app.world_mut();
        assert!(
            world
                .query::<(
                    &MenuButton,
                    Has<bevy::ui::InteractionDisabled>,
                    Has<TabIndex>
                )>()
                .iter(world)
                .any(|(button, disabled, tab)| button.0 == plus && disabled && !tab)
        );
        act(&mut app, Action::ResetPreferences);
        assert_eq!(
            app.world().resource::<Shell>().confirm,
            Some(Action::ResetPreferences)
        );
        act(&mut app, Action::Cancel);
        assert_eq!(
            app.world().non_send::<Player>().volume_percent(0),
            Some(100)
        );
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<MenuButton>(focus).unwrap().0,
            Action::ResetPreferences
        );
        act(&mut app, Action::ResetPreferences);
        act(&mut app, Action::Confirm);
        assert_eq!(app.world().non_send::<Player>().volume_percent(0), Some(50));
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .state
                .read()
                .unwrap()
                .get("n"),
            Ok(&weftora_api::Value::Float(1.0))
        );
        assert!(
            weftora_save::preferences::load(&temp.path().join("data"), "test.preferences")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn quit_flushes_the_latest_preference_change_during_write_cooldown() {
        let (temp, mut app) = crate::preferences::tests::fixture();
        press(&mut app, KeyCode::Escape);
        act(&mut app, Action::Preferences);
        let plus = Action::Volume {
            index: 0,
            increase: true,
        };
        act(&mut app, plus); // First automatic write.
        act(&mut app, plus); // Within cooldown: remains dirty until Back/Quit.
        let window = app.world_mut().spawn_empty().id();
        app.world_mut()
            .write_message(WindowCloseRequested { window });
        app.update();
        act(&mut app, Action::Confirm);
        assert_eq!(
            weftora_save::preferences::load(&temp.path().join("data"), "test.preferences").unwrap()
                ["settings.audio.master"],
            weftora_api::Value::Float(0.7)
        );
    }
    #[test]
    fn retry_appears_after_failure_on_the_open_preferences_page() {
        let (temp, mut app) = crate::preferences::tests::fixture();
        app.add_systems(Update, refresh.after(crate::drive));
        press(&mut app, KeyCode::Escape);
        act(&mut app, Action::Preferences);
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
        act(
            &mut app,
            Action::Volume {
                index: 0,
                increase: true,
            },
        );
        app.world_mut()
            .non_send_mut::<Player>()
            .persist_preferences(std::time::Duration::from_secs(1), true);
        app.update();
        assert!(app.world().non_send::<Player>().preferences.failed);
        let world = app.world_mut();
        assert!(
            world
                .query::<&MenuButton>()
                .iter(world)
                .any(|button| button.0 == Action::RetryPreferences)
        );
        {
            let mut player = app.world_mut().non_send_mut::<Player>();
            player.save_status = player.preferences.status.clone();
        }
        lock.unlock().unwrap();
        act(&mut app, Action::RetryPreferences);
        assert!(app.world().non_send::<Player>().save_status.is_empty());
        assert!(!app.world().non_send::<Player>().preferences.failed);
        assert_eq!(
            weftora_save::preferences::load(&data, "test.preferences").unwrap()["settings.audio.master"],
            weftora_api::Value::Float(0.6)
        );
    }
    #[test]
    fn quit_write_failure_requires_explicit_discard_confirmation() {
        let (temp, mut app) = crate::preferences::tests::fixture();
        app.add_systems(Update, refresh.after(crate::drive));
        press(&mut app, KeyCode::Escape);
        act(&mut app, Action::Preferences);
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
        act(
            &mut app,
            Action::Volume {
                index: 0,
                increase: true,
            },
        );
        let window = app.world_mut().spawn_empty().id();
        app.world_mut()
            .write_message(WindowCloseRequested { window });
        app.update();
        act(&mut app, Action::Confirm);
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        assert!(app.world().resource::<Shell>().quit_preferences_warning);
        let world = app.world_mut();
        assert!(
            world
                .query::<&Text>()
                .iter(world)
                .any(|text| text.0 == "Quit without saving preferences")
        );
        // A successful forced retry should still save before the second confirmation exits.
        lock.unlock().unwrap();
        act(&mut app, Action::Confirm);
        assert_eq!(app.world().resource::<Messages<AppExit>>().len(), 1);
        assert!(!app.world().non_send::<Player>().preferences.failed);
        assert_eq!(
            weftora_save::preferences::load(&data, "test.preferences").unwrap()["settings.audio.master"],
            weftora_api::Value::Float(0.6)
        );
    }
}
