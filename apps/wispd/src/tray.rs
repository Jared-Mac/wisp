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
    unread_messages: u64,
    room_invitations: u64,
}

impl TrayState {
    pub(super) fn new(audio: (bool, bool), video: (bool, bool), unread_messages: u64) -> Self {
        Self {
            audio: AudioState::from_flags(audio.0, audio.1),
            sharing: video.0,
            camera: video.1,
            unread_messages,
            room_invitations: 0,
        }
    }
    pub(super) fn with_room_invitations(mut self, count: u64) -> Self {
        self.room_invitations = count;
        self
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
        let unread = (self.state.unread_messages > 0)
            .then(|| format!("{} unread", self.state.unread_messages));
        if let Some(unread) = unread.as_deref() {
            states.push(unread);
        }
        let invitations = (self.state.room_invitations > 0)
            .then(|| format!("{} voice invite(s)", self.state.room_invitations));
        if let Some(invitations) = invitations.as_deref() {
            states.push(invitations);
        }
        if states.is_empty() {
            "Wisp".into()
        } else {
            format!("Wisp · {}", states.join(" · "))
        }
    }

    fn status(&self) -> Status {
        if self.state.unread_messages > 0 || self.state.room_invitations > 0 {
            Status::NeedsAttention
        } else {
            Status::Active
        }
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let mut icon = wisp_icon(
            32,
            self.audio_state(),
            self.state.sharing,
            self.state.camera,
        );
        if self.state.unread_messages > 0 || self.state.room_invitations > 0 {
            draw_unread_badge(
                &mut icon.data,
                32,
                self.state.unread_messages.max(self.state.room_invitations),
            );
        }
        vec![icon]
    }

    fn attention_icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon_pixmap()
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
            action("Open App", "view-fullscreen", TrayAction::OpenApp),
            action("Show tray popup", "window-new", TrayAction::Show),
            action("Hide tray popup", "window-close", TrayAction::Hide),
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

fn wisp_icon(size: i32, state: AudioState, sharing: bool, camera: bool) -> ksni::Icon {
    // Rendered from the transparent standalone W used by the desktop shell.
    // Status badges stay live without adding a permanent icon background.
    static BRAND_ICON: std::sync::LazyLock<image::RgbaImage> = std::sync::LazyLock::new(|| {
        image::load_from_memory(include_bytes!(
            "../../../quickshell/app/assets/wisp-icon-tray.png"
        ))
        .expect("bundled Wisp tray icon is a valid PNG")
        .into_rgba8()
    });
    let dimension = usize::try_from(size).expect("positive tray icon size");
    let extent = u32::try_from(size).expect("positive tray icon size");
    let pixels = image::imageops::resize(
        &*BRAND_ICON,
        extent,
        extent,
        image::imageops::FilterType::Lanczos3,
    );
    // StatusNotifierItem pixels use network-order ARGB, not PNG's RGBA.
    let mut data: Vec<u8> = pixels
        .pixels()
        .flat_map(|pixel| [pixel[3], pixel[0], pixel[1], pixel[2]])
        .collect();
    if sharing && camera {
        draw_share_badge(&mut data, dimension, (7, 7));
    } else if sharing {
        draw_share_badge(&mut data, dimension, (size - 7, 7));
    }
    if camera {
        draw_camera_badge(&mut data, dimension, (size - 7, 7));
    }
    draw_state_badge(&mut data, dimension, size, state);
    ksni::Icon {
        width: size,
        height: size,
        data,
    }
}

fn draw_unread_badge(data: &mut [u8], dimension: usize, count: u64) {
    for y in 18..32 {
        for x in 0..14 {
            if (x - 7_i32).pow(2) + (y - 25_i32).pow(2) <= 49 {
                set_pixel(data, dimension, x, y, [255, 255, 92, 108]);
            }
        }
    }
    // A legible tiny numeral, or + for ten or more. Audio/camera badges
    // occupy the other corners and remain visible independently.
    let glyph = match count {
        1 => [2, 6, 2, 2, 7],
        2 => [7, 1, 7, 4, 7],
        3 => [7, 1, 7, 1, 7],
        4 => [5, 5, 7, 1, 1],
        5 => [7, 4, 7, 1, 7],
        6 => [7, 4, 7, 5, 7],
        7 => [7, 1, 1, 1, 1],
        8 => [7, 5, 7, 5, 7],
        9 => [7, 5, 7, 1, 7],
        _ => [0, 2, 7, 2, 0],
    };
    for (row, bits) in glyph.iter().enumerate() {
        for column in 0..3 {
            if bits & (1 << (2 - column)) != 0 {
                set_pixel(
                    data,
                    dimension,
                    6 + column,
                    23 + i32::try_from(row).unwrap(),
                    [255, 255, 255, 255],
                );
            }
        }
    }
}

fn draw_share_badge(data: &mut [u8], dimension: usize, center: (i32, i32)) {
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

fn draw_camera_badge(data: &mut [u8], dimension: usize, center: (i32, i32)) {
    let radius = 7;
    for y in (center.1 - radius)..=(center.1 + radius) {
        for x in (center.0 - radius)..=(center.0 + radius) {
            if (x - center.0).pow(2) + (y - center.1).pow(2) <= radius.pow(2) {
                set_pixel(data, dimension, x, y, [255, 72, 220, 150]);
            }
        }
    }

    let ink = [255, 21, 24, 33];
    for y in (center.1 - 3)..=(center.1 + 3) {
        for x in (center.0 - 4)..=(center.0 + 3) {
            set_pixel(data, dimension, x, y, ink);
        }
    }
    for y in (center.1 - 2)..=(center.1 + 2) {
        for x in (center.0 + 4)..=(center.0 + 5) {
            set_pixel(data, dimension, x, y, ink);
        }
    }
    for y in (center.1 - 1)..=(center.1 + 1) {
        for x in (center.0 - 1)..=(center.0 + 1) {
            set_pixel(data, dimension, x, y, [255, 72, 220, 150]);
        }
    }
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
    use super::{AudioState, TrayState, WispTray, wisp_icon};

    #[test]
    fn open_app_is_first_in_tray_context_menu() {
        use ksni::Tray;
        let (actions, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut tray = WispTray {
            actions,
            state: TrayState::new((false, false), (false, false), 0),
        };
        let menu = tray.menu();
        let ksni::MenuItem::Standard(item) = &menu[0] else {
            panic!("expected Open App action")
        };
        assert_eq!(item.label, "Open App");
        (item.activate)(&mut tray);
        assert!(matches!(
            receiver.try_recv(),
            Ok(super::TrayAction::OpenApp)
        ));
    }

    #[test]
    fn tray_icon_is_argb32() {
        let icon = wisp_icon(32, AudioState::Ready, false, false);
        assert_eq!(icon.width, 32);
        assert_eq!(icon.height, 32);
        assert_eq!(icon.data.len(), 32 * 32 * 4);
        assert_eq!(
            icon.data[0], 0,
            "the icon background must remain transparent"
        );
        assert!(icon.data.chunks_exact(4).any(|pixel| pixel[0] == 0));
        assert!(icon.data.chunks_exact(4).any(|pixel| pixel[0] == 255));
    }

    #[test]
    fn unread_messages_add_a_badge_and_request_attention() {
        use ksni::Tray;
        let (actions, _) = tokio::sync::mpsc::unbounded_channel();
        let mut tray = WispTray {
            actions,
            state: TrayState::new((true, true), (true, true), 0),
        };
        let read_icon = tray.icon_pixmap().remove(0).data;
        tray.state.unread_messages = 3;
        assert_eq!(tray.status(), ksni::Status::NeedsAttention);
        assert_ne!(read_icon, tray.icon_pixmap().remove(0).data);
        assert!(tray.title().contains("3 unread"));
        tray.state.unread_messages = 0;
        assert_eq!(tray.status(), ksni::Status::Active);
        assert_eq!(read_icon, tray.icon_pixmap().remove(0).data);
    }

    #[test]
    fn voice_invitations_request_attention_even_when_chat_was_read() {
        use ksni::Tray;
        let (actions, _) = tokio::sync::mpsc::unbounded_channel();
        let mut tray = WispTray {
            actions,
            state: TrayState::new((false, false), (false, false), 0),
        };
        let icon = tray.icon_pixmap().remove(0).data;
        tray.state = tray.state.with_room_invitations(1);
        assert_eq!(tray.status(), ksni::Status::NeedsAttention);
        assert!(tray.title().contains("voice invite"));
        assert_ne!(icon, tray.icon_pixmap().remove(0).data);
        tray.state = tray.state.with_room_invitations(0);
        assert_eq!(tray.status(), ksni::Status::Active);
    }

    #[test]
    fn every_audio_state_has_a_distinct_tray_icon() {
        let states = [
            AudioState::Ready,
            AudioState::Muted,
            AudioState::MutedAndDeafened,
        ];
        let icons = states.map(|state| wisp_icon(32, state, false, false).data);
        for left in 0..icons.len() {
            for right in (left + 1)..icons.len() {
                assert_ne!(icons[left], icons[right]);
            }
        }
    }

    #[test]
    fn screen_sharing_adds_a_distinct_badge() {
        assert_ne!(
            wisp_icon(32, AudioState::Ready, false, false).data,
            wisp_icon(32, AudioState::Ready, true, false).data
        );
    }

    #[test]
    fn camera_adds_a_distinct_badge() {
        let idle = wisp_icon(32, AudioState::Ready, false, false).data;
        let sharing = wisp_icon(32, AudioState::Ready, true, false).data;
        let camera = wisp_icon(32, AudioState::Ready, false, true).data;
        let both = wisp_icon(32, AudioState::Ready, true, true).data;
        assert_ne!(camera, idle);
        assert_ne!(camera, sharing);
        assert_ne!(both, camera);
        assert_ne!(both, sharing);
    }

    #[test]
    fn tray_title_reports_camera_and_screen_independently() {
        let (actions, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let tray = WispTray {
            actions,
            state: TrayState::new((false, false), (true, true), 0),
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
