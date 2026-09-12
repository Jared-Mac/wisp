//! Native account authentication, private durable journals and encrypted backup.
//! Passwords/export keys remain transient; only deliberately encoded private
//! device state may be persisted. Public IPC reports never serialize this state.
pub(super) mod api;
pub(super) mod store;
