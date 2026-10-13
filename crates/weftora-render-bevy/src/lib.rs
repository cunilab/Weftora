//! Bevy presentation adapter for the Weftora player (Phase 0 spike).
//!
//! Story assets are read from the Story folder with `std::fs` through [`Story::asset_path`] and turned into
//! Bevy assets here, so Bevy never opens a path outside the Story. The dialogue-to-screen logic lives in
//! [`presenter`] and has no Bevy types; the systems below only translate input and draw its output.

pub mod presenter;

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::audio::AudioSource;
use bevy::ecs::system::SystemParam;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};
use bevy::window::WindowResolution;
use weftora_core::SharedState;
use weftora_story::Story;
use weftora_yarn::{CompiledDialogue, DialogueRunner};

use presenter::{Action, END_TEXT, Presenter, Screen};

const PANEL_BG: Color = Color::srgba(0.04, 0.04, 0.08, 0.88);
const BUTTON_BG: Color = Color::srgb(0.16, 0.18, 0.28);
const BUTTON_HOVER: Color = Color::srgb(0.26, 0.30, 0.46);
const SPEAKER_COLOR: Color = Color::srgb(0.95, 0.78, 0.35);
const ERROR_COLOR: Color = Color::srgb(1.0, 0.45, 0.45);

/// Opens the game window and runs `story` until the window closes.
pub fn run(story: Story, dialogue: CompiledDialogue, state: SharedState) {
    let title = story.manifest().title.clone();
    let runner = DialogueRunner::new(&dialogue, state);
    let (presenter, actions) = Presenter::new(runner, &story.manifest().entrypoint.clone());
    let player = Player {
        presenter,
        story,
        pending: actions,
        dirty: true,
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title,
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .insert_non_send(player)
        .init_resource::<PickedChoice>()
        .add_systems(Startup, setup)
        .add_systems(Update, (drive, refresh_panel, hover_buttons).chain())
        .run();
}

/// Presenter plus the Story it reads assets from. Not `Send`: the Yarn runner may not be.
struct Player {
    presenter: Presenter<DialogueRunner>,
    story: Story,
    pending: Vec<Action>,
    dirty: bool,
}

/// Set by a choice button's observer, consumed by [`drive`].
#[derive(Resource, Default)]
struct PickedChoice(Option<usize>);

#[derive(Component)]
struct Background;

#[derive(Component)]
struct Panel;

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Background,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        ImageNode::default(),
        Visibility::Hidden,
    ));
    commands.spawn((
        Panel,
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

/// Everything [`perform`] needs to create images and sounds.
#[derive(SystemParam)]
struct Media<'w, 's> {
    commands: Commands<'w, 's>,
    images: ResMut<'w, Assets<Image>>,
    sounds: ResMut<'w, Assets<AudioSource>>,
    background: Query<'w, 's, (&'static mut ImageNode, &'static mut Visibility), With<Background>>,
}

/// Input -> presenter -> side effects.
fn drive(
    mut player: NonSendMut<Player>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut picked: ResMut<PickedChoice>,
    mut exit: MessageWriter<AppExit>,
    mut media: Media,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }

    let before = player.presenter.screen().clone();
    let mut actions = std::mem::take(&mut player.pending);

    let choice_key = DIGIT_KEYS.iter().position(|k| keys.just_pressed(*k));
    if let Some(position) = picked.0.take().or(choice_key) {
        actions.extend(player.presenter.select(position));
    } else if keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::NumpadEnter)
        || mouse.just_pressed(MouseButton::Left)
    {
        actions.extend(player.presenter.advance());
    }

    for action in actions {
        if let Err(message) = perform(&action, &player.story, &mut media) {
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
    if *player.presenter.screen() != before {
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

fn read_asset(story: &Story, id: &str) -> Result<(String, Vec<u8>), String> {
    let path = story.asset_path(id).map_err(|d| d.to_string())?;
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("cannot read asset `{id}` ({}): {e}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    Ok((ext, bytes))
}

fn perform(action: &Action, story: &Story, media: &mut Media) -> Result<(), String> {
    match action {
        Action::SetBackground(id) => {
            let (ext, bytes) = read_asset(story, id)?;
            let image = Image::from_buffer(
                &bytes,
                ImageType::Extension(&ext),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::Default,
                RenderAssetUsages::default(),
            )
            .map_err(|e| format!("asset `{id}` is not a readable image: {e}"))?;
            let handle = media.images.add(image);
            for (mut node, mut visibility) in &mut media.background {
                node.image = handle.clone();
                *visibility = Visibility::Inherited;
            }
        }
        Action::PlaySound(id) => {
            let (_, bytes) = read_asset(story, id)?;
            let handle = media.sounds.add(AudioSource {
                bytes: Arc::from(bytes),
            });
            media
                .commands
                .spawn((AudioPlayer::new(handle), PlaybackSettings::DESPAWN));
        }
    }
    Ok(())
}

/// Rebuilds the dialogue panel when the presenter's screen changed.
fn refresh_panel(
    mut player: NonSendMut<Player>,
    panel: Single<Entity, With<Panel>>,
    mut commands: Commands,
) {
    if !player.dirty {
        return;
    }
    player.dirty = false;
    let panel = *panel;
    commands.entity(panel).despawn_children();
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
                        Node {
                            padding: UiRect::axes(px(16), px(10)),
                            ..default()
                        },
                        BackgroundColor(BUTTON_BG),
                        observe(move |_: On<Activate>, mut picked: ResMut<PickedChoice>| {
                            picked.0 = Some(position);
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

fn hover_buttons(mut buttons: Query<(&Hovered, &mut BackgroundColor), With<Button>>) {
    for (hovered, mut background) in &mut buttons {
        background.0 = if hovered.get() {
            BUTTON_HOVER
        } else {
            BUTTON_BG
        };
    }
}

fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}
