use crate::{Choice, CompiledDialogue, DialogueError, DialogueEvent, DialogueRunner};
use weftora_api::Diagnostic;
use weftora_story::Story;
use yarnspinner_runtime::prelude::{DialogueOption, Language, Line, StringTableTextProvider};

pub(super) enum QueuedEvent {
    Event(DialogueEvent),
    Display(Display),
}

#[derive(Clone)]
pub(super) enum Display {
    Line(Line),
    Choices(Vec<DialogueOption>),
}

impl DialogueRunner {
    pub(super) fn sync_language(&mut self) -> Result<bool, DialogueError> {
        let Some(catalogs) = &self.localization else {
            return Ok(false);
        };
        let language = {
            let state = self
                .state
                .read()
                .map_err(|_| DialogueError::Runtime("state lock is poisoned".into()))?;
            catalogs.language(&state).map_err(DialogueError::Runtime)?
        };
        if self.active_language.as_ref() == Some(&language) {
            return Ok(false);
        }
        let parsed = Language::try_new(&language).map_err(DialogueError::Runtime)?;
        let base = language == catalogs.definition.base_language;
        if !base {
            let provider = self
                .dialogue
                .text_provider_mut()
                .as_any_mut()
                .downcast_mut::<StringTableTextProvider>()
                .ok_or_else(|| {
                    DialogueError::Runtime("localization requires the string table provider".into())
                })?;
            provider.extend_translation(
                parsed.clone(),
                catalogs.languages[&language]
                    .dialogue
                    .iter()
                    .map(|(id, text)| (id.clone().into(), text.clone())),
            );
        }
        self.dialogue.set_language_code(Some(parsed));
        if base {
            self.dialogue.text_provider_mut().set_language(None);
        }
        self.active_language = Some(language);
        Ok(true)
    }

    pub(super) fn render_display(
        &mut self,
        display: &Display,
    ) -> Result<DialogueEvent, DialogueError> {
        self.sync_language()?;
        let mut render = |line: &Line| {
            if self.localization.is_some() {
                self.runtime_call(|dialogue| dialogue.relocalize_line(line))
            } else {
                Ok(line.clone())
            }
        };
        Ok(match display {
            Display::Line(line) => {
                let line = render(line)?;
                DialogueEvent::Line {
                    character: line.character_name().map(str::to_owned),
                    text: line.text_without_character_name(),
                }
            }
            Display::Choices(options) => DialogueEvent::Choices(
                options
                    .iter()
                    .map(|option| {
                        let line = render(&option.line)?;
                        Ok(Choice {
                            index: option.id.0,
                            text: line.text_without_character_name(),
                            available: option.is_available,
                        })
                    })
                    .collect::<Result<_, DialogueError>>()?,
            ),
        })
    }

    /// Retranslates the current blocking snapshot only when the Settings language changes.
    /// No instructions, conditions or host functions are executed; checkpoint capture is retained.
    pub fn refresh_localization(&mut self) -> Result<Option<DialogueEvent>, DialogueError> {
        self.ensure_healthy()?;
        let result = (|| {
            if !self.sync_language()? {
                return Ok(None);
            }
            self.display
                .clone()
                .map(|display| self.render_display(&display))
                .transpose()
        })();
        match result {
            Ok(event) => Ok(event),
            Err(error) => self.fail_with_context(error),
        }
    }
}

pub(super) fn validate(story: &Story, dialogue: &CompiledDialogue) -> Vec<Diagnostic> {
    let Some(catalogs) = story.localization() else {
        return Vec::new();
    };
    let mut errors = Vec::new();
    for language in catalogs.languages.keys() {
        match Language::try_new(language) {
            Ok(parsed) if parsed.to_string() == *language => {},
            _ => errors.push(Diagnostic::new("manifest.jsonc", None,
                format!("localization language {language:?} must be a canonical BCP 47 language identifier"))),
        }
    }
    errors.extend(catalogs.dialogue_errors(&dialogue.strings));
    let Some(key) = catalogs.definition.state_key.as_deref() else {
        return errors;
    };
    if !errors.is_empty() {
        return errors;
    }
    use yarnspinner_core::prelude::{OpCode, OperandValue};
    let mut capture_counts = std::collections::BTreeMap::<String, usize>::new();
    for (name, node) in &dialogue.program.nodes {
        for instruction in &node.instructions {
            let count_operand = match instruction.opcode {
                op if op == OpCode::RunLine as i32 => Some(1),
                op if op == OpCode::AddOption as i32 => Some(2),
                _ => None,
            };
            if let Some(index) = count_operand
                && let Some(OperandValue::StringValue(id)) =
                    instruction.operands.first().and_then(|o| o.value.as_ref())
                && let Some(OperandValue::FloatValue(count)) = instruction
                    .operands
                    .get(index)
                    .and_then(|o| o.value.as_ref())
            {
                // Artifact validation already bounds integer counts to 0..=64. Use the least
                // expansion if a crafted program references one string with different counts.
                capture_counts
                    .entry(id.clone())
                    .and_modify(|n| *n = (*n).min(*count as usize))
                    .or_insert(*count as usize);
            }
            if matches!(instruction.opcode, op if op == OpCode::PushVariable as i32 || op == OpCode::StoreVariable as i32)
                && let Some(OperandValue::StringValue(variable)) =
                    instruction.operands.first().and_then(|o| o.value.as_ref())
                && crate::storage::key_for(variable).ok().as_deref() == Some(key)
            {
                errors.push(Diagnostic::new(dialogue.node_file(name), None, format!("node '{name}' cannot read or write localization Settings key `{key}`; language must not affect gameplay")));
            }
        }
    }
    // Validate grammar with inert values; value-dependent processors run only during play.
    let mut provider = StringTableTextProvider::new();
    provider.extend_base_language(
        dialogue
            .strings
            .iter()
            .map(|(id, text)| (id.clone().into(), text.clone())),
    );
    let state = std::sync::Arc::new(std::sync::RwLock::new(weftora_core::State::new()));
    let mut parser = yarnspinner_runtime::prelude::Dialogue::new(
        Box::new(crate::storage::StateStorage::new(state)),
        Box::new(provider),
    );
    for (language, catalog) in &catalogs.languages {
        let Ok(parsed) = Language::try_new(language) else {
            continue;
        };
        let base = language == &catalogs.definition.base_language;
        if !base {
            parser
                .text_provider_mut()
                .as_any_mut()
                .downcast_mut::<StringTableTextProvider>()
                .expect("constructed provider")
                .extend_translation(
                    parsed.clone(),
                    catalog
                        .dialogue
                        .iter()
                        .map(|(id, text)| (id.clone().into(), text.clone())),
                );
        }
        parser.set_language_code(Some(parsed));
        if base {
            parser.text_provider_mut().set_language(None);
        }
        for id in dialogue.strings.keys() {
            let line = Line {
                id: id.clone().into(),
                text: String::new(),
                attributes: Vec::new(),
                substitutions: vec!["1".into(); capture_counts.get(id).copied().unwrap_or(0)],
            };
            let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                parser.validate_line_syntax(&line)
            }))
            .map_err(|_| "Yarn markup parser panicked on malformed input".to_owned())
            .and_then(|result| result.map_err(|error| error.to_string()));
            if let Err(error) = checked {
                errors.push(Diagnostic::new(
                    &catalogs.definition.languages[language],
                    None,
                    format!("/dialogue/{id}: {error}"),
                ));
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeMap,
        sync::{Arc, RwLock},
    };
    use weftora_api::Value;
    use weftora_core::{Scope, State, StateDefinition};
    use weftora_story::localization::{Catalog, Catalogs, Definition};

    #[test]
    fn provider_round_trips_and_plural_rerender_execute_no_instructions() {
        let state = Arc::new(RwLock::new(State::new()));
        let mut provider = StringTableTextProvider::new();
        let template = "[plural value={0} one=\"one\" other=\"other\"/]";
        provider.extend_base_language([("line:test".into(), template.into())]);
        let mut dialogue = yarnspinner_runtime::prelude::Dialogue::new(
            Box::new(crate::storage::StateStorage::new(state.clone())),
            Box::new(provider),
        );
        let line = Line {
            id: "line:test".into(),
            text: String::new(),
            attributes: Vec::new(),
            substitutions: vec!["1".into()],
        };
        for (language, expected) in [
            ("en", "one"),
            ("id", "other"),
            ("ja", "other"),
            ("id", "other"),
            ("en", "one"),
        ] {
            let language = Language::try_new(language).unwrap();
            dialogue
                .text_provider_mut()
                .as_any_mut()
                .downcast_mut::<StringTableTextProvider>()
                .unwrap()
                .extend_translation(language.clone(), [("line:test".into(), template.into())]);
            dialogue.set_language_code(Some(language));
            assert!(dialogue.text_provider().are_lines_available());
            let rendered = dialogue.relocalize_line(&line).unwrap();
            assert_eq!(rendered.text, expected);
            assert_eq!(rendered.substitutions, line.substitutions);
            assert_eq!(dialogue.last_instruction_count(), 0);
            assert!(state.read().unwrap().iter().next().is_none());
        }
    }

    #[test]
    fn syntax_validation_does_not_evaluate_word_selectors_or_value_dependent_markers() {
        let state = Arc::new(RwLock::new(State::new()));
        let mut provider = StringTableTextProvider::new();
        provider.extend_base_language([(
            "line:test".into(),
            "[select value={0} a=Alpha b=Beta /]".into(),
        )]);
        let mut dialogue = yarnspinner_runtime::prelude::Dialogue::new(
            Box::new(crate::storage::StateStorage::new(state)),
            Box::new(provider),
        );
        let mut line = Line {
            id: "line:test".into(),
            text: String::new(),
            attributes: Vec::new(),
            substitutions: vec!["1".into()],
        };
        dialogue.validate_line_syntax(&line).unwrap();
        line.substitutions[0] = "b".into();
        assert_eq!(dialogue.relocalize_line(&line).unwrap().text, "Beta");
        assert_eq!(dialogue.last_instruction_count(), 0);
    }

    #[test]
    fn queued_snapshots_translate_at_emission_and_clear_after_progress_restart_and_failure() {
        let compiled =
            CompiledDialogue::from_artifact_bytes(include_bytes!("../tests/fixtures/budget.json"))
                .unwrap();
        let mut state = State::new();
        state
            .declare(
                "locale",
                StateDefinition {
                    default: Value::String("en".into()),
                    scope: Scope::Settings,
                },
            )
            .unwrap();
        let state = Arc::new(RwLock::new(state));
        let mut runner = DialogueRunner::new(&compiled, state.clone());
        let languages = [("en", "First"), ("id", "Pertama"), ("ja", "最初")]
            .into_iter()
            .map(|(language, text)| {
                (
                    language.to_owned(),
                    Catalog {
                        localization_schema: 1,
                        ui: BTreeMap::new(),
                        dialogue: BTreeMap::from([("line:budget-first".into(), text.into())]),
                    },
                )
            })
            .collect();
        runner.localization = Some(Catalogs {
            definition: Definition {
                state_key: Some("locale".into()),
                font: Some("unused-model-font".into()),
                base_language: "en".into(),
                languages: BTreeMap::new(),
            },
            languages,
        });
        runner.start("Start").unwrap();
        runner
            .queue
            .push_back(QueuedEvent::Display(Display::Line(Line {
                id: "line:budget-first".into(),
                text: "First".into(),
                attributes: Vec::new(),
                substitutions: Vec::new(),
            })));
        state
            .write()
            .unwrap()
            .set("locale", Value::String("ja".into()))
            .unwrap();
        assert!(
            matches!(runner.next_event().unwrap(),DialogueEvent::Line{text,..} if text=="最初")
        );
        assert_eq!(runner.dialogue.last_instruction_count(), 0);
        runner
            .queue
            .push_back(QueuedEvent::Event(DialogueEvent::Command(
                weftora_api::StoryCommand {
                    name: "background".into(),
                    args: vec!["model".into()],
                },
            )));
        assert!(matches!(
            runner.next_event().unwrap(),
            DialogueEvent::Command(_)
        ));
        assert!(runner.display.is_none());
        state
            .write()
            .unwrap()
            .set("locale", Value::String("id".into()))
            .unwrap();
        assert!(runner.refresh_localization().unwrap().is_none());
        runner.start("Start").unwrap();
        assert!(runner.display.is_none());
        let _: Result<(), _> = runner.fail(DialogueError::Runtime("test failure".into()));
        assert!(runner.display.is_none());
        assert!(runner.refresh_localization().is_err());
    }

    #[test]
    fn retained_substitutions_are_bounded_even_when_not_referenced() {
        let compiled =
            CompiledDialogue::from_artifact_bytes(include_bytes!("../tests/fixtures/budget.json"))
                .unwrap();
        let state = Arc::new(RwLock::new(State::new()));
        let mut runner = DialogueRunner::new(&compiled, state);
        for (substitutions, message) in [
            (vec!["".into(); 65], "substitution count budget (64)"),
            (
                vec!["x".repeat(16384); 2],
                "captured substitution budget (16384)",
            ),
        ] {
            let line = Line {
                id: "line:budget-first".into(),
                text: String::new(),
                attributes: Vec::new(),
                substitutions,
            };
            assert!(
                runner
                    .dialogue
                    .relocalize_line(&line)
                    .unwrap_err()
                    .to_string()
                    .contains(message)
            );
        }
    }
}
