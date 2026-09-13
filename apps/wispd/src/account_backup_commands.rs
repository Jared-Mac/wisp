//! Desktop IPC exposes only public backup state. Native secrets stay below this
//! boundary, and recovering a device installs it before publishing its session.
use crate::{
    AuthMethod, ServerApi,
    account_backup::{
        api::{Api, Status},
        store::Kind,
    },
    privacy::Privacy,
};
use anyhow::{Context, ensure};
use serde_json::{Value, json};
use wisp_crypto::SecretString;

pub(crate) fn handles(name: &str) -> bool {
    matches!(
        name,
        "backup_status"
            | "backup_upgrade"
            | "backup_enable"
            | "backup_sync"
            | "backup_unlock"
            | "backup_repair"
            | "backup_resume"
            | "backup_cancel"
            | "backup_auto_sync"
            | "backup_recover_secure"
            | "backup_recover_classic"
            | "backup_finish_install"
    )
}
pub(crate) async fn connect(server: &ServerApi, privacy: &Privacy) -> anyhow::Result<Api> {
    let auth = server.auth.read().expect("account credential lock").clone();
    let AuthMethod::Device { device_id, .. } = auth else {
        anyhow::bail!("Account backup requires a signed-in Wisp account")
    };
    let token = server.token.read().expect("account session lock").clone();
    Api::connect(
        &server.base_url,
        privacy.backup_store()?,
        Some(device_id),
        Some(token),
    )
    .await
}
fn password(args: &Value, name: &str) -> anyhow::Result<SecretString> {
    let value = args
        .get(name)
        .and_then(Value::as_str)
        .context("Enter your password")?;
    ensure!(
        !value.is_empty() && value.len() <= 1024,
        "Enter a valid password"
    );
    Ok(SecretString::from(value.to_owned()))
}
fn optional_password(args: &Value) -> Option<SecretString> {
    args.get("password")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 1024)
        .map(|value| SecretString::from(value.to_owned()))
}
fn local_status(store: &crate::account_backup::store::Store) -> anyhow::Result<Value> {
    let record = store.record()?;
    Ok(
        json!({"mode":if record.secure {"secure"} else {"unknown"},"username":record.username,
        "identity_ready":record.bundle()?.is_some(),"unlocked":record.key_verified,
        "repair_needed":record.expected.as_ref().is_some_and(|e|e.credential_generation!=e.wrapper_generation),
        "pending":record.pending.as_ref().map(|p|p.kind),"pending_sent":record.pending.as_ref().is_some_and(|p|p.sent),
        "can_cancel":record.pending.as_ref().is_some_and(|p|!p.sent && p.recovery.is_empty()),
        "auto_sync":record.auto_sync,"has_changes":record.dirty!=record.synced_dirty,"last_synced_at":record.last_sync,
        "installation_pending":record.installation_pending && record.installation_ready}),
    )
}
pub(crate) async fn public_status(api: &Api) -> anyhow::Result<Value> {
    let state = api.status_for_recovery(true).await;
    let mut value = local_status(&api.store)?;
    match state {
        Ok(state) => {
            value["mode"] = json!(if matches!(state, Status::Secure { .. }) {
                "secure"
            } else {
                "classic"
            });
            value["username"] = json!(state.username());
            value["repair_needed"] = json!(
                state.expected().credential_generation != state.expected().wrapper_generation
            );
        }
        Err(error) => value["error"] = json!(error.to_string()),
    }
    Ok(value)
}

pub(crate) async fn command(
    server: &ServerApi,
    privacy: &Privacy,
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    let store = privacy.backup_store()?;
    let _action = store.action()?;
    let mut api = match connect(server, privacy).await {
        Ok(api) => api,
        Err(error) if name == "backup_status" => {
            let mut value = local_status(&store)?;
            value["error"] = json!(error.to_string());
            return Ok(value);
        }
        Err(error) => return Err(error),
    };
    match run(&mut api, server, privacy, name, args).await {
        Err(error)
            if error
                .downcast_ref::<crate::account_backup::api::Failure>()
                .is_some_and(|failure| failure.status == 401) =>
        {
            server.renew_session().await?;
            let mut api = connect(server, privacy).await?;
            run(&mut api, server, privacy, name, args).await
        }
        result => result,
    }
}
#[allow(clippy::too_many_lines)] // One dispatcher shares the completion/install boundary.
async fn run(
    api: &mut Api,
    server: &ServerApi,
    privacy: &Privacy,
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    match name {
        "backup_status" => {
            let mut value = public_status(api).await?;
            value["sync_error"] = json!(privacy.backup_error());
            return Ok(value);
        }
        "backup_auto_sync" => {
            let enabled = args
                .get("enabled")
                .and_then(Value::as_bool)
                .context("Choose whether to sync automatically")?;
            api.store.update(None, |record| {
                record.auto_sync = enabled;
                Ok(())
            })?;
        }
        "backup_upgrade" => {
            // The legacy proof checks the current password; native registration
            // uses that same secret locally. No password reset or identity change.
            let current = password(args, "current_password")?;
            api.migrate(current.clone(), current).await?;
        }
        "backup_enable" => {
            api.migrate(
                password(args, "current_password")?,
                password(args, "new_password")?,
            )
            .await?;
        }
        "change_account_password" => {
            api.password_change(
                password(args, "current_password")?,
                password(args, "new_password")?,
            )
            .await?;
        }
        "set_recovery_email" => {
            api.recovery_email(
                args.get("email")
                    .and_then(Value::as_str)
                    .context("Enter your email address")?,
                password(args, "current_password")?,
            )
            .await?;
        }
        "backup_sync" => api.sync().await?,
        "backup_unlock" => {
            let state = api.status().await?;
            let export = api.unlock(password(args, "password")?, &state).await?;
            ensure!(
                api.restore(Some((
                    &export,
                    state
                        .expected()
                        .credential_generation
                        .context("Enable account backup first")?
                )))
                .await?,
                "Use an existing trusted device to repair backup access after the password reset"
            );
        }
        "backup_repair" => api.rewrap(password(args, "password")?).await?,
        "backup_cancel" => api.cancel_prepared()?,
        "backup_resume" => {
            let pending = api
                .store
                .record()?
                .pending
                .context("No interrupted account action")?;
            match pending.kind {
                Kind::Signup | Kind::Migration => {
                    api.resume_enrollment(optional_password(args)).await?;
                }
                Kind::Login => {
                    api.resume_login().await?;
                }
                Kind::Password | Kind::Rewrap | Kind::Email => {
                    api.resume_mutation(optional_password(args)).await?;
                }
                Kind::Sync => api.sync().await?,
                _ => anyhow::bail!("Resume password recovery from the native sign-in window"),
            }
        }
        "backup_recover_secure" => {
            let record = api.store.record()?;
            let username = record
                .username
                .or_else(|| {
                    record
                        .pending
                        .as_ref()
                        .and_then(|pending| pending.username.clone())
                })
                .context("Missing account username")?;
            api.store.update(None, |record| {
                record.installation_select = false;
                Ok(())
            })?;
            api.recover_secure(&username, password(args, "password")?, "Wisp desktop")
                .await?;
        }
        "backup_recover_classic" => {
            api.store.update(None, |record| {
                record.installation_select = false;
                Ok(())
            })?;
            api.recover_classic_migration(password(args, "password")?, "Wisp desktop")
                .await?;
        }
        "backup_finish_install" => (),
        _ => anyhow::bail!("Unknown backup action"),
    }
    if api.store.record()?.installation_pending && api.store.record()?.installation_ready {
        let credential = api.installation()?;
        // Renew from the durable credential even when this is a resumed install
        // and the daemon's original session was revoked during recovery.
        api.session(&credential).await?;
        let registry = server
            .account_registry
            .as_ref()
            .context("Account configuration path is unavailable")?;
        crate::account_backup::install::committed(
            &api.store,
            registry,
            &registry.with_file_name("account.env"),
        )?;
        server.activate_recovered_device(&credential, &api.session_bearer()?);
    }
    if api.store.read(|_, bundle| Ok(bundle.is_some()))? {
        privacy.reload_backup()?;
    }
    if matches!(
        name,
        "backup_sync" | "backup_upgrade" | "backup_enable" | "backup_unlock" | "backup_repair"
    ) {
        privacy.set_backup_error(None);
    }
    public_status(api).await
}

/// The caller must never fall back after this returns an error: an unavailable
/// status or an already latched secure account cannot accept a classic password.
pub(crate) async fn secure_profile_change(
    server: &ServerApi,
    privacy: &Privacy,
    name: &str,
    args: &Value,
) -> anyhow::Result<bool> {
    let store = privacy.backup_store()?;
    let _action = store.action()?;
    let mut api = connect(server, privacy).await?;
    let mut state = api.status().await;
    if state.as_ref().err().is_some_and(unauthorized) {
        server.renew_session().await?;
        api = connect(server, privacy).await?;
        state = api.status().await;
    }
    if matches!(state?, Status::Classic { .. }) {
        return Ok(false);
    }
    if let Err(error) = run(&mut api, server, privacy, name, args).await {
        if !unauthorized(&error) {
            return Err(error);
        }
        server.renew_session().await?;
        let mut api = connect(server, privacy).await?;
        run(&mut api, server, privacy, name, args).await?;
    }
    Ok(true)
}

fn unauthorized(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<crate::account_backup::api::Failure>()
        .is_some_and(|failure| failure.status == 401)
}

pub(crate) async fn background(
    server: &ServerApi,
    privacy: &Privacy,
    media: Option<String>,
) -> anyhow::Result<()> {
    let store = privacy.backup_store()?;
    let record = store.record()?;
    if !record.secure
        || !record.auto_sync
        || !record.key_verified
        || record
            .pending
            .as_ref()
            .is_some_and(|pending| pending.kind != Kind::Sync)
    {
        return Ok(());
    }
    // Account operations are serialized without holding the trust-data lock.
    // A user action in progress simply defers this background pass.
    let Ok(_action) = store.action() else {
        return Ok(());
    };
    privacy.capture_media_key(media)?;
    let api = connect(server, privacy).await?;
    if let Err(error) = api.sync().await {
        if error
            .downcast_ref::<crate::account_backup::api::Failure>()
            .is_none_or(|failure| failure.status != 401)
        {
            return Err(error);
        }
        server.renew_session().await?;
        connect(server, privacy).await?.sync().await?;
    }
    privacy.reload_backup()?;
    privacy.set_backup_error(None);
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::manual_assert_eq)] // Assertions never print private credentials.
    use super::*;
    use crate::account_backup::tests::Fixture;
    use std::sync::{Arc, RwLock};
    const PASSWORD: &str = "synthetic desktop original password";
    fn secret(value: &str) -> SecretString {
        SecretString::from(value.to_owned())
    }
    async fn signed_in(fixture: &Fixture) -> (Api, ServerApi, Privacy) {
        let mut native = fixture.client("desktop").await;
        native
            .signup(
                "desktop",
                "Synthetic desktop",
                "Test desktop",
                secret(PASSWORD),
            )
            .await
            .unwrap();
        let credential = native.installation().unwrap();
        // This daemon fixture is already installed; never use process-global
        // account paths from a unit test.
        native
            .store
            .update(None, |record| {
                record.installation_pending = false;
                Ok(())
            })
            .unwrap();
        let server = ServerApi {
            client: reqwest::Client::new(),
            account_registry: None,
            base_url: fixture.origin.clone(),
            token: Arc::new(RwLock::new(native.session_bearer().unwrap().to_string())),
            auth: Arc::new(RwLock::new(AuthMethod::Device {
                device_id: credential.device_id,
                device_token: credential.device_token,
            })),
        };
        let privacy = Privacy::at(
            fixture.folder.path().join("desktop"),
            &fixture.origin,
            credential.user.id,
        );
        (native, server, privacy)
    }
    #[tokio::test]
    async fn current_password_upgrade_ipc_restores_the_original_identity() {
        let fixture = Fixture::new().await;
        let (native, credential, _) =
            crate::account_backup::tests::classic_client(&fixture, "desktop").await;
        let original = native
            .store
            .record()
            .unwrap()
            .bundle()
            .unwrap()
            .unwrap()
            .identity()
            .unwrap()
            .public();
        let server = ServerApi {
            client: reqwest::Client::new(),
            account_registry: None,
            base_url: fixture.origin.clone(),
            token: Arc::new(RwLock::new(native.session_bearer().unwrap().to_string())),
            auth: Arc::new(RwLock::new(AuthMethod::Device {
                device_id: credential.device_id,
                device_token: credential.device_token,
            })),
        };
        let privacy = Privacy::at(
            fixture.folder.path().join("desktop"),
            &fixture.origin,
            credential.user.id,
        );
        assert!(handles("backup_upgrade"));
        let state = command(
            &server,
            &privacy,
            "backup_upgrade",
            &json!({"current_password":"synthetic original classic password"}),
        )
        .await
        .unwrap();
        assert!(
            state["mode"] == "secure" && state["unlocked"] == true && state["pending"].is_null()
        );
        assert!(privacy.backup_store().unwrap().record().unwrap().identity == Some(original));
    }
    #[tokio::test]
    async fn secure_profile_password_change_never_uses_classic_password_endpoint() {
        let fixture = Fixture::new().await;
        let (native, server, privacy) = signed_in(&fixture).await;
        // This sentinel would be consumed if any plaintext password route ran.
        fixture.fail_before("/v1/accounts/password");
        *server.token.write().unwrap() = "expired synthetic session".into();
        let original = native
            .store
            .record()
            .unwrap()
            .local_key()
            .unwrap()
            .save_private();
        crate::account_profile::command(
            &server,
            &privacy,
            "change_account_password",
            &json!({"current_password":PASSWORD,"new_password":"synthetic replacement password"}),
        )
        .await
        .unwrap();
        assert!(fixture.faults.lock().unwrap().is_some());
        assert!(
            native
                .store
                .record()
                .unwrap()
                .local_key()
                .unwrap()
                .save_private()
                .as_slice()
                == original.as_slice()
        );
        let mut remote = fixture.client("remote").await;
        assert!(
            remote
                .login(
                    "desktop",
                    secret("synthetic replacement password"),
                    "Remote"
                )
                .await
                .unwrap()
        );
        assert!(remote.store.record().unwrap().identity == native.store.record().unwrap().identity);
        let profile =
            crate::account_profile::command(&server, &privacy, "account_profile", &json!({}))
                .await
                .unwrap();
        assert!(profile["password_available"] == true);
    }
    #[tokio::test]
    async fn secure_missing_backup_never_enrolls_replacement_keys_and_status_latches_mode() {
        let fixture = Fixture::new().await;
        let (native, server, _) = signed_in(&fixture).await;
        let identity = native.store.record().unwrap().identity.unwrap();
        let account = native.store.record().unwrap().scope.unwrap().account;
        let root = fixture.folder.path().join("empty");
        let privacy = Privacy::at(root.clone(), &fixture.origin, account);
        assert!(privacy.initialize(&server).await.is_err());
        let record = privacy.backup_store().unwrap().record().unwrap();
        assert!(
            record.secure
                && record.identity == Some(identity)
                && record.bundle().unwrap().is_none()
        );
        assert!(!root.join(record.network.unwrap().to_string()).exists());
        assert!(privacy.active().is_err());
        let status = command(&server, &privacy, "backup_status", &json!({}))
            .await
            .unwrap();
        assert!(status["mode"] == "secure" && status["identity_ready"] == false);
        assert!(secure_profile_change(&server,&privacy,"change_account_password",
            &json!({"current_password":"wrong password","new_password":"another synthetic password"})).await.is_err());
        assert!(
            privacy
                .backup_store()
                .unwrap()
                .record()
                .unwrap()
                .bundle()
                .unwrap()
                .is_none()
        );
    }
    #[tokio::test]
    async fn installed_sign_in_adoption_waits_for_completion_and_preserves_audio_without_rejoining()
    {
        use crate::{Daemon, MediaManager, ServerView, ShortcutManager};
        use std::sync::atomic::Ordering;
        let fixture = Fixture::new().await;
        let (mut native, server, privacy) = signed_in(&fixture).await;
        let mut snapshot = server.snapshot().await.unwrap();
        snapshot.self_state.muted = true;
        snapshot.self_state.deafened = true;
        let view = ServerView {
            id: "fixture".into(),
            name: "Synthetic".into(),
            url: fixture.origin.clone(),
            connected: true,
        };
        let (media, _) = MediaManager::new(false, None);
        let mut daemon = Daemon::new(
            "Synthetic desktop".into(),
            view.clone(),
            vec![view],
            server,
            snapshot,
            None,
            media,
            false,
            std::time::Duration::from_secs(30),
            ShortcutManager::from_environment(),
        );
        daemon.privacy = privacy;
        let status = daemon
            .run_command(&crate::CommandEnvelope::new(
                "backup",
                "backup_status",
                json!({"server_id":""}),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(status["mode"] == "secure");
        let old = daemon.api.auth.read().unwrap().clone();
        native
            .login("desktop", secret(PASSWORD), "Replacement desktop")
            .await
            .unwrap();
        let credential = native.installation().unwrap();
        let account = crate::accounts::ServerAccount {
            id: "fixture".into(),
            name: "Synthetic".into(),
            server_url: fixture.origin.clone(),
            profile: credential.user.display_name.clone(),
            device_id: credential.device_id,
            device_token: credential.device_token.clone(),
            media_key: None,
        };
        // A config observer must ignore an installation not fully committed yet.
        daemon.adopt_installed_account(&account).await.unwrap();
        assert!(*daemon.api.auth.read().unwrap() == old);
        native
            .store
            .update(None, |record| {
                record.installation_pending = false;
                Ok(())
            })
            .unwrap();
        daemon.local_voice_left.store(false, Ordering::Release);
        daemon.adopt_installed_account(&account).await.unwrap();
        assert!(
            matches!(&*daemon.api.auth.read().unwrap(),AuthMethod::Device{device_id,..} if *device_id==credential.device_id)
        );
        assert!(
            daemon.local_voice_left.load(Ordering::Acquire)
                && daemon.voice_recovery_blocked.load(Ordering::Acquire)
        );
        let state = daemon.state.read().await;
        assert!(
            state.self_state.hangout_id.is_none()
                && state.self_state.muted
                && state.self_state.deafened
        );
        assert!(
            !state.self_state.media.microphone_published
                && !state.self_state.media.camera.active
                && !state.self_state.media.screen_share.active
        );
    }
    #[tokio::test]
    async fn captured_media_is_noop_when_unchanged_and_rejects_conflicting_keys() {
        let fixture = Fixture::new().await;
        let (native, _, privacy) = signed_in(&fixture).await;
        let media = "synthetic scoped media key".to_owned();
        privacy.capture_media_key(Some(media.clone())).unwrap();
        let revision = native.store.record().unwrap().revision;
        privacy.capture_media_key(Some(media)).unwrap();
        assert!(native.store.record().unwrap().revision == revision);
        assert!(
            privacy
                .capture_media_key(Some("different synthetic media key".into()))
                .is_err()
        );
        assert!(native.store.record().unwrap().revision == revision);
    }
}
