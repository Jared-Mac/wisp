use ksni::{Category, MenuItem, Status, ToolTip, TrayMethods, menu};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Activate { x: i32, y: i32 },
    Show,
    Hide,
    ToggleMuted,
    ToggleDeafened,
    SetAnchor(&'static str),
    Exit,
}

pub(super) struct WispTray {
    actions: mpsc::UnboundedSender<TrayAction>,
}

impl WispTray {
    fn send(&self, action: TrayAction) {
        let _ = self.actions.send(action);
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
        "Wisp".into()
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_name(&self) -> String {
        "dev.wisp".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![waveform_icon(32)]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: "dev.wisp".into(),
            icon_pixmap: self.icon_pixmap(),
            title: "Wisp".into(),
            description: "Friends and hangouts".into(),
        }
    }

    fn activate(&mut self, x: i32, y: i32) {
        self.send(TrayAction::Activate { x, y });
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayAction::ToggleMuted);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        use menu::{StandardItem, SubMenu};

        let action = |label: &str, icon_name: &str, tray_action| {
            StandardItem {
                label: label.into(),
                icon_name: icon_name.into(),
                activate: Box::new(move |tray: &mut Self| tray.send(tray_action)),
                ..Default::default()
            }
            .into()
        };

        vec![
            action("Show Wisp", "window-new", TrayAction::Show),
            action("Hide Wisp", "window-close", TrayAction::Hide),
            MenuItem::Separator,
            action(
                "Toggle microphone mute",
                "microphone-sensitivity-muted",
                TrayAction::ToggleMuted,
            ),
            action(
                "Toggle deafen",
                "audio-volume-muted",
                TrayAction::ToggleDeafened,
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

pub(super) async fn spawn()
-> anyhow::Result<(mpsc::UnboundedReceiver<TrayAction>, ksni::Handle<WispTray>)> {
    let (actions, receiver) = mpsc::unbounded_channel();
    let handle = WispTray { actions }.spawn().await?;
    Ok((receiver, handle))
}

fn waveform_icon(size: i32) -> ksni::Icon {
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
                let offset = (usize::try_from(py).expect("nonnegative y") * dimension
                    + usize::try_from(px).expect("nonnegative x"))
                    * 4;
                data[offset] = 255;
                data[offset + 1] = 47;
                data[offset + 2] = 140;
                data[offset + 3] = 255;
            }
        }
    }
    ksni::Icon {
        width: size,
        height: size,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::waveform_icon;

    #[test]
    fn tray_icon_is_argb32() {
        let icon = waveform_icon(32);
        assert_eq!(icon.width, 32);
        assert_eq!(icon.height, 32);
        assert_eq!(icon.data.len(), 32 * 32 * 4);
        assert!(icon.data.chunks_exact(4).any(|pixel| pixel[0] == 255));
    }
}
