use super::{Daemon, chat_images::AttachmentDraft, message_actions, string_arg};
use anyhow::{Context, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::Stdio,
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wisp_plugins::{
    Document, MAX_JSON, MAX_MODULE, Manifest, Permission, Request, Response, bundles, files,
    package::{Installed, Packages, hash, read_bounded},
};
use wisp_protocol::{CommandEnvelope, MessageContext};

#[derive(Serialize, Deserialize)]
struct Invocation {
    manifest: Manifest,
    module: PathBuf,
    request: Request,
}
pub(super) fn worker() -> anyhow::Result<()> {
    use rustix::process::{Resource, Rlimit, setrlimit};
    for (resource, limit) in [
        (Resource::As, 1024 * 1024 * 1024),
        (Resource::Cpu, 4),
        (Resource::Core, 0),
    ] {
        setrlimit(
            resource,
            Rlimit {
                current: Some(limit),
                maximum: Some(limit),
            },
        )?;
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .take((MAX_JSON * 2) as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_JSON * 2, "Worker input exceeds limit");
    let input: Invocation = serde_json::from_slice(&bytes)?;
    let module = read_bounded(&input.module, MAX_MODULE)?;
    let response = wisp_plugins::runtime::invoke(&module, &input.manifest, &input.request)?;
    serde_json::to_writer(std::io::stdout(), &response)?;
    Ok(())
}
fn packages() -> anyhow::Result<Packages> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .context("Local data directory unavailable")?;
    Ok(Packages {
        root: data.join("wisp/plugins"),
    })
}
fn local_path(value: &str) -> anyhow::Result<PathBuf> {
    let path = if value.starts_with("file:") {
        url::Url::parse(value)?
            .to_file_path()
            .map_err(|()| anyhow::anyhow!("Choose a local file"))?
    } else {
        PathBuf::from(value)
    };
    ensure!(path.is_absolute(), "Choose an absolute local path");
    Ok(path)
}
async fn invoke(
    packages: &Packages,
    installed: &Installed,
    request: Request,
) -> anyhow::Result<Response> {
    packages.verify(installed)?;
    let input = serde_json::to_vec(&Invocation {
        manifest: installed.manifest.clone(),
        module: packages.module_path(installed)?,
        request,
    })?;
    let mut child = tokio::process::Command::new(std::env::current_exe()?)
        .arg("--plugin-worker")
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child.stdin.take().context("Missing worker input")?;
    let stdout = child.stdout.take().context("Missing worker output")?;
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        stdin.write_all(&input).await?;
        drop(stdin);
        let mut bytes = Vec::new();
        stdout
            .take(MAX_JSON as u64 + 1)
            .read_to_end(&mut bytes)
            .await?;
        ensure!(bytes.len() <= MAX_JSON, "Worker output exceeds limit");
        ensure!(
            child.wait().await?.success(),
            "Plugin stopped: invalid module, resource limit, or guest error"
        );
        let response: Response = serde_json::from_slice(&bytes)?;
        response.view.validate(&installed.manifest)?;
        Ok::<_, anyhow::Error>(response)
    })
    .await
    .context("Plugin exceeded its 5-second time limit")?;
    // kill_on_drop also covers timeout and oversized output.
    result
}
impl Daemon {
    #[allow(clippy::too_many_lines)]
    pub(super) async fn plugin_command(
        &self,
        command: &CommandEnvelope,
    ) -> anyhow::Result<Option<Value>> {
        // Catalog reads/settings are independent of a local mod download or install.
        let _operation = if matches!(
            command.name.as_str(),
            "plugin_server_catalog" | "plugin_server_enable"
        ) {
            None
        } else {
            Some(
                self.plugin_operation
                    .try_lock()
                    .context("Another plugin operation is running; try again when it finishes")?,
            )
        };
        let packages = packages()?;
        let args = &command.args;
        let mut name = command.name.as_str();
        ensure!(
            serde_json::to_vec(args)?.len() <= MAX_JSON,
            "Plugin command exceeds limit"
        );
        if matches!(
            name,
            "plugin_server_catalog" | "plugin_server_enable" | "plugin_server_prepare"
        ) {
            let session = self
                .message_session(args["server_id"].as_str().unwrap_or_default())
                .await?;
            let api = session.api();
            if name == "plugin_server_catalog" {
                return Ok(Some(
                    crate::decode(
                        api.request(reqwest::Method::GET, "/v1/server/plugins")
                            .send()
                            .await?,
                    )
                    .await?,
                ));
            }
            let id = string_arg(args, "plugin_id")?;
            wisp_plugins::identifier(&id)?;
            if name == "plugin_server_enable" {
                let enabled = args["enabled"]
                    .as_bool()
                    .context("Choose enabled or disabled")?;
                return Ok(Some(
                    crate::decode(
                        api.request(reqwest::Method::PUT, &format!("/v1/server/plugins/{id}"))
                            .json(&json!({"enabled":enabled,"digest":string_arg(args,"digest")?}))
                            .send()
                            .await?,
                    )
                    .await?,
                ));
            }
            let digest = string_arg(args, "digest")?;
            // Check this client's reviewed catalog before asking the server for any module.
            wisp_plugins::catalog::module(&id, &digest)?;
            let mut response = api
                .request(
                    reqwest::Method::GET,
                    &format!("/v1/server/plugins/{id}/package"),
                )
                .send()
                .await?;
            ensure!(
                response.status().is_success(),
                "This server plugin is unavailable. Refresh the plugin list."
            );
            ensure!(
                response
                    .headers()
                    .get("x-wisp-plugin-digest")
                    .and_then(|h| h.to_str().ok())
                    == Some(digest.as_str()),
                "Server plugin changed; refresh before opening"
            );
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                ensure!(
                    bytes.len() + chunk.len() <= MAX_MODULE,
                    "Server plugin exceeds size limit"
                );
                bytes.extend_from_slice(&chunk);
            }
            let installed = wisp_plugins::catalog::prepare(&packages, &id, &digest, &bytes)?;
            return Ok(Some(
                json!({"installed":installed,"plugins":packages.list()?}),
            ));
        }
        match name {
            "plugin_list" => return Ok(Some(json!({"plugins":packages.list()?}))),
            "plugin_inspect" => {
                let path = local_path(&string_arg(args, "path")?)?;
                let (manifest, _, digest) = Packages::inspect(&path)?;
                return Ok(Some(
                    json!({"manifest":manifest,"digest":digest,"path":path}),
                ));
            }
            "plugin_install" => {
                let item = packages.install(
                    &local_path(&string_arg(args, "path")?)?,
                    &string_arg(args, "digest")?,
                )?;
                return Ok(Some(json!({"installed":item,"plugins":packages.list()?})));
            }
            "plugin_enable" => {
                packages.enable(
                    &string_arg(args, "plugin_id")?,
                    args["enabled"]
                        .as_bool()
                        .context("Choose enabled or disabled")?,
                )?;
                return Ok(Some(json!({"plugins":packages.list()?})));
            }
            "plugin_remove" => {
                packages.uninstall(&string_arg(args, "plugin_id")?)?;
                return Ok(Some(json!({"plugins":packages.list()?})));
            }
            _ => {}
        }
        let id = string_arg(args, "plugin_id")?;
        let installed = packages.get(&id)?;
        ensure!(installed.enabled, "Plugin is disabled");
        let session = self
            .message_session(args["server_id"].as_str().unwrap_or_default())
            .await?;
        let server_pack = args["server_pack"].as_bool().unwrap_or(false);
        let workspace: Value = if server_pack {
            let value = crate::decode(
                session
                    .api()
                    .request(
                        reqwest::Method::POST,
                        &format!("/v1/server/plugins/{id}/workspace"),
                    )
                    .send()
                    .await?,
            )
            .await?;
            match &session {
                message_actions::Session::Primary(_) => {
                    self.refresh("plugin_workspace").await?;
                }
                message_actions::Session::Linked(server) => {
                    self.refresh_linked(server, "plugin_workspace").await?;
                }
            }
            value
        } else {
            Value::Null
        };
        let can_publish = !server_pack || workspace["can_publish"].as_bool().unwrap_or(false);
        if server_pack && !matches!(name, "plugin_open" | "plugin_apply" | "plugin_load_server") {
            ensure!(
                can_publish,
                "Only server admins can change or publish the server pack"
            );
        }
        let conversation_id = if server_pack {
            workspace["conversation_id"]
                .as_str()
                .context("Server workspace unavailable")?
        } else {
            args["conversation_id"].as_str().unwrap_or_default()
        };
        let snapshot = match &session {
            message_actions::Session::Primary(d) => d.state.read().await.clone(),
            message_actions::Session::Linked(s) => s.state.read().await.clone(),
        };
        if !conversation_id.is_empty() {
            ensure!(
                snapshot
                    .conversations
                    .iter()
                    .any(|c| c.id == conversation_id && !c.pending_access),
                "This conversation is not available"
            );
        }
        let scope = serde_json::to_string(&(
            session.api().base_url.clone(),
            snapshot.self_state.user.id,
            conversation_id,
        ))?;
        let mut instance = packages.instance(&id, &scope)?;
        if name != "plugin_open" {
            ensure!(
                args["generation"].as_u64() == Some(instance.generation),
                "This plugin view changed. Reopen it before continuing"
            );
        }
        let latest = workspace["latest_message_id"].as_str();
        let mut imported_message = args["message_id"].as_str().map(str::to_owned);
        if server_pack
            && (name == "plugin_load_server"
                || (name == "plugin_open"
                    && !instance.draft_changed
                    && latest.is_some()
                    && latest != instance.shared_revision.as_deref()))
        {
            imported_message = Some(
                latest
                    .context("No server pack has been published for your membership yet")?
                    .to_owned(),
            );
            name = "plugin_import";
        }
        if server_pack && name == "plugin_import" {
            ensure!(
                imported_message.as_deref() == latest,
                "Load the current published server revision"
            );
        }
        let mut request = Request {
            api: 1,
            event: "open".into(),
            action: String::new(),
            fields: Value::Null,
            state: instance.state.clone(),
            document: None,
        };
        let mut feedback = if server_pack && latest.is_none() && !can_publish {
            "No server pack is available to your membership yet. Ask a server admin to publish it."
                .into()
        } else {
            String::new()
        };
        let mut completed = None;
        match name {
            "plugin_open" => {}
            "plugin_seed_personal" => {
                ensure!(
                    server_pack && can_publish,
                    "Choose the server pack as an admin first"
                );
                let personal_scope = serde_json::to_string(&(
                    session.api().base_url.clone(),
                    snapshot.self_state.user.id,
                    "",
                ))?;
                let personal = packages.instance(&id, &personal_scope)?;
                ensure!(
                    !personal.state.is_null(),
                    "No saved personal draft is available"
                );
                instance.archives = personal.archives;
                request.state = personal.state;
                feedback = "Personal draft copied. Resolve and publish to make it available to the server.".into();
            }
            "plugin_inspect_archive" | "plugin_add_archive" => {
                ensure!(
                    installed
                        .manifest
                        .permissions
                        .contains(&Permission::LocalArchives),
                    "Update this plugin and review its local-archive permission first"
                );
                let path = local_path(&string_arg(args, "path")?)?;
                let read_path = path.clone();
                let (bytes, artifact, dependencies) = tokio::task::spawn_blocking(move || {
                    let bytes = read_bounded(&read_path, bundles::MAX_PRIVATE_BYTES)?;
                    let (artifact, dependencies) = files::local_archive(&bytes)?;
                    Ok::<_, anyhow::Error>((bytes, artifact, dependencies))
                })
                .await??;
                if name == "plugin_inspect_archive" {
                    return Ok(Some(
                        json!({"archive_review":{"path":path,"artifact":artifact,"dependencies":dependencies},"feedback":"Review this private ZIP before adding it. It stays local until you share a revision."}),
                    ));
                }
                ensure!(
                    args["sha256"].as_str() == Some(&artifact.sha256),
                    "ZIP changed after review; select it again"
                );
                let key = files::package_key(&artifact.package)?;
                instance
                    .archives
                    .retain(|f| files::package_key(&f.package).ok().as_ref() != Some(&key));
                instance.archives.push(artifact.clone());
                files::validate_plan(&instance.archives)?;
                ensure!(
                    instance.archives.iter().map(|f| f.size).sum::<u64>()
                        <= bundles::MAX_PRIVATE_BYTES as u64,
                    "Private ZIPs exceed 128 MiB per draft"
                );
                let cache = packages.root.join("cache");
                tokio::task::spawn_blocking(move || {
                    super::plugin_downloads::cache_bytes(&cache, &bytes, files::Source::Local)
                })
                .await??;
                request.event = "archive_added".into();
                request.document = Some(json!({"artifact":artifact,"dependencies":dependencies}));
                feedback =
                    "Private ZIP added. Resolve dependencies before sharing or installing.".into();
            }
            "plugin_remove_archive" => {
                ensure!(
                    installed
                        .manifest
                        .permissions
                        .contains(&Permission::LocalArchives),
                    "Local-archive permission is required"
                );
                let sha256 = string_arg(args, "sha256")?;
                ensure!(
                    instance.archives.iter().any(|f| f.sha256 == sha256),
                    "Private archive is not in this draft"
                );
                instance.archives.retain(|f| f.sha256 != sha256);
                request.event = "archive_removed".into();
                request.fields = json!({"sha256":sha256});
            }
            "plugin_action" => {
                request.event = "action".into();
                request.action = string_arg(args, "action")?;
                request.fields = args["fields"].clone();
            }
            "plugin_import" => {
                ensure!(
                    installed
                        .manifest
                        .permissions
                        .contains(&Permission::Documents),
                    "Document permission is required"
                );
                let message = session
                    .message(
                        imported_message
                            .as_deref()
                            .context("Choose a shared pack")?
                            .parse()?,
                    )
                    .await?;
                ensure!(
                    !conversation_id.is_empty()
                        && message.conversation_id == conversation_id
                        && message.encryption_version == 1
                        && message.content_type == "application/octet-stream",
                    "Choose an encrypted plugin document in this conversation"
                );
                let content = session.privacy().content(message.id)?;
                let attachment = content
                    .attachment
                    .context("Missing authenticated attachment")?;
                let bundle = content.payload["file_name"]
                    .as_str()
                    .is_some_and(|n| n.ends_with(".wisp-plugin.zip"));
                let limit = if bundle {
                    bundles::MAX_BUNDLE
                } else {
                    MAX_JSON
                };
                ensure!(
                    attachment.plaintext_bytes <= limit as u64
                        && attachment.ciphertext_bytes <= (limit + 1024 * 1024) as u64,
                    "Shared document exceeds limit"
                );
                let file = super::privacy_transfers::receive_attachment(
                    session.api(),
                    session
                        .privacy()
                        .active()?
                        .context("Encryption is not ready")?,
                    message.id,
                    attachment,
                    |_, _| {},
                )
                .await?;
                let path = file.path().to_owned();
                let (document, archives, document_hash) = tokio::task::spawn_blocking(move || {
                    let bytes = read_bounded(&path, limit)?;
                    let (document, archives) = if bundle {
                        bundles::decode(&bytes)?
                    } else {
                        (Document::parse(&bytes)?, Vec::new())
                    };
                    Ok::<_, anyhow::Error>((document, archives, hash(&bytes)))
                })
                .await??;
                ensure!(
                    document.plugin == id,
                    "Shared document requires a different plugin"
                );
                ensure!(
                    archives.is_empty()
                        || installed
                            .manifest
                            .permissions
                            .contains(&Permission::LocalArchives),
                    "Update this plugin and review its local-archive permission before importing a private pack"
                );
                instance.archives = archives
                    .iter()
                    .map(|(artifact, _)| artifact.clone())
                    .collect();
                let cache = packages.root.join("cache");
                tokio::task::spawn_blocking(move || {
                    for (_, bytes) in archives {
                        super::plugin_downloads::cache_bytes(&cache, &bytes, files::Source::Local)?;
                    }
                    Ok::<_, anyhow::Error>(())
                })
                .await??;
                request.event = "import".into();
                request.document = Some(document.data);
                request.fields = json!({"document_hash":document_hash});
                if server_pack {
                    instance.shared_revision = imported_message.clone();
                    instance.draft_changed = false;
                }
                feedback = format!(
                    "Imported from {}. Review before installing.",
                    message.sender.display_name
                );
            }
            "plugin_resolve" | "plugin_apply" | "plugin_share" => {
                let rendered = invoke(&packages, &installed, request.clone()).await?;
                let view = rendered.view.clone();
                files::authorize_private(&view.files, &instance.archives)?;
                files::authorize_private(&view.archives, &instance.archives)?;
                request.state = rendered.state.clone();
                if name == "plugin_resolve" {
                    let files = tokio::time::timeout(
                        Duration::from_secs(300),
                        super::plugin_downloads::resolve(
                            &view.packages,
                            &view.archives,
                            &packages.root.join("cache"),
                            installed
                                .manifest
                                .permissions
                                .contains(&Permission::Thunderstore),
                        ),
                    )
                    .await
                    .context("Dependency resolution timed out")??;
                    request.event = "resolved".into();
                    request.document = Some(json!(files));
                } else if name == "plugin_apply" {
                    ensure!(
                        installed
                            .manifest
                            .permissions
                            .contains(&Permission::ManagedFiles),
                        "Managed-file permission is required"
                    );
                    ensure!(
                        args["digest"].as_str() == Some(&files::plan_digest(&view.files)?),
                        "Installation plan changed after review"
                    );
                    let parent = local_path(&string_arg(args, "path")?)?;
                    tokio::time::timeout(
                        Duration::from_secs(300),
                        super::plugin_downloads::obtain(&view.files, &packages.root.join("cache")),
                    )
                    .await
                    .context("Package downloads timed out")??;
                    let cache = packages.root.join("cache");
                    let files = view.files;
                    let folder = tokio::task::spawn_blocking(move || {
                        files::install_profile(&parent, &files, &cache)
                    })
                    .await??;
                    feedback = format!("Profile ready: {}", folder.display());
                    instance.folder = Some(folder);
                } else {
                    if server_pack {
                        ensure!(
                            latest.is_none() || latest == instance.shared_revision.as_deref(),
                            "The server pack changed. Load its current revision before publishing your changes."
                        );
                    }
                    ensure!(
                        !conversation_id.is_empty(),
                        "Choose a friend group or server channel before sharing"
                    );
                    ensure!(
                        session.privacy().active()?.is_some(),
                        "Encrypted chat must be set up before sharing plugin documents"
                    );
                    let document = Document {
                        format: 1,
                        plugin: id.clone(),
                        plugin_version: installed.manifest.version.clone(),
                        title: view.title,
                        data: view
                            .document
                            .context("This plugin has no shareable document yet")?,
                    };
                    let private: Vec<_> = view
                        .files
                        .iter()
                        .filter(|f| f.source == files::Source::Local)
                        .cloned()
                        .collect();
                    let extension = if private.is_empty() { "json" } else { "zip" };
                    let cache = packages.root.join("cache");
                    let title = document.title.clone();
                    let bytes = tokio::task::spawn_blocking(move || {
                        if private.is_empty() {
                            let bytes = serde_json::to_vec(&document)?;
                            Document::parse(&bytes)?;
                            Ok(bytes)
                        } else {
                            bundles::encode(document, private, &cache)
                        }
                    })
                    .await??;
                    let mut file = tempfile::NamedTempFile::new()?;
                    file.write_all(&bytes)?;
                    let draft = AttachmentDraft::from_temporary(
                        &file,
                        format!("{}-{}.wisp-plugin.{extension}", id, &hash(&bytes)[..12]),
                        false,
                    )?;
                    let sent = message_actions::send_content(
                        &session,
                        conversation_id,
                        format!("{title} · shared plugin document"),
                        MessageContext {
                            reply_to: None,
                            forwarded_from: None,
                        },
                        Some(draft),
                        true,
                        |_, _| {},
                    )
                    .await?;
                    match &session {
                        message_actions::Session::Primary(_) => {
                            let _ = self.refresh("message_created").await;
                        }
                        message_actions::Session::Linked(server) => {
                            let _ = self.refresh_linked(server, "message_created").await;
                        }
                    }
                    if server_pack {
                        instance.shared_revision = Some(sent.id.to_string());
                        instance.draft_changed = false;
                    }
                    feedback = format!(
                        "Shared this revision in the selected conversation ({}).",
                        sent.id
                    );
                }
                if name != "plugin_resolve" {
                    completed = Some(rendered);
                }
            }
            _ => bail!("Unknown plugin command"),
        }
        let response = match completed {
            Some(response) => response,
            None => invoke(&packages, &installed, request).await?,
        };
        files::authorize_private(&response.view.files, &instance.archives)?;
        files::authorize_private(&response.view.archives, &instance.archives)?;
        if server_pack
            && matches!(
                name,
                "plugin_action"
                    | "plugin_add_archive"
                    | "plugin_remove_archive"
                    | "plugin_resolve"
                    | "plugin_seed_personal"
            )
        {
            instance.draft_changed = true;
        }
        instance.state = response.state;
        instance.generation = instance
            .generation
            .checked_add(1)
            .context("Plugin state generation overflow")?;
        packages.save(&id, &scope, &instance)?;
        let digest = files::plan_digest(&response.view.files)?;
        let shared: Vec<_> = snapshot
            .messages
            .iter()
            .filter(|m| {
                m.conversation_id == conversation_id
                    && m.encryption_version == 1
                    && m.payload["file_name"].as_str().is_some_and(|n| {
                        n.ends_with(".wisp-plugin.json") || n.ends_with(".wisp-plugin.zip")
                    })
            })
            .map(
                |m| json!({"id":m.id,"author":m.sender.display_name,"name":m.payload["file_name"]}),
            )
            .collect();
        Ok(Some(
            json!({"server_pack":server_pack,"can_publish":can_publish,"server_revision":instance.shared_revision,"server_draft":instance.draft_changed,"update_available":name!="plugin_share" && server_pack && latest.is_some() && latest!=instance.shared_revision.as_deref(),"view":response.view,"generation":instance.generation,"digest":digest,"folder":instance.folder,"folder_url":instance.folder.as_ref().and_then(|p|url::Url::from_directory_path(p).ok().map(|url|url.to_string())),"shared":shared,"feedback":feedback}),
        ))
    }
}
