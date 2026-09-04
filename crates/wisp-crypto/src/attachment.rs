//! Large attachments never require an in-memory plaintext/ciphertext copy.
//! A manifest must be obtained ONLY from an authenticated signed message.
use crate::{Identity, PublicIdentity, encrypt_stream};
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use tempfile::NamedTempFile;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub ciphertext_sha256: [u8; 32],
    pub ciphertext_bytes: u64,
    pub plaintext_bytes: u64,
}

pub struct Prepared {
    pub file: NamedTempFile,
    pub manifest: Manifest,
}

struct HashWriter<W> {
    inner: W,
    hash: Sha256,
    bytes: u64,
}
impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let count = self.inner.write(buffer)?;
        self.hash.update(&buffer[..count]);
        self.bytes = self
            .bytes
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("Attachment length overflow"))?;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub fn prepare(
    input: impl Read,
    recipients: &[PublicIdentity],
    private_directory: &Path,
) -> anyhow::Result<Prepared> {
    let mut file = NamedTempFile::new_in(private_directory)?;
    let mut writer = HashWriter {
        inner: file.as_file_mut(),
        hash: Sha256::new(),
        bytes: 0,
    };
    let plaintext_bytes = encrypt_stream(input, &mut writer, recipients)?;
    writer.flush()?;
    let manifest = Manifest {
        plaintext_bytes,
        ciphertext_bytes: writer.bytes,
        ciphertext_sha256: writer.hash.finalize().into(),
    };
    file.as_file().sync_all()?;
    file.as_file_mut().seek(SeekFrom::Start(0))?;
    Ok(Prepared { file, manifest })
}

/// Nothing is exposed at a final destination until the caller persists this
/// verified temporary file. Failure drops/unlinks partial plaintext, including
/// authentication failure at EOF. Never pass an unsigned server manifest here.
pub fn decrypt(
    identity: &Identity,
    mut ciphertext: impl Read + Seek,
    manifest: &Manifest,
    private_directory: &Path,
) -> anyhow::Result<NamedTempFile> {
    ciphertext.seek(SeekFrom::Start(0))?;
    let mut hash = HashWriter {
        inner: std::io::sink(),
        hash: Sha256::new(),
        bytes: 0,
    };
    std::io::copy(&mut ciphertext, &mut hash)?;
    ensure!(
        hash.bytes == manifest.ciphertext_bytes,
        "Encrypted attachment size mismatch"
    );
    let digest: [u8; 32] = hash.hash.finalize().into();
    ensure!(
        digest == manifest.ciphertext_sha256,
        "Encrypted attachment does not match the sender's signed manifest"
    );
    ciphertext.seek(SeekFrom::Start(0))?;
    let mut plaintext = NamedTempFile::new_in(private_directory)?;
    let bytes = identity.decrypt_stream(ciphertext, plaintext.as_file_mut())?;
    ensure!(
        bytes == manifest.plaintext_bytes,
        "Decrypted attachment size mismatch"
    );
    plaintext.as_file().sync_all()?;
    plaintext.as_file_mut().seek(SeekFrom::Start(0))?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn verifies_file_signature_binding_and_leaves_no_partial_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let sender = Identity::generate().unwrap();
        let recipient = Identity::generate().unwrap();
        let content = vec![73; 5 * 1024 * 1024 + 17];
        let prepared = prepare(
            content.as_slice(),
            &[sender.public(), recipient.public()],
            dir.path(),
        )
        .unwrap();
        let mut plain = decrypt(
            &recipient,
            prepared.file.reopen().unwrap(),
            &prepared.manifest,
            dir.path(),
        )
        .unwrap();
        assert_eq!(
            plain.as_file().metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
        let mut output = Vec::new();
        plain.read_to_end(&mut output).unwrap();
        assert_eq!(output, content);
        drop(plain);
        let before = std::fs::read_dir(dir.path()).unwrap().count();
        let different = prepare(&b"substituted"[..], &[recipient.public()], dir.path()).unwrap();
        assert!(
            decrypt(
                &recipient,
                different.file.reopen().unwrap(),
                &prepared.manifest,
                dir.path()
            )
            .is_err()
        );
        drop(different);
        let mut invalid = prepared.manifest.clone();
        invalid.plaintext_bytes += 1;
        assert!(
            decrypt(
                &recipient,
                prepared.file.reopen().unwrap(),
                &invalid,
                dir.path()
            )
            .is_err()
        );
        let mut bytes = std::fs::read(prepared.file.path()).unwrap();
        bytes.pop();
        assert!(
            decrypt(
                &recipient,
                Cursor::new(bytes),
                &prepared.manifest,
                dir.path()
            )
            .is_err()
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), before);
    }
}
