//! Shared wire types for Wisp's localhost IPC and coordination API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u8 = 1;

pub type UserId = Uuid;
pub type HangoutId = Uuid;
pub type MessageId = Uuid;
pub type KnockId = Uuid;
pub type DeviceId = Uuid;
pub type InviteId = Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    #[default]
    Open,
    Knock,
    Closed,
    Away,
}

impl fmt::Display for Presence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Open => "open",
            Self::Knock => "knock",
            Self::Closed => "closed",
            Self::Away => "away",
        })
    }
}

impl FromStr for Presence {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "knock" => Ok(Self::Knock),
            "closed" => Ok(Self::Closed),
            "away" => Ok(Self::Away),
            _ => Err(format!("unknown presence: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    #[default]
    Offline,
    ConnectingToServer,
    Available,
    Joining,
    Connected,
    Reconnecting,
    Failed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioPreset {
    Natural,
    #[default]
    Clear,
    Studio,
}

impl fmt::Display for AudioPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Natural => "natural",
            Self::Clear => "clear",
            Self::Studio => "studio",
        })
    }
}

impl FromStr for AudioPreset {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "natural" => Ok(Self::Natural),
            "clear" => Ok(Self::Clear),
            "studio" => Ok(Self::Studio),
            _ => Err(format!("unknown audio preset: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioState {
    #[serde(default)]
    pub input_devices: Vec<AudioDevice>,
    #[serde(default)]
    pub output_devices: Vec<AudioDevice>,
    #[serde(default)]
    pub selected_input_id: Option<String>,
    #[serde(default)]
    pub selected_output_id: Option<String>,
    #[serde(default)]
    pub preset: AudioPreset,
    #[serde(default)]
    pub input_level: u8,
    /// Whether microphone frames are currently routed through the neural
    /// denoiser before publication.
    #[serde(default)]
    pub denoiser_active: bool,
    /// Stable implementation name for diagnostics and UI copy.
    #[serde(default)]
    pub denoiser: Option<String>,
    /// Algorithmic frame latency introduced by the selected processor.
    #[serde(default)]
    pub processing_latency_ms: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenShareState {
    #[serde(default)]
    pub starting: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_width: Option<u32>,
    #[serde(default)]
    pub source_height: Option<u32>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<u32>,
    #[serde(default)]
    pub published_frames: u64,
    #[serde(default)]
    pub encoder_backend: Option<String>,
    #[serde(default)]
    pub viewers: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoSource {
    #[default]
    ScreenShare,
    Camera,
}

impl fmt::Display for VideoSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ScreenShare => "screen_share",
            Self::Camera => "camera",
        })
    }
}

impl FromStr for VideoSource {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "screen" | "screen_share" | "screenshare" => Ok(Self::ScreenShare),
            "camera" => Ok(Self::Camera),
            _ => Err(format!("unknown video source: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoQualityPreset {
    Balanced,
    #[default]
    High,
    Ultra,
}

impl fmt::Display for VideoQualityPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Balanced => "balanced",
            Self::High => "high",
            Self::Ultra => "ultra",
        })
    }
}

impl FromStr for VideoQualityPreset {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "balanced" => Ok(Self::Balanced),
            "high" => Ok(Self::High),
            "ultra" => Ok(Self::Ultra),
            _ => Err(format!("unknown video quality preset: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodecPreference {
    #[default]
    H264,
    Vp8,
    Av1,
}

impl fmt::Display for VideoCodecPreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::H264 => "h264",
            Self::Vp8 => "vp8",
            Self::Av1 => "av1",
        })
    }
}

impl FromStr for VideoCodecPreference {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "h264" => Ok(Self::H264),
            "vp8" => Ok(Self::Vp8),
            "av1" => Ok(Self::Av1),
            _ => Err(format!("unknown video codec: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteVideoTarget {
    pub participant: String,
    pub source: VideoSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraState {
    #[serde(default)]
    pub devices: Vec<VideoDevice>,
    #[serde(default)]
    pub selected_device_id: Option<String>,
    #[serde(default)]
    pub starting: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<u32>,
    #[serde(default)]
    pub published_frames: u64,
    #[serde(default)]
    pub encoder_backend: Option<String>,
    #[serde(default)]
    pub viewers: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoSettings {
    pub quality: VideoQualityPreset,
    pub codec: VideoCodecPreference,
    #[serde(default)]
    pub available_codecs: Vec<VideoCodecPreference>,
    #[serde(default)]
    pub encoder_backend: String,
    #[serde(default)]
    pub available_encoder_backends: Vec<String>,
    #[serde(default)]
    pub hardware_acceleration: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            quality: VideoQualityPreset::default(),
            codec: VideoCodecPreference::default(),
            available_codecs: vec![
                VideoCodecPreference::H264,
                VideoCodecPreference::Vp8,
                VideoCodecPreference::Av1,
            ],
            encoder_backend: "software".into(),
            available_encoder_backends: vec!["software".into()],
            hardware_acceleration: false,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteVideoState {
    #[serde(flatten)]
    pub target: RemoteVideoTarget,
    pub mime_type: String,
    pub simulcasted: bool,
    #[serde(default)]
    pub subscribed: bool,
    #[serde(default)]
    pub surface_open: bool,
    #[serde(default)]
    pub surface_visible: bool,
    #[serde(default)]
    pub requested_quality: Option<String>,
    #[serde(default)]
    pub received_frames: u64,
    #[serde(default)]
    pub rendered_frames: u64,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushToTalkState {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub shortcut_backend: Option<String>,
    #[serde(default)]
    pub shortcut_replaced: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: UserId,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendState {
    #[serde(flatten)]
    pub user: UserSummary,
    pub presence: Presence,
    pub online: bool,
    pub hangout_id: Option<HangoutId>,
    #[serde(default)]
    pub activity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HangoutView {
    pub id: HangoutId,
    pub label: Option<String>,
    pub members: Vec<UserSummary>,
    #[serde(default)]
    pub sharing: Vec<UserSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    Direct,
    Circle,
    Hangout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationView {
    pub id: String,
    pub kind: ConversationKind,
    pub label: String,
    #[serde(default)]
    pub spot_id: Option<String>,
    pub members: Vec<UserSummary>,
    #[serde(default)]
    pub last_message: Option<Box<Message>>,
    #[serde(default)]
    pub unread_count: u64,
    #[serde(default)]
    pub tab_closed: bool,
    #[serde(default)]
    pub history_cleared_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub can_clear_for_everyone: bool,
    #[serde(default)]
    pub self_role: String,
    #[serde(default)]
    pub member_roles: std::collections::BTreeMap<UserId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpotView {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub active_hangout_id: Option<HangoutId>,
    #[serde(default)]
    pub members: Vec<UserSummary>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaState {
    pub livekit_connected: bool,
    pub microphone_published: bool,
    #[serde(default)]
    pub e2ee_enabled: bool,
    pub received_audio_frames: u64,
    #[serde(default)]
    pub remote_audio_participants: Vec<String>,
    #[serde(default)]
    pub remote_muted_participants: Vec<String>,
    #[serde(default)]
    pub remote_video_participants: Vec<String>,
    #[serde(default)]
    pub active_speakers: Vec<String>,
    #[serde(default)]
    pub received_video_frames: u64,
    #[serde(default)]
    pub rendered_video_frames: u64,
    #[serde(default)]
    pub surface_open: bool,
    #[serde(default)]
    pub microphone: Option<String>,
    #[serde(default)]
    pub speaker: Option<String>,
    #[serde(default)]
    pub audio: AudioState,
    #[serde(default)]
    pub screen_share: ScreenShareState,
    #[serde(default)]
    pub camera: CameraState,
    #[serde(default)]
    pub video: VideoSettings,
    #[serde(default)]
    pub remote_videos: Vec<RemoteVideoState>,
    #[serde(default)]
    pub last_audio_from: Option<String>,
    #[serde(default)]
    pub last_video_from: Option<String>,
    #[serde(default)]
    pub video_width: Option<u32>,
    #[serde(default)]
    pub video_height: Option<u32>,
    #[serde(default)]
    pub surface_error: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfState {
    #[serde(flatten)]
    pub user: UserSummary,
    pub presence: Presence,
    pub connection: ConnectionState,
    pub muted: bool,
    pub deafened: bool,
    pub sharing: bool,
    pub hangout_id: Option<HangoutId>,
    #[serde(default)]
    pub push_to_talk: PushToTalkState,
    #[serde(default)]
    pub media: MediaState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub seq: u64,
    #[serde(rename = "self")]
    pub self_state: SelfState,
    pub friends: Vec<FriendState>,
    pub hangouts: Vec<HangoutView>,
    #[serde(default)]
    pub knocks: Vec<KnockRequestView>,
    #[serde(default)]
    pub conversations: Vec<ConversationView>,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub spots: Vec<SpotView>,
    #[serde(default)]
    pub devices: Vec<DeviceView>,
    #[serde(default)]
    pub last_invite: Option<DeviceInvite>,
}

impl Snapshot {
    #[must_use]
    pub fn connected(&self) -> bool {
        self.self_state.hangout_id.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub v: u8,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub args: Value,
}

impl CommandEnvelope {
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, args: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            kind: "command".into(),
            name: name.into(),
            args,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.v != PROTOCOL_VERSION {
            return Err("unsupported_protocol_version");
        }
        if self.kind != "command" {
            return Err("invalid_envelope_type");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonEnvelope {
    Hello {
        v: u8,
        daemon: String,
        profile: String,
    },
    Snapshot {
        v: u8,
        snapshot: Box<Snapshot>,
    },
    Result {
        v: u8,
        id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<ProtocolError>,
    },
    Event {
        v: u8,
        seq: u64,
        name: String,
        payload: Value,
    },
}

impl DaemonEnvelope {
    #[must_use]
    pub fn success(id: impl Into<String>, value: Option<Value>) -> Self {
        Self::Result {
            v: PROTOCOL_VERSION,
            id: id.into(),
            ok: true,
            value,
            error: None,
        }
    }

    #[must_use]
    pub fn failure(
        id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Result {
            v: PROTOCOL_VERSION,
            id: id.into(),
            ok: false,
            value: None,
            error: Some(ProtocolError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevSessionRequest {
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevSession {
    pub token: String,
    pub user: UserSummary,
    pub protocol_version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDirectConversationRequest {
    pub friend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupConversationRequest {
    pub request_id: Uuid,
    pub name: String,
    pub members: Vec<UserId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkConversationReadRequest {
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetConversationTabRequest {
    pub conversation_id: String,
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinSpotRequest {
    pub spot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInviteRequest {
    pub profile: String,
    #[serde(default)]
    pub expires_in_minutes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInvite {
    pub id: InviteId,
    pub code: String,
    pub profile: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceRequest {
    pub invite_code: String,
    pub device_name: String,
    pub protocol_version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapDeviceRequest {
    pub bootstrap_token: String,
    pub profile: String,
    pub device_name: String,
    pub protocol_version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCredential {
    pub device_id: DeviceId,
    pub device_token: String,
    pub user: UserSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSessionRequest {
    pub device_id: DeviceId,
    pub device_token: String,
    pub protocol_version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSession {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user: UserSummary,
    pub device_id: DeviceId,
    pub protocol_version: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceView {
    pub id: DeviceId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub last_seen_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPresenceRequest {
    pub presence: Presence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinFriendRequest {
    pub friend: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnockRequestView {
    pub id: KnockId,
    pub from: UserSummary,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum JoinFriendResult {
    Joined {
        hangout_id: HangoutId,
    },
    KnockSent {
        knock_id: KnockId,
        expires_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnockResponse {
    Accept,
    Later,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RespondKnockRequest {
    pub knock_id: KnockId,
    pub response: KnockResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RespondKnockResult {
    Accepted { hangout_id: HangoutId },
    Later,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinHangoutRequest {
    pub hangout_id: HangoutId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveKitTokenResponse {
    pub url: String,
    pub room: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub conversation_id: String,
    pub sender: UserSummary,
    pub created_at: DateTime<Utc>,
    pub content_type: String,
    pub payload: Value,
    pub encryption_version: i64,
    #[serde(default)]
    pub edited_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditMessageRequest {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub conversation_id: String,
    #[serde(default = "default_content_type")]
    pub content_type: String,
    pub payload: Value,
    #[serde(default)]
    pub encryption_version: i64,
}

pub const MAX_CHAT_IMAGE_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_CHAT_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;
pub const MAX_CHAT_FILE_BYTES: usize = 25 * 1024 * 1024;
/// Transport chunk bound, not a file size limit. Legacy JSON uploads use the
/// old `MAX_CHAT_FILE_BYTES` bound; current clients use chunked transfers.
pub const CHAT_FILE_CHUNK_BYTES: usize = 4 * 1024 * 1024;

#[must_use]
pub fn file_retention_hours(size: u64) -> Option<i64> {
    if size > 5_000_000_000 {
        Some(24)
    } else if size > 1_000_000_000 {
        Some(48)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeginFileUpload {
    pub id: Uuid,
    pub conversation_id: String,
    pub file_name: String,
    pub size: u64,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub keep: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadStatus {
    pub received_bytes: u64,
    pub next_chunk: u64,
    pub message_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFileRetention {
    pub keep: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearChatHistoryRequest {
    pub conversation_id: String,
    #[serde(default)]
    pub for_everyone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMemberRequest {
    pub conversation_id: String,
    pub user_id: UserId,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetRoomAdminRequest {
    pub conversation_id: String,
    pub user_id: UserId,
    pub admin: bool,
}
pub const MAX_CHAT_DRAFTS: usize = 8;

#[must_use]
pub fn valid_chat_file_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= 200
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendFileMessageRequest {
    pub conversation_id: String,
    pub file_name: String,
    pub data_base64: String,
    #[serde(default)]
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendImageMessageRequest {
    pub conversation_id: String,
    pub png_base64: String,
    #[serde(default)]
    pub caption: String,
}

fn default_content_type() -> String {
    "text/plain".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEvent {
    pub seq: u64,
    pub name: String,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    pub payload: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn attachment_names_cannot_escape_the_download_directory() {
        for name in [
            "",
            ".",
            "..",
            "../notes",
            "/tmp/notes",
            "a\\b",
            "new\nline",
            "nul\0byte",
        ] {
            assert!(!valid_chat_file_name(name), "accepted {name:?}");
        }
        assert!(!valid_chat_file_name(&"a".repeat(201)));
        assert!(valid_chat_file_name("Screenshot 2026-09-03.png"));
        assert!(valid_chat_file_name("notes—final.txt"));
    }

    #[test]
    fn presence_round_trip_is_stable() {
        for state in [
            Presence::Open,
            Presence::Knock,
            Presence::Closed,
            Presence::Away,
        ] {
            let encoded = serde_json::to_string(&state).unwrap();
            let decoded: Presence = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, state);
            assert_eq!(state.to_string().parse::<Presence>().unwrap(), state);
        }
    }

    #[test]
    fn command_envelope_is_versioned() {
        let command = CommandEnvelope::new("a1", "set_muted", json!({"muted": true}));
        assert!(command.validate().is_ok());
        assert_eq!(serde_json::to_value(command).unwrap()["v"], 1);
    }

    #[test]
    fn rejects_unknown_protocol_versions() {
        let command = CommandEnvelope {
            v: 2,
            ..CommandEnvelope::new("a1", "hello", json!({}))
        };
        assert_eq!(command.validate(), Err("unsupported_protocol_version"));
    }

    #[test]
    fn older_self_state_defaults_media_details() {
        let state: SelfState = serde_json::from_value(json!({
            "id": "00000000-0000-4000-8000-000000000001",
            "display_name": "Jared",
            "presence": "open",
            "connection": "available",
            "muted": false,
            "deafened": false,
            "sharing": false,
            "hangout_id": null
        }))
        .unwrap();
        assert_eq!(state.media, MediaState::default());
        assert_eq!(state.push_to_talk, PushToTalkState::default());
    }

    #[test]
    fn older_media_state_defaults_video_details() {
        let state: MediaState = serde_json::from_value(json!({
            "livekit_connected": true,
            "microphone_published": true,
            "received_audio_frames": 42
        }))
        .unwrap();
        assert!(state.livekit_connected);
        assert_eq!(state.received_audio_frames, 42);
        assert!(state.remote_audio_participants.is_empty());
        assert!(state.remote_muted_participants.is_empty());
        assert!(state.remote_video_participants.is_empty());
        assert!(state.active_speakers.is_empty());
        assert_eq!(state.received_video_frames, 0);
        assert_eq!(state.rendered_video_frames, 0);
        assert!(!state.surface_open);
        assert_eq!(state.audio, AudioState::default());
        assert_eq!(state.surface_error, None);
        assert_eq!(state.error_code, None);
    }

    #[test]
    fn audio_presets_round_trip_and_default_to_clear() {
        for preset in [
            AudioPreset::Natural,
            AudioPreset::Clear,
            AudioPreset::Studio,
        ] {
            let encoded = serde_json::to_string(&preset).unwrap();
            let decoded: AudioPreset = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, preset);
            assert_eq!(preset.to_string().parse::<AudioPreset>().unwrap(), preset);
        }
        assert_eq!(AudioState::default().preset, AudioPreset::Clear);
    }
}
