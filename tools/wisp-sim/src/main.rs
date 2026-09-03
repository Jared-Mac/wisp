use anyhow::{Context, bail};
use clap::{Parser, ValueEnum};
use futures_util::StreamExt;
use livekit::{
    options::{TrackPublishOptions, VideoCodec},
    prelude::{
        LocalAudioTrack, LocalTrack, LocalVideoTrack, RemoteTrack, Room, RoomEvent, RoomOptions,
        RtcAudioSource, TrackSource,
    },
    webrtc::{
        audio_frame::AudioFrame,
        audio_source::{AudioSourceOptions, native::NativeAudioSource},
        audio_stream::native::NativeAudioStream,
        prelude::{I420Buffer, RtcVideoSource, VideoFrame, VideoResolution, VideoRotation},
        video_source::native::NativeVideoSource,
        video_stream::native::NativeVideoStream,
    },
};
use std::{
    f64::consts::TAU,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;
use url::Url;
use wisp_protocol::{
    DevSession, DevSessionRequest, JoinFriendRequest, KnockId, KnockResponse, LiveKitTokenResponse,
    Presence, RespondKnockRequest, ServerEvent, SetPresenceRequest,
};

#[derive(Debug, Parser)]
#[command(about = "Simulate a Wisp friend from one development machine")]
struct Args {
    #[arg(long)]
    profile: String,
    #[arg(long, env = "WISP_SERVER_URL", default_value = "http://127.0.0.1:8787")]
    server_url: String,
    #[arg(long, default_value = "open")]
    presence: Presence,
    #[arg(long)]
    join: Option<String>,
    #[arg(long, conflicts_with_all = ["publish_file", "silent"])]
    publish_tone: bool,
    #[arg(long, conflicts_with_all = ["publish_tone", "silent"])]
    publish_file: Option<PathBuf>,
    #[arg(long, conflicts_with_all = ["publish_tone", "publish_file"])]
    silent: bool,
    #[arg(long)]
    publish_video: bool,
    #[arg(long, value_enum)]
    auto_respond_knocks: Option<SimKnockResponse>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SimKnockResponse {
    Join,
    Later,
}

impl From<SimKnockResponse> for KnockResponse {
    fn from(value: SimKnockResponse) -> Self {
        match value {
            SimKnockResponse::Join => Self::Accept,
            SimKnockResponse::Later => Self::Later,
        }
    }
}

#[derive(Clone)]
enum AudioSource {
    Tone,
    File(PathBuf),
    Silent,
}

#[derive(Clone)]
struct MediaConfig {
    audio: Option<AudioSource>,
    publish_video: bool,
}

impl Args {
    fn media_config(&self) -> Option<MediaConfig> {
        let audio = if self.publish_tone {
            Some(AudioSource::Tone)
        } else if let Some(path) = &self.publish_file {
            Some(AudioSource::File(path.clone()))
        } else if self.silent {
            Some(AudioSource::Silent)
        } else {
            None
        };
        (audio.is_some() || self.publish_video).then_some(MediaConfig {
            audio,
            publish_video: self.publish_video,
        })
    }
}

struct SimMedia {
    room: Arc<Room>,
    source_tasks: Vec<JoinHandle<()>>,
    event_task: JoinHandle<()>,
}

impl SimMedia {
    async fn connect(
        credentials: LiveKitTokenResponse,
        media_config: MediaConfig,
    ) -> anyhow::Result<Self> {
        let (room, events) =
            Room::connect(&credentials.url, &credentials.token, RoomOptions::default())
                .await
                .with_context(|| format!("connect to LiveKit room {}", credentials.room))?;
        let room = Arc::new(room);

        let mut source_tasks = Vec::new();
        match media_config.audio {
            Some(AudioSource::Tone) => {
                let source = NativeAudioSource::new(AudioSourceOptions::default(), 48_000, 1, 0);
                publish_native_track(&room, "synthetic-tone", &source).await?;
                info!(room = %credentials.room, frequency_hz = 440, "publishing synthetic tone");
                source_tasks.push(tokio::spawn(publish_tone(source)));
            }
            Some(AudioSource::File(path)) => {
                let wav = load_wav(&path)?;
                let source = NativeAudioSource::new(
                    AudioSourceOptions::default(),
                    wav.sample_rate,
                    wav.channels,
                    0,
                );
                publish_native_track(&room, "audio-file", &source).await?;
                info!(room = %credentials.room, file = %path.display(), "publishing looping audio file");
                source_tasks.push(tokio::spawn(publish_wav(source, wav)));
            }
            Some(AudioSource::Silent) | None => {
                info!(room = %credentials.room, "joined LiveKit silently");
            }
        }
        if media_config.publish_video {
            let source = NativeVideoSource::new(
                VideoResolution {
                    width: 640,
                    height: 360,
                },
                false,
            );
            publish_video_track(&room, &source).await?;
            info!(room = %credentials.room, width = 640, height = 360, fps = 15, "publishing synthetic video");
            source_tasks.push(tokio::spawn(publish_test_video(source)));
        }
        let event_task = tokio::spawn(log_room_events(events));
        Ok(Self {
            room,
            source_tasks,
            event_task,
        })
    }

    async fn close(self) {
        if let Err(error) = self.room.close().await {
            warn!(%error, "simulator LiveKit room did not close cleanly");
        }
        for task in self.source_tasks {
            task.abort();
            let _ = task.await;
        }
        self.event_task.abort();
        let _ = self.event_task.await;
    }
}

async fn publish_video_track(room: &Room, source: &NativeVideoSource) -> anyhow::Result<()> {
    let track = LocalVideoTrack::create_video_track(
        "synthetic-video",
        RtcVideoSource::Native(source.clone()),
    );
    room.local_participant()
        .publish_track(
            LocalTrack::Video(track),
            TrackPublishOptions {
                source: TrackSource::Screenshare,
                video_codec: VideoCodec::VP8,
                simulcast: false,
                ..Default::default()
            },
        )
        .await
        .context("publish simulator video")?;
    Ok(())
}

async fn publish_test_video(source: NativeVideoSource) {
    const WIDTH: u32 = 640;
    const HEIGHT: u32 = 360;
    let mut tick = 0_usize;
    let mut interval = tokio::time::interval(Duration::from_millis(1000 / 15));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let mut buffer = I420Buffer::new(WIDTH, HEIGHT);
        let (luma, chroma_u, chroma_v) = buffer.data_mut();
        for (index, pixel) in luma.iter_mut().enumerate() {
            let bar = (index / 80 + tick / 3) % 8;
            *pixel = 32_u8.saturating_add(u8::try_from(bar).unwrap_or(0).saturating_mul(28));
        }
        chroma_u.fill(96_u8.saturating_add(u8::try_from(tick % 64).unwrap_or(0)));
        chroma_v.fill(160_u8.saturating_sub(u8::try_from(tick % 64).unwrap_or(0)));
        source.capture_frame(&VideoFrame::new(VideoRotation::VideoRotation0, buffer));
        tick = tick.wrapping_add(1);
    }
}

async fn publish_native_track(
    room: &Room,
    name: &str,
    source: &NativeAudioSource,
) -> anyhow::Result<()> {
    let track = LocalAudioTrack::create_audio_track(name, RtcAudioSource::Native(source.clone()));
    room.local_participant()
        .publish_track(
            LocalTrack::Audio(track),
            TrackPublishOptions {
                source: TrackSource::Microphone,
                ..Default::default()
            },
        )
        .await
        .context("publish simulator audio")?;
    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
async fn publish_tone(source: NativeAudioSource) {
    const SAMPLE_RATE: u32 = 48_000;
    const SAMPLES_PER_FRAME: u32 = SAMPLE_RATE / 100;
    let mut phase = 0.0_f64;
    let phase_step = TAU * 440.0 / f64::from(SAMPLE_RATE);
    let mut interval = tokio::time::interval(Duration::from_millis(10));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let data = (0..SAMPLES_PER_FRAME)
            .map(|_| {
                let sample = (phase.sin() * f64::from(i16::MAX) * 0.12) as i16;
                phase = (phase + phase_step) % TAU;
                sample
            })
            .collect::<Vec<_>>();
        let frame = AudioFrame {
            data: data.into(),
            sample_rate: SAMPLE_RATE,
            num_channels: 1,
            samples_per_channel: SAMPLES_PER_FRAME,
        };
        if let Err(error) = source.capture_frame(&frame).await {
            warn!(%error, "synthetic tone publication stopped");
            break;
        }
    }
}

#[derive(Clone)]
struct WavData {
    sample_rate: u32,
    channels: u32,
    samples: Vec<i16>,
}

#[allow(clippy::cast_possible_truncation)]
fn load_wav(path: &Path) -> anyhow::Result<WavData> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("open audio file {}", path.display()))?;
    let spec = reader.spec();
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| sample.map(|value| (value * f32::from(i16::MAX)) as i16))
            .collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => {
            let shift = spec.bits_per_sample.saturating_sub(16);
            reader
                .samples::<i32>()
                .map(|sample| {
                    sample.map(|value| {
                        let scaled = if spec.bits_per_sample > 16 {
                            value >> shift
                        } else {
                            value << (16 - spec.bits_per_sample)
                        };
                        scaled.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    anyhow::ensure!(!samples.is_empty(), "audio file contains no samples");
    Ok(WavData {
        sample_rate: spec.sample_rate,
        channels: u32::from(spec.channels),
        samples,
    })
}

async fn publish_wav(source: NativeAudioSource, wav: WavData) {
    let samples_per_channel = wav.sample_rate / 100;
    let Ok(samples_per_frame) = usize::try_from(samples_per_channel * wav.channels) else {
        warn!("audio frame size is unsupported on this platform");
        return;
    };
    let mut position = 0_usize;
    let mut interval = tokio::time::interval(Duration::from_millis(10));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let data = (0..samples_per_frame)
            .map(|offset| wav.samples[(position + offset) % wav.samples.len()])
            .collect::<Vec<_>>();
        position = (position + samples_per_frame) % wav.samples.len();
        let frame = AudioFrame {
            data: data.into(),
            sample_rate: wav.sample_rate,
            num_channels: wav.channels,
            samples_per_channel,
        };
        if let Err(error) = source.capture_frame(&frame).await {
            warn!(%error, "audio file publication stopped");
            break;
        }
    }
}

async fn log_room_events(mut events: tokio::sync::mpsc::UnboundedReceiver<RoomEvent>) {
    while let Some(event) = events.recv().await {
        match event {
            RoomEvent::Reconnecting => warn!("simulator media reconnecting"),
            RoomEvent::Reconnected => info!("simulator media reconnected"),
            RoomEvent::Disconnected { reason } => {
                warn!(?reason, "simulator media disconnected");
                break;
            }
            RoomEvent::TrackSubscribed {
                track: RemoteTrack::Audio(track),
                participant,
                ..
            } => {
                let participant = participant.identity().to_string();
                debug!(%participant, "simulator subscribed to audio");
                tokio::spawn(async move {
                    let mut stream = NativeAudioStream::new(track.rtc_track(), 48_000, 1);
                    if stream.next().await.is_some() {
                        info!(%participant, "simulator received remote audio frames");
                    }
                });
            }
            RoomEvent::TrackSubscribed {
                track: RemoteTrack::Video(track),
                participant,
                ..
            } => {
                let participant = participant.identity().to_string();
                debug!(%participant, "simulator subscribed to video");
                tokio::spawn(async move {
                    let mut stream = NativeVideoStream::new(track.rtc_track());
                    if stream.next().await.is_some() {
                        info!(%participant, "simulator received remote video frames");
                    }
                });
            }
            other => debug!(?other, "simulator LiveKit event"),
        }
    }
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "wisp_sim=info".into()),
        )
        .init();
    let args = Args::parse();
    let media_config = args.media_config();
    let auto_respond_knocks = args.auto_respond_knocks;
    if let Some(MediaConfig {
        audio: Some(AudioSource::File(path)),
        ..
    }) = &media_config
    {
        anyhow::ensure!(
            path.is_file(),
            "audio file does not exist: {}",
            path.display()
        );
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let session_response = client
        .post(format!("{}/v1/dev/session", args.server_url))
        .json(&DevSessionRequest {
            profile: args.profile.clone(),
        })
        .send()
        .await?;
    let session: DevSession = checked_json(session_response).await?;
    checked(
        client
            .post(format!("{}/v1/presence", args.server_url))
            .bearer_auth(&session.token)
            .json(&SetPresenceRequest {
                presence: args.presence,
            })
            .send()
            .await?,
    )
    .await?;
    if let Some(friend) = args.join {
        checked(
            client
                .post(format!("{}/v1/hangouts/join-friend", args.server_url))
                .bearer_auth(&session.token)
                .json(&JoinFriendRequest { friend })
                .send()
                .await?,
        )
        .await?;
    }

    let mut events_url = Url::parse(&args.server_url)?;
    events_url
        .set_scheme(if events_url.scheme() == "https" {
            "wss"
        } else {
            "ws"
        })
        .map_err(|()| anyhow::anyhow!("unsupported server URL"))?;
    events_url.set_path("/v1/events");
    events_url
        .query_pairs_mut()
        .append_pair("token", &session.token);
    let (stream, _) = connect_async(events_url.as_str())
        .await
        .context("connect event stream")?;
    info!(profile = %session.user.display_name, presence = %args.presence, "simulator online; Ctrl+C to stop");
    let (_, mut incoming) = stream.split();
    let mut media = None;
    let mut media_retry = tokio::time::interval(Duration::from_secs(1));
    media_retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            message = incoming.next() => match message {
                Some(Ok(message)) if message.is_text() => {
                    let text = message.to_text().unwrap_or_default();
                    debug!(event = %text, "server event");
                    if let Some(response) = auto_respond_knocks
                        && let Ok(event) = serde_json::from_str::<ServerEvent>(text)
                        && event.name == "knock_requested"
                        && event.payload.get("to")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|id| id.parse().ok()) == Some(session.user.id)
                        && let Some(knock_id) = event.payload.get("knock_id")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|id| id.parse().ok())
                    {
                        respond_to_knock(
                            &client,
                            &args.server_url,
                            &session.token,
                            knock_id,
                            response.into(),
                        ).await?;
                    }
                },
                Some(Ok(message)) if message.is_close() => bail!("server closed event stream"),
                Some(Ok(_)) => {},
                Some(Err(error)) => return Err(error.into()),
                None => bail!("server event stream ended"),
            },
            _ = media_retry.tick(), if media.is_none() && media_config.is_some() => {
                match fetch_livekit_token(&client, &args.server_url, &session.token).await {
                    Ok(Some(credentials)) => {
                        media = Some(SimMedia::connect(
                            credentials,
                            media_config.clone().expect("media config checked"),
                        ).await?);
                    }
                    Ok(None) => debug!("waiting to join a hangout before starting simulator media"),
                    Err(error) => warn!(%error, "simulator media connection is not ready"),
                }
            }
        }
    }

    if let Some(media) = media {
        media.close().await;
    }
    let _ = checked(
        client
            .post(format!("{}/v1/hangouts/leave", args.server_url))
            .bearer_auth(&session.token)
            .send()
            .await?,
    )
    .await;
    info!("simulator offline");
    Ok(())
}

async fn respond_to_knock(
    client: &reqwest::Client,
    server_url: &str,
    token: &str,
    knock_id: KnockId,
    response: KnockResponse,
) -> anyhow::Result<()> {
    checked(
        client
            .post(format!("{server_url}/v1/knocks/respond"))
            .bearer_auth(token)
            .json(&RespondKnockRequest { knock_id, response })
            .send()
            .await?,
    )
    .await?;
    info!(%knock_id, ?response, "automatically responded to knock");
    Ok(())
}

async fn fetch_livekit_token(
    client: &reqwest::Client,
    server_url: &str,
    token: &str,
) -> anyhow::Result<Option<LiveKitTokenResponse>> {
    let response = client
        .post(format!("{server_url}/v1/livekit/token"))
        .bearer_auth(token)
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::BAD_REQUEST {
        return Ok(None);
    }
    checked_json(response).await.map(Some)
}

async fn checked(response: reqwest::Response) -> anyhow::Result<()> {
    if response.status().is_success() {
        return Ok(());
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    bail!("server returned {status}: {body}")
}

async fn checked_json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> anyhow::Result<T> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("server returned {status}: {body}");
    }
    Ok(response.json().await?)
}
