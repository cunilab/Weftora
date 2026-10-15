//! Backend-free built-in presentation commands and linear transitions.
use crate::{Scope, State};
use std::collections::BTreeMap;
use std::time::Duration;
use weftora_api::{StoryCommand, Value};

pub const COMMANDS: &[(&str, usize)] = &[
    ("background", 1),
    ("sound", 1),
    ("show", 3),
    ("hide", 2),
    ("move", 6),
    ("music", 2),
    ("ambience", 2),
    ("voice", 2),
    ("effect", 2),
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Slot {
    Background,
    Left,
    Right,
    Cg,
    Overlay,
}
impl Slot {
    pub const ALL: [Self; 5] = [
        Self::Background,
        Self::Left,
        Self::Right,
        Self::Cg,
        Self::Overlay,
    ];
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "background" => Ok(Self::Background),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "cg" => Ok(Self::Cg),
            "overlay" => Ok(Self::Overlay),
            _ => Err(format!("unknown view slot `{s}`")),
        }
    }
    pub fn fill(self) -> bool {
        matches!(self, Self::Background | Self::Cg)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Channel {
    Music,
    Effects,
    Ambience,
    Voice,
}
impl Channel {
    pub const ALL: [Self; 4] = [Self::Music, Self::Effects, Self::Ambience, Self::Voice];
    pub fn key(self) -> &'static str {
        match self {
            Self::Music => "settings.audio.music",
            Self::Effects => "settings.audio.effects",
            Self::Ambience => "settings.audio.ambience",
            Self::Voice => "settings.audio.voice",
        }
    }
    pub fn looping(self) -> bool {
        matches!(self, Self::Music | Self::Ambience)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Background(String),
    Sound(String),
    Show {
        slot: Slot,
        image: String,
        duration: Duration,
    },
    Hide {
        slot: Slot,
        duration: Duration,
    },
    Move {
        slot: Slot,
        pose: Pose,
        duration: Duration,
    },
    Audio {
        channel: Channel,
        asset: Option<String>,
        duration: Duration,
    },
}
impl Request {
    pub fn parse(command: &StoryCommand) -> Result<Self, String> {
        let count = COMMANDS
            .iter()
            .find(|(name, _)| *name == command.name)
            .map(|(_, count)| *count)
            .ok_or_else(|| format!("unknown command `{}`", command.name))?;
        if count != command.args.len() {
            return Err(format!(
                "command `{}` argument count: expected {count}, got {}",
                command.name,
                command.args.len()
            ));
        }
        if command.args.iter().any(|arg| arg.contains('{')) {
            return Err(format!(
                "`{}` needs literal arguments, expressions are not supported",
                command.name
            ));
        }
        let a = &command.args;
        let ms = |s: &str| -> Result<Duration, String> {
            let value = s
                .parse::<u64>()
                .map_err(|_| "duration must be integer milliseconds in 0..=10000")?;
            if value > 10_000 {
                return Err("duration must be integer milliseconds in 0..=10000".into());
            }
            Ok(Duration::from_millis(value))
        };
        let number = |s: &str, min: f32, max: f32| -> Result<f32, String> {
            let value = s
                .parse::<f64>()
                .map_err(|_| format!("invalid number `{s}`"))?;
            if !value.is_finite() || !(f64::from(min)..=f64::from(max)).contains(&value) {
                return Err(format!(
                    "number `{s}` must be finite and within {min}..={max}"
                ));
            }
            Ok(value as f32)
        };
        let result = match command.name.as_str() {
            "background" => Self::Background(a[0].clone()),
            "sound" => Self::Sound(a[0].clone()),
            "show" => Self::Show {
                slot: Slot::parse(&a[0])?,
                image: a[1].clone(),
                duration: ms(&a[2])?,
            },
            "hide" => Self::Hide {
                slot: Slot::parse(&a[0])?,
                duration: ms(&a[1])?,
            },
            "move" => {
                let scale = number(&a[3], 0.0, 4.0)?;
                if scale == 0.0 {
                    return Err("scale must be greater than zero".into());
                }
                Self::Move {
                    slot: Slot::parse(&a[0])?,
                    pose: Pose {
                        x: number(&a[1], -1.0, 2.0)?,
                        y: number(&a[2], -1.0, 2.0)?,
                        scale,
                        opacity: number(&a[4], 0.0, 1.0)?,
                    },
                    duration: ms(&a[5])?,
                }
            }
            name => Self::Audio {
                channel: match name {
                    "music" => Channel::Music,
                    "ambience" => Channel::Ambience,
                    "voice" => Channel::Voice,
                    "effect" => Channel::Effects,
                    _ => unreachable!("command table"),
                },
                asset: (a[0] != "none").then(|| a[0].clone()),
                duration: ms(&a[1])?,
            },
        };
        if result.image().or(result.audio()).is_some_and(str::is_empty) {
            return Err("asset ID must not be empty".into());
        }
        Ok(result)
    }
    pub fn image(&self) -> Option<&str> {
        match self {
            Self::Background(id) | Self::Show { image: id, .. } => Some(id),
            _ => None,
        }
    }
    pub fn audio(&self) -> Option<&str> {
        match self {
            Self::Sound(id)
            | Self::Audio {
                asset: Some(id), ..
            } => Some(id),
            _ => None,
        }
    }
    pub fn requires_media(&self) -> bool {
        !matches!(self, Self::Background(_) | Self::Sound(_))
    }
}

/// Known user-volume settings are optional; absent settings have neutral gain.
/// Values remain in canonical state, and the presentation layer owns their interpretation.
pub fn volumes(state: &State) -> Result<[f32; 5], String> {
    let mut result = [1.0; 5];
    for (i, key) in std::iter::once("settings.audio.master")
        .chain(Channel::ALL.map(Channel::key))
        .enumerate()
    {
        if !state.exists(key) {
            continue;
        }
        if state.scope(key).map_err(|e| e.to_string())? != Scope::Settings {
            return Err(format!("`{key}` must use settings scope"));
        }
        match state.get(key).map_err(|e| e.to_string())? {
            Value::Float(n) if n.is_finite() && (0.0..=1.0).contains(n) => result[i] = *n as f32,
            _ => return Err(format!("`{key}` must be a float in 0..=1")),
        }
    }
    Ok(result)
}
pub fn effective_volume(volumes: &[f32; 5], channel: Channel, envelope: f32) -> f32 {
    volumes[0]
        * volumes[1 + Channel::ALL
            .iter()
            .position(|c| *c == channel)
            .expect("known channel")]
        * envelope
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    from: f32,
    to: f32,
    start: Duration,
    duration: Duration,
}
impl Tween {
    pub fn fixed(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            start: Duration::ZERO,
            duration: Duration::ZERO,
        }
    }
    pub fn new(from: f32, to: f32, now: Duration, duration: Duration) -> Self {
        Self {
            from,
            to,
            start: now,
            duration,
        }
    }
    pub fn sample(self, now: Duration) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }
        let t = (now.saturating_sub(self.start).as_secs_f32() / self.duration.as_secs_f32())
            .clamp(0.0, 1.0);
        self.from + (self.to - self.from) * t
    }
    pub fn retarget(&mut self, to: f32, now: Duration, duration: Duration) {
        *self = Self::new(self.sample(now), to, now, duration);
    }
    pub fn settled(self, now: Duration) -> bool {
        now.saturating_sub(self.start) >= self.duration
    }
    fn cleared(self, now: Duration) -> bool {
        self.to == 0.0 && now.saturating_sub(self.start) >= self.duration
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: f32,
}
impl Pose {
    fn for_slot(slot: Slot) -> Self {
        Self {
            x: match slot {
                Slot::Left => 0.25,
                Slot::Right => 0.75,
                _ => 0.5,
            },
            y: 0.5,
            scale: 1.0,
            opacity: 1.0,
        }
    }
}
#[derive(Debug, Clone)]
struct PoseTween {
    from: Pose,
    to: Pose,
    start: Duration,
    duration: Duration,
}
impl PoseTween {
    fn sample(&self, now: Duration) -> Pose {
        let tween = |from, to| Tween::new(from, to, self.start, self.duration).sample(now);
        Pose {
            x: tween(self.from.x, self.to.x),
            y: tween(self.from.y, self.to.y),
            scale: tween(self.from.scale, self.to.scale),
            opacity: tween(self.from.opacity, self.to.opacity),
        }
    }
}
#[derive(Debug, Clone)]
struct ImageCue {
    asset: String,
    alpha: Tween,
}
#[derive(Debug, Clone)]
struct View {
    pose: PoseTween,
    current: Option<ImageCue>,
    outgoing: Option<ImageCue>,
}
#[derive(Debug, Clone)]
pub struct Views(BTreeMap<Slot, View>);
impl Default for Views {
    fn default() -> Self {
        Self(
            Slot::ALL
                .into_iter()
                .map(|slot| {
                    let pose = Pose::for_slot(slot);
                    (
                        slot,
                        View {
                            pose: PoseTween {
                                from: pose,
                                to: pose,
                                start: Duration::ZERO,
                                duration: Duration::ZERO,
                            },
                            current: None,
                            outgoing: None,
                        },
                    )
                })
                .collect(),
        )
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct ViewSample {
    pub slot: Slot,
    pub outgoing: bool,
    pub asset: String,
    pub pose: Pose,
    pub alpha: f32,
}
impl Views {
    pub fn settled(&self, now: Duration) -> bool {
        self.0.values().all(|view| {
            view.outgoing.is_none()
                && now.saturating_sub(view.pose.start) >= view.pose.duration
                && view
                    .current
                    .as_ref()
                    .is_none_or(|cue| cue.alpha.settled(now))
        })
    }
    pub fn apply(&mut self, request: &Request, now: Duration) {
        let (slot, image, duration) = match request {
            Request::Background(id) => (Slot::Background, Some(id), Duration::ZERO),
            Request::Show {
                slot,
                image,
                duration,
            } => (*slot, Some(image), *duration),
            Request::Hide { slot, duration } => {
                let view = self.0.get_mut(slot).expect("fixed slots");
                for cue in [&mut view.current, &mut view.outgoing]
                    .into_iter()
                    .flatten()
                {
                    cue.alpha.retarget(0.0, now, *duration);
                }
                return;
            }
            Request::Move {
                slot,
                pose,
                duration,
            } => {
                let view = self.0.get_mut(slot).expect("fixed slots");
                let duration = if view.current.is_none() {
                    Duration::ZERO
                } else {
                    *duration
                };
                view.pose = PoseTween {
                    from: view.pose.sample(now),
                    to: *pose,
                    start: now,
                    duration,
                };
                return;
            }
            _ => return,
        };
        let view = self.0.get_mut(&slot).expect("fixed slots");
        let id = image.expect("show image");
        if view.current.as_ref().is_some_and(|c| c.asset == *id) {
            view.current
                .as_mut()
                .unwrap()
                .alpha
                .retarget(1.0, now, duration);
            return;
        }
        view.outgoing = if duration.is_zero() {
            None
        } else {
            view.current.take().map(|mut cue| {
                cue.alpha.retarget(0.0, now, duration);
                cue
            })
        };
        view.current = Some(ImageCue {
            asset: id.clone(),
            alpha: Tween::new(
                if duration.is_zero() { 1.0 } else { 0.0 },
                1.0,
                now,
                duration,
            ),
        });
    }
    pub fn tick(&mut self, now: Duration) {
        for view in self.0.values_mut() {
            for cue in [&mut view.current, &mut view.outgoing] {
                if cue.as_ref().is_some_and(|c| c.alpha.cleared(now)) {
                    *cue = None;
                }
            }
        }
    }
    pub fn samples(&self, now: Duration) -> Vec<ViewSample> {
        self.0
            .iter()
            .flat_map(|(slot, view)| {
                [(true, &view.outgoing), (false, &view.current)]
                    .into_iter()
                    .filter_map(move |(outgoing, cue)| {
                        cue.as_ref().map(|cue| ViewSample {
                            slot: *slot,
                            outgoing,
                            asset: cue.asset.clone(),
                            pose: view.pose.sample(now),
                            alpha: cue.alpha.sample(now) * view.pose.sample(now).opacity,
                        })
                    })
            })
            .collect()
    }
}
#[derive(Debug, Clone)]
pub struct AudioCue {
    pub token: u64,
    pub channel: Channel,
    pub asset: String,
    pub envelope: Tween,
    pub stopping: bool,
}
#[derive(Debug, Clone, Default)]
pub struct Audio {
    pub cues: Vec<AudioCue>,
    next_token: u64,
}
impl Audio {
    pub fn settled(&self, now: Duration) -> bool {
        self.cues
            .iter()
            .all(|cue| !cue.stopping && cue.envelope.settled(now))
    }
    pub fn apply(&mut self, request: &Request, now: Duration) {
        let (channel, asset, duration) = match request {
            Request::Sound(id) => (Channel::Effects, Some(id), Duration::ZERO),
            Request::Audio {
                channel,
                asset,
                duration,
            } => (*channel, asset.as_ref(), *duration),
            _ => return,
        };
        if channel.looping()
            && asset.is_some()
            && self
                .cues
                .iter()
                .any(|cue| cue.channel == channel && !cue.stopping && Some(&cue.asset) == asset)
        {
            return;
        }
        if channel != Channel::Effects || asset.is_none() {
            // Keep only the newest primary before replacing it; at most one outgoing cue.
            self.cues
                .retain(|cue| cue.channel != channel || !cue.stopping);
            for cue in self.cues.iter_mut().filter(|cue| cue.channel == channel) {
                cue.envelope.retarget(0.0, now, duration);
                cue.stopping = true;
            }
        }
        self.tick(now);
        if let Some(asset) = asset {
            self.next_token = self.next_token.wrapping_add(1);
            self.cues.push(AudioCue {
                token: self.next_token,
                channel,
                asset: asset.clone(),
                envelope: Tween::new(
                    if duration.is_zero() { 1.0 } else { 0.0 },
                    1.0,
                    now,
                    duration,
                ),
                stopping: false,
            });
            if channel == Channel::Effects
                && self.cues.iter().filter(|c| c.channel == channel).count() > 8
            {
                let oldest = self.cues.iter().position(|c| c.channel == channel).unwrap();
                self.cues.remove(oldest);
            }
        }
    }
    pub fn tick(&mut self, now: Duration) {
        self.cues
            .retain(|cue| !cue.stopping || !cue.envelope.cleared(now));
    }
    pub fn finished(&mut self, token: u64) {
        self.cues.retain(|cue| cue.token != token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }
    fn parse(name: &str, args: &[&str]) -> Result<Request, String> {
        Request::parse(&StoryCommand {
            name: name.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
        })
    }
    #[test]
    fn parser_checks_every_command_and_boundary() {
        for (name, args) in [
            ("background", vec!["bg"]),
            ("sound", vec!["sfx"]),
            ("show", vec!["left", "hero", "1000"]),
            ("hide", vec!["cg", "0"]),
            ("move", vec!["right", "-1", "2", "4", "0", "10000"]),
            ("music", vec!["none", "0"]),
            ("ambience", vec!["rain", "10"]),
            ("voice", vec!["line", "50"]),
            ("effect", vec!["sfx", "50"]),
        ] {
            assert!(parse(name, &args).is_ok(), "{name}");
            assert!(parse(name, &[]).is_err());
        }
        for (name, args) in [
            ("show", vec!["unknown", "x", "0"]),
            ("show", vec!["left", "x", "10001"]),
            ("hide", vec!["left", "1.5"]),
            ("move", vec!["left", "NaN", "0", "1", "1", "0"]),
            ("move", vec!["left", "0", "0", "0", "1", "0"]),
            ("music", vec!["{state}", "0"]),
            ("sound", vec![""]),
        ] {
            assert!(parse(name, &args).is_err(), "{name}");
        }
        assert!(parse("move", &["left", "0.5", "0.5", "1", "1.00000001", "0"]).is_err());
        assert!(
            parse("unknown", &[])
                .unwrap_err()
                .contains("unknown command")
        );
    }
    #[test]
    fn tween_samples_and_retargets_from_current_value() {
        let mut t = Tween::new(0.0, 1.0, ms(0), ms(1000));
        assert_eq!(t.sample(ms(0)), 0.0);
        assert_eq!(t.sample(ms(500)), 0.5);
        t.retarget(0.0, ms(500), ms(1000));
        assert_eq!(t.sample(ms(1000)), 0.25);
        assert_eq!(t.sample(ms(2000)), 0.0);
    }
    #[test]
    fn views_crossfade_retarget_move_hide_and_bound_outgoing() {
        let mut views = Views::default();
        views.apply(&parse("show", &["left", "a", "1000"]).unwrap(), ms(0));
        assert_eq!(views.samples(ms(500))[0].alpha, 0.5);
        views.apply(&parse("show", &["left", "b", "1000"]).unwrap(), ms(1000));
        let samples = views.samples(ms(1500));
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].alpha, 0.5);
        assert_eq!(samples[1].alpha, 0.5);
        views.apply(&parse("show", &["left", "c", "1000"]).unwrap(), ms(1500));
        assert_eq!(views.samples(ms(1500)).len(), 2);
        assert!(!views.samples(ms(1500)).iter().any(|s| s.asset == "a"));
        views.apply(
            &parse("move", &["left", "0.75", "0.5", "2", "0.5", "1000"]).unwrap(),
            ms(1500),
        );
        let p = views.samples(ms(2000))[0].pose;
        assert_eq!(p.x, 0.5);
        assert_eq!(p.scale, 1.5);
        assert_eq!(p.opacity, 0.75);
        views.apply(&parse("hide", &["left", "1000"]).unwrap(), ms(2000));
        views.tick(ms(3000));
        assert!(views.samples(ms(3000)).is_empty());
    }
    #[test]
    fn audio_replaces_loops_fades_stop_and_caps_effects() {
        let mut audio = Audio::default();
        let play = parse("music", &["a", "1000"]).unwrap();
        audio.apply(&play, ms(0));
        let token = audio.cues[0].token;
        audio.apply(&play, ms(500));
        assert_eq!(audio.cues[0].token, token);
        assert_eq!(audio.cues[0].envelope.sample(ms(500)), 0.5);
        audio.apply(&parse("music", &["b", "1000"]).unwrap(), ms(500));
        assert_eq!(audio.cues.len(), 2);
        audio.apply(&parse("music", &["c", "1000"]).unwrap(), ms(600));
        assert_eq!(audio.cues.len(), 2);
        audio.apply(&parse("music", &["none", "1000"]).unwrap(), ms(700));
        audio.tick(ms(1700));
        assert!(audio.cues.is_empty());
        for _ in 0..20 {
            audio.apply(&parse("effect", &["sfx", "100"]).unwrap(), ms(2000));
        }
        assert_eq!(audio.cues.len(), 8);
        audio.apply(&parse("effect", &["none", "100"]).unwrap(), ms(2100));
        audio.tick(ms(2200));
        assert!(audio.cues.is_empty());
    }
    #[test]
    fn canonical_volume_is_scoped_bounded_and_multiplied() {
        let mut state = State::new();
        assert_eq!(volumes(&state).unwrap(), [1.0; 5]);
        for (key, n) in [
            ("settings.audio.master", 0.5),
            ("settings.audio.music", 0.4),
        ] {
            state
                .declare(
                    key,
                    crate::StateDefinition {
                        default: Value::Float(n),
                        scope: Scope::Settings,
                    },
                )
                .unwrap();
        }
        assert_eq!(
            effective_volume(&volumes(&state).unwrap(), Channel::Music, 0.5),
            0.1
        );
        state
            .set("settings.audio.master", Value::Float(1.1))
            .unwrap();
        assert!(volumes(&state).is_err());
    }
    #[test]
    fn settled_save_gate_tracks_fades_moves_and_stops_but_not_playback() {
        let mut views = Views::default();
        assert!(views.settled(ms(0)));
        views.apply(&parse("show", &["left", "hero", "1000"]).unwrap(), ms(0));
        assert!(!views.settled(ms(999)));
        views.tick(ms(1000));
        assert!(views.settled(ms(1000)));
        views.apply(
            &parse("move", &["left", "0.75", "0.5", "1", "1", "500"]).unwrap(),
            ms(1000),
        );
        assert!(!views.settled(ms(1499)));
        assert!(views.settled(ms(1500)));
        let mut audio = Audio::default();
        audio.apply(&parse("music", &["track", "500"]).unwrap(), ms(0));
        assert!(!audio.settled(ms(499)));
        assert!(audio.settled(ms(500)));
        audio.apply(&parse("sound", &["effect"]).unwrap(), ms(500));
        assert!(audio.settled(ms(500)));
        audio.apply(&parse("music", &["none", "500"]).unwrap(), ms(500));
        assert!(!audio.settled(ms(999)));
        audio.tick(ms(1000));
        assert!(audio.settled(ms(1000)));
    }
}
