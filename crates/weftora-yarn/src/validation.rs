//! Shared pre-activation checks for CLI compilation and player artifacts.
use crate::{CompiledDialogue, check_state};
use weftora_api::Diagnostic;
use weftora_story::Story;

pub fn check_story(story: &Story, dialogue: &CompiledDialogue) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    match story.initial_state() {
        Ok(state) => {
            errors.extend(
                check_state(dialogue, &state)
                    .into_iter()
                    .map(|error| Diagnostic::new("manifest.jsonc", None, error.to_string())),
            );
            if media_enabled(story)
                && let Err(e) = weftora_core::presentation::volumes(&state)
            {
                errors.push(Diagnostic::new("manifest.jsonc", None, e));
            }
        }
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
        if let Some(message) = command_problem(story, &site.command) {
            errors.push(Diagnostic::new(
                &site.file,
                site.line,
                format!("{message} (node `{}`)", site.node),
            ));
        }
    }
    if errors.is_empty() && (!story.manifest().screens.is_empty() || media_enabled(story)) {
        // Validate bindings against manifest state plus the same Yarn default initialization as play.
        let state = std::sync::Arc::new(std::sync::RwLock::new(
            story.initial_state().expect("checked above"),
        ));
        let runner = crate::DialogueRunner::new(dialogue, state.clone());
        if let Some(message) = runner.failure {
            errors.push(Diagnostic::new("manifest.jsonc", None, message));
        } else {
            let mut state = state.write().expect("validation owns its state lock");
            if media_enabled(story)
                && let Err(e) = weftora_core::presentation::volumes(&state)
            {
                errors.push(Diagnostic::new("manifest.jsonc", None, e));
            }
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
                        if media_enabled(story)
                            && let Err(e) = weftora_core::presentation::volumes(&state)
                        {
                            errors.push(Diagnostic::new(
                                &screen.file,
                                None,
                                format!("{path}/action/value: {e}"),
                            ));
                        }
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

pub fn media_enabled(story: &Story) -> bool {
    story
        .manifest()
        .requires
        .as_ref()
        .is_some_and(|r| r.capabilities.iter().any(|c| c == "presentation.media/1"))
}

pub fn presentation_commands() -> weftora_core::command::CommandRegistry {
    use weftora_core::command::{CommandError, CommandRegistry, ValueType};
    let mut registry = CommandRegistry::default();
    for &(name, arity) in weftora_core::presentation::COMMANDS {
        registry
            .register(
                name,
                vec![ValueType::String; arity],
                ValueType::Bool,
                move |arguments| {
                    let command = weftora_api::StoryCommand {
                        name: name.into(),
                        args: arguments
                            .into_iter()
                            .map(|v| match v {
                                weftora_api::Value::String(s) => s,
                                _ => unreachable!("registry checked strings"),
                            })
                            .collect(),
                    };
                    Box::pin(async move {
                        weftora_core::presentation::Request::parse(&command)
                            .map(|_| weftora_api::Value::Bool(true))
                            .map_err(CommandError::Failed)
                    })
                },
            )
            .expect("unique built-in command IDs");
    }
    registry
}

fn command_problem(story: &Story, command: &weftora_api::StoryCommand) -> Option<String> {
    let request = match weftora_core::presentation::Request::parse(command) {
        Ok(r) => r,
        Err(e) => return Some(e),
    };
    if request.requires_media() && !media_enabled(story) {
        return Some(format!(
            "`{}` requires `presentation.media/1` in requires.capabilities",
            command.name
        ));
    }
    for (id, kind) in [
        (request.image(), weftora_story::AssetKind::Image),
        (request.audio(), weftora_story::AssetKind::Audio),
    ] {
        if let Some(id) = id
            && let Err(e) = story.asset_path_of(kind, id)
        {
            return Some(format!("`{}`: {}", command.name, e.message));
        }
    }
    None
}
