// Wisp modification, 2026. Licensed under Apache-2.0; see NOTICE.md.
/// WebRTC returns -1 when its audio device module cannot enumerate devices.
/// Casting that to usize creates an effectively endless allocating iterator.
pub(crate) fn checked_device_count(count: i16) -> usize {
    usize::try_from(count).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn enumeration_errors_are_empty_not_unsigned_wraparound() {
        for count in i16::MIN..=i16::MAX {
            let actual = super::checked_device_count(count);
            if count < 0 {
                assert_eq!(actual, 0);
            } else {
                assert_eq!(actual, usize::from(u16::try_from(count).unwrap()));
            }
        }
        assert_eq!((0..super::checked_device_count(-1)).count(), 0);
    }
}
