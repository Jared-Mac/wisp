# Wisp LiveKit lifecycle patch

Vendored from the crates.io `livekit` 0.8.4 release (Apache-2.0).
Original crate checksum: `d591ed539d9d3f6a268c7b8c8636a1239882e99d453ae9ebbe19f9579d39cb44`.

In `src/rtc_engine/mod.rs`: explicitly close
an RTC session when its initial peer-connection wait fails, before returning the
error or retrying. At this point signaling and background RTC tasks have already
been created; an ordinary drop does not perform the asynchronous close.

In `src/platform_audio/`: convert native signed device counts with a checked
conversion. WebRTC returns -1 on enumeration failure; a direct `as usize` cast
turned that into an enormous iterator, growing the collected device list until
the desktop ran out of memory. The live diagnostic backtrace was inside that
iterator. The added helper has an exhaustive signed-16-bit regression test.

Keep this patch narrow and remove it when the pinned upstream version includes
equivalent cleanup. Wisp additionally disables automatic repeated failed joins
and applies desktop service memory budgets. This fixes a confirmed cleanup gap,
and prevents the confirmed signed-count allocation runaway.
