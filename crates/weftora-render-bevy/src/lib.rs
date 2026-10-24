//! Bevy presentation adapter for the Weftora player.
//!
//! Story assets are read from the Story folder with `std::fs` through [`Story::asset_path_of`] and turned into
//! Bevy assets here, so Bevy never opens a path outside the Story. The dialogue-to-screen logic lives in
//! [`presenter`] and has no Bevy types; the systems below only translate input and draw its output.

mod debug_tools;
mod declarative;
#[cfg(test)]
mod localization_tests;
mod media;
mod preferences;
pub mod presenter;
mod saves;
mod shell;
mod theme;

use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::audio::AddAudioSource;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::input_focus::{InputFocus, InputFocusVisible, tab_navigation::TabNavigationPlugin};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};
use bevy::window::WindowResolution;
use declarative::{ButtonBase, FocusKey, Intent, Intents, Ui};
use weftora_core::SharedState;
use weftora_story::ui::UiAction;
use weftora_story::{AssetKind, Story};
use weftora_yarn::{CompiledDialogue, DialogueRunner};

use media::Media;
use presenter::{Action, END_TEXT, Presenter, Screen};

const PANEL_BG: Color = theme::PANEL;
const BUTTON_BG: Color = theme::BUTTON;
const SPEAKER_COLOR: Color = theme::ACCENT;
const ERROR_COLOR: Color = Color::srgb(1.0, 0.45, 0.45);

/// Opens the game window and runs `story` until the window closes.
pub fn run(story: Story, dialogue: CompiledDialogue, state: SharedState) -> Result<(), String> {
    run_player(story, dialogue, state, false, None)
}

/// Opens the player with explicitly enabled developer state tools.
pub fn run_with_debug_tools(
    story: Story,
    dialogue: CompiledDialogue,
    state: SharedState,
) -> Result<(), String> {
    run_player(story, dialogue, state, true, None)
}

/// Starts a fresh developer session at an exact compiled node; save/load are disabled.
pub fn run_at_node_with_debug_tools(
    story: Story,
    dialogue: CompiledDialogue,
    node: &str,
) -> Result<(), String> {
    let state = Arc::new(std::sync::RwLock::new(
        story.initial_state().map_err(|e| e.to_string())?,
    ));
    run_player(story, dialogue, state, true, Some(node))
}

fn run_player(
    story: Story,
    dialogue: CompiledDialogue,
    state: SharedState,
    debug_tools: bool,
    node: Option<&str>,
) -> Result<(), String> {
    let title = if debug_tools {
        format!("{} — Developer tools (F12)", story.manifest().title)
    } else {
        story.manifest().title.clone()
    };
    let title = if let Some(node) = node {
        format!("{title} — node: {node} — save/load disabled")
    } else {
        title
    };
    let mut player = prepare_player(story, dialogue, state, debug_tools, node)?;
    if node.is_none() {
        preferences::initialize(&mut player);
    }

    let failure = RuntimeFailure::default();
    let outcome = failure.0.clone();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            close_when_requested: false,
            primary_window: Some(Window {
                title,
                resolution: WindowResolution::new(1280, 720),
                resize_constraints: bevy::window::WindowResizeConstraints {
                    min_width: 800.0,
                    min_height: 600.0,
                    ..default()
                },
                ..default()
            }),
            ..default()
        }))
        .add_plugins(TabNavigationPlugin)
        .add_audio_source::<media::LoopAudio>()
        .insert_non_send(player)
        .insert_resource(ClearColor(theme::PANEL))
        .insert_resource(failure)
        .insert_resource(debug_tools::Inspector::new(debug_tools))
        .insert_resource(shell::Shell::new(node.is_some()))
        .init_resource::<SaveFeedback>()
        .init_resource::<Intents>()
        .init_resource::<Ui>()
        .init_resource::<media::Presentation>()
        .add_systems(Startup, (setup, declarative::setup, debug_tools::setup))
        .add_systems(
            Update,
            (
                debug_tools::scroll,
                debug_tools::input,
                drive,
                media::sync_views,
                declarative::sync_layers,
                declarative::sync_choices,
                declarative::sync_advance_buttons,
                declarative::restore_focus.run_if(shell::playing),
                declarative::update_bindings,
                media::sync_audio,
                refresh_panel,
                refresh_save_status,
                shell::refresh,
                hover_buttons,
                debug_tools::refresh,
                remember_failure,
            )
                .chain(),
        )
        .run();
    match outcome
        .lock()
        .map_err(|_| "runtime result lock poisoned")?
        .take()
    {
        Some(message) => Err(message),
        None => Ok(()),
    }
}

#[derive(Resource, Default)]
struct RuntimeFailure(Arc<Mutex<Option<String>>>);

fn prepare_player(
    story: Story,
    dialogue: CompiledDialogue,
    state: SharedState,
    debug_tools: bool,
    node: Option<&str>,
) -> Result<Player, String> {
    if let Some(node) = node
        && !dialogue.node_names().iter().any(|name| name == node)
    {
        return Err(weftora_yarn::DialogueError::MissingNode(node.into()).to_string());
    }
    let mut runner = DialogueRunner::for_story(&dialogue, Arc::clone(&state), &story);
    if debug_tools {
        runner.enable_rule_trace();
    }
    if let Ok(mut state) = state.write() {
        state.drain_changes();
    }
    let (presenter, actions) = if let Some(node) = node {
        Presenter::new(runner, node)
    } else {
        (Presenter::dormant(runner), Vec::new())
    };
    Ok(Player {
        presenter,
        state,
        save_store: if node.is_some() {
            Some(Err(
                "save/load is disabled for --start-node previews; relaunch without --start-node"
                    .into(),
            ))
        } else {
            saves::store(&story)
        },
        compiled: dialogue,
        save_status: String::new(),
        startup_error: None,
        preferences: preferences::Preferences::temporary(),
        story,
        pending: actions,
        dirty: true,
    })
}

fn remember_failure(player: NonSend<Player>, failure: Res<RuntimeFailure>) {
    if let Screen::Error(message) = player.presenter.screen() {
        let mut first = failure.0.lock().unwrap_or_else(|e| e.into_inner());
        first.get_or_insert_with(|| message.clone());
    }
}

/// Presenter plus the Story it reads assets from. Not `Send`: the Yarn runner may not be.
struct Player {
    presenter: Presenter<DialogueRunner>,
    story: Story,
    state: SharedState,
    pending: Vec<Action>,
    dirty: bool,
    compiled: CompiledDialogue,
    save_store: Option<Result<weftora_save::storage::Store, String>>,
    save_status: String,
    startup_error: Option<String>,
    preferences: preferences::Preferences,
}

#[derive(Component)]
struct Panel(Option<u64>);

#[derive(Component)]
enum PanelBinding {
    Speaker,
    Text,
    Choice(usize),
}

#[derive(Component)]
struct SaveStatus;

#[derive(Resource, Default)]
struct SaveFeedback {
    until: Option<std::time::Duration>,
    failed: bool,
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn((
            SaveStatus,
            label("", 16.0, theme::MUTED),
            Node {
                position_type: PositionType::Absolute,
                top: px(16),
                left: px(24),
                max_width: percent(85),
                padding: UiRect::axes(px(16), px(10)),
                border_radius: BorderRadius::all(px(8)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(theme::PANEL),
            GlobalZIndex(500000),
        ))
        .insert(Pickable {
            is_hoverable: false,
            should_block_lower: true,
        });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        observe(|event: On<PointerClick>, mut intents: ResMut<Intents>| {
            if event.button != PointerButton::Primary {
                return;
            }
            intents.push(Intent::Action {
                layer: None,
                action: UiAction::Advance,
            });
        }),
    ));
    commands.spawn((
        Panel(None),
        GlobalZIndex(100000),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: percent(5),
            right: percent(5),
            bottom: px(28),
            min_height: px(180),
            padding: UiRect::all(px(28)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(12)),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            ..default()
        },
        BackgroundColor(PANEL_BG),
        BorderColor::all(theme::BORDER),
        Outline::default(),
    ));
}

/// Input -> presenter -> side effects.
#[allow(clippy::too_many_arguments)]
fn drive(
    mut player: NonSendMut<Player>,
    mut inspector: ResMut<debug_tools::Inspector>,
    keys: Res<ButtonInput<KeyCode>>,
    mut intents: ResMut<Intents>,
    mut ui: ResMut<Ui>,
    mut focus: ResMut<InputFocus>,
    buttons: Query<(), With<Button>>,
    focus_keys: Query<&FocusKey>,
    mut exit: MessageWriter<AppExit>,
    mut media: Media,
    time: Res<Time>,
    mut shell_input: shell::Input,
    mut feedback: ResMut<SaveFeedback>,
) {
    // Menu navigation blocks Story input; fades and music continue without replaying commands.
    media.begin_frame(
        time.delta(),
        matches!(player.presenter.screen(), Screen::Error(_)),
    );
    let previous_status = player.save_status.clone();
    let route = shell_input.route(
        &mut player,
        &mut ui,
        &mut media,
        &keys,
        &mut inspector,
        &mut focus,
        &mut exit,
    );
    if player.save_status != previous_status {
        feedback.until = None;
        feedback.failed = !player.save_status.is_empty();
    }
    if route.blocked {
        intents.drain().for_each(drop);
    }
    let before = player.presenter.screen().clone();
    let before_revision = player.presenter.revision();
    let mut actions = std::mem::take(&mut player.pending);
    let mut requested = route.request;

    let focused_button = focus.get().is_some_and(|e| buttons.contains(e));
    if !route.blocked && !inspector.captured && keys.just_pressed(KeyCode::F9) {
        requested = Some(UiAction::Load);
    } else if !route.blocked && !inspector.captured && keys.just_pressed(KeyCode::F5) {
        requested = Some(UiAction::Save);
    }
    if !route.blocked && !inspector.captured && !ui.modal() && requested.is_none() {
        if let Some(position) = DIGIT_KEYS.iter().position(|k| keys.just_pressed(*k)) {
            intents.push(Intent::Select {
                layer: None,
                position,
            });
        } else if !focused_button
            && [KeyCode::Space, KeyCode::Enter, KeyCode::NumpadEnter]
                .iter()
                .any(|k| keys.just_pressed(*k))
        {
            intents.push(Intent::Action {
                layer: None,
                action: UiAction::Advance,
            });
        }
    }
    let mut progressed = false;
    for intent in intents.drain() {
        if inspector.captured || route.blocked {
            continue;
        }
        if matches!(player.presenter.screen(), Screen::Error(_)) {
            continue;
        }
        let (origin, action) = match intent {
            Intent::Select { layer, position } => {
                if ui.accepts(&layer) && !progressed {
                    progressed = true;
                    actions.extend(player.presenter.select(position));
                }
                continue;
            }
            Intent::Action { layer, action } => (layer, action),
        };
        if !ui.accepts(&origin) {
            continue;
        }
        let result = match action {
            UiAction::Save | UiAction::Load => {
                requested.get_or_insert(action);
                Ok(())
            }
            UiAction::Advance if !progressed => {
                progressed = true;
                actions.extend(player.presenter.advance());
                Ok(())
            }
            UiAction::SetState { .. } | UiAction::CallCommand { .. } => player
                .presenter
                .source_mut()
                .ui_action(&action)
                .map_err(|e| e.to_string()),
            UiAction::Open { .. } | UiAction::Close => ui.navigate(
                &action,
                &player.story,
                focus
                    .get()
                    .and_then(|e| focus_keys.get(e).ok())
                    .map(|k| k.0.clone()),
            ),
            _ => Ok(()),
        };
        if let Err(e) = result {
            player.presenter.fail(e);
            break;
        }
    }

    player.presenter.refresh_localization();
    let commands_ran = !actions.is_empty();
    for action in actions {
        if matches!(player.presenter.screen(), Screen::Error(_)) {
            break;
        }
        if let Err(message) = media.apply(&action, &player.story) {
            error!("{message}");
            player.presenter.fail(message);
            break;
        }
    }

    if let Some(request) = requested.as_ref()
        && !route.confirmed
        && !progressed
        && !commands_ran
    {
        let available = if matches!(request, UiAction::Save) {
            shell::save_available(&player, &ui, &media.presentation)
        } else {
            player.load_available()
        };
        if let Err(reason) = available {
            player.save_status = format!(
                "Couldn't {}: {reason}",
                if matches!(request, UiAction::Save) {
                    "save"
                } else {
                    "load"
                }
            );
            feedback.failed = true;
            feedback.until = None;
            requested = None;
        }
    }
    if let Some(request) = requested.as_ref()
        && !route.confirmed
        && !progressed
        && !commands_ran
        && !matches!(player.presenter.screen(), Screen::Error(_))
    {
        shell_input
            .shell
            .confirm_request(request.clone(), &mut focus);
        requested = None;
    }
    if let Some(request) = requested {
        let result: Result<String, String> = (|| {
            if progressed || commands_ran {
                return Err("wait until dialogue and presentation commands have finished".into());
            }
            if matches!(player.presenter.screen(), Screen::Error(_)) {
                return Err("save/load is unavailable after a Story error".into());
            }
            match request {
                UiAction::Save => {
                    if !ui.at_initial_base(&player.story) {
                        return Err(
                            "close overlays and return to the initial screen before saving".into(),
                        );
                    }
                    if !media.settled() {
                        return Err("wait for the view/audio transition before saving".into());
                    }
                    player.write_save()?;
                    Ok("Saved.".to_string())
                }
                UiAction::Load => {
                    let candidate = player.prepare_load()?;
                    media.restore(&candidate.actions, &player.story)?;
                    player.state = candidate.state;
                    player.presenter = candidate.presenter;
                    player.pending.clear();
                    player.dirty = true;
                    ui.reset(&player.story);
                    shell_input.shell.loaded(&mut focus);
                    Ok("Loaded.".to_string())
                }
                _ => unreachable!("save/load request"),
            }
        })();
        shell_input.shell.invalidate_load();
        feedback.failed = result.is_err();
        feedback.until = result
            .is_ok()
            .then(|| time.elapsed() + std::time::Duration::from_secs(4));
        player.save_status = match result {
            Ok(message) => message,
            Err(message) => format!(
                "Couldn't {}: {message}",
                if matches!(request, UiAction::Save) {
                    "save"
                } else {
                    "load"
                }
            ),
        };
    }

    if let Screen::Error(message) = player.presenter.screen()
        && !matches!(before, Screen::Error(_))
    {
        error!("{message}");
    }
    if let Ok(mut state) = player.state.write() {
        state.drain_changes();
    }
    let was_failed = player.preferences.failed;
    player.persist_preferences(time.elapsed(), false);
    if player.preferences.failed && !was_failed {
        player.save_status = player.preferences.status.clone();
        feedback.failed = true;
        feedback.until = None;
    }
    if player.presenter.revision() != before_revision || player.presenter.screen() != &before {
        player.dirty = true;
    }
}

const DIGIT_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

fn read_asset(story: &Story, id: &str, kind: AssetKind) -> Result<(String, Vec<u8>), String> {
    let path = story.asset_path_of(kind, id).map_err(|d| d.to_string())?;
    let bytes = weftora_story::paths::read_bytes(path.clone())
        .map_err(|e| format!("cannot read asset `{id}` ({}): {e}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    Ok((ext, bytes))
}

/// Match Bevy's decoder construction, but return failures before playback is spawned.
fn validate_audio(id: &str, bytes: &Arc<[u8]>) -> Result<(), String> {
    rodio::Decoder::builder()
        .with_byte_len(bytes.len() as u64)
        .with_data(std::io::Cursor::new(Arc::clone(bytes)))
        .build()
        .map(|_| ())
        .map_err(|error| format!("asset `{id}` is not readable audio: {error}"))
}

const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
fn image_cost(id: &str, ext: &str, bytes: &[u8]) -> Result<u64, String> {
    let dimensions: Result<u64, String> = (|| {
        let format =
            image::ImageFormat::from_extension(ext).ok_or("unsupported format".to_string())?;
        let (width, height) = image::ImageReader::with_format(std::io::Cursor::new(bytes), format)
            .into_dimensions()
            .map_err(|e| e.to_string())?;
        // Covers even RGBA32F conversion before allocating decoded pixels.
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("image exceeds 4096px or the 64 MiB decode budget".into());
        }
        let cost = u64::from(width) * u64::from(height) * 16;
        if cost > MAX_IMAGE_BYTES {
            return Err("image exceeds 4096px or the 64 MiB decode budget".into());
        }
        Ok(cost)
    })();
    dimensions.map_err(|e| format!("asset `{id}` is not a readable image: {e}"))
}
fn decode_image(id: &str, ext: &str, bytes: &[u8]) -> Result<Image, String> {
    image_cost(id, ext, bytes)?;
    Image::from_buffer(
        bytes,
        ImageType::Extension(ext),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    )
    .map_err(|error| format!("asset `{id}` is not a readable image: {error}"))
}

/// Rebuilds the dialogue panel when the presenter's screen changed.
fn refresh_panel(
    mut player: NonSendMut<Player>,
    panel: Single<(Entity, &mut Visibility, &mut Panel)>,
    mut texts: Query<(&PanelBinding, &mut Text, &mut Node)>,
    mut commands: Commands,
    ui: Res<Ui>,
    shell: Res<shell::Shell>,
) {
    let (panel, mut visibility, mut cached) = panel.into_inner();
    let show = shell.is_playing()
        && (player.story.manifest().initial_screen.is_none()
            || matches!(player.presenter.screen(), Screen::Error(_)));
    *visibility = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if !player.dirty {
        return;
    }
    player.dirty = false;
    let same_shape = match player.presenter.screen() {
        Screen::Line { .. } => {
            texts.iter().count() == 2
                && texts
                    .iter()
                    .all(|(b, _, _)| matches!(b, PanelBinding::Speaker | PanelBinding::Text))
        }
        Screen::Choices(choices) => {
            texts.iter().count()
                == choices.len()
                    + if player.presenter.decision_context().is_some() {
                        2
                    } else {
                        0
                    }
                && texts.iter().all(|(b, _, _)| match b {
                    PanelBinding::Choice(i) => *i < choices.len(),
                    PanelBinding::Speaker | PanelBinding::Text => {
                        player.presenter.decision_context().is_some()
                    }
                })
        }
        _ => false,
    };
    if show && cached.0 == Some(player.presenter.revision()) && same_shape {
        for (binding, mut text, mut node) in &mut texts {
            if matches!(binding, PanelBinding::Speaker) {
                node.display = if matches!(
                    player.presenter.screen(),
                    Screen::Line {
                        speaker: Some(_),
                        ..
                    }
                ) {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            text.0 = match (binding, player.presenter.screen()) {
                (PanelBinding::Speaker, Screen::Line { speaker, .. }) => {
                    speaker.clone().unwrap_or_default()
                }
                (PanelBinding::Text, Screen::Line { text, .. }) => text.clone(),
                (PanelBinding::Choice(i), Screen::Choices(choices)) => {
                    format!("{}. {}", i + 1, choices[*i].text)
                }
                (PanelBinding::Speaker, Screen::Choices(_)) => player
                    .presenter
                    .decision_context()
                    .and_then(|(speaker, _)| speaker)
                    .unwrap_or_default()
                    .into(),
                (PanelBinding::Text, Screen::Choices(_)) => player
                    .presenter
                    .decision_context()
                    .map(|(_, text)| text)
                    .unwrap_or_default()
                    .into(),
                _ => unreachable!("shape checked"),
            };
        }
        return;
    }
    cached.0 = Some(player.presenter.revision());
    commands.entity(panel).despawn_children();
    if !show {
        return;
    }
    match player.presenter.screen().clone() {
        Screen::Line { speaker, text } => {
            commands.entity(panel).with_children(|p| {
                p.spawn((
                    PanelBinding::Speaker,
                    Node {
                        display: if speaker.is_some() {
                            Display::Flex
                        } else {
                            Display::None
                        },
                        ..default()
                    },
                    label_with_font(
                        speaker.unwrap_or_default(),
                        26.0,
                        SPEAKER_COLOR,
                        ui.default_font.clone(),
                    ),
                ));
                p.spawn((
                    PanelBinding::Text,
                    label_with_font(text, 30.0, Color::WHITE, ui.default_font.clone()),
                ));
                p.spawn(label_with_font(
                    "CLICK  /  SPACE  /  ENTER    Continue",
                    16.0,
                    theme::MUTED,
                    ui.default_font.clone(),
                ));
            });
        }
        Screen::Choices(choices) => {
            let context = player
                .presenter
                .decision_context()
                .map(|(speaker, text)| (speaker.map(str::to_owned), text.to_owned()));
            commands.entity(panel).with_children(|p| {
                if let Some((speaker, text)) = context {
                    p.spawn((
                        PanelBinding::Speaker,
                        label_with_font(
                            speaker.unwrap_or_default(),
                            22.0,
                            SPEAKER_COLOR,
                            ui.default_font.clone(),
                        ),
                    ));
                    p.spawn((
                        PanelBinding::Text,
                        label_with_font(text, 28.0, theme::TEXT, ui.default_font.clone()),
                    ));
                }
                for (position, choice) in choices.into_iter().enumerate() {
                    p.spawn((
                        Button,
                        Hovered::default(),
                        bevy::input_focus::tab_navigation::TabIndex(0),
                        theme::button_node(),
                        BackgroundColor(BUTTON_BG),
                        BorderColor::all(theme::BORDER),
                        Outline::default(),
                        observe(move |_: On<Activate>, mut intents: ResMut<Intents>| {
                            intents.push(Intent::Select {
                                layer: None,
                                position,
                            });
                        }),
                        children![(
                            PanelBinding::Choice(position),
                            label_with_font(
                                format!("{}. {}", position + 1, choice.text),
                                28.0,
                                Color::WHITE,
                                ui.default_font.clone()
                            )
                        )],
                    ));
                }
            });
        }
        Screen::End => {
            commands.entity(panel).with_children(|p| {
                p.spawn(label_with_font(
                    END_TEXT,
                    30.0,
                    Color::WHITE,
                    ui.default_font.clone(),
                ));
            });
        }
        Screen::Error(message) => {
            commands.entity(panel).with_children(|p| {
                p.spawn(label_with_font(
                    "Story error",
                    26.0,
                    ERROR_COLOR,
                    ui.default_font.clone(),
                ));
                p.spawn(label_with_font(
                    message,
                    22.0,
                    Color::WHITE,
                    ui.default_font.clone(),
                ));
                p.spawn(label_with_font(
                    "Esc / right-click opens the game menu",
                    16.0,
                    theme::MUTED,
                    ui.default_font.clone(),
                ));
            });
        }
    }
}

fn refresh_save_status(
    player: NonSend<Player>,
    ui: Res<Ui>,
    time: Res<Time>,
    feedback: Res<SaveFeedback>,
    mut text: Single<(&mut Text, &mut Node, &mut TextFont, &mut TextColor), With<SaveStatus>>,
) {
    let (ref mut text, ref mut node, ref mut font, ref mut color) = *text;
    let source = ui.default_font.clone().map(Into::into).unwrap_or_default();
    if font.font != source {
        font.font = source;
    }
    let (value, tone) = if !player.save_status.is_empty()
        && !feedback.until.is_some_and(|until| time.elapsed() >= until)
    {
        (
            player.save_status.as_str(),
            if feedback.failed {
                theme::ACCENT
            } else {
                theme::SUCCESS
            },
        )
    } else if let Some(Err(message)) = &player.save_store {
        (message.as_str(), ERROR_COLOR)
    } else {
        ("", theme::MUTED)
    };
    let display = if value.is_empty() {
        Display::None
    } else {
        Display::Flex
    };
    if node.display != display {
        node.display = display;
    }
    if color.0 != tone {
        color.0 = tone;
    }
    if text.0 != value {
        text.0 = value.into();
    }
}

#[allow(clippy::type_complexity)]
fn hover_buttons(
    focus: Res<InputFocus>,
    visible: Res<InputFocusVisible>,
    mut buttons: Query<
        (
            Entity,
            &Hovered,
            Option<&ButtonBase>,
            Has<bevy::ui::Pressed>,
            &mut BackgroundColor,
            Option<&mut BorderColor>,
            Option<&mut Outline>,
        ),
        With<Button>,
    >,
) {
    for (entity, hovered, base, pressed, mut background, border, outline) in &mut buttons {
        let base = base.map_or(BUTTON_BG, |b| b.0);
        let focused = visible.0 && focus.get() == Some(entity);
        let fill = if pressed {
            base.mix(&Color::BLACK, 0.20)
        } else if hovered.get() {
            base.mix(&Color::WHITE, 0.12)
        } else {
            base
        };
        background.set_if_neq(BackgroundColor(fill));
        if let Some(mut border) = border {
            border.set_if_neq(BorderColor::all(if focused {
                theme::ACCENT
            } else {
                theme::BORDER
            }));
        }
        if let Some(mut outline) = outline {
            let color = if focused { theme::ACCENT } else { Color::NONE };
            if outline.color != color || outline.width != px(2) || outline.offset != px(2) {
                *outline = Outline::new(px(2), px(2), color);
            }
        }
    }
}

fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    label_with_font(text, size, color, None)
}
fn label_with_font(
    text: impl Into<String>,
    size: f32,
    color: Color,
    font: Option<Handle<Font>>,
) -> impl Bundle {
    (
        Text::new(text),
        Pickable::IGNORE,
        TextFont {
            font_size: FontSize::Px(size),
            font: font.map(Into::into).unwrap_or_default(),
            ..default()
        },
        TextColor(color),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::audio::AudioSource;
    use bevy::input_focus::InputFocusVisible;

    const WAV: &[u8] = include_bytes!("../../../examples/hello-story/assets/chime.wav");
    const PNG: &[u8] = include_bytes!("../../../examples/hello-story/assets/room.png");

    #[test]
    fn valid_wav_passes_audio_validation() {
        assert!(validate_audio("chime", &Arc::from(WAV)).is_ok());
    }

    #[test]
    fn invalid_audio_returns_asset_specific_error() {
        for bytes in [PNG, &[]] {
            let error = validate_audio("bad-sound", &Arc::from(bytes)).unwrap_err();
            assert!(error.contains("asset `bad-sound` is not readable audio"));
        }
    }

    #[test]
    fn valid_png_decodes() {
        assert!(decode_image("room", "png", PNG).is_ok());
    }

    #[test]
    fn invalid_images_return_asset_specific_error() {
        for (ext, bytes) in [("png", WAV), ("png", &[][..]), ("unsupported", PNG)] {
            let error = decode_image("bad-background", ext, bytes).unwrap_err();
            assert!(error.contains("asset `bad-background` is not a readable image"));
        }
    }
    pub(super) fn input_app() -> App {
        let story = Story::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello-story"),
        )
        .unwrap();
        let compiled = CompiledDialogue::from_artifact_bytes(include_bytes!(
            "../../weftora-yarn/tests/fixtures/budget.json"
        ))
        .unwrap();
        app_for(story, compiled)
    }
    pub(super) fn app_for(story: Story, compiled: CompiledDialogue) -> App {
        app_for_debug(story, compiled, false)
    }
    pub(super) fn app_for_debug(story: Story, compiled: CompiledDialogue, debug: bool) -> App {
        let state = Arc::new(std::sync::RwLock::new(story.initial_state().unwrap()));
        let mut player = prepare_player(story, compiled, state, debug, None).unwrap();
        let mut runner =
            DialogueRunner::for_story(&player.compiled, player.state.clone(), &player.story);
        if debug {
            runner.enable_rule_trace();
        }
        let (presenter, pending) = Presenter::new(runner, &player.story.manifest().entrypoint);
        player.presenter = presenter;
        player.pending = pending;
        let mut app = App::new();
        app.insert_non_send(player)
            .init_resource::<RuntimeFailure>()
            .insert_resource(debug_tools::Inspector::new(debug))
            .add_message::<bevy::input::keyboard::KeyboardInput>()
            .init_resource::<Time>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<InputFocus>()
            .init_resource::<InputFocusVisible>()
            .insert_resource(shell::Shell::new(true))
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<bevy::window::WindowCloseRequested>()
            .init_resource::<SaveFeedback>()
            .init_resource::<Intents>()
            .init_resource::<Ui>()
            .init_resource::<media::Presentation>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<AudioSource>>()
            .add_message::<AppExit>()
            .add_systems(
                Update,
                (debug_tools::input, drive, remember_failure).chain(),
            );
        app
    }
    #[test]
    fn repeated_escape_toggles_menu_without_quitting_or_progressing() {
        let mut app = input_app();
        app.update();
        let before = app.world().non_send::<Player>().presenter.screen().clone();
        for _ in 0..6 {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Space);
            app.update();
            assert!(app.world().resource::<Messages<AppExit>>().is_empty());
            assert_eq!(app.world().non_send::<Player>().presenter.screen(), &before);
        }
    }

    #[test]
    fn themed_button_hover_preserves_base_and_keyboard_focus_has_its_own_border() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = input_app();
        let base = Color::srgb(0.3, 0.1, 0.2);
        let button = app
            .world_mut()
            .spawn((
                Button,
                Hovered::default(),
                ButtonBase(base),
                BackgroundColor(base),
                BorderColor::all(theme::BORDER),
                Outline::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(button, bevy::input_focus::FocusCause::Navigated);
        app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
        app.world_mut().run_system_once(hover_buttons).unwrap();
        assert_eq!(app.world().get::<BackgroundColor>(button).unwrap().0, base);
        assert_eq!(
            *app.world().get::<BorderColor>(button).unwrap(),
            BorderColor::all(theme::ACCENT)
        );
        assert_eq!(
            app.world().get::<Outline>(button).unwrap().color,
            theme::ACCENT
        );
        app.world_mut().resource_mut::<InputFocusVisible>().0 = false;
        app.world_mut().run_system_once(hover_buttons).unwrap();
        assert_eq!(
            *app.world().get::<BorderColor>(button).unwrap(),
            BorderColor::all(theme::BORDER)
        );
    }

    #[test]
    fn runtime_failure_survives_escape_and_keeps_the_first_error() {
        let mut app = input_app();
        app.world_mut()
            .non_send_mut::<Player>()
            .presenter
            .fail("first failure".into());
        app.world_mut()
            .non_send_mut::<Player>()
            .presenter
            .fail("later failure".into());
        assert!(
            matches!(app.world().non_send::<Player>().presenter.screen(), Screen::Error(message) if message == "first failure")
        );
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(
            app.world()
                .resource::<RuntimeFailure>()
                .0
                .lock()
                .unwrap()
                .as_deref(),
            Some("first failure")
        );
    }

    #[test]
    fn duplicate_progress_intents_advance_only_one_line() {
        let mut app = input_app();
        for _ in 0..2 {
            app.world_mut()
                .resource_mut::<Intents>()
                .push(Intent::Action {
                    layer: None,
                    action: UiAction::Advance,
                });
        }
        app.update();
        assert!(
            matches!(app.world().non_send::<Player>().presenter.screen(), Screen::Line { text, .. } if text == "Second")
        );
    }
    #[test]
    fn focused_enter_and_button_action_do_not_advance_dialogue() {
        let mut app = input_app();
        let entity = app
            .world_mut()
            .spawn((
                Button,
                observe(|_: On<Activate>, mut intents: ResMut<Intents>| {
                    intents.push(Intent::Action {
                        layer: None,
                        action: UiAction::SetState {
                            key: "curious".into(),
                            value: weftora_api::Value::Bool(true),
                        },
                    });
                }),
            ))
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(entity, bevy::input_focus::FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.world_mut().trigger(Activate { entity });
        app.update();
        let player = app.world().non_send::<Player>();
        assert!(matches!(player.presenter.screen(), Screen::Line { text, .. } if text == "First"));
        assert_eq!(
            player.state.read().unwrap().get("curious"),
            Ok(&weftora_api::Value::Bool(true))
        );
        assert!(player.state.write().unwrap().drain_changes().is_empty());
    }
    pub(super) fn hello_app() -> App {
        let mut app = input_app();
        let mut compiled =
            CompiledDialogue::from_artifact_bytes(include_bytes!("../tests/fixtures/hello.json"))
                .unwrap();
        let player = &mut *app.world_mut().non_send_mut::<Player>();
        compiled
            .verify_sources(&player.story.dialogue_sources().unwrap())
            .unwrap();
        let (presenter, pending) = Presenter::new(
            DialogueRunner::new(&compiled, player.state.clone()),
            "Start",
        );
        player.presenter = presenter;
        player.pending = pending;
        app
    }
    #[test]
    fn oversized_image_is_rejected_from_header_before_decode() {
        // A PNG header with valid CRC and no pixel data: dimensions alone must reject it.
        let mut bytes = PNG.to_vec();
        bytes[16..20].copy_from_slice(&100_000u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&100_000u32.to_be_bytes());
        let mut crc = !0u32;
        for b in &bytes[12..29] {
            crc ^= u32::from(*b);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xEDB88320u32 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
        let error = decode_image("oversized", "png", &bytes).unwrap_err();
        assert!(error.contains("decode budget"), "{error}");
    }
    #[test]
    fn initial_presentation_error_discards_pending_media() {
        let mut app = hello_app();
        app.world_mut()
            .non_send_mut::<Player>()
            .presenter
            .fail("startup asset failure".into());
        app.update();
        assert_eq!(
            app.world().non_send::<Player>().presenter.screen(),
            &Screen::Error("startup asset failure".into())
        );
        assert!(
            app.world()
                .resource::<media::Presentation>()
                .audio
                .cues
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<media::Presentation>()
                .views
                .samples(std::time::Duration::ZERO)
                .is_empty()
        );
    }
}
