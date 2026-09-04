//! Private, demand-driven local RGBA transport for Qt Quick tiles. Never writes
//! frames to disk or a network listener. Each consumer has one frame in flight;
//! slow/hidden windows skip frames instead of building a playback backlog.
use crate::{Daemon, surface::RgbaFrame};
use anyhow::{Context, bail};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    sync::watch,
};
use wisp_protocol::RemoteVideoTarget;

type FrameSender = watch::Sender<Option<Arc<RgbaFrame>>>;
#[derive(Clone, Default)]
pub(crate) struct VideoBridge(Arc<Mutex<HashMap<RemoteVideoTarget, FrameSender>>>);
impl VideoBridge {
    pub fn open(&self, target: RemoteVideoTarget) {
        self.0
            .lock()
            .expect("video lock")
            .entry(target)
            .or_insert_with(|| watch::channel(None).0);
    }
    pub fn close(&self, target: &RemoteVideoTarget) {
        self.0.lock().expect("video lock").remove(target);
    }
    pub fn clear(&self) {
        self.0.lock().expect("video lock").clear();
    }
    pub fn contains(&self, target: &RemoteVideoTarget) -> bool {
        self.0.lock().expect("video lock").contains_key(target)
    }
    pub fn send(&self, target: &RemoteVideoTarget, frame: RgbaFrame) {
        if let Some(tx) = self.0.lock().expect("video lock").get(target) {
            tx.send_replace(Some(Arc::new(frame)));
        }
    }
    fn subscribe(
        &self,
        target: &RemoteVideoTarget,
    ) -> Option<watch::Receiver<Option<Arc<RgbaFrame>>>> {
        self.0
            .lock()
            .expect("video lock")
            .get(target)
            .map(watch::Sender::subscribe)
    }
    fn unused(&self, target: &RemoteVideoTarget) -> bool {
        self.0
            .lock()
            .expect("video lock")
            .get(target)
            .is_some_and(|tx| tx.receiver_count() == 0)
    }
}

pub(crate) async fn serve(stream: UnixStream, daemon: Arc<Daemon>) -> anyhow::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    // Limit the unauthenticated local handshake (the socket itself is mode 0600).
    tokio::time::timeout(
        Duration::from_secs(5),
        (&mut reader).take(4096).read_line(&mut line),
    )
    .await??;
    if !line.ends_with('\n') {
        bail!("invalid video handshake");
    }
    let target: RemoteVideoTarget = serde_json::from_str(&line)?;
    let frames = daemon.media.video_bridge.clone();
    let receiver = frames
        .subscribe(&target)
        .context("video is not being watched")?;
    let result = stream_frames(&mut reader, receiver, &daemon, &target).await;
    // Closing/crashing the UI must not leave an invisible subscribed viewer.
    tokio::time::sleep(Duration::from_millis(500)).await;
    if frames.unused(&target) {
        let _ = daemon.set_video_watched(target, false).await;
    }
    result
}

async fn stream_frames(
    reader: &mut BufReader<UnixStream>,
    mut receiver: watch::Receiver<Option<Arc<RgbaFrame>>>,
    daemon: &Daemon,
    target: &RemoteVideoTarget,
) -> anyhow::Result<()> {
    receiver.mark_changed();
    let mut last_size = (0, 0);
    loop {
        let mut line = String::new();
        if (&mut *reader).take(128).read_line(&mut line).await? == 0 {
            return Ok(());
        }
        if !line.ends_with('\n') {
            bail!("invalid video frame request");
        }
        let size = line
            .split_whitespace()
            .map(str::parse::<u32>)
            .collect::<Result<Vec<_>, _>>()?;
        if size.len() != 2 {
            bail!("invalid viewport");
        }
        if last_size != (size[0], size[1]) {
            last_size = (size[0], size[1]);
            daemon
                .media
                .set_surface_dimensions(target, size[0], size[1])
                .await;
        }
        let frame = loop {
            tokio::select! {
                changed = receiver.changed() => { if changed.is_err() { return Ok(()); } }
                _ = reader.read_u8() => return Ok(()), // EOF while a publisher is paused.
            }
            if let Some(frame) = receiver.borrow_and_update().clone() {
                break frame;
            }
        };
        let writer = reader.get_mut();
        writer.write_all(&frame.width.to_le_bytes()).await?;
        writer.write_all(&frame.height.to_le_bytes()).await?;
        writer.write_all(&frame.data).await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target() -> RemoteVideoTarget {
        RemoteVideoTarget {
            participant: "Fixture".into(),
            source: wisp_protocol::VideoSource::Camera,
        }
    }
    #[test]
    fn only_explicitly_watched_targets_are_readable_and_slow_consumers_skip_frames() {
        let frames = VideoBridge::default();
        let target = target();
        assert!(frames.subscribe(&target).is_none());
        frames.open(target.clone());
        let mut receiver = frames.subscribe(&target).unwrap();
        for value in 0..20_u8 {
            frames.send(
                &target,
                RgbaFrame {
                    width: 1,
                    height: 1,
                    data: vec![value, 0, 0, 255],
                },
            );
        }
        assert_eq!(receiver.borrow_and_update().as_ref().unwrap().data[0], 19);
        assert!(!frames.unused(&target));
        drop(receiver);
        assert!(frames.unused(&target));
    }
    #[test]
    fn closing_or_leaving_revokes_readers_and_discards_frames() {
        let frames = VideoBridge::default();
        let target = target();
        frames.open(target.clone());
        let reader = frames.subscribe(&target).unwrap();
        frames.close(&target);
        assert!(reader.has_changed().is_err());
        frames.open(target.clone());
        let reader = frames.subscribe(&target).unwrap();
        frames.clear();
        assert!(reader.has_changed().is_err());
        assert!(!frames.contains(&target));
    }
}
