//! Native account authentication, private durable journals and encrypted backup.
//! Passwords/export keys remain transient; only deliberately encoded private
//! device state may be persisted. Public IPC reports never serialize this state.
pub(super) mod api;
mod enrollment;
mod login;
mod mutations;
pub(super) mod reset;
pub(super) mod store;
mod sync;
#[cfg(test)]
mod tests;
