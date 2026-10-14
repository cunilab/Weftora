//! Bounded Bevy assets/sinks for the backend-free presentation model.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use crate::{Player, Screen, decode_image, image_cost, read_asset, validate_audio};
use bevy::audio::{AudioSink, AudioSinkPlayback, AudioSource, Decodable, Volume};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use weftora_core::presentation::{Audio, Request, Slot, Views, effective_volume, volumes};
use weftora_story::{AssetKind, Story};

const VIEW_BUDGET: u64 = 128 * 1024 * 1024;
const AUDIO_BUDGET: u64 = 64 * 1024 * 1024;
struct CachedImage {
    handle: Handle<Image>,
    cost: u64,
    size: UVec2,
}
struct CachedAudio {
    handle: Handle<AudioSource>,
    looping: Option<Handle<LoopAudio>>,
    cost: u64,
}
/// Rodio rewinds the encoded cursor instead of buffering an entire decoded track.
#[derive(Asset, TypePath, Clone, Debug)]
pub(crate) struct LoopAudio(AudioSource);
impl Decodable for LoopAudio {
    type Decoder = rodio::decoder::LoopedDecoder<std::io::Cursor<AudioSource>>;
    fn decoder(&self) -> Self::Decoder {
        rodio::Decoder::builder()
            .with_byte_len(self.0.bytes.len() as u64)
            .with_data(std::io::Cursor::new(self.0.clone()))
            .build_looped()
            .expect("immutable audio passed the same decoder constructor before caching")
    }
}
#[derive(Resource, Default)]
pub(crate) struct Presentation {
    pub now: Duration,
    pub views: Views,
    pub audio: Audio,
    images: BTreeMap<String, CachedImage>,
    sounds: BTreeMap<String, CachedAudio>,
    view_nodes: BTreeMap<(Slot, bool), Entity>,
    audio_nodes: BTreeMap<u64, Entity>,
    image_decoded: u64,
    audio_read: u64,
}
#[derive(Component)]
pub(crate) struct ViewImage;
#[derive(Component)]
pub(crate) struct CueToken(u64);
#[derive(SystemParam)]
pub(crate) struct Media<'w> {
    images: ResMut<'w, Assets<Image>>,
    sounds: ResMut<'w, Assets<AudioSource>>,
    pub presentation: ResMut<'w, Presentation>,
}
impl Media<'_> {
    pub fn begin_frame(&mut self, delta: Duration, failed: bool) {
        self.presentation.image_decoded = 0;
        self.presentation.audio_read = 0;
        if !failed {
            self.presentation.now += delta;
        }
        let now = self.presentation.now;
        self.presentation.views.tick(now);
        self.presentation.audio.tick(now);
    }
    pub fn apply(&mut self, request: &Request, story: &Story) -> Result<(), String> {
        let now = self.presentation.now;
        info!(?request, "presentation request");
        let mut views = self.presentation.views.clone();
        views.apply(request, now);
        views.tick(now);
        let mut audio = self.presentation.audio.clone();
        audio.apply(request, now);
        audio.tick(now);
        let view_ids: BTreeSet<_> = views.samples(now).into_iter().map(|s| s.asset).collect();
        let audio_ids: BTreeSet<_> = audio.cues.iter().map(|s| s.asset.clone()).collect();
        if let Some(id) = request.image()
            && !self.presentation.images.contains_key(id)
        {
            let (ext, bytes) = read_asset(story, id, AssetKind::Image)?;
            let cost = image_cost(id, &ext, &bytes)?;
            let resident: u64 = view_ids
                .iter()
                .filter_map(|id| self.presentation.images.get(id).map(|c| c.cost))
                .sum();
            if resident + cost > VIEW_BUDGET || self.presentation.image_decoded + cost > VIEW_BUDGET
            {
                return Err(format!(
                    "asset `{id}` exceeds the 128 MiB resident/per-frame view budget"
                ));
            }
            let image = decode_image(id, &ext, &bytes)?;
            let size = image.size();
            self.presentation.image_decoded += cost;
            self.presentation.images.insert(
                id.into(),
                CachedImage {
                    handle: self.images.add(image),
                    cost,
                    size,
                },
            );
        }
        if let Some(id) = request.audio()
            && !self.presentation.sounds.contains_key(id)
        {
            let (_, bytes) = read_asset(story, id, AssetKind::Audio)?;
            let cost = bytes.len() as u64;
            let resident: u64 = audio_ids
                .iter()
                .filter_map(|id| self.presentation.sounds.get(id).map(|c| c.cost))
                .sum();
            if resident + cost > AUDIO_BUDGET || self.presentation.audio_read + cost > AUDIO_BUDGET
            {
                return Err(format!(
                    "asset `{id}` exceeds the 64 MiB resident/per-frame audio budget"
                ));
            }
            let bytes = Arc::from(bytes);
            validate_audio(id, &bytes)?;
            self.presentation.audio_read += cost;
            self.presentation.sounds.insert(
                id.into(),
                CachedAudio {
                    handle: self.sounds.add(AudioSource { bytes }),
                    looping: None,
                    cost,
                },
            );
        }
        self.presentation.views = views;
        self.presentation.audio = audio;
        self.presentation
            .images
            .retain(|id, _| view_ids.contains(id));
        self.presentation
            .sounds
            .retain(|id, _| audio_ids.contains(id));
        Ok(())
    }
}
fn view_node(sample: &weftora_core::presentation::ViewSample, size: UVec2, viewport: Vec2) -> Node {
    let reference = (viewport.x / 1280.0).min(viewport.y / 720.0);
    let image_size = if sample.slot.fill() {
        viewport
    } else {
        size.as_vec2() * reference
    } * sample.pose.scale;
    Node {
        position_type: PositionType::Absolute,
        left: px(sample.pose.x * viewport.x - image_size.x * 0.5),
        top: px(sample.pose.y * viewport.y - image_size.y * 0.5),
        width: px(image_size.x),
        height: px(image_size.y),
        ..default()
    }
}
pub(crate) fn sync_views(
    mut presentation: ResMut<Presentation>,
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut nodes: Query<(&mut Node, &mut ImageNode), With<ViewImage>>,
) {
    let viewport = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(1280.0, 720.0));
    let samples = presentation.views.samples(presentation.now);
    let wanted: BTreeSet<_> = samples.iter().map(|s| (s.slot, s.outgoing)).collect();
    let stale: Vec<_> = presentation
        .view_nodes
        .keys()
        .filter(|key| !wanted.contains(key))
        .copied()
        .collect();
    for key in stale {
        if let Some(entity) = presentation.view_nodes.remove(&key) {
            commands.entity(entity).despawn();
        }
    }
    for sample in samples {
        let key = (sample.slot, sample.outgoing);
        let asset = &presentation.images[&sample.asset];
        let node = view_node(&sample, asset.size, viewport);
        let image = ImageNode {
            image: asset.handle.clone(),
            color: Color::srgba(1.0, 1.0, 1.0, sample.alpha),
            ..default()
        };
        if let Some(entity) = presentation.view_nodes.get(&key)
            && let Ok((mut current_node, mut current_image)) = nodes.get_mut(*entity)
        {
            current_node.set_if_neq(node);
            if current_image.image != image.image || current_image.color != image.color {
                *current_image = image;
            }
        } else {
            let entity = commands
                .spawn((
                    ViewImage,
                    node,
                    image,
                    Pickable::IGNORE,
                    GlobalZIndex(1 + sample.slot as i32 * 2 + i32::from(!sample.outgoing)),
                ))
                .id();
            presentation.view_nodes.insert(key, entity);
        }
    }
    let ids: BTreeSet<_> = presentation
        .views
        .samples(presentation.now)
        .into_iter()
        .map(|s| s.asset)
        .collect();
    presentation.images.retain(|id, _| ids.contains(id));
}
pub(crate) fn sync_audio(
    mut player: NonSendMut<Player>,
    mut presentation: ResMut<Presentation>,
    mut commands: Commands,
    mut sinks: Query<(
        Entity,
        &CueToken,
        &mut PlaybackSettings,
        Option<&mut AudioSink>,
    )>,
    sources: Res<Assets<AudioSource>>,
    mut loops: ResMut<Assets<LoopAudio>>,
) {
    let gains = if matches!(player.presenter.screen(), Screen::Error(_)) {
        Ok([0.0; 5])
    } else if weftora_yarn::media_enabled(&player.story) {
        // The read guard ends here, before touching sinks or calling the presenter.
        player
            .state
            .read()
            .map_err(|_| "canonical state lock poisoned".to_string())
            .and_then(|state| volumes(&state))
    } else {
        Ok([1.0; 5])
    };
    let gains = match gains {
        Ok(gains) => gains,
        Err(e) => {
            player.presenter.fail(e);
            player.dirty = true;
            [0.0; 5]
        }
    };
    if matches!(player.presenter.screen(), Screen::Error(_)) {
        presentation.audio.cues.clear();
    }
    let wanted: BTreeSet<_> = presentation
        .audio
        .cues
        .iter()
        .map(|cue| cue.token)
        .collect();
    let mut finished = Vec::new();
    for (entity, token, mut settings, sink) in &mut sinks {
        if !wanted.contains(&token.0) {
            if let Some(sink) = sink {
                sink.stop();
            }
            commands.entity(entity).despawn();
            presentation.audio_nodes.remove(&token.0);
            continue;
        }
        let cue = presentation
            .audio
            .cues
            .iter()
            .find(|cue| cue.token == token.0)
            .expect("wanted cue");
        let gain = Volume::Linear(effective_volume(
            &gains,
            cue.channel,
            cue.envelope.sample(presentation.now),
        ));
        settings.volume = gain;
        if let Some(mut sink) = sink {
            sink.set_volume(gain);
            if sink.empty() {
                finished.push(token.0);
                commands.entity(entity).despawn();
            }
        }
    }
    for token in finished {
        presentation.audio.finished(token);
        presentation.audio_nodes.remove(&token);
    }
    let cues = presentation.audio.cues.clone();
    for cue in cues {
        if presentation.audio_nodes.contains_key(&cue.token) {
            continue;
        }
        let settings = PlaybackSettings {
            volume: Volume::Linear(effective_volume(
                &gains,
                cue.channel,
                cue.envelope.sample(presentation.now),
            )),
            ..PlaybackSettings::ONCE
        };
        let cached = presentation
            .sounds
            .get_mut(&cue.asset)
            .expect("validated retained source");
        let entity = if cue.channel.looping() {
            let handle = if let Some(handle) = &cached.looping {
                handle.clone()
            } else {
                let source = sources
                    .get(&cached.handle)
                    .expect("cache owns its strong handle")
                    .clone();
                let handle = loops.add(LoopAudio(source));
                cached.looping = Some(handle.clone());
                handle
            };
            commands
                .spawn((
                    CueToken(cue.token),
                    AudioPlayer::<LoopAudio>(handle),
                    settings,
                ))
                .id()
        } else {
            commands
                .spawn((
                    CueToken(cue.token),
                    AudioPlayer::new(cached.handle.clone()),
                    settings,
                ))
                .id()
        };
        presentation.audio_nodes.insert(cue.token, entity);
    }
    let ids: BTreeSet<_> = presentation
        .audio
        .cues
        .iter()
        .map(|s| s.asset.clone())
        .collect();
    presentation.sounds.retain(|id, _| ids.contains(id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use weftora_core::presentation::{Channel, Pose, ViewSample};
    #[test]
    fn native_size_center_position_scale_and_alpha_are_drawn() {
        let sample = ViewSample {
            slot: Slot::Left,
            outgoing: false,
            asset: "hero".into(),
            pose: Pose {
                x: 0.25,
                y: 0.5,
                scale: 2.0,
                opacity: 0.5,
            },
            alpha: 0.5,
        };
        let node = view_node(&sample, UVec2::new(100, 200), Vec2::new(1280.0, 720.0));
        assert_eq!(node.left, px(220.0));
        assert_eq!(node.top, px(160.0));
        assert_eq!(node.width, px(200.0));
        assert_eq!(node.height, px(400.0));
    }
    fn demo_app() -> App {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/presentation-demo");
        let story = Story::load(&root).unwrap();
        let compiled = weftora_yarn::CompiledDialogue::from_artifact_bytes(include_bytes!(
            "../tests/fixtures/presentation.json"
        ))
        .unwrap();
        compiled
            .verify_sources(&story.dialogue_sources().unwrap())
            .unwrap();
        assert!(weftora_yarn::check_story(&story, &compiled).is_empty());
        let mut app = crate::tests::app_for(story, compiled);
        app.init_resource::<Assets<LoopAudio>>()
            .add_systems(Update, (sync_views, sync_audio).chain().after(crate::drive));
        app
    }
    #[test]
    fn adapter_samples_fades_and_applies_canonical_volume_before_a_sink_exists() {
        use crate::declarative::{Intent, Intents};
        use weftora_story::ui::UiAction;
        let mut app = demo_app();
        app.update();
        let token = app
            .world()
            .resource::<Presentation>()
            .audio
            .cues
            .iter()
            .find(|c| c.channel == Channel::Music)
            .unwrap()
            .token;
        let entity = app.world().resource::<Presentation>().audio_nodes[&token];
        assert_eq!(
            app.world().get::<PlaybackSettings>(entity).unwrap().volume,
            Volume::Linear(0.0)
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(500));
        app.update();
        assert_eq!(
            app.world().get::<PlaybackSettings>(entity).unwrap().volume,
            Volume::Linear(0.25)
        );
        let character = app.world().resource::<Presentation>().view_nodes[&(Slot::Left, false)];
        assert_eq!(
            app.world()
                .get::<ImageNode>(character)
                .unwrap()
                .color
                .to_srgba()
                .alpha,
            1.0
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::ZERO);
        app.world_mut()
            .resource_mut::<Intents>()
            .push(Intent::Action {
                layer: None,
                action: UiAction::SetState {
                    key: "settings.audio.master".into(),
                    value: weftora_api::Value::Float(0.0),
                },
            });
        app.update();
        assert_eq!(
            app.world().get::<PlaybackSettings>(entity).unwrap().volume,
            Volume::Linear(0.0)
        );
        assert_eq!(
            app.world()
                .non_send::<Player>()
                .state
                .read()
                .unwrap()
                .get("settings.audio.master"),
            Ok(&weftora_api::Value::Float(0.0))
        );
    }
    #[test]
    fn many_decodes_in_one_frame_fail_with_controlled_budget_error() {
        let mut app = demo_app();
        app.world_mut().non_send_mut::<Player>().pending = (0..20)
            .map(|i| Request::Background(if i % 2 == 0 { "bg.day" } else { "bg.night" }.into()))
            .collect();
        app.update();
        assert!(
            matches!(app.world().non_send::<Player>().presenter.screen(),Screen::Error(e) if e.contains("per-frame view budget"))
        );
        assert!(app.world().resource::<Presentation>().image_decoded <= VIEW_BUDGET);
    }
    #[test]
    fn effect_burst_has_at_most_eight_entities_and_one_shared_source() {
        let mut app = demo_app();
        app.world_mut().non_send_mut::<Player>().pending = (0..20)
            .map(|_| Request::Sound("audio.effect".into()))
            .collect();
        app.update();
        let presentation = app.world().resource::<Presentation>();
        assert_eq!(presentation.audio.cues.len(), 8);
        assert_eq!(presentation.audio_nodes.len(), 8);
        assert_eq!(presentation.sounds.len(), 1);
    }
    #[test]
    fn invalid_live_volume_fails_once_and_keeps_the_original_error() {
        let mut app = demo_app();
        app.update();
        app.world()
            .non_send::<Player>()
            .state
            .write()
            .unwrap()
            .set("settings.audio.master", weftora_api::Value::Float(2.0))
            .unwrap();
        app.update();
        let revision = app.world().non_send::<Player>().presenter.revision();
        let screen = app.world().non_send::<Player>().presenter.screen().clone();
        assert!(matches!(&screen,Screen::Error(e) if e.contains("float in 0..=1")));
        app.update();
        assert_eq!(
            app.world().non_send::<Player>().presenter.revision(),
            revision
        );
        assert_eq!(app.world().non_send::<Player>().presenter.screen(), &screen);
        app.world_mut()
            .non_send_mut::<Player>()
            .presenter
            .fail("earlier asset failure".into());
        app.update();
        assert_eq!(
            app.world().non_send::<Player>().presenter.screen(),
            &Screen::Error("earlier asset failure".into())
        );
    }
    #[test]
    fn crossfade_completion_despawns_outgoing_entity_and_releases_cache() {
        let mut app = demo_app();
        app.update();
        app.world_mut().non_send_mut::<Player>().pending = vec![Request::Show {
            slot: Slot::Background,
            image: "bg.night".into(),
            duration: Duration::from_millis(500),
        }];
        app.update();
        let old = app.world().resource::<Presentation>().view_nodes[&(Slot::Background, true)];
        let world = app.world_mut();
        for (pick, z) in world
            .query_filtered::<(&Pickable, &GlobalZIndex), With<ViewImage>>()
            .iter(world)
        {
            assert_eq!(*pick, Pickable::IGNORE);
            assert!(z.0 < 20);
        }
        assert!(
            world
                .resource::<Presentation>()
                .images
                .contains_key("bg.day")
        );
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(500));
        app.update();
        assert!(app.world().get_entity(old).is_err());
        assert!(
            !app.world()
                .resource::<Presentation>()
                .images
                .contains_key("bg.day")
        );
        assert!(
            !app.world()
                .resource::<Presentation>()
                .view_nodes
                .contains_key(&(Slot::Background, true))
        );
    }
    #[test]
    fn music_uses_streamed_loop_source_and_rewinds_without_pcm_buffering() {
        let mut app = demo_app();
        app.update();
        let presentation = app.world().resource::<Presentation>();
        let token = presentation
            .audio
            .cues
            .iter()
            .find(|cue| cue.channel == Channel::Music)
            .unwrap()
            .token;
        let entity = presentation.audio_nodes[&token];
        assert!(app.world().get::<AudioPlayer<LoopAudio>>(entity).is_some());
        assert!(
            app.world()
                .get::<AudioPlayer<AudioSource>>(entity)
                .is_none()
        );
        assert!(matches!(
            app.world().get::<PlaybackSettings>(entity).unwrap().mode,
            bevy::audio::PlaybackMode::Once
        ));
        let source = LoopAudio(AudioSource {
            bytes: Arc::from(
                include_bytes!("../../../examples/presentation-demo/assets/music.wav").as_slice(),
            ),
        });
        let mut looped = source.decoder();
        let first: Vec<_> = looped.by_ref().take(22050).collect();
        let second: Vec<_> = looped.take(22050).collect();
        assert_eq!(first.len(), 22050);
        assert_eq!(first, second);
    }
}
