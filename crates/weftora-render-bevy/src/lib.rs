//! Bevy presentation adapter for the Weftora player.
//!
//! Story assets are read from the Story folder with `std::fs` through [`Story::asset_path_of`] and turned into
//! Bevy assets here, so Bevy never opens a path outside the Story. The dialogue-to-screen logic lives in
//! [`presenter`] and has no Bevy types; the systems below only translate input and draw its output.

mod declarative;
mod media;
pub mod presenter;

use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::audio::AddAudioSource;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::input_focus::{InputFocus, tab_navigation::TabNavigationPlugin};
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

const PANEL_BG: Color = Color::srgba(0.04, 0.04, 0.08, 0.88);
const BUTTON_BG: Color = Color::srgb(0.16, 0.18, 0.28);
const BUTTON_HOVER: Color = Color::srgb(0.26, 0.30, 0.46);
const SPEAKER_COLOR: Color = Color::srgb(0.95, 0.78, 0.35);
const ERROR_COLOR: Color = Color::srgb(1.0, 0.45, 0.45);

/// Opens the game window and runs `story` until the window closes.
pub fn run(story: Story, dialogue: CompiledDialogue, state: SharedState) -> Result<(), String> {
    let title = story.manifest().title.clone();
    let runner = DialogueRunner::new(&dialogue, Arc::clone(&state));
    if let Ok(mut state) = state.write() {
        state.drain_changes();
    }
    let (presenter, actions) = Presenter::new(runner, &story.manifest().entrypoint.clone());
    let player = Player {
        presenter,
        state,
        story,
        pending: actions,
        dirty: true,
    };

    let failure = RuntimeFailure::default();
    let outcome = failure.0.clone();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title,
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(TabNavigationPlugin)
        .add_audio_source::<media::LoopAudio>()
        .insert_non_send(player)
        .insert_resource(failure)
        .init_resource::<Intents>()
        .init_resource::<Ui>()
        .init_resource::<media::Presentation>()
        .add_systems(Startup, (setup, declarative::setup))
        .add_systems(
            Update,
            (
                drive,
                media::sync_views,
                declarative::sync_layers,
                declarative::sync_choices,
                declarative::restore_focus,
                declarative::update_bindings,
                media::sync_audio,
                refresh_panel,
                hover_buttons,
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
}

#[derive(Component)]
struct Panel;

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
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
        Panel,
        GlobalZIndex(100000),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: px(24),
            right: px(24),
            bottom: px(24),
            min_height: px(180),
            padding: UiRect::all(px(20)),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            ..default()
        },
        BackgroundColor(PANEL_BG),
    ));
}

/// Input -> presenter -> side effects.
#[allow(clippy::too_many_arguments)]
fn drive(
    mut player: NonSendMut<Player>,
    keys: Res<ButtonInput<KeyCode>>,
    mut intents: ResMut<Intents>,
    mut ui: ResMut<Ui>,
    focus: Res<InputFocus>,
    buttons: Query<(), With<Button>>,
    focus_keys: Query<&FocusKey>,
    mut exit: MessageWriter<AppExit>,
    mut media: Media,
    time: Res<Time>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }

    media.begin_frame(
        time.delta(),
        matches!(player.presenter.screen(), Screen::Error(_)),
    );
    let before = player.presenter.screen().clone();
    let before_revision = player.presenter.revision();
    let mut actions = std::mem::take(&mut player.pending);

    let focused_button = focus.get().is_some_and(|e| buttons.contains(e));
    if !ui.modal() {
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
            UiAction::Advance if !progressed => {
                progressed = true;
                actions.extend(player.presenter.advance());
                Ok(())
            }
            UiAction::SetState { ref key, ref value } => (|| {
                let mut state = player
                    .state
                    .write()
                    .map_err(|_| "canonical state lock poisoned".to_string())?;
                state.get(key).map_err(|e| e.to_string())?;
                state.set(key, value.clone()).map_err(|e| e.to_string())
            })(),
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

    if let Screen::Error(message) = player.presenter.screen()
        && !matches!(before, Screen::Error(_))
    {
        error!("{message}");
    }
    if let Ok(mut state) = player.state.write() {
        state.drain_changes();
    }
    if player.presenter.revision() != before_revision {
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
    panel: Single<(Entity, &mut Visibility), With<Panel>>,
    mut commands: Commands,
) {
    let (panel, mut visibility) = panel.into_inner();
    let show = player.story.manifest().initial_screen.is_none()
        || matches!(player.presenter.screen(), Screen::Error(_));
    *visibility = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if !player.dirty {
        return;
    }
    player.dirty = false;

    commands.entity(panel).despawn_children();
    if !show {
        return;
    }
    match player.presenter.screen().clone() {
        Screen::Line { speaker, text } => {
            commands.entity(panel).with_children(|p| {
                if let Some(speaker) = speaker {
                    p.spawn(label(speaker, 26.0, SPEAKER_COLOR));
                }
                p.spawn(label(text, 30.0, Color::WHITE));
                p.spawn(label(
                    "click / Space to continue",
                    16.0,
                    Color::srgb(0.6, 0.6, 0.7),
                ));
            });
        }
        Screen::Choices(choices) => {
            commands.entity(panel).with_children(|p| {
                for (position, choice) in choices.into_iter().enumerate() {
                    p.spawn((
                        Button,
                        Hovered::default(),
                        bevy::input_focus::tab_navigation::TabIndex(0),
                        Node {
                            padding: UiRect::axes(px(16), px(10)),
                            ..default()
                        },
                        BackgroundColor(BUTTON_BG),
                        observe(move |_: On<Activate>, mut intents: ResMut<Intents>| {
                            intents.push(Intent::Select {
                                layer: None,
                                position,
                            });
                        }),
                        children![label(
                            format!("{}. {}", position + 1, choice.text),
                            28.0,
                            Color::WHITE
                        )],
                    ));
                }
            });
        }
        Screen::End => {
            commands.entity(panel).with_children(|p| {
                p.spawn(label(END_TEXT, 30.0, Color::WHITE));
            });
        }
        Screen::Error(message) => {
            commands.entity(panel).with_children(|p| {
                p.spawn(label("Story error", 26.0, ERROR_COLOR));
                p.spawn(label(message, 22.0, Color::WHITE));
                p.spawn(label("press Esc to quit", 16.0, Color::srgb(0.6, 0.6, 0.7)));
            });
        }
    }
}

fn hover_buttons(
    focus: Res<InputFocus>,
    mut buttons: Query<(Entity, &Hovered, Option<&ButtonBase>, &mut BackgroundColor), With<Button>>,
) {
    for (entity, hovered, base, mut background) in &mut buttons {
        background.0 = if hovered.get() || focus.get() == Some(entity) {
            BUTTON_HOVER
        } else {
            base.map_or(BUTTON_BG, |b| b.0)
        };
    }
}

fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        Pickable::IGNORE,
        TextFont {
            font_size: FontSize::Px(size),
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
        let state = Arc::new(std::sync::RwLock::new(story.initial_state().unwrap()));
        let runner = DialogueRunner::new(&compiled, state.clone());
        let (presenter, pending) = Presenter::new(runner, "Start");
        let mut app = App::new();
        app.insert_non_send(Player {
            story,
            state,
            presenter,
            pending,
            dirty: true,
        })
        .init_resource::<RuntimeFailure>()
        .init_resource::<Time>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>()
        .init_resource::<Intents>()
        .init_resource::<Ui>()
        .init_resource::<media::Presentation>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<AudioSource>>()
        .add_message::<AppExit>()
        .add_systems(Update, (drive, remember_failure).chain());
        app
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
