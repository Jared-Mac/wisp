//! Bounded, canonical audio for server soundboards. No decoder is needed on the server.
pub const SAMPLE_RATE: u32 = 48_000;
pub const MAX_SAMPLES: usize = SAMPLE_RATE as usize * 10;
pub const MAX_WAV_BYTES: usize = 44 + MAX_SAMPLES * 2;
pub const MAX_SOUNDS: i64 = 64;

#[must_use]
pub fn valid_name(name: &str) -> bool {
    let count = name.chars().count();
    (1..=32).contains(&count) && name.trim() == name && !name.chars().any(char::is_control)
}

#[must_use]
pub fn encode(samples: &[i16]) -> Option<Vec<u8>> {
    if samples.is_empty() || samples.len() > MAX_SAMPLES {
        return None;
    }
    let size = u32::try_from(samples.len() * 2).ok()?;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + size).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(b"\x02\0\x10\0data");
    out.extend_from_slice(&size.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    Some(out)
}

#[must_use]
pub fn validate(bytes: &[u8]) -> Option<u32> {
    if !(46..=MAX_WAV_BYTES).contains(&bytes.len()) || !bytes.len().is_multiple_of(2) {
        return None;
    }
    let data = u32::try_from(bytes.len() - 44).ok()?;
    let header = encode(&[0])?;
    if bytes[..4] != *b"RIFF"
        || bytes[8..40] != header[8..40]
        || bytes[4..8] != (data + 36).to_le_bytes()
        || bytes[40..44] != data.to_le_bytes()
    {
        return None;
    }
    Some(data / 2 * 1000 / SAMPLE_RATE)
}

#[must_use]
pub fn decode(bytes: &[u8]) -> Option<Vec<i16>> {
    validate(bytes)?;
    Some(
        bytes[44..]
            .chunks_exact(2)
            .map(|p| i16::from_le_bytes([p[0], p[1]]))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_audio_is_bounded_and_bit_exact() {
        let samples = [-32768, -1, 0, 1, 32767];
        let wav = encode(&samples).unwrap();
        assert_eq!(decode(&wav).unwrap(), samples);
        assert!(encode(&[]).is_none());
        assert!(encode(&vec![0; MAX_SAMPLES + 1]).is_none());
        assert_eq!(
            validate(&encode(&vec![0; MAX_SAMPLES]).unwrap()),
            Some(10_000)
        );
        for offset in [0, 4, 8, 20, 22, 24, 28, 32, 34, 36, 40] {
            let mut bad = wav.clone();
            bad[offset] ^= 1;
            assert!(validate(&bad).is_none());
        }
        assert!(validate(&wav[..wav.len() - 1]).is_none());
        assert!(valid_name("Applause 👏"));
        assert!(!valid_name(" "));
        assert!(!valid_name("bad\nname"));
    }
}
