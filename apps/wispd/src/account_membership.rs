use crate::{CommandEnvelope, Context, Daemon, Value, accounts, decode, string_arg};
use anyhow::ensure;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::Method;
use serde_json::json;
use wisp_crypto::invitation::{Invitation, Link};

pub(crate) fn handles(name: &str) -> bool {
    matches!(
        name,
        "create_server_invite"
            | "revoke_server_invite"
            | "list_server_invites"
            | "lookup_person"
            | "set_public_handle"
            | "account_overview"
            | "block_person"
            | "unblock_person"
            | "leave_server"
    )
}

pub(crate) fn media_key(origin: &str, fallback: Option<String>) -> Option<String> {
    accounts::default_path()
        .and_then(|path| accounts::AccountRegistry::load(&path).ok())
        .and_then(|registry| {
            registry.servers.into_iter().find(|account| {
                account.server_url.trim_end_matches('/') == origin.trim_end_matches('/')
            })
        })
        .and_then(|account| account.media_key)
        .or(fallback)
}

impl Daemon {
    #[allow(clippy::too_many_lines)] // Shared routing keeps all account actions scoped to one API.
    pub(crate) async fn membership_command(
        &self,
        command: &CommandEnvelope,
    ) -> anyhow::Result<Value> {
        let id = command.args["server_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.primary_server.id);
        let linked = if id == self.primary_server.id {
            None
        } else {
            Some(
                self.linked_servers
                    .read()
                    .await
                    .get(id)
                    .context("Account is not connected")?
                    .clone(),
            )
        };
        let api = linked.as_ref().map_or(&self.api, |server| &server.api);
        let args = &command.args;
        let result = match command.name.as_str() {
            "create_server_invite" => {
                let fallback = linked
                    .as_ref()
                    .map_or_else(|| self.primary_media_key.clone(), |s| s.media_key.clone());
                let key = media_key(&api.base_url, fallback);
                ensure!(
                    !api.base_url.starts_with("https://")
                        || key.as_ref().is_some_and(|k| k.len() >= 16),
                    "Restore this device's media encryption key before creating invitations"
                );
                let invite:Value=decode(api.request(Method::POST,"/v2/server-invites").json(&json!({"expires_in_minutes":args["expires_in_minutes"].as_u64().unwrap_or(720)})).send().await?).await?;
                let invitation = Invitation {
                    v: 2,
                    server: wisp_crypto::invitation::origin(&api.base_url)?,
                    id: serde_json::from_value(invite["id"].clone())?,
                    code: string_arg(&invite, "code")?,
                    server_name: string_arg(&invite, "server_name")?,
                    inviter: serde_json::from_value(invite["inviter"].clone())?,
                    expires_at: serde_json::from_value(invite["expires_at"].clone())?,
                    media_key: key,
                };
                let link = Link::create(&api.base_url)?;
                let envelope = link.seal(&invitation)?;
                let _: Value = decode(
                    api.request(
                        Method::PUT,
                        &format!("/v2/server-invites/{}/envelope", invitation.id),
                    )
                    .json(&json!({"lookup_id":link.lookup_id(),"envelope":envelope}))
                    .send()
                    .await?,
                )
                .await?;
                let mut uri = link.uri();
                if let (Some(origin), Some(label)) = (
                    invite["short_origin"].as_str(),
                    invite["short_label"].as_str(),
                ) {
                    let short = wisp_crypto::short_invitation::ShortLink::create(origin, label)?;
                    let envelope = short.seal(&uri)?;
                    let _: Value = decode(
                        api.request(
                            Method::PUT,
                            &format!("/v2/server-invites/{}/short", invitation.id),
                        )
                        .json(&json!({"lookup_id":short.lookup_id(),"envelope":envelope}))
                        .send()
                        .await?,
                    )
                    .await?;
                    uri = short.uri();
                }
                let qr = qrcode::QrCode::new(uri.as_bytes())?
                    .render::<qrcode::render::svg::Color>()
                    .min_dimensions(256, 256)
                    .build();
                json!({"id":invitation.id,"uri":uri,"expires_at":invitation.expires_at,"server_name":invitation.server_name,"qr":"data:image/svg+xml;base64,".to_owned()+&STANDARD.encode(qr)})
            }
            "revoke_server_invite" => {
                let invite: uuid::Uuid = string_arg(args, "invite_id")?.parse()?;
                decode(
                    api.request(Method::DELETE, &format!("/v2/server-invites/{invite}"))
                        .send()
                        .await?,
                )
                .await?
            }
            "list_server_invites" => {
                decode(
                    api.request(Method::GET, "/v2/server-invites")
                        .send()
                        .await?,
                )
                .await?
            }
            "account_overview" => {
                decode(api.request(Method::GET, "/v2/accounts/me").send().await?).await?
            }
            "lookup_person" => {
                decode(
                    api.request(Method::POST, "/v2/people/lookup")
                        .json(&json!({"handle":string_arg(args,"handle")?}))
                        .send()
                        .await?,
                )
                .await?
            }
            "set_public_handle" => {
                decode(
                    api.request(Method::PUT, "/v2/accounts/handle")
                        .json(&json!({"handle":args["handle"].as_str().unwrap_or("")}))
                        .send()
                        .await?,
                )
                .await?
            }
            "block_person" | "unblock_person" => {
                let user: uuid::Uuid = string_arg(args, "user_id")?.parse()?;
                decode(
                    api.request(
                        if command.name == "block_person" {
                            Method::PUT
                        } else {
                            Method::DELETE
                        },
                        &format!("/v2/people/{user}/block"),
                    )
                    .send()
                    .await?,
                )
                .await?
            }
            "leave_server" => {
                if *self.voice_server_id.read().await == id {
                    self.leave_voice_locally().await;
                }
                decode(api.request(Method::POST, "/v2/server/leave").send().await?).await?
            }
            _ => unreachable!(),
        };
        if matches!(
            command.name.as_str(),
            "leave_server" | "block_person" | "unblock_person"
        ) {
            if let Some(server) = linked {
                self.refresh_linked(&server, "server_membership_changed")
                    .await?;
            } else {
                self.refresh("server_membership_changed").await?;
            }
        }
        Ok(result)
    }
}
