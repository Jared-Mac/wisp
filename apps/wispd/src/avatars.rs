use super::{ServerApi, privacy, string_arg};
use anyhow::{bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::io::{Cursor, Read};

pub(super) async fn command(api: &ServerApi, name: &str, args: &Value) -> anyhow::Result<Value> {
    let request = match name {
        "upload_avatar" => {
            let path = privacy::local_path(&string_arg(args, "path")?)?;
            let bytes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
                let file = std::fs::File::open(path)?;
                ensure!(file.metadata()?.is_file(), "Choose an image file");
                let mut bytes = Vec::new();
                file.take(2_097_153).read_to_end(&mut bytes)?;
                ensure!(bytes.len() <= 2_097_152, "Choose an image under 2 MB");
                Ok(bytes)
            })
            .await??;
            api.request(reqwest::Method::POST, "/v1/accounts/avatar")
                .json(&json!({"data":STANDARD.encode(bytes)}))
        }
        "remove_avatar" => api.request(reqwest::Method::DELETE, "/v1/accounts/avatar"),
        "avatar_image" => {
            let user: uuid::Uuid = string_arg(args, "user_id")?.parse()?;
            api.request(reqwest::Method::GET, &format!("/v1/accounts/{user}/avatar"))
        }
        _ => bail!("Unknown avatar action"),
    };
    let mut response = request
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Could not reach the profile server"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        if name == "avatar_image" {
            return Ok(json!({"url":""}));
        }
        bail!("This server needs an update before profile pictures are available");
    }
    if !response.status().is_success() {
        bail!(
            "Could not save the profile picture. Choose a PNG, JPEG, GIF or WebP under 2 MB, up to 4096 × 4096"
        );
    }
    if name != "avatar_image" {
        return Ok(json!({"ok":true}));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 512 * 1024,
            "Profile picture is too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    let png = tokio::task::spawn_blocking(move || round_image(bytes)).await??;
    Ok(json!({"url":format!("data:image/png;base64,{}",STANDARD.encode(png))}))
}

// Decode again at the client trust boundary, discard metadata, and apply a
// circular mask here so every QML surface can display the same safe thumbnail.
fn round_image(bytes: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(256);
    limits.max_image_height = Some(256);
    limits.max_alloc = Some(1024 * 1024);
    reader.limits(limits);
    let mut image = reader
        .decode()?
        .resize_to_fill(128, 128, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let radius = ((f64::from(x) - 63.5).powi(2) + (f64::from(y) - 63.5).powi(2)).sqrt();
        let coverage = (64.0 - radius).clamp(0.0, 1.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // Alpha is 0–255 and coverage is clamped to 0–1.
        let alpha = (f64::from(pixel[3]) * coverage).round() as u8;
        pixel[3] = alpha;
    }
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn avatar_cache_rejects_oversized_images_and_masks_corners() {
        let mut png = Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(256, 256, image::Rgba([255, 60, 20, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let result = image::load_from_memory(&round_image(png.into_inner()).unwrap())
            .unwrap()
            .into_rgba8();
        assert_eq!(result.get_pixel(0, 0)[3], 0);
        assert_eq!(result.get_pixel(64, 64)[3], 255);
        assert_eq!(result.dimensions(), (128, 128));
        let mut large = Cursor::new(Vec::new());
        image::RgbaImage::new(257, 256)
            .write_to(&mut large, image::ImageFormat::Png)
            .unwrap();
        assert!(round_image(large.into_inner()).is_err());
        assert!(round_image(b"<svg/>".to_vec()).is_err());
    }
}
