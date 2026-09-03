use ksni::{Category, MenuItem, Status, ToolTip, TrayMethods, menu};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Activate { x: i32, y: i32 },
    Show,
    Hide,
    OpenApp,
    ToggleMuted,
    ToggleDeafened,
    ToggleShare,
    ToggleCamera,
    SetAnchor(&'static str),
    Exit,
}

pub(super) struct WispTray {
    actions: mpsc::UnboundedSender<TrayAction>,
    state: TrayState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct TrayState {
    audio: AudioState,
    sharing: bool,
    camera: bool,
}

impl TrayState {
    pub(super) fn new(audio: (bool, bool), video: (bool, bool)) -> Self {
        Self {
            audio: AudioState::from_flags(audio.0, audio.1),
            sharing: video.0,
            camera: video.1,
        }
    }
}

impl WispTray {
    fn send(&self, action: TrayAction) {
        let _ = self.actions.send(action);
    }

    pub(super) fn set_state(&mut self, state: TrayState) {
        self.state = state;
    }

    fn audio_state(&self) -> AudioState {
        self.state.audio
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum AudioState {
    #[default]
    Ready,
    Muted,
    MutedAndDeafened,
}

impl AudioState {
    fn from_flags(muted: bool, deafened: bool) -> Self {
        match (muted, deafened) {
            (_, true) => Self::MutedAndDeafened,
            (true, false) => Self::Muted,
            (false, false) => Self::Ready,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Ready => "Audio ready",
            Self::Muted => "Microphone muted",
            Self::MutedAndDeafened => "Microphone muted and deafened",
        }
    }

    fn muted(self) -> bool {
        self != Self::Ready
    }

    fn deafened(self) -> bool {
        self == Self::MutedAndDeafened
    }
}

impl ksni::Tray for WispTray {
    fn id(&self) -> String {
        "dev.wisp".into()
    }

    fn category(&self) -> Category {
        Category::Communications
    }

    fn title(&self) -> String {
        let mut states = Vec::new();
        if self.state.sharing {
            states.push("Sharing screen");
        }
        if self.state.camera {
            states.push("Camera on");
        }
        if self.audio_state() != AudioState::Ready {
            states.push(self.audio_state().label());
        }
        if states.is_empty() {
            "Wisp".into()
        } else {
            format!("Wisp · {}", states.join(" · "))
        }
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![waveform_icon(
            32,
            self.audio_state(),
            self.state.sharing || self.state.camera,
        )]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: String::new(),
            icon_pixmap: self.icon_pixmap(),
            title: "Wisp".into(),
            description: self.title().trim_start_matches("Wisp · ").into(),
        }
    }

    fn activate(&mut self, x: i32, y: i32) {
        self.send(TrayAction::Activate { x, y });
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayAction::ToggleMuted);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        use menu::{CheckmarkItem, StandardItem, SubMenu};

        let action = |label: &str, icon_name: &str, tray_action| {
            StandardItem {
                label: label.into(),
                icon_name: icon_name.into(),
                activate: Box::new(move |tray: &mut Self| tray.send(tray_action)),
                ..Default::default()
            }
            .into()
        };

        let muted = CheckmarkItem {
            label: if self.audio_state().deafened() {
                "Unmute microphone and undeafen".into()
            } else {
                "Microphone muted".into()
            },
            checked: self.audio_state().muted(),
            icon_name: "microphone-sensitivity-muted".into(),
            activate: Box::new(|tray: &mut Self| tray.send(TrayAction::ToggleMuted)),
            ..Default::default()
        };
        let deafened = CheckmarkItem {
            label: "Deafened".into(),
            checked: self.audio_state().deafened(),
            icon_name: "audio-volume-muted".into(),
            activate: Box::new(|tray: &mut Self| tray.send(TrayAction::ToggleDeafened)),
            ..Default::default()
        };

        vec![
            action("Show Wisp panel", "window-new", TrayAction::Show),
            action("Hide Wisp panel", "window-close", TrayAction::Hide),
            action("Open Wisp app", "view-fullscreen", TrayAction::OpenApp),
            MenuItem::Separator,
            muted.into(),
            deafened.into(),
            action(
                if self.state.sharing {
                    "Stop screen sharing"
                } else {
                    "Share a screen or window"
                },
                "video-display",
                TrayAction::ToggleShare,
            ),
            action(
                if self.state.camera {
                    "Turn camera off"
                } else {
                    "Turn camera on"
                },
                "camera-web",
                TrayAction::ToggleCamera,
            ),
            SubMenu {
                label: "Panel anchor".into(),
                icon_name: "transform-move".into(),
                submenu: vec![
                    action("Auto · near system tray", "", TrayAction::SetAnchor("auto")),
                    action("Bottom right", "", TrayAction::SetAnchor("bottom-right")),
                    action("Bottom left", "", TrayAction::SetAnchor("bottom-left")),
                    action("Top right", "", TrayAction::SetAnchor("top-right")),
                    action("Top left", "", TrayAction::SetAnchor("top-left")),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            action("Exit Wisp", "application-exit", TrayAction::Exit),
        ]
    }
}

pub(super) async fn spawn(
    state: TrayState,
) -> anyhow::Result<(mpsc::UnboundedReceiver<TrayAction>, ksni::Handle<WispTray>)> {
    let (actions, receiver) = mpsc::unbounded_channel();
    let handle = WispTray { actions, state }.spawn().await?;
    Ok((receiver, handle))
}

fn waveform_icon(size: i32, state: AudioState, sharing: bool) -> ksni::Icon {
    let dimension = usize::try_from(size).expect("positive tray icon size");
    let mut data = vec![0_u8; dimension * dimension * 4];
    let center = size / 2;
    let heights = [8, 16, 24, 14, 26, 18, 10];
    for (index, height) in heights.into_iter().enumerate() {
        let x = 4 + i32::try_from(index).expect("small icon index") * 4;
        let top = center - height / 2;
        let bottom = center + height / 2;
        for px in x..=(x + 2) {
            for py in top..=bottom {
                if px < 0 || py < 0 || px >= size || py >= size {
                    continue;
                }
                set_pixel(&mut data, dimension, px, py, [255, 47, 140, 255]);
            }
        }
    }
    if sharing {
        draw_share_badge(&mut data, dimension, size);
    }
    draw_state_badge(&mut data, dimension, size, state);
    ksni::Icon {
        width: size,
        height: size,
        data,
    }
}

fn draw_share_badge(data: &mut [u8], dimension: usize, size: i32) {
    let center = (size - 7, 7);
    let radius = 7;
    for y in (center.1 - radius)..=(center.1 + radius) {
        for x in (center.0 - radius)..=(center.0 + radius) {
            if (x - center.0).pow(2) + (y - center.1).pow(2) <= radius.pow(2) {
                set_pixel(data, dimension, x, y, [255, 50, 230, 244]);
            }
        }
    }
    let ink = [255, 21, 24, 33];
    for x in (center.0 - 4)..=(center.0 + 4) {
        set_pixel(data, dimension, x, center.1 - 3, ink);
        set_pixel(data, dimension, x, center.1 + 2, ink);
    }
    for y in (center.1 - 3)..=(center.1 + 2) {
        set_pixel(data, dimension, center.0 - 4, y, ink);
        set_pixel(data, dimension, center.0 + 4, y, ink);
    }
    for x in (center.0 - 2)..=(center.0 + 2) {
        set_pixel(data, dimension, x, center.1 + 4, ink);
    }
    set_pixel(data, dimension, center.0, center.1 + 3, ink);
}

fn set_pixel(data: &mut [u8], dimension: usize, x: i32, y: i32, color: [u8; 4]) {
    let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
        return;
    };
    if x >= dimension || y >= dimension {
        return;
    }
    let offset = (y * dimension + x) * 4;
    data[offset..offset + 4].copy_from_slice(&color);
}

fn draw_state_badge(data: &mut [u8], dimension: usize, size: i32, state: AudioState) {
    if state == AudioState::Ready {
        return;
    }
    let center = (size - 7, size - 7);
    let radius = 7;
    let color = if state == AudioState::Muted {
        [255, 245, 185, 76]
    } else {
        [255, 255, 92, 108]
    };
    for y in (center.1 - radius)..=(center.1 + radius) {
        for x in (center.0 - radius)..=(center.0 + radius) {
            let distance = (x - center.0).pow(2) + (y - center.1).pow(2);
            if distance <= radius.pow(2) {
                set_pixel(data, dimension, x, y, color);
            }
        }
    }

    let ink = [255, 21, 24, 33];
    match state {
        AudioState::Muted => {
            for offset in -4..=4 {
                set_pixel(data, dimension, center.0 + offset, center.1 - offset, ink);
                set_pixel(
                    data,
                    dimension,
                    center.0 + offset,
                    center.1 - offset + 1,
                    ink,
                );
            }
        }
        AudioState::MutedAndDeafened => {
            for offset in -4..=4 {
                set_pixel(data, dimension, center.0 + offset, center.1 + offset, ink);
                set_pixel(data, dimension, center.0 + offset, center.1 - offset, ink);
            }
        }
        AudioState::Ready => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioState, TrayState, WispTray, waveform_icon};

    #[test]
    fn tray_icon_is_argb32() {
        let icon = waveform_icon(32, AudioState::Ready, false);
        assert_eq!(icon.width, 32);
        assert_eq!(icon.height, 32);
        assert_eq!(icon.data.len(), 32 * 32 * 4);
        assert!(icon.data.chunks_exact(4).any(|pixel| pixel[0] == 255));
    }

    #[test]
    fn every_audio_state_has_a_distinct_tray_icon() {
        let states = [
            AudioState::Ready,
            AudioState::Muted,
            AudioState::MutedAndDeafened,
        ];
        let icons = states.map(|state| waveform_icon(32, state, false).data);
        for left in 0..icons.len() {
            for right in (left + 1)..icons.len() {
                assert_ne!(icons[left], icons[right]);
            }
        }
    }

    #[test]
    fn screen_sharing_adds_a_distinct_badge() {
        assert_ne!(
            waveform_icon(32, AudioState::Ready, false).data,
            waveform_icon(32, AudioState::Ready, true).data
        );
    }

    #[test]
    fn tray_title_reports_camera_and_screen_independently() {
        let (actions, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let tray = WispTray {
            actions,
            state: TrayState::new((false, false), (true, true)),
        };
        assert_eq!(
            ksni::Tray::title(&tray),
            "Wisp · Sharing screen · Camera on"
        );
    }

    #[test]
    fn deafened_state_always_uses_the_combined_indicator() {
        assert_eq!(
            AudioState::from_flags(false, true),
            AudioState::MutedAndDeafened
        );
        assert_eq!(
            AudioState::from_flags(true, true),
            AudioState::MutedAndDeafened
        );
    }
}
