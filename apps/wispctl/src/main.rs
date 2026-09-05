use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
use wisp_protocol::{
    AudioPreset, CommandEnvelope, DaemonEnvelope, Presence, VideoCodecPreference,
    VideoQualityPreset, VideoSource,
};

#[derive(Debug, Parser)]
#[command(about = "Control the local Wisp daemon")]
struct Args {
    #[arg(long, env = "WISP_SOCKET")]
    socket: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Status,
    Privacy {
        #[command(subcommand)]
        command: PrivacyCommand,
    },
    Presence {
        state: Presence,
    },
    Join {
        friend: String,
    },
    JoinHangout {
        id: String,
    },
    JoinRoom {
        room: String,
    },
    Dm {
        friend: String,
        text: String,
    },
    Message {
        conversation_id: String,
        text: String,
    },
    Read {
        conversation_id: String,
    },
    Invite {
        profile: String,
        #[arg(long, default_value_t = 30)]
        expires_minutes: u32,
    },
    Devices,
    RevokeDevice {
        device_id: String,
    },
    Knock {
        id: String,
        action: KnockAction,
    },
    Mute,
    Unmute,
    Deafen,
    Undeafen,
    Surface {
        #[command(subcommand)]
        command: SurfaceCommand,
    },
    Audio {
        #[command(subcommand)]
        command: AudioCommand,
    },
    Video {
        #[command(subcommand)]
        command: VideoCommand,
    },
    Camera {
        #[command(subcommand)]
        command: CameraCommand,
    },
    Watch {
        participant: String,
        #[arg(default_value = "screen_share")]
        source: VideoSource,
    },
    Unwatch {
        participant: String,
        #[arg(default_value = "screen_share")]
        source: VideoSource,
    },
    Ptt {
        #[command(subcommand)]
        command: PushToTalkCommand,
    },
    Leave,
    Share {
        /// Compatibility label; Wisp always opens the desktop portal picker.
        #[arg(default_value = "portal")]
        source: String,
    },
    StopShare,
}

#[derive(Debug, Subcommand)]
enum SurfaceCommand {
    Open,
    Close,
}

#[derive(Debug, Subcommand)]
enum PrivacyCommand {
    Status,
    Enable {
        #[arg(long)]
        backup_file: PathBuf,
        #[arg(long)]
        recovery_file: Option<PathBuf>,
    },
    Export {
        #[arg(long)]
        backup_file: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum AudioCommand {
    Devices,
    Refresh,
    Input { id: String },
    Output { id: String },
    Preset { preset: AudioPreset },
    Strength { strength: u8 },
}

#[derive(Debug, Subcommand)]
enum VideoCommand {
    Devices,
    Refresh,
    Camera { id: String },
    Quality { quality: VideoQualityPreset },
    Codec { codec: VideoCodecPreference },
}

#[derive(Debug, Subcommand)]
enum CameraCommand {
    On,
    Off,
}

#[derive(Debug, Subcommand)]
enum PushToTalkCommand {
    Enable,
    Disable,
    Press,
    Release,
    Shortcut { shortcut: String },
    ClearShortcut,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum KnockAction {
    Join,
    Later,
}

impl Command {
    #[allow(clippy::too_many_lines)]
    fn envelope(&self) -> CommandEnvelope {
        let (name, args) = match self {
            Self::Status => ("status", json!({})),
            Self::Privacy { command } => match command {
                PrivacyCommand::Status => ("privacy_status", json!({})),
                PrivacyCommand::Enable {
                    backup_file,
                    recovery_file,
                } => (
                    "privacy_enable",
                    json!({"backup_file":backup_file,"recovery_file":recovery_file}),
                ),
                PrivacyCommand::Export { backup_file } => {
                    ("privacy_export", json!({"backup_file":backup_file}))
                }
            },
            Self::Presence { state } => ("set_presence", json!({"presence": state})),
            Self::Join { friend } => ("join_friend", json!({"friend": friend})),
            Self::JoinHangout { id } => ("join_hangout", json!({"hangout_id": id})),
            Self::JoinRoom { room } => ("join_spot", json!({"spot_id": room})),
            Self::Dm { friend, text } => ("send_direct", json!({"friend": friend, "text": text})),
            Self::Message {
                conversation_id,
                text,
            } => (
                "send_message",
                json!({"conversation_id": conversation_id, "text": text}),
            ),
            Self::Read { conversation_id } => (
                "mark_conversation_read",
                json!({"conversation_id": conversation_id}),
            ),
            Self::Invite {
                profile,
                expires_minutes,
            } => (
                "create_invite",
                json!({"profile": profile, "expires_in_minutes": expires_minutes}),
            ),
            Self::Devices => ("list_devices", json!({})),
            Self::RevokeDevice { device_id } => ("revoke_device", json!({"device_id": device_id})),
            Self::Knock { id, action } => (
                "respond_knock",
                json!({
                    "knock_id": id,
                    "response": match action {
                        KnockAction::Join => "accept",
                        KnockAction::Later => "later",
                    }
                }),
            ),
            Self::Mute => ("set_muted", json!({"muted": true})),
            Self::Unmute => ("set_muted", json!({"muted": false})),
            Self::Deafen => ("set_deafened", json!({"deafened": true})),
            Self::Undeafen => ("set_deafened", json!({"deafened": false})),
            Self::Surface {
                command: SurfaceCommand::Open,
            } => ("open_surface", json!({})),
            Self::Surface {
                command: SurfaceCommand::Close,
            } => ("close_surface", json!({})),
            Self::Audio {
                command: AudioCommand::Devices | AudioCommand::Refresh,
            } => ("refresh_audio_devices", json!({})),
            Self::Audio {
                command: AudioCommand::Input { id },
            } => ("set_input_device", json!({"id": id})),
            Self::Audio {
                command: AudioCommand::Output { id },
            } => ("set_output_device", json!({"id": id})),
            Self::Audio {
                command: AudioCommand::Preset { preset },
            } => ("set_audio_preset", json!({"preset": preset})),
            Self::Audio {
                command: AudioCommand::Strength { strength },
            } => ("set_deepfilter_strength", json!({"strength": strength})),
            Self::Video {
                command: VideoCommand::Devices | VideoCommand::Refresh,
            } => ("refresh_video_devices", json!({})),
            Self::Video {
                command: VideoCommand::Camera { id },
            } => ("set_camera_device", json!({"id": id})),
            Self::Video {
                command: VideoCommand::Quality { quality },
            } => ("set_video_quality", json!({"quality": quality})),
            Self::Video {
                command: VideoCommand::Codec { codec },
            } => ("set_video_codec", json!({"codec": codec})),
            Self::Camera {
                command: CameraCommand::On,
            } => ("camera", json!({"enabled": true})),
            Self::Camera {
                command: CameraCommand::Off,
            } => ("camera", json!({"enabled": false})),
            Self::Watch {
                participant,
                source,
            } => (
                "watch_video",
                json!({"participant": participant, "source": source, "open": true}),
            ),
            Self::Unwatch {
                participant,
                source,
            } => (
                "watch_video",
                json!({"participant": participant, "source": source, "open": false}),
            ),
            Self::Ptt {
                command: PushToTalkCommand::Enable,
            } => ("set_push_to_talk", json!({"enabled": true})),
            Self::Ptt {
                command: PushToTalkCommand::Disable,
            } => ("set_push_to_talk", json!({"enabled": false})),
            Self::Ptt {
                command: PushToTalkCommand::Press,
            } => ("push_to_talk_press", json!({})),
            Self::Ptt {
                command: PushToTalkCommand::Release,
            } => ("push_to_talk_release", json!({})),
            Self::Ptt {
                command: PushToTalkCommand::Shortcut { shortcut },
            } => ("set_push_to_talk_shortcut", json!({"shortcut": shortcut})),
            Self::Ptt {
                command: PushToTalkCommand::ClearShortcut,
            } => ("set_push_to_talk_shortcut", json!({"shortcut": null})),
            Self::Leave => ("leave", json!({})),
            Self::Share { source } => ("share", json!({"enabled": true, "source": source})),
            Self::StopShare => ("share", json!({"enabled": false})),
        };
        CommandEnvelope::new("ctl", name, args)
    }
}

fn socket_path() -> anyhow::Result<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR is not set")?;
    Ok(PathBuf::from(runtime).join("wisp/wispd.sock"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let path = args.socket.map_or_else(socket_path, Ok)?;
    let stream = UnixStream::connect(&path)
        .await
        .with_context(|| format!("connect to {}; is wispd running?", path.display()))?;
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    let hello = CommandEnvelope::new("hello", "hello", json!({"client": "wispctl"}));
    writer
        .write_all(format!("{}\n", serde_json::to_string(&hello)?).as_bytes())
        .await?;
    let command = args.command.envelope();
    writer
        .write_all(format!("{}\n", serde_json::to_string(&command)?).as_bytes())
        .await?;

    while let Some(line) = lines.next_line().await? {
        let envelope: DaemonEnvelope = serde_json::from_str(&line)?;
        if let DaemonEnvelope::Result {
            id,
            ok,
            value,
            error,
            ..
        } = envelope
        {
            if id != "ctl" {
                continue;
            }
            if !ok {
                let error = error.context("daemon rejected command without an error")?;
                bail!("{}: {}", error.code, error.message);
            }
            if let Some(value) = value {
                print_value(&value)?;
            }
            return Ok(());
        }
    }
    bail!("wispd disconnected before replying")
}

fn print_value(value: &Value) -> anyhow::Result<()> {
    if value.is_null() {
        return Ok(());
    }
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
