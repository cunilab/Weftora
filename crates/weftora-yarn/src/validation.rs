//! Shared pre-activation checks for CLI compilation and player artifacts.
use crate::{CompiledDialogue, check_state};
use weftora_api::Diagnostic;
use weftora_story::Story;

pub fn check_story(story: &Story, dialogue: &CompiledDialogue) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    match story.initial_state() {
        Ok(state) => errors.extend(
            check_state(dialogue, &state)
                .into_iter()
                .map(|error| Diagnostic::new("manifest.jsonc", None, error.to_string())),
        ),
        Err(error) => errors.push(Diagnostic::new(
            "manifest.jsonc",
            None,
            format!("initialState: {error}"),
        )),
    }
    let entrypoint = &story.manifest().entrypoint;
    if !dialogue.node_names().contains(entrypoint) {
        errors.push(Diagnostic::new(
            "manifest.jsonc",
            None,
            format!("entrypoint `{entrypoint}` is not a node in the dialogue files"),
        ));
    }
    let registry = presentation_commands();
    for site in dialogue.command_sites() {
        if !story.manifest().dialogue.contains(&site.file) {
            errors.push(Diagnostic::new(
                "manifest.jsonc",
                None,
                format!(
                    "command provenance file `{}` is not a declared dialogue source (node `{}`)",
                    site.file, site.node
                ),
            ));
            continue;
        }
        if let Some(message) = command_problem(story, &registry, &site.command) {
            errors.push(Diagnostic::new(
                &site.file,
                site.line,
                format!("{message} (node `{}`)", site.node),
            ));
        }
    }
    if errors.is_empty() && !story.manifest().screens.is_empty() {
        // Validate bindings against manifest state plus the same Yarn default initialization as play.
        let state = std::sync::Arc::new(std::sync::RwLock::new(
            story.initial_state().expect("checked above"),
        ));
        let runner = crate::DialogueRunner::new(dialogue, state.clone());
        if let Some(message) = runner.failure {
            errors.push(Diagnostic::new("manifest.jsonc", None, message));
        } else {
            let mut state = state.write().expect("validation owns its state lock");
            for (_, screen) in story.screens() {
                errors.extend(screen.state_errors(&state));
                for (path, widget) in screen.widgets() {
                    if let Some(weftora_story::ui::UiAction::SetState { key, value }) =
                        &widget.action
                        && let Ok(previous) = state.get(key).cloned()
                        && state.set(key, value.clone()).is_ok()
                    {
                        errors.extend(check_state(dialogue, &state).into_iter().map(|e| {
                            Diagnostic::new(&screen.file, None, format!("{path}/action/value: {e}"))
                        }));
                        state
                            .set(key, previous)
                            .expect("restore checked scratch state");
                        // This is a private validation copy, not the runtime notification queue.
                        state.drain_changes();
                    }
                }
            }
        }
    }
    errors
}

pub fn presentation_commands() -> weftora_core::command::CommandRegistry {
    use weftora_core::command::{CommandRegistry, ValueType};
    let mut registry = CommandRegistry::default();
    for name in ["background", "sound"] {
        registry
            .register(name, vec![ValueType::String], ValueType::Bool, |_| {
                Box::pin(async { Ok(weftora_api::Value::Bool(true)) })
            })
            .expect("unique built-in command IDs");
    }
    registry
}

fn command_problem(
    story: &Story,
    registry: &weftora_core::command::CommandRegistry,
    command: &weftora_api::StoryCommand,
) -> Option<String> {
    let arguments: Vec<_> = command
        .args
        .iter()
        .cloned()
        .map(weftora_api::Value::String)
        .collect();
    if let Err(error) = registry.validate(&command.name, &arguments) {
        return Some(error.to_string());
    }
    let id = command.args.first()?;
    if id.contains('{') {
        return Some(format!(
            "`{}` needs a literal asset ID, expressions are not supported",
            command.name
        ));
    }
    let kind = match command.name.as_str() {
        "background" => weftora_story::AssetKind::Image,
        "sound" => weftora_story::AssetKind::Audio,
        _ => unreachable!("validated command schema"),
    };
    if let Err(error) = story.asset_path_of(kind, id) {
        return Some(format!("`{}`: {}", command.name, error.message));
    }
    None
}
