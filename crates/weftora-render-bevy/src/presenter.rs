//! Bevy-free presentation logic: turns dialogue events into what the screen shows and what to do next.

pub use weftora_core::presentation::Request as Action;
use weftora_yarn::{DialogueError, DialogueEvent, DialogueRunner};

/// The calls the presenter needs from a dialogue runner. Implemented by [`DialogueRunner`]; tests use a fake.
pub trait EventSource {
    fn start(&mut self, node: &str) -> Result<(), DialogueError>;
    fn next_event(&mut self) -> Result<DialogueEvent, DialogueError>;
    fn choose(&mut self, index: usize) -> Result<(), DialogueError>;
    /// Sources with scene boundaries must change this token when crossing one.
    fn presentation_context(&self) -> u64 {
        0
    }
}

impl EventSource for DialogueRunner {
    fn presentation_context(&self) -> u64 {
        DialogueRunner::presentation_context(self)
    }
    fn start(&mut self, node: &str) -> Result<(), DialogueError> {
        DialogueRunner::start(self, node)
    }
    fn next_event(&mut self) -> Result<DialogueEvent, DialogueError> {
        DialogueRunner::next_event(self)
    }
    fn choose(&mut self, index: usize) -> Result<(), DialogueError> {
        DialogueRunner::choose(self, index)
    }
}

/// A choice the player may pick. `index` is what `choose` expects; position in the list is the 1-based key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceView {
    pub index: usize,
    pub text: String,
}

/// What the dialogue panel should currently show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Line {
        speaker: Option<String>,
        text: String,
    },
    Choices(Vec<ChoiceView>),
    End,
    Error(String),
}

pub const END_TEXT: &str = "The End \u{2014} open the game menu to return to title or quit";

pub struct Presenter<S: EventSource> {
    source: S,
    screen: Screen,
    revision: u64,
    decision_context: Option<(Option<String>, String)>,
}

fn visible_choices(choices: Vec<weftora_yarn::Choice>) -> Vec<ChoiceView> {
    choices
        .into_iter()
        .filter(|c| c.available)
        .map(|c| ChoiceView {
            index: c.index,
            text: c.text,
        })
        .collect()
}

impl Presenter<DialogueRunner> {
    pub(crate) fn refresh_localization(&mut self) {
        if matches!(self.screen, Screen::Error(_) | Screen::End) {
            return;
        }
        let screen = match self.source.refresh_localization() {
            Ok(Some(DialogueEvent::Line { character, text })) => {
                self.decision_context = Some((character.clone(), text.clone()));
                Screen::Line {
                    speaker: character,
                    text,
                }
            }
            Ok(Some(DialogueEvent::Choices(choices))) => {
                if self.decision_context.is_some() {
                    match self.source.retranslate_previous_line() {
                        Ok(Some(DialogueEvent::Line { character, text })) => {
                            self.decision_context = Some((character, text));
                        }
                        Ok(_) => self.decision_context = None,
                        Err(error) => {
                            self.fail(error.to_string());
                            return;
                        }
                    }
                }
                Screen::Choices(visible_choices(choices))
            }
            Ok(_) => return,
            Err(error) => {
                self.fail(error.to_string());
                return;
            }
        };
        // A translation is the same offer; retain its revision and any focused choice entity.
        self.screen = screen;
    }
}

impl<S: EventSource> Presenter<S> {
    /// Holds an unstarted runner while the player shows its title screen.
    pub fn dormant(source: S) -> Self {
        Self {
            source,
            screen: Screen::End,
            revision: 0,
            decision_context: None,
        }
    }

    /// Starts the dialogue at `entrypoint` and runs to the first thing worth showing.
    pub fn new(mut source: S, entrypoint: &str) -> (Self, Vec<Action>) {
        let started = source.start(entrypoint);
        let mut presenter = Self::dormant(source);
        let actions = match started {
            Ok(()) => presenter.run(),
            Err(err) => {
                presenter.fail(err.to_string());
                Vec::new()
            }
        };
        (presenter, actions)
    }

    pub(crate) fn source(&self) -> &S {
        &self.source
    }

    pub(crate) fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// The preceding line for this decision, when it belongs to the same scene context.
    pub fn decision_context(&self) -> Option<(Option<&str>, &str)> {
        if !matches!(self.screen, Screen::Choices(_)) {
            return None;
        }
        self.decision_context
            .as_ref()
            .map(|(speaker, text)| (speaker.as_deref(), text.as_str()))
    }

    /// The user asked to move on (click / Space / Enter). Only meaningful while a line is showing.
    pub fn advance(&mut self) -> Vec<Action> {
        if matches!(self.screen, Screen::Line { .. }) {
            self.run()
        } else {
            Vec::new()
        }
    }

    /// The user picked the `position`-th (0-based) visible choice.
    pub fn select(&mut self, position: usize) -> Vec<Action> {
        let Screen::Choices(choices) = &self.screen else {
            return Vec::new();
        };
        let Some(choice) = choices.get(position) else {
            return Vec::new();
        };
        self.decision_context = None;
        match self.source.choose(choice.index) {
            Ok(()) => self.run(),
            Err(err) => {
                self.fail(err.to_string());
                Vec::new()
            }
        }
    }

    /// Reports a renderer-side failure (for example an asset that could not be loaded) on screen.
    pub fn fail(&mut self, message: String) {
        if matches!(self.screen, Screen::Error(_)) {
            return;
        }
        self.decision_context = None;
        self.screen = Screen::Error(message);
        self.revision = self.revision.wrapping_add(1);
    }

    /// Pulls events until one needs the user, collecting command actions on the way.
    fn run(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        loop {
            let context = self.source.presentation_context();
            let event = self.source.next_event();
            if context != self.source.presentation_context() {
                self.decision_context = None;
            }
            match event {
                Ok(DialogueEvent::Line { character, text }) => {
                    self.decision_context = Some((character.clone(), text.clone()));
                    self.screen = Screen::Line {
                        speaker: character,
                        text,
                    };
                }
                Ok(DialogueEvent::Choices(choices)) => {
                    let visible = visible_choices(choices);
                    if visible.is_empty() {
                        self.fail("the dialogue offered no available choices".to_string());
                    } else {
                        self.screen = Screen::Choices(visible);
                    }
                }
                Ok(DialogueEvent::Command(command)) => match Action::parse(&command) {
                    Ok(action) => {
                        if matches!(
                            action,
                            Action::Background(_)
                                | Action::Show {
                                    slot: weftora_core::presentation::Slot::Background,
                                    ..
                                }
                                | Action::Hide {
                                    slot: weftora_core::presentation::Slot::Background,
                                    ..
                                }
                        ) {
                            self.decision_context = None;
                        }
                        actions.push(action);
                        continue;
                    }
                    Err(message) => self.fail(message),
                },
                Ok(DialogueEvent::End) => {
                    self.decision_context = None;
                    self.screen = Screen::End;
                }
                Err(err) => self.fail(err.to_string()),
            }
            self.revision = self.revision.wrapping_add(1);
            return actions;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use weftora_api::StoryCommand;
    use weftora_yarn::Choice;

    #[derive(Default)]
    struct Fake {
        events: VecDeque<Result<DialogueEvent, DialogueError>>,
        chosen: Vec<usize>,
        started: Vec<String>,
        fail_start: bool,
    }

    impl EventSource for Fake {
        fn start(&mut self, node: &str) -> Result<(), DialogueError> {
            self.started.push(node.to_string());
            if self.fail_start {
                Err(DialogueError::MissingNode(node.to_string()))
            } else {
                Ok(())
            }
        }
        fn next_event(&mut self) -> Result<DialogueEvent, DialogueError> {
            self.events.pop_front().unwrap_or(Ok(DialogueEvent::End))
        }
        fn choose(&mut self, index: usize) -> Result<(), DialogueError> {
            if index > 5 {
                return Err(DialogueError::InvalidChoice(index));
            }
            self.chosen.push(index);
            Ok(())
        }
    }

    fn fake(events: Vec<DialogueEvent>) -> Fake {
        Fake {
            events: events.into_iter().map(Ok).collect(),
            ..Fake::default()
        }
    }

    fn line(who: Option<&str>, text: &str) -> DialogueEvent {
        DialogueEvent::Line {
            character: who.map(str::to_string),
            text: text.to_string(),
        }
    }

    fn cmd(name: &str, args: &[&str]) -> DialogueEvent {
        DialogueEvent::Command(StoryCommand {
            name: name.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        })
    }

    fn choice(index: usize, text: &str, available: bool) -> Choice {
        Choice {
            index,
            text: text.to_string(),
            available,
        }
    }

    fn runner(script: &str) -> DialogueRunner {
        let compiled = weftora_yarn_compiler::compile(&[weftora_story::DialogueSource {
            file: "main.yarn".into(),
            contents: script.into(),
        }])
        .unwrap();
        DialogueRunner::new(
            &compiled,
            std::sync::Arc::new(std::sync::RwLock::new(weftora_core::State::new())),
        )
    }

    #[test]
    fn choices_retain_prompt_and_speaker_but_next_offer_does_not_reuse_it() {
        let (mut presenter, _) = Presenter::new(
            fake(vec![
                line(Some("Guide"), "Which path?"),
                cmd("sound", &["sfx.chime"]),
                DialogueEvent::Choices(vec![choice(0, "Left", true)]),
                DialogueEvent::Choices(vec![choice(0, "Again", true)]),
            ]),
            "Start",
        );
        assert!(presenter.decision_context().is_none());
        presenter.advance();
        assert_eq!(
            presenter.decision_context(),
            Some((Some("Guide"), "Which path?"))
        );
        presenter.select(0);
        assert!(presenter.decision_context().is_none());
    }

    #[test]
    fn node_and_checkpoint_boundaries_clear_prior_prompt() {
        for checkpoint in ["", "<<checkpoint next>>\n"] {
            let script = format!(
                "title: Start\n---\nGuide: Old scene\n<<jump Decision>>\n===\ntitle: Decision\n---\n{checkpoint}-> Begin\n===\n"
            );
            let (mut presenter, _) = Presenter::new(runner(&script), "Start");
            presenter.advance();
            assert!(matches!(presenter.screen(), Screen::Choices(_)));
            assert!(presenter.decision_context().is_none(), "{checkpoint}");
        }
    }

    #[test]
    fn background_change_clears_prior_prompt_even_inside_the_same_node() {
        for command in [
            cmd("background", &["bg.next"]),
            cmd("show", &["background", "bg.next", "0"]),
            cmd("hide", &["background", "0"]),
        ] {
            let (mut presenter, _) = Presenter::new(
                fake(vec![
                    line(None, "Old scene"),
                    command,
                    DialogueEvent::Choices(vec![choice(0, "Begin", true)]),
                ]),
                "Start",
            );
            presenter.advance();
            assert!(matches!(presenter.screen(), Screen::Choices(_)));
            assert!(presenter.decision_context().is_none());
            presenter.fail("failed".into());
            assert!(presenter.decision_context().is_none());
        }
    }

    #[test]
    fn commands_run_before_first_line_and_line_waits() {
        let (p, actions) = Presenter::new(
            fake(vec![
                cmd("background", &["bg.room"]),
                cmd("sound", &["sfx.chime"]),
                line(Some("Guide"), "Hi"),
            ]),
            "Start",
        );
        assert_eq!(p.source.started, ["Start"]);
        assert_eq!(
            actions,
            [
                Action::Background("bg.room".into()),
                Action::Sound("sfx.chime".into())
            ]
        );
        assert_eq!(
            p.screen(),
            &Screen::Line {
                speaker: Some("Guide".into()),
                text: "Hi".into()
            }
        );
    }

    #[test]
    fn dormant_presenter_does_not_start_or_consume_the_story() {
        let presenter = Presenter::dormant(fake(vec![line(None, "First")]));
        assert!(presenter.source.started.is_empty());
        assert_eq!(presenter.source.events.len(), 1);
        assert_eq!(presenter.screen(), &Screen::End);
        assert_eq!(presenter.revision(), 0);
    }

    #[test]
    fn advance_moves_to_next_line_then_end() {
        let (mut p, _) = Presenter::new(fake(vec![line(None, "a"), line(None, "b")]), "S");
        p.advance();
        assert_eq!(
            p.screen(),
            &Screen::Line {
                speaker: None,
                text: "b".into()
            }
        );
        p.advance();
        assert_eq!(p.screen(), &Screen::End);
        assert!(p.advance().is_empty());
        assert_eq!(p.screen(), &Screen::End);
    }

    #[test]
    fn choices_hide_unavailable_and_select_maps_to_runner_index() {
        let (mut p, _) = Presenter::new(
            fake(vec![
                DialogueEvent::Choices(vec![
                    choice(0, "no", false),
                    choice(1, "yes", true),
                    choice(2, "maybe", true),
                ]),
                line(None, "after"),
            ]),
            "S",
        );
        let Screen::Choices(views) = p.screen().clone() else {
            panic!("expected choices");
        };
        assert_eq!(views.iter().map(|c| c.index).collect::<Vec<_>>(), [1, 2]);
        // Advance is ignored while choices show; out-of-range selection is ignored.
        p.advance();
        p.select(5);
        assert!(p.source.chosen.is_empty());
        p.select(1);
        assert_eq!(p.source.chosen, [2]);
        assert!(matches!(p.screen(), Screen::Line { .. }));
    }

    #[test]
    fn no_available_choices_is_an_error() {
        let (p, _) = Presenter::new(
            fake(vec![DialogueEvent::Choices(vec![choice(0, "x", false)])]),
            "S",
        );
        assert!(matches!(p.screen(), Screen::Error(_)));
    }

    #[test]
    fn bad_commands_become_error_screens() {
        for (name, args, needle) in [
            ("teleport", vec!["x"], "unknown command `teleport`"),
            ("background", vec![], "argument count: expected 1"),
            ("sound", vec!["a", "b"], "got 2"),
        ] {
            let (mut p, _) = Presenter::new(fake(vec![cmd(name, &args)]), "S");
            let Screen::Error(msg) = p.screen().clone() else {
                panic!("expected error for {name}");
            };
            assert!(msg.contains(needle), "{msg}");
            assert!(p.advance().is_empty());
            assert!(matches!(p.screen(), Screen::Error(_)));
        }
    }

    #[test]
    fn runner_errors_become_error_screens() {
        let mut source = fake(vec![]);
        source
            .events
            .push_back(Err(DialogueError::Runtime("boom".into())));
        let (p, _) = Presenter::new(source, "S");
        assert_eq!(p.screen(), &Screen::Error("dialogue error: boom".into()));

        let mut source = fake(vec![]);
        source.fail_start = true;
        let (p, _) = Presenter::new(source, "Nope");
        assert_eq!(
            p.screen(),
            &Screen::Error("dialogue node `Nope` does not exist".into())
        );
    }

    #[test]
    fn choose_failure_and_renderer_failure_show_errors() {
        let (mut p, _) = Presenter::new(
            fake(vec![DialogueEvent::Choices(vec![choice(9, "bad", true)])]),
            "S",
        );
        p.select(0);
        assert!(matches!(p.screen(), Screen::Error(_)));

        let (mut p, _) = Presenter::new(fake(vec![line(None, "x")]), "S");
        p.fail("asset missing".into());
        assert_eq!(p.screen(), &Screen::Error("asset missing".into()));
    }
    #[test]
    fn identical_consecutive_choice_offers_have_distinct_revisions() {
        let (mut presenter, _) = Presenter::new(
            fake(vec![
                DialogueEvent::Choices(vec![choice(0, "Again", true)]),
                DialogueEvent::Choices(vec![choice(0, "Again", true)]),
            ]),
            "Start",
        );
        let before = presenter.screen().clone();
        let revision = presenter.revision();
        presenter.select(0);
        assert_eq!(presenter.screen(), &before);
        assert_ne!(presenter.revision(), revision);
    }
}
