//! Declarative UI schema and validation, with no rendering types.
use serde::Deserialize;
use std::collections::BTreeSet;
use weftora_api::{Diagnostic, Value};
use weftora_core::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenDocument {
    pub screen_schema: u32,
    pub root: Widget,
}
#[derive(Debug, Clone)]
pub struct ScreenDefinition {
    pub file: String,
    pub document: ScreenDocument,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WidgetKind {
    Screen,
    Overlay,
    Panel,
    List,
    Text,
    Image,
    Button,
    ChoiceList,
}
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Binding {
    State { state: String },
    Dialogue { dialogue: DialogueBinding },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DialogueBinding {
    Text,
    Speaker,
}
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "do", deny_unknown_fields)]
pub enum UiAction {
    #[serde(rename = "save")]
    Save,
    #[serde(rename = "load")]
    Load,
    #[serde(rename = "dialogue.advance")]
    Advance,
    #[serde(rename = "state.set")]
    SetState { key: String, value: Value },
    #[serde(rename = "command.call")]
    CallCommand {
        command: String,
        #[serde(default)]
        args: Vec<Value>,
    },
    #[serde(rename = "screen.open")]
    Open { screen: String },
    #[serde(rename = "screen.close")]
    Close,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Widget {
    #[serde(rename = "type")]
    pub kind: WidgetKind,
    pub id: Option<String>,
    #[serde(default)]
    pub layout: Layout,
    #[serde(default)]
    pub children: Vec<Widget>,
    pub text: Option<String>,
    pub bind: Option<Binding>,
    pub image: Option<String>,
    pub font: Option<String>,
    pub size: Option<f32>,
    pub color: Option<String>,
    pub background: Option<String>,
    pub action: Option<UiAction>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    #[serde(default)]
    pub position: Position,
    pub left: Option<Length>,
    pub right: Option<Length>,
    pub top: Option<Length>,
    pub bottom: Option<Length>,
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub padding: Option<f32>,
    pub gap: Option<f32>,
    #[serde(default)]
    pub direction: Direction,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Position {
    #[default]
    Flow,
    Absolute,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    #[default]
    Column,
    Row,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Length {
    Pixels(f32),
    Percent(String),
}
impl Length {
    pub fn value(&self) -> Result<(f32, bool), String> {
        let (value, percent) = match self {
            Self::Pixels(n) => (*n, false),
            Self::Percent(s) => (
                s.strip_suffix('%')
                    .ok_or("length strings must end in %")?
                    .parse::<f32>()
                    .map_err(|_| "invalid percentage")?,
                true,
            ),
        };
        let max = if percent { 100.0 } else { 8192.0 };
        if !value.is_finite() || !(0.0..=max).contains(&value) {
            return Err(format!("length must be finite and within 0..={max}"));
        }
        Ok((value, percent))
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
}
pub fn valid_color(s: &str) -> bool {
    s.starts_with('#') && matches!(s.len(), 7 | 9) && s[1..].bytes().all(|c| c.is_ascii_hexdigit())
}

impl ScreenDefinition {
    pub fn structure_errors(&self) -> Vec<Diagnostic> {
        let mut errors = Vec::new();
        if !matches!(
            self.document.root.kind,
            WidgetKind::Screen | WidgetKind::Overlay
        ) {
            errors.push(Diagnostic::new(
                &self.file,
                None,
                "/root/type: root must be screen or overlay",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut count = 0;
        let mut choices = 0;
        self.walk(
            &self.document.root,
            "/root",
            0,
            &mut ids,
            &mut count,
            &mut choices,
            &mut errors,
        );
        if self.document.root.kind == WidgetKind::Screen && choices != 1 {
            errors.push(Diagnostic::new(
                &self.file,
                None,
                "/root: a dialogue screen must contain exactly one choiceList",
            ));
        }
        if self.document.root.kind == WidgetKind::Overlay && choices != 0 {
            errors.push(Diagnostic::new(
                &self.file,
                None,
                "/root: overlays cannot contain choiceList",
            ));
        }
        errors
    }
    #[allow(clippy::too_many_arguments)]
    fn walk(
        &self,
        w: &Widget,
        path: &str,
        depth: usize,
        ids: &mut BTreeSet<String>,
        count: &mut usize,
        choices: &mut usize,
        errors: &mut Vec<Diagnostic>,
    ) {
        let mut error = |field: &str, message: String| {
            errors.push(Diagnostic::new(
                &self.file,
                None,
                format!("{path}{field}: {message}"),
            ))
        };
        *count += 1;
        if depth > 16 || *count > 512 {
            error("", "UI exceeds 16 nesting levels or 512 widgets".into());
            return;
        }
        if let Some(id) = &w.id {
            if !valid_id(id) {
                error(
                    "/id",
                    "invalid widget ID (lowercase letters, digits, _, . and -, max 64 bytes)"
                        .into(),
                );
            } else if !ids.insert(id.clone()) {
                error("/id", format!("duplicate widget ID `{id}`"));
            }
        } else if depth == 0 || matches!(w.kind, WidgetKind::Button | WidgetKind::ChoiceList) {
            error(
                "/id",
                "ID is required on roots, buttons and choiceList".into(),
            );
        }
        if depth > 0 && matches!(w.kind, WidgetKind::Screen | WidgetKind::Overlay) {
            error("/type", "screen and overlay are root-only widgets".into());
        }
        let container = matches!(
            w.kind,
            WidgetKind::Screen | WidgetKind::Overlay | WidgetKind::Panel | WidgetKind::List
        );
        let label = matches!(w.kind, WidgetKind::Text | WidgetKind::Button);
        let typography = label || w.kind == WidgetKind::ChoiceList;
        if !container && !w.children.is_empty() {
            error("/children", "this widget cannot have children".into());
        }
        if w.children.len() > 128 {
            error("/children", "at most 128 direct children".into());
        }
        if label && (w.text.is_some() == w.bind.is_some()) {
            error("/text", "exactly one of text or bind is required".into());
        }
        if !label && (w.text.is_some() || w.bind.is_some()) {
            error("/text", "text/bind is only valid on text and button".into());
        }
        if w.text.as_ref().is_some_and(|s| s.len() > 4096) {
            error("/text", "literal text exceeds 4096 bytes".into());
        }
        if let Some(Binding::State { state }) = &w.bind
            && let Err(e) = weftora_core::state::validate_key(state)
        {
            error("/bind/state", e.to_string());
        }
        if (w.kind == WidgetKind::Image) != w.image.is_some() {
            error(
                "/image",
                "image requires an asset ID, and other widgets cannot set image".into(),
            );
        }
        if (w.kind == WidgetKind::Button) != w.action.is_some() {
            error(
                "/action",
                "button requires an action, and other widgets cannot set action".into(),
            );
        }
        if !typography && (w.font.is_some() || w.size.is_some() || w.color.is_some()) {
            error(
                "/font",
                "font/size/color are only valid on text, button and choiceList".into(),
            );
        }
        if let Some(size) = w.size
            && (!size.is_finite() || !(6.0..=200.0).contains(&size))
        {
            error(
                "/size",
                "font size must be finite and within 6..=200".into(),
            );
        }
        if w.background.is_some() && !container && w.kind != WidgetKind::Button {
            error(
                "/background",
                "background is only valid on containers and buttons".into(),
            );
        }
        for (field, color) in [("/color", &w.color), ("/background", &w.background)] {
            if let Some(color) = color
                && !valid_color(color)
            {
                error(field, "use #RRGGBB or #RRGGBBAA".into());
            }
        }
        for (field, length) in [
            ("left", &w.layout.left),
            ("right", &w.layout.right),
            ("top", &w.layout.top),
            ("bottom", &w.layout.bottom),
            ("width", &w.layout.width),
            ("height", &w.layout.height),
        ] {
            if let Some(length) = length
                && let Err(e) = length.value()
            {
                error(&format!("/layout/{field}"), e);
            }
        }
        for (field, value) in [("padding", w.layout.padding), ("gap", w.layout.gap)] {
            if let Some(n) = value
                && (!n.is_finite() || !(0.0..=8192.0).contains(&n))
            {
                error(
                    &format!("/layout/{field}"),
                    "must be finite and within 0..=8192".into(),
                );
            }
        }
        if matches!(w.action, Some(UiAction::Close))
            && self.document.root.kind != WidgetKind::Overlay
        {
            error("/action", "screen.close is only valid in overlays".into());
        }
        if let Some(UiAction::SetState { key, value }) = &w.action {
            if let Err(e) = weftora_core::state::validate_key(key) {
                error("/action/key", e.to_string());
            }
            if matches!(value, Value::String(s) if s.len() > 4096) {
                error("/action/value", "literal text exceeds 4096 bytes".into());
            }
            if let Err(e) = value.validate() {
                error("/action/value", e.to_string());
            }
        }
        if w.kind == WidgetKind::ChoiceList {
            *choices += 1;
        }
        for (i, child) in w.children.iter().take(128).enumerate() {
            if *count >= 512 {
                error_count_limit(&self.file, path, errors);
                break;
            }
            self.walk(
                child,
                &format!("{path}/children/{i}"),
                depth + 1,
                ids,
                count,
                choices,
                errors,
            );
        }
    }
    pub fn widgets(&self) -> Vec<(String, &Widget)> {
        fn collect<'a>(w: &'a Widget, path: String, out: &mut Vec<(String, &'a Widget)>) {
            out.push((path.clone(), w));
            for (i, child) in w.children.iter().enumerate() {
                collect(child, format!("{path}/children/{i}"), out);
            }
        }
        let mut out = Vec::new();
        collect(&self.document.root, "/root".into(), &mut out);
        out
    }
    pub fn state_errors(&self, state: &State) -> Vec<Diagnostic> {
        fn check(w: &Widget, path: &str, state: &State, file: &str, out: &mut Vec<Diagnostic>) {
            if let Some(Binding::State { state: key }) = &w.bind {
                let problem = match state.get(key) {
                    Err(e) => Some(e.to_string()),
                    Ok(value) if value.is_collection() => {
                        Some("text bindings require scalar state; expose a scalar view".into())
                    }
                    _ => None,
                };
                if let Some(problem) = problem {
                    out.push(Diagnostic::new(
                        file,
                        None,
                        format!("{path}/bind/state: {problem}"),
                    ));
                }
            }
            if let Some(UiAction::SetState { key, value }) = &w.action {
                match state.get(key) {
                    Err(e) => out.push(Diagnostic::new(
                        file,
                        None,
                        format!("{path}/action/key: {e}"),
                    )),
                    Ok(current) if current.type_name() != value.type_name() => {
                        out.push(Diagnostic::new(
                            file,
                            None,
                            format!(
                                "{path}/action/value: state `{key}` expects {}, got {}",
                                current.type_name(),
                                value.type_name()
                            ),
                        ))
                    }
                    _ => {}
                }
            }
            for (i, child) in w.children.iter().enumerate() {
                check(child, &format!("{path}/children/{i}"), state, file, out);
            }
        }
        let mut out = Vec::new();
        check(&self.document.root, "/root", state, &self.file, &mut out);
        out
    }
}

fn error_count_limit(file: &str, path: &str, errors: &mut Vec<Diagnostic>) {
    errors.push(Diagnostic::new(
        file,
        None,
        format!("{path}: UI exceeds 512 widgets"),
    ));
}

pub(crate) fn load(
    root: &std::path::Path,
    manifest: &crate::Manifest,
) -> Result<weftora_core::catalog::Catalog<ScreenDefinition>, Vec<Diagnostic>> {
    let mut screens = weftora_core::catalog::Catalog::default();
    let mut errors = Vec::new();
    let mut files = BTreeSet::new();
    if manifest.screens.len() > 64 {
        return Err(vec![Diagnostic::new(
            crate::MANIFEST_FILE,
            None,
            "/screens: at most 64 screen files",
        )]);
    }
    for file in &manifest.screens {
        if !files.insert(file) {
            errors.push(Diagnostic::new(
                crate::MANIFEST_FILE,
                None,
                format!("/screens: duplicate source `{file}`"),
            ));
            continue;
        }
        let text = match crate::resolve_in_root(root, file)
            .map_err(|e| e.message)
            .and_then(crate::paths::read_text)
        {
            Ok(text) => text,
            Err(e) => {
                errors.push(Diagnostic::new(file, None, e));
                continue;
            }
        };
        if text.len() > 256 * 1024 {
            errors.push(Diagnostic::new(file, None, "screen file exceeds 256 KiB"));
            continue;
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Header {
            screen_schema: u32,
        }
        let header: Header = match crate::jsonc::from_str(&text) {
            Ok(header) => header,
            Err(e) => {
                errors.push(Diagnostic::new(
                    file,
                    u32::try_from(e.line()).ok().filter(|line| *line > 0),
                    e.to_string(),
                ));
                continue;
            }
        };
        if header.screen_schema != 1 {
            errors.push(Diagnostic::new(
                file,
                None,
                format!(
                    "unsupported screenSchema {}; supported: 1",
                    header.screen_schema
                ),
            ));
            continue;
        }
        let document: ScreenDocument = match crate::jsonc::from_str(&text) {
            Ok(doc) => doc,
            Err(e) => {
                errors.push(Diagnostic::new(
                    file,
                    u32::try_from(e.line()).ok().filter(|line| *line > 0),
                    e.to_string(),
                ));
                continue;
            }
        };
        let definition = ScreenDefinition {
            file: file.clone(),
            document,
        };
        let problems = definition.structure_errors();
        if !problems.is_empty() {
            errors.extend(problems);
            continue;
        }
        let id = definition
            .document
            .root
            .id
            .clone()
            .expect("validated root ID");
        if let Err(e) = screens.insert(&id, definition, file, "/root/id") {
            errors.push(e);
        }
    }
    if errors.is_empty() {
        Ok(screens)
    } else {
        Err(errors)
    }
}

pub(crate) fn reference_errors(story: &crate::Story) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    match (
        &story.manifest().initial_screen,
        story.manifest().screens.is_empty(),
    ) {
        (None, false) => errors.push(Diagnostic::new(
            crate::MANIFEST_FILE,
            None,
            "/initialScreen: required when screens are listed",
        )),
        (Some(_), true) => errors.push(Diagnostic::new(
            crate::MANIFEST_FILE,
            None,
            "/initialScreen: requires screens",
        )),
        (Some(id), false) => match story.screen(id) {
            Ok(screen) if screen.document.root.kind == WidgetKind::Screen => {}
            _ => errors.push(Diagnostic::new(
                crate::MANIFEST_FILE,
                None,
                format!("/initialScreen: `{id}` must name an existing screen (overlays are not valid initial screens)"),
            )),
        },
        _ => {}
    }
    for (_, screen) in story.screens() {
        for (path, widget) in screen.widgets() {
            for (field, id, kind) in [
                ("image", &widget.image, crate::AssetKind::Image),
                ("font", &widget.font, crate::AssetKind::Font),
            ] {
                if let Some(id) = id {
                    match story.asset_kind(id) {
                        Ok(Some(actual)) if actual == kind => {}
                        _ => errors.push(Diagnostic::new(
                            &screen.file,
                            None,
                            format!(
                                "{path}/{field}: `{id}` must name a typed {} catalog asset",
                                kind.category()
                            ),
                        )),
                    }
                }
            }
            if matches!(widget.action, Some(UiAction::Save | UiAction::Load))
                && story.manifest().save_schema.is_none()
            {
                errors.push(Diagnostic::new(
                    &screen.file,
                    None,
                    format!("{path}/action: save/load requires saveSchema and save/1"),
                ));
            }
            if let Some(UiAction::CallCommand { command, args }) = &widget.action
                && let Err(error) = story.rules().validate_command(command, args)
            {
                errors.push(Diagnostic::new(
                    &screen.file,
                    None,
                    format!("{path}/action: {error}"),
                ));
            }
            if let Some(UiAction::Open { screen: id }) = &widget.action
                && story.screen(id).is_err()
            {
                errors.push(Diagnostic::new(
                    &screen.file,
                    None,
                    format!("{path}/action/screen: missing screen ID `{id}`"),
                ));
            }
        }
    }
    errors
}
