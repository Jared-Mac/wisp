//! Published-pack tracking is separate from the editable guest state. Polling
//! never imports over a draft or hashes the installation again unnecessarily.
use super::{
    Installed, MAX_JSON, Packages, PathBuf, Permission, Request, Value, files, hash, invoke,
    message_actions, read_bounded, shared_document,
};
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wisp_plugins::package::{Instance, write_json};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Published {
    pub revision: String,
    pub module_digest: String,
    pub files: Vec<files::Artifact>,
}
#[derive(Serialize, Deserialize)]
struct Verified {
    folder: PathBuf,
    digest: String,
    state: String,
    checked_at: u64,
}
#[derive(Default, Serialize, Deserialize)]
struct Tracking {
    published: Option<Published>,
    verified: Option<Verified>,
}
fn path(packages: &Packages, id: &str, scope: &str) -> anyhow::Result<PathBuf> {
    Ok(packages
        .instance_path(id, scope)?
        .with_file_name("sync.json"))
}
fn read(packages: &Packages, id: &str, scope: &str) -> anyhow::Result<Tracking> {
    let path = path(packages, id, scope)?;
    if !path.exists() {
        return Ok(Tracking::default());
    }
    // A failed cache write or an older cache must not block the editable pack.
    Ok(serde_json::from_slice(&read_bounded(&path, MAX_JSON)?).unwrap_or_default())
}
#[allow(clippy::too_many_arguments)]
pub(super) async fn published(
    session: &message_actions::Session<'_>,
    packages: &Packages,
    installed: &Installed,
    id: &str,
    scope: &str,
    conversation: &str,
    latest: Option<&str>,
    instance: &Instance,
) -> anyhow::Result<Option<Published>> {
    let Some(revision) = latest else {
        return Ok(None);
    };
    let mut tracking = read(packages, id, scope)?;
    if let Some(plan) = &tracking.published
        && plan.revision == revision
        && plan.module_digest == installed.digest
    {
        files::validate_plan(&plan.files)?;
        return Ok(Some(plan.clone()));
    }
    ensure!(
        installed
            .manifest
            .permissions
            .contains(&Permission::Documents),
        "Document permission is required"
    );
    let (request, archives) =
        if instance.shared_revision.as_deref() == latest && !instance.draft_changed {
            (
                Request {
                    api: 1,
                    event: "open".into(),
                    action: String::new(),
                    fields: Value::Null,
                    state: instance.state.clone(),
                    document: None,
                },
                instance.archives.clone(),
            )
        } else {
            let (document, archives, document_hash, _) =
                shared_document(session, packages, installed, id, conversation, revision).await?;
            (
                Request {
                    api: 1,
                    event: "import".into(),
                    action: String::new(),
                    fields: json!({"document_hash":document_hash}),
                    state: Value::Null,
                    document: Some(document.data),
                },
                archives,
            )
        };
    let response = invoke(packages, installed, request).await?;
    files::authorize_private(&response.view.files, &archives)?;
    let plan = Published {
        revision: revision.into(),
        module_digest: installed.digest.clone(),
        files: response.view.files,
    };
    tracking.published = Some(plan.clone());
    write_json(&path(packages, id, scope)?, &tracking)?;
    Ok(Some(plan))
}
/// Private archives are recoverable from the encrypted publication if the user
/// cleared or damaged the download cache. Never consult an unrelated draft.
pub(super) async fn restore_private(
    session: &message_actions::Session<'_>,
    packages: &Packages,
    installed: &Installed,
    id: &str,
    conversation: &str,
    plan: &Published,
) -> anyhow::Result<()> {
    let files = plan.files.clone();
    let cache = packages.root.join("cache");
    let intact = tokio::task::spawn_blocking(move || {
        files
            .iter()
            .filter(|f| f.source == files::Source::Local)
            .all(|f| {
                read_bounded(&files::cache_path(&cache, f), files::MAX_ARCHIVE)
                    .is_ok_and(|bytes| bytes.len() as u64 == f.size && hash(&bytes) == f.sha256)
            })
    })
    .await?;
    if !intact {
        let (_, archives, _, _) = shared_document(
            session,
            packages,
            installed,
            id,
            conversation,
            &plan.revision,
        )
        .await?;
        files::authorize_private(&plan.files, &archives)?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) async fn status(
    session: &message_actions::Session<'_>,
    packages: &Packages,
    installed: &Installed,
    id: &str,
    scope: &str,
    conversation: &str,
    latest: Option<&str>,
    instance: &Instance,
    verify: bool,
) -> anyhow::Result<Value> {
    let Some(plan) = published(
        session,
        packages,
        installed,
        id,
        scope,
        conversation,
        latest,
        instance,
    )
    .await?
    else {
        return Ok(
            json!({"state":"no_pack","detail":"No server pack has been published for your membership yet."}),
        );
    };
    ensure!(
        !plan.files.is_empty(),
        "Published pack has no resolved mods"
    );
    let digest = files::plan_digest(&plan.files)?;
    let mut tracking = read(packages, id, scope)?;
    if let Some(folder) = &instance.folder {
        if verify
            || tracking
                .verified
                .as_ref()
                .is_none_or(|v| &v.folder != folder || v.digest != digest)
        {
            let folder = folder.clone();
            let target = digest.clone();
            tracking.verified = Some(
                tokio::task::spawn_blocking(move || Verified {
                    state: files::profile_state(&folder, &target).into(),
                    folder,
                    digest: target,
                    checked_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                })
                .await?,
            );
            write_json(&path(packages, id, scope)?, &tracking)?;
        }
    } else {
        tracking.verified = None;
    }
    let state = tracking
        .verified
        .as_ref()
        .map_or("not_installed", |v| v.state.as_str());
    let detail = match state {
        "up_to_date" => "Installed profile matches the published server pack.",
        "update_available" => {
            "Your installed profile is from a different pack. Sync to the current server version."
        }
        "modified" => {
            "Installed files were changed, added or removed. Sync creates a fresh profile and keeps your changes in the old one."
        }
        "missing" => "The installed profile folder is missing. Sync to recreate it.",
        "not_installed" => "Install the published server pack on this PC.",
        _ => "Could not verify this profile. Check its location and try again.",
    };
    Ok(
        json!({"state":state,"detail":detail,"revision":plan.revision,"digest":digest,
        "checked_at":tracking.verified.as_ref().map(|v|v.checked_at),"can_sync":true,
        "needs_folder":instance.folder.as_ref().and_then(|p|p.parent()).is_none_or(|p| !p.is_dir())}),
    )
}
