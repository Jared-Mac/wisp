use anyhow::{Context, bail};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::PathBuf,
    sync::Arc,
    time::SystemTime,
};
use tokio::sync::Mutex;
use uuid::Uuid;
use wisp_protocol::{MAX_CHAT_DRAFTS, MAX_CHAT_IMAGE_BYTES, MAX_CHAT_IMAGE_PIXELS};

#[derive(Clone)]
pub(crate) struct AttachmentDraft {
    pub(crate) bytes: Vec<u8>,
    pub(crate) file_name: String,
    pub(crate) is_image: bool,
    pub(crate) size: u64,
    source: Option<Arc<std::fs::File>>,
    modified: Option<SystemTime>,
}

impl AttachmentDraft {
    pub(crate) fn reader(self) -> DraftReader {
        DraftReader {
            draft: self,
            offset: 0,
        }
    }
    pub(crate) async fn chunk(&self, offset: u64) -> anyhow::Result<Vec<u8>> {
        let length = usize::try_from(
            self.size
                .saturating_sub(offset)
                .min(wisp_protocol::CHAT_FILE_CHUNK_BYTES as u64),
        )?;
        if let Some(source) = self.source.clone() {
            let size = self.size;
            let modified = self.modified;
            tokio::task::spawn_blocking(move || {
                use std::os::unix::fs::FileExt;
                let validate = || -> anyhow::Result<()> {
                    let metadata = source.metadata()?;
                    if metadata.len() != size || metadata.modified().ok() != modified {
                        bail!("File changed since it was attached; remove it and attach again");
                    }
                    Ok(())
                };
                validate()?;
                let mut bytes = vec![0; length];
                source.read_exact_at(&mut bytes, offset)?;
                validate()?;
                Ok(bytes)
            })
            .await?
        } else {
            let offset = usize::try_from(offset)?;
            Ok(self
                .bytes
                .get(offset..offset + length)
                .context("Invalid attachment offset")?
                .to_vec())
        }
    }
}

pub(crate) struct DraftReader {
    draft: AttachmentDraft,
    offset: u64,
}
impl Read for DraftReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let length = buffer.len().min(
            usize::try_from(self.draft.size.saturating_sub(self.offset)).unwrap_or(usize::MAX),
        );
        if let Some(file) = &self.draft.source {
            let metadata = file.metadata()?;
            if metadata.len() != self.draft.size || metadata.modified().ok() != self.draft.modified
            {
                return Err(std::io::Error::other(
                    "Attachment changed during encryption",
                ));
            }
            std::os::unix::fs::FileExt::read_exact_at(
                file.as_ref(),
                &mut buffer[..length],
                self.offset,
            )?;
        } else {
            let offset = usize::try_from(self.offset).map_err(std::io::Error::other)?;
            let bytes = self
                .draft
                .bytes
                .get(offset..offset + length)
                .ok_or_else(|| std::io::Error::other("Invalid attachment length"))?;
            buffer[..length].copy_from_slice(bytes);
        }
        self.offset += length as u64;
        Ok(length)
    }
}

fn read_dropped_file(uri: &str) -> anyhow::Result<AttachmentDraft> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = url::Url::parse(uri)?
        .to_file_path()
        .map_err(|()| anyhow::anyhow!("Drop local files, not web links"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("File has no supported filename")?
        .to_owned();
    if !wisp_protocol::valid_chat_file_name(&file_name) {
        bail!("Filename must be 1–200 bytes without control characters");
    }
    // Do not follow links or read directories/devices/pipes from a drop.
    if !std::fs::symlink_metadata(&path)?.file_type().is_file() {
        bail!("Drop regular files, not folders or symbolic links");
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .context("Open dropped file")?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        bail!("Drop regular files only");
    }
    let mut bytes = Vec::new();
    if metadata.len() <= MAX_CHAT_IMAGE_BYTES as u64 {
        (&file)
            .take(MAX_CHAT_IMAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
    }
    let mut is_image = matches!(
        image::guess_format(&bytes),
        Ok(image::ImageFormat::Png
            | image::ImageFormat::Jpeg
            | image::ImageFormat::Gif
            | image::ImageFormat::WebP)
    );
    if is_image {
        let preview = (|| -> anyhow::Result<Vec<u8>> {
            let mut reader = image::ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(16384);
            limits.max_image_height = Some(16384);
            limits.max_alloc = Some(128 * 1024 * 1024);
            reader.limits(limits);
            let decoded = reader.decode().context("Decode dropped image")?;
            if u64::from(decoded.width()) * u64::from(decoded.height()) > MAX_CHAT_IMAGE_PIXELS {
                bail!("Image exceeds 32 megapixels");
            }
            let mut png = Cursor::new(Vec::new());
            decoded.write_to(&mut png, image::ImageFormat::Png)?;
            let bytes = png.into_inner();
            if bytes.len() > MAX_CHAT_IMAGE_BYTES {
                bail!("Converted image exceeds 12 MB");
            }
            Ok(bytes)
        })();
        if let Ok(png) = preview {
            bytes = png;
        } else {
            is_image = false;
        }
    }
    if !is_image {
        bytes.clear();
    }
    let size = if is_image {
        bytes.len() as u64
    } else {
        metadata.len()
    };
    Ok(AttachmentDraft {
        bytes,
        file_name,
        is_image,
        size,
        source: if is_image { None } else { Some(Arc::new(file)) },
        modified: metadata.modified().ok(),
    })
}

#[derive(Default)]
pub(crate) struct ImageStore {
    drafts: Mutex<HashMap<Uuid, AttachmentDraft>>,
    pub(crate) cache_operation: Mutex<()>,
    // Linux clipboard ownership must outlive the copy request and viewer window.
    clipboard: Arc<std::sync::Mutex<Option<arboard::Clipboard>>>,
}

fn clipboard_image(bytes: Vec<u8>) -> anyhow::Result<arboard::ImageData<'static>> {
    if bytes.len() > MAX_CHAT_IMAGE_BYTES {
        bail!("Image exceeds 12 MB");
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().context("Decode image for clipboard")?;
    if u64::from(decoded.width()) * u64::from(decoded.height()) > MAX_CHAT_IMAGE_PIXELS {
        bail!("Image exceeds 32 megapixels");
    }
    let pixels = decoded.to_rgba8();
    Ok(arboard::ImageData {
        width: pixels.width() as usize,
        height: pixels.height() as usize,
        bytes: std::borrow::Cow::Owned(pixels.into_raw()),
    })
}

fn cache_directory(conversation_id: Option<&str>) -> anyhow::Result<PathBuf> {
    use std::fmt::Write as _;
    use std::os::unix::fs::PermissionsExt;
    let directory = if let Some(conversation_id) = conversation_id {
        let key = conversation_id
            .bytes()
            .fold(String::new(), |mut key, byte| {
                let _ = write!(key, "{byte:02x}");
                key
            });
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
            .context("no cache directory available")?
            .join("wisp/chat-images")
            .join(key)
    } else {
        PathBuf::from(
            std::env::var_os("XDG_RUNTIME_DIR").context("no runtime directory available")?,
        )
        .join("wisp/chat-drafts")
    };
    std::fs::create_dir_all(&directory)?;
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
    Ok(directory)
}

pub(crate) fn cache_path(id: Uuid, conversation_id: Option<&str>) -> anyhow::Result<PathBuf> {
    Ok(cache_directory(conversation_id)?.join(format!("{id}.png")))
}

pub(crate) fn clear_cache(conversation_id: &str) -> anyhow::Result<()> {
    // Only this conversation's generated image files, including older pages.
    for entry in std::fs::read_dir(cache_directory(Some(conversation_id))?)? {
        let path = entry?.path();
        if path
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| Uuid::parse_str(name).ok())
            .is_some()
            && matches!(
                path.extension().and_then(|name| name.to_str()),
                Some("png" | "part")
            )
        {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub(crate) fn file_url(path: &std::path::Path) -> anyhow::Result<String> {
    Ok(url::Url::from_file_path(path)
        .map_err(|()| anyhow::anyhow!("invalid image path"))?
        .into())
}

impl ImageStore {
    pub(crate) async fn copy_image(&self, path: PathBuf) -> anyhow::Result<()> {
        let owner = self.clipboard.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(MAX_CHAT_IMAGE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            let image = clipboard_image(bytes)?;
            let mut clipboard = owner
                .lock()
                .map_err(|_| anyhow::anyhow!("Clipboard lock unavailable"))?;
            if clipboard.is_none() {
                *clipboard = Some(arboard::Clipboard::new().context("Open clipboard")?);
            }
            clipboard
                .as_mut()
                .context("Clipboard unavailable")?
                .set_image(image)
                .context("Copy image to clipboard")?;
            Ok(())
        })
        .await??;
        Ok(())
    }
    pub(crate) async fn paste(&self) -> anyhow::Result<Value> {
        let value = tokio::task::spawn_blocking(|| -> anyhow::Result<(Option<Vec<u8>>, String)> {
            let mut clipboard = arboard::Clipboard::new().context("open clipboard")?;
            if let Ok(image) = clipboard.get_image() {
                if image.width as u64 * image.height as u64 > MAX_CHAT_IMAGE_PIXELS {
                    bail!("Screenshot exceeds 32 megapixels");
                }
                let pixels = image::RgbaImage::from_raw(
                    u32::try_from(image.width)?,
                    u32::try_from(image.height)?,
                    image.bytes.into_owned(),
                )
                .context("invalid clipboard pixels")?;
                let mut png = Cursor::new(Vec::new());
                image::DynamicImage::ImageRgba8(pixels)
                    .write_to(&mut png, image::ImageFormat::Png)?;
                Ok((Some(png.into_inner()), String::new()))
            } else {
                let text = clipboard
                    .get_text()
                    .context("Clipboard has no supported image or text")?;
                Ok((None, text))
            }
        })
        .await??;
        let Some(png) = value.0 else {
            return Ok(json!({"text": value.1}));
        };
        let mut staged = self
            .stage(vec![AttachmentDraft {
                size: png.len() as u64,
                is_image: png.len() <= MAX_CHAT_IMAGE_BYTES,
                bytes: png,
                file_name: "Screenshot.png".into(),
                source: None,
                modified: None,
            }])
            .await?;
        Ok(staged.remove(0))
    }

    pub(crate) async fn import_files(&self, urls: Vec<String>) -> anyhow::Result<Value> {
        if urls.is_empty() || urls.len() > MAX_CHAT_DRAFTS {
            bail!("Drop between 1 and 8 files at a time");
        }
        let values = tokio::task::spawn_blocking(move || {
            urls.iter()
                .map(|uri| read_dropped_file(uri))
                .collect::<anyhow::Result<Vec<_>>>()
        })
        .await??;
        Ok(json!({"attachments": self.stage(values).await?}))
    }

    async fn stage(&self, values: Vec<AttachmentDraft>) -> anyhow::Result<Vec<Value>> {
        let mut drafts = self.drafts.lock().await;
        if drafts.len() + values.len() > MAX_CHAT_DRAFTS {
            bail!("Remove or send another pending attachment first (8 draft limit)");
        }
        let mut staged = Vec::new();
        let mut previews = Vec::new();
        for draft in values {
            let token = Uuid::new_v4();
            let preview = if draft.is_image {
                (|| -> anyhow::Result<String> {
                    let path = cache_path(token, None)?;
                    previews.push(path.clone());
                    std::fs::write(&path, &draft.bytes)?;
                    file_url(&path)
                })()
            } else {
                Ok(String::new())
            };
            let url = match preview {
                Ok(url) => url,
                Err(error) => {
                    for path in previews {
                        let _ = std::fs::remove_file(path);
                    }
                    return Err(error);
                }
            };
            staged.push((token, draft, url));
        }
        let mut result = Vec::new();
        for (token, draft, url) in staged {
            result.push(
                json!({"token": token, "url": url, "file_name": draft.file_name,
                "size": draft.size, "is_image": draft.is_image}),
            );
            drafts.insert(token, draft);
        }
        Ok(result)
    }

    pub(crate) async fn draft(&self, token: Uuid) -> anyhow::Result<AttachmentDraft> {
        self.drafts
            .lock()
            .await
            .get(&token)
            .cloned()
            .context("Attachment draft expired; attach it again")
    }

    pub(crate) async fn discard(&self, token: Uuid) {
        if self.drafts.lock().await.remove(&token).is_some()
            && let Ok(path) = cache_path(token, None)
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl Drop for ImageStore {
    fn drop(&mut self) {
        for token in self.drafts.get_mut().keys() {
            if let Ok(path) = cache_path(*token, None) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn drops_accept_regular_files_and_reject_unsafe_sources() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.txt");
        std::fs::write(&path, b"hello").unwrap();
        let draft = read_dropped_file(&file_url(&path).unwrap()).unwrap();
        assert_eq!(draft.chunk(0).await.unwrap(), b"hello");
        assert!(draft.bytes.is_empty());
        assert_eq!(draft.file_name, "notes.txt");
        assert!(!draft.is_image);
        assert!(read_dropped_file("https://example.com/notes.txt").is_err());
        assert!(read_dropped_file(&file_url(directory.path()).unwrap()).is_err());
        let symlink = directory.path().join("symlink.txt");
        std::os::unix::fs::symlink(&path, &symlink).unwrap();
        assert!(read_dropped_file(&file_url(&symlink).unwrap()).is_err());
        std::fs::File::create(&path)
            .unwrap()
            .set_len(10_000_000_000)
            .unwrap();
        let large = read_dropped_file(&file_url(&path).unwrap()).unwrap();
        assert_eq!(large.size, 10_000_000_000);
        assert!(large.bytes.is_empty());
        assert_eq!(large.chunk(9_999_999_999).await.unwrap(), [0]);
        assert!(draft.chunk(0).await.is_err());
    }

    #[test]
    fn clipboard_conversion_preserves_native_pixels_and_alpha() {
        let original = image::RgbaImage::from_fn(3, 2, |x, y| {
            image::Rgba([
                u8::try_from(x).unwrap() * 40,
                u8::try_from(y).unwrap() * 60,
                123,
                127,
            ])
        });
        let mut png = Cursor::new(Vec::new());
        original
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let copied = clipboard_image(png.into_inner()).unwrap();
        assert_eq!((copied.width, copied.height), (3, 2));
        assert_eq!(copied.bytes.as_ref(), original.as_raw());
        assert!(clipboard_image(b"not an image".to_vec()).is_err());
        assert!(clipboard_image(vec![0; MAX_CHAT_IMAGE_BYTES + 1]).is_err());
    }

    #[test]
    fn dropped_images_are_normalized_to_png() {
        let directory = tempfile::tempdir().unwrap();
        let image = image::RgbImage::from_pixel(2, 3, image::Rgb([10, 20, 30]));
        for (name, format) in [
            ("image.jpg", image::ImageFormat::Jpeg),
            ("image.png", image::ImageFormat::Png),
            ("image.gif", image::ImageFormat::Gif),
            ("image.webp", image::ImageFormat::WebP),
        ] {
            let path = directory.path().join(name);
            image.save_with_format(&path, format).unwrap();
            let draft = read_dropped_file(&file_url(&path).unwrap()).unwrap();
            assert!(draft.is_image);
            assert_eq!(
                image::guess_format(&draft.bytes).unwrap(),
                image::ImageFormat::Png
            );
            let decoded = image::load_from_memory(&draft.bytes).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (2, 3));
        }
    }

    #[tokio::test]
    async fn file_batches_are_atomic_bounded_and_discardable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.txt");
        std::fs::write(&path, b"hello").unwrap();
        let uri = file_url(&path).unwrap();
        let store = ImageStore::default();
        assert!(
            store
                .import_files(vec![uri.clone(), "https://example.com/file".into()])
                .await
                .is_err()
        );
        assert!(store.drafts.lock().await.is_empty());
        let result = store
            .import_files(vec![uri.clone(); MAX_CHAT_DRAFTS])
            .await
            .unwrap();
        assert_eq!(
            result["attachments"].as_array().unwrap().len(),
            MAX_CHAT_DRAFTS
        );
        assert!(store.import_files(vec![uri]).await.is_err());
        let token = result["attachments"][0]["token"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            store.draft(token).await.unwrap().chunk(0).await.unwrap(),
            b"hello"
        );
        store.discard(token).await;
        assert!(store.draft(token).await.is_err());
    }
}
