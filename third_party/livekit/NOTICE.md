# Wisp LiveKit lifecycle patch

Vendored from the crates.io `livekit` 0.8.4 release (Apache-2.0).
Original crate checksum: `d591ed539d9d3f6a268c7b8c8636a1239882e99d453ae9ebbe19f9579d39cb44`.

The only upstream source change is in `src/rtc_engine/mod.rs`: explicitly close
an RTC session when its initial peer-connection wait fails, before returning the
error or retrying. At this point signaling and background RTC tasks have already
been created; an ordinary drop does not perform the asynchronous close.

Keep this patch narrow and remove it when the pinned upstream version includes
equivalent cleanup. Wisp additionally disables automatic repeated failed joins
and applies desktop service memory budgets. This fixes a confirmed cleanup gap,
but does not prove it was the sole cause of the observed out-of-memory incident.
