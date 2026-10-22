//! Story widgets use Bevy UI; bindings patch existing text entities.
use std::collections::{BTreeMap, VecDeque};

use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy::input_focus::{FocusCause, InputFocus, InputFocusVisible};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};
use weftora_api::Value;
use weftora_story::ui::{
    Binding, DialogueBinding, Direction, Length, Position, UiAction, Widget, WidgetKind,
};

use crate::{BUTTON_BG, END_TEXT, Player, Screen, decode_image, read_asset, theme};

#[derive(Clone, Debug)]
pub(crate) enum Intent {
    Action {
        layer: Option<String>,
        action: UiAction,
    },
    Select {
        layer: Option<String>,
        position: usize,
    },
}
#[derive(Resource, Default)]
pub(crate) struct Intents(VecDeque<Intent>);
impl Intents {
    pub fn push(&mut self, intent: Intent) {
        // Bound simultaneous input; stale/repeated progress is discarded by drive.
        if self.0.len() < 32 {
            self.0.push_back(intent);
        }
    }
    pub fn drain(&mut self) -> impl Iterator<Item = Intent> + '_ {
        self.0.drain(..)
    }
}
#[derive(Clone)]
struct Layer {
    id: String,
    return_focus: Option<String>,
}
#[derive(Resource, Default)]
pub(crate) struct Ui {
    layers: Vec<Layer>,
    built: Vec<String>,
    rebuild_choices: bool,
    choices_revision: u64,
    first_key: Option<String>,
    choices: Option<Vec<crate::presenter::ChoiceView>>,
    restore: Option<String>,
    focus_first: bool,
    images: BTreeMap<String, Handle<Image>>,
    fonts: BTreeMap<String, Handle<Font>>,
    pub(super) default_font: Option<Handle<Font>>,
}
impl Ui {
    pub fn at_initial_base(&self, story: &weftora_story::Story) -> bool {
        match &story.manifest().initial_screen {
            Some(initial) => self.layers.len() == 1 && self.layers[0].id == *initial,
            None => self.layers.is_empty(),
        }
    }
    pub fn reset(&mut self, story: &weftora_story::Story) {
        self.layers = story
            .manifest()
            .initial_screen
            .iter()
            .map(|id| Layer {
                id: id.clone(),
                return_focus: None,
            })
            .collect();
        self.built.clear();
        self.choices = None;
        self.restore = None;
        self.focus_first = true;
        self.rebuild_choices = true;
    }
    pub fn modal(&self) -> bool {
        self.layers.len() > 1
    }
    pub fn accepts(&self, origin: &Option<String>) -> bool {
        match origin {
            Some(id) => self.layers.last().is_some_and(|l| l.id == *id),
            None => !self.modal(),
        }
    }
    pub fn navigate(
        &mut self,
        action: &UiAction,
        story: &weftora_story::Story,
        focus: Option<String>,
    ) -> Result<(), String> {
        match action {
            UiAction::Open { screen } => {
                let def = story.screen(screen).map_err(|e| e.to_string())?;
                if def.document.root.kind == WidgetKind::Screen {
                    self.layers = vec![Layer {
                        id: screen.clone(),
                        return_focus: None,
                    }];
                    self.restore = None;
                } else if !self.layers.iter().any(|l| l.id == *screen) {
                    if self.layers.len() >= 5 {
                        return Err("UI overlay limit (4) exceeded".into());
                    }
                    self.layers.push(Layer {
                        id: screen.clone(),
                        return_focus: focus,
                    });
                    self.restore = None;
                } else {
                    return Ok(());
                }
                self.focus_first = true;
            }
            UiAction::Close if self.modal() => {
                self.restore = self.layers.pop().and_then(|l| l.return_focus);
                self.focus_first = self.restore.is_none();
            }
            _ => {}
        }
        Ok(())
    }
}
#[derive(Component)]
pub(crate) struct UiRoot;
#[derive(Component)]
pub(crate) struct FocusKey(pub String);
#[derive(Component)]
pub(crate) struct BoundText {
    binding: Binding,
    source: String,
}
#[derive(Component)]
pub(crate) struct Choices {
    layer: String,
    widget: Widget,
    source: String,
}
#[derive(Component)]
pub(crate) struct AdvanceButton;

#[derive(Component)]
pub(crate) struct ButtonBase(pub Color);

pub(crate) fn setup(
    mut player: NonSendMut<Player>,
    mut ui: ResMut<Ui>,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    mut font_cx: ResMut<bevy::text::FontCx>,
) {
    let initial = player.story.manifest().initial_screen.clone();
    let result = (|| {
        let mut asset_cost = 0u64;
        ui.default_font = Some(fonts.add(decode_font(
            "player.interface",
            include_bytes!("../assets/FiraSans-Regular.ttf").to_vec(),
            &mut font_cx,
        )?));
        if let Some(id) = player
            .story
            .localization()
            .and_then(|c| c.definition.font.as_ref())
        {
            let (_, bytes) = read_asset(&player.story, id, weftora_story::AssetKind::Font)?;
            asset_cost += bytes.len() as u64;
            if asset_cost > crate::MAX_IMAGE_BYTES {
                return Err("localization font exceeds the 64 MiB preload budget".into());
            }
            let handle = fonts.add(decode_font(id, bytes, &mut font_cx)?);
            ui.default_font = Some(handle.clone());
            ui.fonts.insert(id.clone(), handle);
        }
        for (_, screen) in player.story.screens() {
            for (path, widget) in screen.widgets() {
                let source = format!("{}:{path}", screen.file);
                if let Some(id) = &widget.image
                    && !ui.images.contains_key(id)
                {
                    let (ext, bytes) =
                        read_asset(&player.story, id, weftora_story::AssetKind::Image)
                            .map_err(|e| format!("{source}/image: {e}"))?;
                    asset_cost += crate::image_cost(id, &ext, &bytes)
                        .map_err(|e| format!("{source}/image: {e}"))?;
                    if asset_cost > crate::MAX_IMAGE_BYTES {
                        return Err(format!(
                            "{source}/image: UI assets exceed the 64 MiB preload budget"
                        ));
                    }
                    let image = decode_image(id, &ext, &bytes)
                        .map_err(|e| format!("{source}/image: {e}"))?;
                    ui.images.insert(id.clone(), images.add(image));
                }
                if let Some(id) = &widget.font
                    && !ui.fonts.contains_key(id)
                {
                    let (_, bytes) = read_asset(&player.story, id, weftora_story::AssetKind::Font)
                        .map_err(|e| format!("{source}/font: {e}"))?;
                    asset_cost += bytes.len() as u64;
                    if asset_cost > crate::MAX_IMAGE_BYTES {
                        return Err(format!(
                            "{source}/font: UI assets exceed the 64 MiB preload budget"
                        ));
                    }
                    let font = decode_font(id, bytes, &mut font_cx)
                        .map_err(|e| format!("{source}/font: {e}"))?;
                    ui.fonts.insert(id.clone(), fonts.add(font));
                }
            }
        }
        Ok::<_, String>(())
    })();
    match result {
        Ok(()) => {
            if let Some(initial) = initial {
                ui.layers.push(Layer {
                    id: initial,
                    return_focus: None,
                });
                ui.focus_first = true;
            }
        }
        Err(e) => {
            player.presenter.fail(e);
            player.dirty = true;
        }
    }
}
fn decode_font(id: &str, bytes: Vec<u8>, font_cx: &mut bevy::text::FontCx) -> Result<Font, String> {
    let font = Font::from_bytes(bytes);
    if font_cx
        .collection
        .register_fonts(font.data.clone(), None)
        .is_empty()
    {
        Err(format!("asset `{id}` is not a readable font"))
    } else {
        Ok(font)
    }
}

fn length(value: &Option<Length>) -> Val {
    match value.as_ref().and_then(|v| v.value().ok()) {
        Some((n, true)) => percent(n),
        Some((n, false)) => px(n),
        None => Val::Auto,
    }
}
fn node(w: &Widget) -> Node {
    Node {
        position_type: if w.layout.position == Position::Absolute {
            PositionType::Absolute
        } else {
            PositionType::Relative
        },
        left: length(&w.layout.left),
        right: length(&w.layout.right),
        top: length(&w.layout.top),
        bottom: length(&w.layout.bottom),
        width: length(&w.layout.width),
        height: length(&w.layout.height),
        padding: UiRect::all(px(w.layout.padding.unwrap_or(0.0))),
        row_gap: px(w.layout.gap.unwrap_or(0.0)),
        column_gap: px(w.layout.gap.unwrap_or(0.0)),
        border: if w.kind == WidgetKind::Panel && w.background.is_some() {
            UiRect::all(px(1))
        } else {
            UiRect::ZERO
        },
        border_radius: if w.kind == WidgetKind::Panel {
            BorderRadius::all(px(12))
        } else {
            BorderRadius::ZERO
        },
        min_width: px(0),
        flex_wrap: if w.layout.direction == Direction::Row {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        },
        align_items: if w.layout.direction == Direction::Row {
            AlignItems::Center
        } else {
            AlignItems::Stretch
        },
        flex_direction: if w.layout.direction == Direction::Row {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        ..default()
    }
}
fn color(s: &Option<String>, fallback: Color) -> Color {
    s.as_ref()
        .and_then(|s| Srgba::hex(s).ok().map(Color::from))
        .unwrap_or(fallback)
}
fn font(w: &Widget, ui: &Ui) -> TextFont {
    let mut font = TextFont {
        font_size: FontSize::Px(w.size.unwrap_or(20.0)),
        ..default()
    };
    if let Some(handle) = w
        .font
        .as_ref()
        .and_then(|id| ui.fonts.get(id))
        .or(ui.default_font.as_ref())
    {
        font.font = handle.clone().into();
    }
    font
}
fn spawn_widget(
    commands: &mut Commands,
    parent: Entity,
    w: &Widget,
    path: &str,
    layer: &str,
    file: &str,
    ui: &Ui,
) -> Entity {
    let entity = commands.spawn(node(w)).id();
    commands.entity(parent).add_child(entity);
    if w.kind != WidgetKind::Button {
        commands.entity(entity).insert(Pickable::IGNORE);
    }
    if w.kind == WidgetKind::List
        && w.children
            .iter()
            .any(|child| child.kind == WidgetKind::Button)
    {
        // Gaps between toolbar buttons must not advance the dialogue behind the toolbar.
        commands.entity(entity).insert(Pickable {
            is_hoverable: false,
            should_block_lower: true,
        });
    }
    if w.background.is_some() {
        commands
            .entity(entity)
            .insert(BackgroundColor(color(&w.background, Color::NONE)));
        if w.kind == WidgetKind::Panel {
            commands
                .entity(entity)
                .insert(BorderColor::all(theme::BORDER));
        }
    }
    match w.kind {
        WidgetKind::Text | WidgetKind::Button => {
            // Button's text is a child so its bounds remain the picking target.
            let text = commands
                .spawn((
                    Text::new(w.text.clone().unwrap_or_default()),
                    font(w, ui),
                    TextColor(color(&w.color, theme::TEXT)),
                    bevy::text::LineHeight::RelativeToFont(1.35),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(entity).add_child(text);
            if let Some(binding) = &w.bind {
                commands.entity(text).insert(BoundText {
                    binding: binding.clone(),
                    source: format!("{file}:{path}/bind"),
                });
            }
            if w.kind == WidgetKind::Button {
                let action = w.action.clone().expect("validated button");
                if matches!(action, UiAction::Advance) {
                    commands.entity(entity).insert(AdvanceButton);
                }
                let origin = Some(layer.to_string());
                let mut button_layout = node(w);
                let defaults = theme::button_node();
                button_layout.min_height = defaults.min_height;
                button_layout.min_width = defaults.min_width;
                button_layout.border = defaults.border;
                button_layout.border_radius = defaults.border_radius;
                button_layout.align_items = defaults.align_items;
                button_layout.flex_shrink = 0.0;
                if w.layout.padding.is_none() {
                    button_layout.padding = defaults.padding;
                }
                commands.entity(entity).insert((
                    button_layout,
                    BorderColor::all(theme::BORDER),
                    Outline::default(),
                    Button,
                    Hovered::default(),
                    TabIndex(0),
                    FocusKey(format!("{layer}/{}", w.id.as_ref().expect("validated ID"))),
                    BackgroundColor(color(&w.background, BUTTON_BG)),
                    ButtonBase(color(&w.background, BUTTON_BG)),
                    observe(move |_: On<Activate>, mut intents: ResMut<Intents>| {
                        intents.push(Intent::Action {
                            layer: origin.clone(),
                            action: action.clone(),
                        });
                    }),
                ));
            }
        }
        WidgetKind::Image => {
            commands.entity(entity).insert(ImageNode::new(
                ui.images[w.image.as_ref().expect("validated image")].clone(),
            ));
        }
        WidgetKind::ChoiceList => {
            commands.entity(entity).insert(Choices {
                layer: layer.into(),
                widget: w.clone(),
                source: format!("{file}:{path}"),
            });
        }
        _ => {}
    }
    for (i, child) in w.children.iter().enumerate() {
        spawn_widget(
            commands,
            entity,
            child,
            &format!("{path}/children/{i}"),
            layer,
            file,
            ui,
        );
    }
    entity
}

pub(crate) fn sync_layers(
    player: NonSend<Player>,
    mut ui: ResMut<Ui>,
    mut commands: Commands,
    roots: Query<Entity, With<UiRoot>>,
    keys: Query<&FocusKey>,
    mut focus: ResMut<InputFocus>,
) {
    let desired: Vec<_> = if matches!(player.presenter.screen(), Screen::Error(_)) {
        vec![]
    } else {
        ui.layers.iter().map(|l| l.id.clone()).collect()
    };
    if desired == ui.built {
        return;
    }
    if !ui.focus_first && ui.restore.is_none() {
        ui.restore = focus
            .get()
            .and_then(|e| keys.get(e).ok())
            .map(|k| k.0.clone());
    }
    focus.clear();
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    ui.first_key = desired.last().and_then(|id| {
        player
            .story
            .screen(id)
            .ok()?
            .widgets()
            .into_iter()
            .find_map(|(_, w)| {
                (w.kind == WidgetKind::Button)
                    .then(|| format!("{id}/{}", w.id.as_ref().expect("validated ID")))
            })
    });
    ui.built = desired.clone();
    ui.rebuild_choices = true;
    for (order, id) in desired.iter().enumerate() {
        let screen = player.story.screen(id).expect("validated screen");
        let root = commands
            .spawn((
                UiRoot,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                GlobalZIndex(20 + order as i32),
                TabGroup {
                    order: order as i32,
                    // Tab from empty focus searches non-modal groups; expose only the top layer.
                    modal: order + 1 != desired.len(),
                },
            ))
            .id();
        if order == 0 {
            commands.entity(root).insert(Pickable::IGNORE);
        }
        // Overlay root catches clicks across the window, including outside its content.
        spawn_widget(
            &mut commands,
            root,
            &screen.document.root,
            "/root",
            id,
            &screen.file,
            &ui,
        );
    }
}
#[derive(Component)]
pub(crate) struct ChoiceText(usize);

pub(crate) fn sync_choices(
    mut player: NonSendMut<Player>,
    mut ui: ResMut<Ui>,
    lists: Query<(Entity, &Choices)>,
    mut commands: Commands,
    mut focus: ResMut<InputFocus>,
    focus_visible: Res<InputFocusVisible>,
    mut texts: Query<(&ChoiceText, &mut Text)>,
) {
    let choices = match player.presenter.screen() {
        Screen::Choices(v) => v.clone(),
        _ => vec![],
    };
    let same_offer = ui.choices_revision == player.presenter.revision()
        && ui.choices.as_ref().is_some_and(|old| {
            old.len() == choices.len() && old.iter().zip(&choices).all(|(a, b)| a.index == b.index)
        });
    if same_offer && !ui.rebuild_choices && ui.choices.as_ref() != Some(&choices) {
        if let Err(error) = check_choices(&choices, "localized choices") {
            player.presenter.fail(error);
            player.dirty = true;
            return;
        }
        for (binding, mut text) in &mut texts {
            if let Some(choice) = choices.get(binding.0) {
                let value = format!("{}. {}", binding.0 + 1, choice.text);
                if text.0 != value {
                    text.0 = value;
                }
            }
        }
        ui.choices = Some(choices);
        return;
    }
    let changed = ui.choices.as_ref() != Some(&choices)
        || (matches!(player.presenter.screen(), Screen::Choices(_))
            && ui.choices_revision != player.presenter.revision());
    ui.choices_revision = player.presenter.revision();
    if !changed && !ui.rebuild_choices {
        return;
    }
    ui.rebuild_choices = false;
    ui.choices = Some(choices.clone());
    for (entity, list) in &lists {
        commands.entity(entity).despawn_children();
        if let Err(error) = check_choices(&choices, &list.source) {
            player.presenter.fail(error);
            player.dirty = true;
            return;
        }
        for (position, choice) in choices.iter().enumerate() {
            let origin = Some(list.layer.clone());
            let button = commands
                .spawn((
                    Button,
                    Hovered::default(),
                    TabIndex(0),
                    FocusKey(format!(
                        "{}/{}/{position}",
                        list.layer,
                        list.widget.id.as_ref().expect("validated ID")
                    )),
                    theme::button_node(),
                    BorderColor::all(theme::BORDER),
                    Outline::default(),
                    BackgroundColor(BUTTON_BG),
                    ButtonBase(BUTTON_BG),
                    observe(move |_: On<Activate>, mut intents: ResMut<Intents>| {
                        intents.push(Intent::Select {
                            layer: origin.clone(),
                            position,
                        });
                    }),
                    children![(
                        ChoiceText(position),
                        Text::new(format!("{}. {}", position + 1, choice.text)),
                        font(&list.widget, &ui),
                        TextColor(color(&list.widget.color, Color::WHITE)),
                        Pickable::IGNORE
                    )],
                ))
                .id();
            commands.entity(entity).add_child(button);
            if position == 0 && changed && !ui.modal() && focus_visible.0 {
                focus.clear();
                ui.restore = Some(format!(
                    "{}/{}/0",
                    list.layer,
                    list.widget.id.as_ref().expect("validated ID")
                ));
                ui.focus_first = false;
            }
        }
    }
}
pub(crate) fn sync_advance_buttons(
    player: NonSend<Player>,
    mut buttons: Query<(Entity, &mut Node), With<AdvanceButton>>,
    mut focus: ResMut<InputFocus>,
) {
    let display = if matches!(player.presenter.screen(), Screen::Line { .. }) {
        Display::Flex
    } else {
        Display::None
    };
    for (entity, mut node) in &mut buttons {
        if node.display != display {
            node.display = display;
        }
        if display == Display::None && focus.get() == Some(entity) {
            focus.clear();
        }
    }
}

pub(crate) fn restore_focus(
    mut ui: ResMut<Ui>,
    keys: Query<(Entity, &FocusKey, Option<&Node>)>,
    buttons: Query<(), With<Button>>,
    mut focus: ResMut<InputFocus>,
    focus_visible: Res<InputFocusVisible>,
) {
    if focus.get().is_some_and(|e| !buttons.contains(e)) {
        focus.clear();
        ui.focus_first = true;
    }
    if let Some(key) = ui.restore.take()
        && let Some((entity, _, _)) = keys
            .iter()
            .find(|(_, k, n)| k.0 == key && n.is_none_or(|n| n.display != Display::None))
    {
        focus.set(entity, FocusCause::Auto);
        ui.focus_first = false;
    }
    if ui.focus_first
        && focus_visible.0
        && let Some(key) = &ui.first_key
        && let Some((entity, _, _)) = keys
            .iter()
            .find(|(_, k, n)| k.0 == *key && n.is_none_or(|n| n.display != Display::None))
    {
        focus.set(entity, FocusCause::Auto);
    }
    ui.focus_first = false;
}
fn value_text(value: &Value) -> Result<String, String> {
    Ok(match value {
        Value::Bool(v) => v.to_string(),
        Value::Int(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::List(_) | Value::Map(_) => {
            return Err("text bindings require scalar state; expose a scalar view".into());
        }
    })
}
pub(crate) fn checked_text(text: String, source: &str) -> Result<String, String> {
    if text.len() > 4096 {
        Err(format!("{source}: bound text exceeds 4096 bytes"))
    } else {
        Ok(text)
    }
}
pub(crate) fn check_choices(
    choices: &[crate::presenter::ChoiceView],
    source: &str,
) -> Result<(), String> {
    if choices.iter().any(|choice| choice.text.len() > 4096) {
        Err(format!("{source}: choice text exceeds 4096 bytes"))
    } else {
        Ok(())
    }
}
pub(crate) fn validate_display(
    story: &weftora_story::Story,
    state: &weftora_core::State,
    screen: &Screen,
) -> Result<(), String> {
    if let Some(catalogs) = story
        .localization()
        .filter(|c| c.definition.state_key.is_some())
    {
        catalogs.language(state)?;
    }
    if let Screen::Choices(choices) = screen {
        check_choices(choices, "loaded checkpoint")?;
    }
    for (_, definition) in story.screens() {
        let errors = definition.state_errors(state);
        if !errors.is_empty() {
            return Err(errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"));
        }
        for (path, widget) in definition.widgets() {
            if let Some(binding) = &widget.bind {
                let text = match binding {
                    Binding::Localized { localized } => story
                        .localization()
                        .ok_or("localized binding requires localization")?
                        .ui_text(state, localized)?
                        .to_owned(),
                    Binding::State { state: key } => {
                        value_text(state.get(key).map_err(|e| e.to_string())?)?
                    }
                    Binding::Dialogue { dialogue } => match (dialogue, screen) {
                        (DialogueBinding::Text, Screen::Line { text, .. }) => text.clone(),
                        (DialogueBinding::Speaker, Screen::Line { speaker, .. }) => {
                            speaker.clone().unwrap_or_default()
                        }
                        _ => String::new(),
                    },
                };
                checked_text(text, &format!("{}:{path}", definition.file))?;
            }
        }
    }
    Ok(())
}
pub(crate) fn update_bindings(
    mut player: NonSendMut<Player>,
    mut texts: Query<(&BoundText, &mut Text)>,
) {
    // Snapshot once and release the state lock before touching ECS or the runner.
    let snapshot = (|| {
        let state = player
            .state
            .read()
            .map_err(|_| "canonical state lock poisoned".to_string())?;
        let mut values = BTreeMap::new();
        let mut localized = BTreeMap::new();
        for (binding, _) in &texts {
            if let Binding::Localized { localized: key } = &binding.binding {
                let text = player
                    .story
                    .localization()
                    .ok_or("localized binding requires localization")?
                    .ui_text(&state, key)?;
                localized.insert(key.clone(), checked_text(text.to_owned(), &binding.source)?);
            }
            if let Binding::State { state: key } = &binding.binding
                && !values.contains_key(key)
            {
                let value = state
                    .get(key)
                    .map_err(|e| format!("{}: {e}", binding.source))?;
                values.insert(
                    key.clone(),
                    checked_text(value_text(value)?, &binding.source)?,
                );
            }
        }
        Ok::<_, String>((values, localized))
    })();
    let result = (|| {
        let (values, localized) = snapshot?;
        for (binding, mut text) in &mut texts {
            let value = match &binding.binding {
                Binding::State { state } => values[state].clone(),
                Binding::Localized { localized: key } => localized[key].clone(),
                Binding::Dialogue { dialogue } => match (dialogue, player.presenter.screen()) {
                    (DialogueBinding::Text, Screen::Line { text, .. }) => text.clone(),
                    (DialogueBinding::Speaker, Screen::Line { speaker, .. }) => {
                        speaker.clone().unwrap_or_default()
                    }
                    (DialogueBinding::Text, Screen::End) => END_TEXT.into(),
                    _ => String::new(),
                },
            };
            let value = checked_text(value, &binding.source)?;
            if text.0 != value {
                text.0 = value;
            }
        }
        Ok::<_, String>(())
    })();
    if let Err(e) = result {
        player.presenter.fail(e);
        player.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_is_bounded_and_preserves_order() {
        let mut queue = Intents::default();
        for position in 0..100 {
            queue.push(Intent::Select {
                layer: None,
                position,
            });
        }
        let values: Vec<_> = queue.drain().collect();
        assert_eq!(values.len(), 32);
        assert!(matches!(values[0], Intent::Select { position: 0, .. }));
        assert!(matches!(values[31], Intent::Select { position: 31, .. }));
    }
    #[test]
    fn modal_rejects_global_and_lower_layer_input() {
        let ui = Ui {
            layers: vec![
                Layer {
                    id: "base".into(),
                    return_focus: None,
                },
                Layer {
                    id: "overlay".into(),
                    return_focus: Some("base/button".into()),
                },
            ],
            ..default()
        };
        assert!(!ui.accepts(&None));
        assert!(!ui.accepts(&Some("base".into())));
        assert!(ui.accepts(&Some("overlay".into())));
    }
    #[test]
    fn invalid_font_fails_before_bevy_registration() {
        assert!(
            decode_font(
                "bad",
                b"not a font".to_vec(),
                &mut bevy::text::FontCx::default()
            )
            .unwrap_err()
            .contains("asset `bad`")
        );
    }
    #[test]
    fn engine_default_font_passes_validation() {
        decode_font(
            "default",
            bevy::text::DEFAULT_FONT_DATA.to_vec(),
            &mut bevy::text::FontCx::default(),
        )
        .unwrap();
    }
    #[test]
    fn binding_patch_keeps_focused_entity_and_change_notifications() {
        let mut app = crate::tests::input_app();
        app.add_systems(Update, (update_bindings, restore_focus).chain());
        let button = app
            .world_mut()
            .spawn((Button, FocusKey("base/button".into())))
            .id();
        let label = app
            .world_mut()
            .spawn((
                Text::new("old"),
                BoundText {
                    binding: Binding::State {
                        state: "curious".into(),
                    },
                    source: "screen.jsonc:/root/bind".into(),
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(button, FocusCause::Navigated);
        let state = app.world().non_send::<Player>().state.clone();
        state
            .write()
            .unwrap()
            .set("curious", Value::Bool(true))
            .unwrap();
        use bevy::ecs::system::RunSystemOnce;
        app.world_mut().run_system_once(update_bindings).unwrap();
        app.world_mut().run_system_once(restore_focus).unwrap();
        assert_eq!(app.world().get::<Text>(label).unwrap().0, "true");
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));
        assert_eq!(state.write().unwrap().drain_changes().len(), 1);
    }
    #[test]
    fn closing_overlay_restores_saved_focus_key() {
        let mut app = crate::tests::input_app();
        let story = &app.world().non_send::<Player>().story;
        let mut ui = Ui {
            layers: vec![Layer {
                id: "dialogue".into(),
                return_focus: None,
            }],
            ..default()
        };
        ui.navigate(
            &UiAction::Open {
                screen: "details".into(),
            },
            story,
            Some("dialogue/details".into()),
        )
        .unwrap();
        assert!(ui.modal());
        ui.navigate(&UiAction::Close, story, None).unwrap();
        assert_eq!(ui.restore.as_deref(), Some("dialogue/details"));
        assert!(!ui.modal());
        let entity = app
            .world_mut()
            .spawn((Button, FocusKey("dialogue/details".into())))
            .id();
        app.insert_resource(ui).add_systems(Update, restore_focus);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(entity));
    }
    #[test]
    fn widget_systems_build_demo_and_patch_text_without_rebuilding() {
        let mut app = crate::tests::input_app();
        app.init_resource::<Assets<Font>>()
            .init_resource::<bevy::text::FontCx>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    sync_layers,
                    sync_choices,
                    sync_advance_buttons,
                    restore_focus,
                    update_bindings,
                )
                    .chain()
                    .after(crate::drive),
            );
        app.update();
        let world = app.world_mut();
        let text_entities: Vec<_> = world
            .query_filtered::<Entity, With<BoundText>>()
            .iter(world)
            .collect();
        assert!(!text_entities.is_empty());
        let focus = world.resource::<InputFocus>().get();
        assert!(
            focus.is_none(),
            "startup must not acquire focus before keyboard navigation"
        );
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), focus);
        for entity in text_entities {
            assert!(app.world().get::<Text>(entity).is_some());
        }
    }
    fn rendered_app() -> App {
        let mut app = crate::tests::hello_app();
        app.init_resource::<Assets<Font>>()
            .init_resource::<bevy::text::FontCx>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    sync_layers,
                    sync_choices,
                    sync_advance_buttons,
                    restore_focus,
                    update_bindings,
                )
                    .chain()
                    .after(crate::drive),
            );
        app
    }
    fn offer_choices(app: &mut App) {
        let player = &mut *app.world_mut().non_send_mut::<Player>();
        player.presenter.advance();
        player.presenter.advance();
        assert!(matches!(player.presenter.screen(), Screen::Choices(_)));
    }
    #[test]
    fn escape_closing_developer_tools_never_arms_quit_or_dismisses_story_overlay() {
        for modal in [false, true] {
            let mut app = screens_app();
            app.insert_resource(crate::debug_tools::Inspector::new(true));
            if modal {
                activate(&mut app, "menu/pause");
            }
            let before = app.world().non_send::<Player>().presenter.screen().clone();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::F12);
            app.update();
            assert!(app.world().resource::<crate::debug_tools::Inspector>().open);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
            app.update();
            assert!(!app.world().resource::<crate::debug_tools::Inspector>().open);
            assert_eq!(app.world().resource::<Ui>().modal(), modal);
            assert!(
                app.world()
                    .resource::<crate::ExitConfirmation>()
                    .0
                    .is_none()
            );
            assert!(app.world().resource::<Messages<AppExit>>().is_empty());
            assert_eq!(app.world().non_send::<Player>().presenter.screen(), &before);
        }
    }

    #[test]
    fn continue_hides_during_choices_and_returns_for_the_next_line() {
        let mut app = rendered_app();
        app.update();
        let next = {
            let world = app.world_mut();
            world
                .query_filtered::<Entity, With<AdvanceButton>>()
                .single(world)
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(next, FocusCause::Navigated);
        offer_choices(&mut app);
        app.update();
        assert_eq!(
            app.world().get::<Node>(next).unwrap().display,
            Display::None
        );
        assert_ne!(app.world().resource::<InputFocus>().get(), Some(next));
        app.world_mut().non_send_mut::<Player>().presenter.select(0);
        app.update();
        assert_eq!(
            app.world().get::<Node>(next).unwrap().display,
            Display::Flex
        );
    }

    #[test]
    fn escape_dismisses_overlay_without_progression_or_quit_and_restores_focus() {
        let mut app = screens_app();
        app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
        activate(&mut app, "menu/pause");
        let before = app.world().non_send::<Player>().presenter.screen().clone();
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action: UiAction::Advance,
            });
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<Ui>().modal());
        assert_eq!(app.world().non_send::<Player>().presenter.screen(), &before);
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        assert!(
            app.world()
                .resource::<crate::ExitConfirmation>()
                .0
                .is_none()
        );
        let focused = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<FocusKey>(focused).unwrap().0,
            "menu/pause"
        );
    }

    #[test]
    fn space_after_choices_appear_does_not_select_without_keyboard_navigation() {
        let mut app = rendered_app();
        app.update();
        offer_choices(&mut app);
        app.update();
        assert!(app.world().resource::<InputFocus>().get().is_none());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        assert!(matches!(
            app.world().non_send::<Player>().presenter.screen(),
            Screen::Choices(_)
        ));
    }
    #[test]
    fn new_choices_reset_keyboard_focus_to_first_choice() {
        let mut app = rendered_app();
        app.update();
        app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
        offer_choices(&mut app);
        app.update();
        let entity = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<FocusKey>(entity).unwrap().0,
            "dialogue/choices/0"
        );
    }
    #[test]
    fn tab_from_empty_focus_enters_overlay_and_stays_there() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::input_focus::tab_navigation::{NavAction, TabNavigation};
        let mut app = rendered_app();
        app.update();
        let story = app.world().non_send::<Player>().story.clone();
        app.world_mut()
            .resource_mut::<Ui>()
            .navigate(
                &UiAction::Open {
                    screen: "details".into(),
                },
                &story,
                None,
            )
            .unwrap();
        app.update();
        assert!(app.world().resource::<InputFocus>().get().is_none());
        // This fixture omits render plugins; native Bevy propagates visibility in PostUpdate.
        let world = app.world_mut();
        for mut visibility in world.query::<&mut InheritedVisibility>().iter_mut(world) {
            *visibility = InheritedVisibility::VISIBLE;
        }
        let entity = app
            .world_mut()
            .run_system_once(|nav: TabNavigation, focus: Res<InputFocus>| {
                nav.navigate(&focus, NavAction::Next)
            })
            .unwrap()
            .unwrap();
        assert!(
            app.world()
                .get::<FocusKey>(entity)
                .unwrap()
                .0
                .starts_with("details/")
        );
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Navigated);
        for _ in 0..4 {
            let next = app
                .world_mut()
                .run_system_once(|nav: TabNavigation, focus: Res<InputFocus>| {
                    nav.navigate(&focus, NavAction::Next)
                })
                .unwrap()
                .unwrap();
            assert!(
                app.world()
                    .get::<FocusKey>(next)
                    .unwrap()
                    .0
                    .starts_with("details/")
            );
            app.world_mut()
                .resource_mut::<InputFocus>()
                .set(next, FocusCause::Navigated);
        }
    }
    #[test]
    fn choice_updates_canonical_state_bound_label_and_later_yarn_condition() {
        for position in 0..2 {
            let mut app = rendered_app();
            app.update();
            offer_choices(&mut app);
            app.update();
            app.world_mut()
                .resource_mut::<Intents>()
                .push(Intent::Select {
                    layer: None,
                    position,
                });
            app.update();
            let player = app.world().non_send::<Player>();
            let expected = position == 0;
            assert_eq!(
                player.state.read().unwrap().get("curious"),
                Ok(&Value::Bool(expected))
            );
            assert!(
                matches!(player.presenter.screen(), Screen::Line { text, .. } if text.contains(if expected { "now true" } else { "stays false" }))
            );
            let world = app.world_mut();
            for (binding, text) in world.query::<(&BoundText, &Text)>().iter(world) {
                if matches!(&binding.binding, Binding::State { state } if state == "curious") {
                    assert_eq!(text.0, expected.to_string());
                }
            }
        }
    }
    fn screens_app() -> App {
        let story = weftora_story::Story::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/screens-demo"),
        )
        .unwrap();
        let compiled = weftora_yarn_compiler::compile(&story.dialogue_sources().unwrap()).unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        app.init_resource::<Assets<Font>>()
            .init_resource::<bevy::text::FontCx>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    sync_layers,
                    sync_choices,
                    sync_advance_buttons,
                    restore_focus,
                    update_bindings,
                )
                    .chain()
                    .after(crate::drive),
            );
        app.update();
        app
    }
    fn widget(app: &mut App, key: &str) -> Entity {
        let world = app.world_mut();
        world
            .query::<(Entity, &FocusKey)>()
            .iter(world)
            .find_map(|(entity, value)| (value.0 == key).then_some(entity))
            .unwrap_or_else(|| panic!("missing widget {key}"))
    }
    fn activate(app: &mut App, key: &str) {
        let entity = widget(app, key);
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Navigated);
        app.world_mut().trigger(Activate { entity });
        app.update();
    }
    fn assert_tab_group(app: &mut App, prefix: &str) {
        // Native Bevy propagates visibility in PostUpdate; this fixture omits render plugins.
        let world = app.world_mut();
        for mut visibility in world.query::<&mut InheritedVisibility>().iter_mut(world) {
            *visibility = InheritedVisibility::VISIBLE;
        }
        use bevy::ecs::system::RunSystemOnce;
        use bevy::input_focus::tab_navigation::{NavAction, TabNavigation};
        for direction in [NavAction::Next, NavAction::Previous] {
            for _ in 0..6 {
                let next = app
                    .world_mut()
                    .run_system_once(move |nav: TabNavigation, focus: Res<InputFocus>| {
                        nav.navigate(&focus, direction)
                    })
                    .unwrap()
                    .unwrap();
                assert!(
                    app.world()
                        .get::<FocusKey>(next)
                        .unwrap()
                        .0
                        .starts_with(prefix)
                );
                app.world_mut()
                    .resource_mut::<InputFocus>()
                    .set(next, FocusCause::Navigated);
            }
        }
    }
    #[test]
    fn authored_screens_keep_dialogue_and_state_across_navigation_and_modal_input() {
        let mut app = screens_app();
        app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
        let before = app.world().non_send::<Player>().presenter.screen().clone();
        activate(&mut app, "menu/activity");
        assert_eq!(app.world().resource::<Ui>().built, ["activity"]);
        assert_eq!(app.world().non_send::<Player>().presenter.screen(), &before);
        let panel = {
            let world = app.world_mut();
            world
                .query::<(&Node, &BackgroundColor)>()
                .iter(world)
                .any(|(node, bg)| {
                    node.width == percent(45) && bg.0 == Color::from(Srgba::hex("152235").unwrap())
                })
        };
        assert!(
            panel,
            "authored layout and theme must reach Bevy components"
        );
        activate(&mut app, "activity/pause");
        assert!(app.world().resource::<Ui>().modal());
        assert_tab_group(&mut app, "pause/");
        // A stale underlying activation, global progression and digit selection all refuse.
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: Some("activity".into()),
                action: UiAction::SetState {
                    key: "prepared".into(),
                    value: Value::Bool(true),
                },
            });
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action: UiAction::Advance,
            });
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit1);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert_eq!(app.world().non_send::<Player>().presenter.screen(), &before);
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .state
                .read()
                .unwrap()
                .get("prepared"),
            Ok(&Value::Bool(false))
        );
        activate(&mut app, "pause/resume");
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<FocusKey>(focus).unwrap().0,
            "activity/pause"
        );
        activate(&mut app, "activity/prepare");
        let world = app.world_mut();
        assert!(
            world
                .query::<(&BoundText, &Text)>()
                .iter(world)
                .any(|(binding, text)| {
                    matches!(&binding.binding, Binding::State { state } if state == "prepared")
                        && text.0 == "true"
                })
        );
        activate(&mut app, "activity/dialogue");
        assert_eq!(app.world().resource::<Ui>().built, ["dialogue"]);
        activate(&mut app, "dialogue/next");
        assert!(matches!(
            app.world().non_send::<Player>().presenter.screen(),
            Screen::Choices(_)
        ));
        let choices = app.world().non_send::<Player>().presenter.screen().clone();
        activate(&mut app, "dialogue/pause");
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit1);
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Select {
                layer: Some("dialogue".into()),
                position: 0,
            });
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert_eq!(
            app.world().non_send::<Player>().presenter.screen(),
            &choices
        );
        activate(&mut app, "pause/resume");
        activate(&mut app, "dialogue/activity");
        assert_eq!(
            app.world().non_send::<Player>().presenter.screen(),
            &choices
        );
        widget(&mut app, "activity/choices/0");
        activate(&mut app, "activity/menu");
        widget(&mut app, "menu/choices/0");
        assert_eq!(
            app.world().non_send::<Player>().presenter.screen(),
            &choices
        );
        activate(&mut app, "menu/dialogue");
        let entity = widget(&mut app, "dialogue/choices/0");
        app.world_mut().trigger(Activate { entity });
        app.update();
        assert!(
            matches!(app.world().non_send::<Player>().presenter.screen(), Screen::Line { text, .. } if text == "Your prepared kit is ready for the station shift.")
        );
        activate(&mut app, "dialogue/activity");
        let world = app.world_mut();
        assert!(
            world
                .query::<(&BoundText, &Text)>()
                .iter(world)
                .any(|(binding, text)| {
                    matches!(&binding.binding, Binding::State { state } if state == "task")
                        && text.0 == "Station shift"
                })
        );
    }
    #[test]
    fn authored_overlay_return_to_menu_clears_layers_and_rejects_stale_origins() {
        let mut app = screens_app();
        app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
        activate(&mut app, "menu/activity");
        activate(&mut app, "activity/pause");
        activate(&mut app, "pause/menu");
        assert_eq!(app.world().resource::<Ui>().built, ["menu"]);
        assert!(!app.world().resource::<Ui>().modal());
        let focus = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().get::<FocusKey>(focus).unwrap().0,
            "menu/activity"
        );
        let before = app.world().non_send::<Player>().presenter.screen().clone();
        for layer in ["pause", "activity"] {
            app.world_mut()
                .resource_mut::<Intents>()
                .push(Intent::Action {
                    layer: Some(layer.into()),
                    action: UiAction::Advance,
                });
        }
        app.update();
        assert_eq!(app.world().non_send::<Player>().presenter.screen(), &before);
        assert_tab_group(&mut app, "menu/");
    }
}
