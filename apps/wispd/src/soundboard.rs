use crate::{ServerApi, decode, privacy, string_arg};
use anyhow::{Context, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    io::Read,
    sync::Mutex,
    time::{Duration, Instant},
};
use wisp_protocol::soundboard::{self as format, MAX_SAMPLES, MAX_WAV_BYTES};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "soundboard_list"
            | "soundboard_upload"
            | "soundboard_remove"
            | "soundboard_play"
            | "soundboard_preview"
            | "soundboard_stop"
            | "soundboard_status"
    )
}

pub(super) async fn catalog_command(
    api: &ServerApi,
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    match name {
        "soundboard_list" => {
            decode(
                api.request(reqwest::Method::GET, "/v1/soundboard")
                    .send()
                    .await?,
            )
            .await
        }
        "soundboard_remove" => {
            let id: uuid::Uuid = string_arg(args, "sound_id")?.parse()?;
            decode(
                api.request(reqwest::Method::DELETE, &format!("/v1/soundboard/{id}"))
                    .send()
                    .await?,
            )
            .await
        }
        "soundboard_upload" => {
            let name = string_arg(args, "name")?.trim().to_owned();
            ensure!(format::valid_name(&name), "Use a name of 1–32 characters");
            let path = privacy::local_path(&string_arg(args, "path")?)?;
            let samples = tokio::task::spawn_blocking(move || decode_file(&path)).await??;
            let wav =
                format::encode(&samples).context("Choose a clip between 0.01 and 10 seconds")?;
            decode(
                api.request(reqwest::Method::POST, "/v1/soundboard")
                    .json(&json!({"name":name,"data":STANDARD.encode(wav)}))
                    .send()
                    .await?,
            )
            .await
        }
        _ => bail!("Unknown soundboard action"),
    }
}

pub(super) async fn download(api: &ServerApi, args: &Value) -> anyhow::Result<Vec<i16>> {
    let id: uuid::Uuid = string_arg(args, "sound_id")?.parse()?;
    let mut response = api
        .request(reqwest::Method::GET, &format!("/v1/soundboard/{id}"))
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "Sound unavailable. Refresh this server's soundboard."
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= MAX_WAV_BYTES,
            "Server returned an oversized sound"
        );
        bytes.extend_from_slice(&chunk);
    }
    format::decode(&bytes).context("Server returned invalid sound audio")
}

fn decode_file(path: &std::path::Path) -> anyhow::Result<Vec<i16>> {
    let file = std::fs::File::open(path).context("Open sound file")?;
    ensure!(
        file.metadata()?.is_file() && file.metadata()?.len() <= 20 * 1024 * 1024,
        "Choose an audio file under 20 MB"
    );
    let mut bytes = Vec::new();
    file.take(20 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 20 * 1024 * 1024,
        "Choose an audio file under 20 MB"
    );
    // Accept audio containers only; never follow playlists or remote URIs.
    let audio_container = bytes.starts_with(b"RIFF")
        || bytes.starts_with(b"OggS")
        || bytes.starts_with(b"fLaC")
        || bytes.starts_with(b"ID3")
        || bytes
            .get(..2)
            .is_some_and(|p| p[0] == 0xff && p[1] & 0xe0 == 0xe0);
    ensure!(audio_container, "Choose a WAV, MP3, Ogg, or FLAC file");
    // Decode a bounded local copy in a separate process. Protocol restrictions
    // prevent a file from resolving external media; no decoder plugins are
    // required in the daemon. Both output size and process lifetime are bounded.
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("input.audio");
    let output = directory.path().join("output.pcm");
    std::fs::write(&input, bytes)?;
    let mut child = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "file",
            "-i",
        ])
        .arg(&input)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-sn",
            "-dn",
            "-t",
            "10.001",
            "-ac",
            "1",
            "-ar",
            "48000",
            "-c:a",
            "pcm_s16le",
            "-f",
            "s16le",
            "-y",
        ])
        .arg(&output)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Install ffmpeg to import WAV, MP3, Ogg, or FLAC sounds")?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < Duration::from_secs(15) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                result?;
                bail!("Sound decoding timed out");
            }
        }
    };
    ensure!(
        status.success(),
        "Could not decode this sound; choose a WAV, MP3, Ogg, or FLAC file"
    );
    let pcm = std::fs::read(output)?;
    ensure!(
        pcm.len() <= MAX_SAMPLES * 2,
        "Sounds must be 10 seconds or shorter"
    );
    let samples: Vec<i16> = pcm
        .chunks_exact(2)
        .map(|p| i16::from_le_bytes([p[0], p[1]]))
        .collect();
    ensure!(
        samples.len() >= 480,
        "Choose a clip between 0.01 and 10 seconds"
    );
    Ok(samples)
}

#[derive(Default)]
pub(crate) struct Mixer {
    state: Mutex<Option<Playing>>,
}

pub(crate) fn effect_sample(sample: i16, volume: u8) -> i16 {
    i16::try_from(i32::from(sample) * i32::from(volume.min(100)) * 55 / 10_000)
        .expect("soundboard effect stays in PCM range")
}
struct Playing {
    samples: Vec<i16>,
    offset: usize,
    volume: u8,
}
impl Mixer {
    pub fn play(&self, samples: Vec<i16>, volume: u8) {
        *self.state.lock().expect("soundboard lock") = Some(Playing {
            samples,
            offset: 0,
            volume: volume.min(100),
        });
    }
    pub fn stop(&self) {
        *self.state.lock().expect("soundboard lock") = None;
    }
    pub fn active(&self) -> bool {
        self.state.lock().expect("soundboard lock").is_some()
    }
    pub fn mix(&self, frame: &mut [i16]) {
        let mut state = self.state.lock().expect("soundboard lock");
        let Some(sound) = state.as_mut() else { return };
        for mic in frame {
            let Some(sample) = sound.samples.get(sound.offset) else {
                break;
            };
            // Retain voice underneath the effect with enough headroom that even
            // full-scale voice + sound cannot clip. No speech denoiser touches it.
            let mixed =
                i32::from(*mic) * 40 / 100 + i32::from(effect_sample(*sample, sound.volume));
            *mic = i16::try_from(mixed).expect("soundboard mix stays in PCM range");
            sound.offset += 1;
        }
        if sound.offset >= sound.samples.len() {
            *state = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_are_bounded_replaceable_and_do_not_clip_or_replay() {
        let mixer = Mixer::default();
        mixer.play(vec![32767; 480], 100);
        let mut frame = [32767; 480];
        mixer.mix(&mut frame);
        assert!(frame.iter().all(|v| *v < 32767 && *v > 30000));
        assert!(!mixer.active());
        mixer.play(vec![1000; 960], 100);
        mixer.play(vec![-1000; 480], 100);
        let mut frame = [0; 480];
        mixer.mix(&mut frame);
        assert!(frame.iter().all(|v| *v == -550));
        mixer.play(vec![1000; 480], 100);
        mixer.stop();
        let mut frame = [123; 480];
        mixer.mix(&mut frame);
        assert_eq!(frame, [123; 480]);
    }
    #[test]
    fn decode_wave_and_reject_long_or_non_audio_input() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sound.wav");
        std::fs::write(&path, format::encode(&vec![1234; 4800]).unwrap()).unwrap();
        assert_eq!(decode_file(&path).unwrap(), vec![1234; 4800]);
        let mut long = format::encode(&vec![1234; MAX_SAMPLES]).unwrap();
        long.extend_from_slice(&[0; 960]);
        let size = u32::try_from(long.len()).unwrap();
        long[4..8].copy_from_slice(&(size - 8).to_le_bytes());
        long[40..44].copy_from_slice(&(size - 44).to_le_bytes());
        std::fs::write(&path, long).unwrap();
        assert!(
            decode_file(&path)
                .unwrap_err()
                .to_string()
                .contains("10 seconds")
        );
        std::fs::write(&path, b"#EXTM3U\nhttps://example.invalid/audio").unwrap();
        assert!(decode_file(&path).is_err());
    }
}
