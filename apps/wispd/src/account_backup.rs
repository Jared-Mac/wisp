//! Native account authentication, private durable journals and encrypted backup.
//! Passwords/export keys remain transient; only deliberately encoded private
//! device state may be persisted. Public IPC reports never serialize this state.
#[path = "account_backup/api.rs"]
pub(super) mod api;
#[path = "account_backup/catalog.rs"]
mod catalog;
#[path = "account_backup/enrollment.rs"]
mod enrollment;
#[path = "account_backup/install.rs"]
pub(super) mod install;
#[path = "account_backup/login.rs"]
mod login;
#[path = "account_backup/mutations.rs"]
mod mutations;
#[path = "account_backup/recovery.rs"]
mod recovery;
#[path = "account_backup/reset.rs"]
pub(super) mod reset;
#[path = "account_backup/store.rs"]
pub(super) mod store;
#[path = "account_backup/sync.rs"]
mod sync;
#[cfg(test)]
#[path = "account_backup/tests.rs"]
pub(crate) mod tests;

pub(crate) fn default_root() -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context;
    let root = if let Some(path) = std::env::var_os("WISP_PRIVACY_DIR") {
        std::path::PathBuf::from(path)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
            })
            .context("A private account configuration directory is required")?
            .join("wisp/privacy")
    };
    anyhow::ensure!(
        root.is_absolute(),
        "Private account storage must be absolute"
    );
    Ok(root)
}
