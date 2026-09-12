use super::{ServerApi, privacy};
use anyhow::{Context, bail};
use serde_json::{Value, json};
use wisp_protocol::{AccountProfile, ChangePasswordRequest};

// Never relay arbitrary response bodies to IPC errors or logs, particularly on
// the password route: proxies may echo the request body in an error page.
async fn response(mut response: reqwest::Response) -> anyhow::Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let mut body = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        if body.len() + chunk.len() > 8192 {
            break;
        }
        body.extend_from_slice(&chunk);
    }
    let code = serde_json::from_slice::<wisp_protocol::ProtocolError>(&body)
        .ok()
        .map(|e| e.code);
    let message = match code.as_deref() {
        Some("profile_in_call") => "Leave voice on your devices before changing your display name.",
        Some("display_name_taken") => "That display name is already in use.",
        Some("profile_changed") => "Your profile changed on another device. Refresh and try again.",
        Some("current_password_incorrect") => "Current password is incorrect.",
        Some("invalid_email") => "Enter a valid email address.",
        Some("recovery_email_unavailable") => "That email address is unavailable for this account.",
        Some("mail_unavailable") => "Recovery email is temporarily unavailable. Try again later.",
        Some("recovery_rate_limited") => "Too many attempts. Wait before requesting another email.",
        Some("invalid_password") => {
            "Use a password of at least 12 characters and at most 1024 bytes."
        }
        Some("password_changed") => "Your password changed on another device. Try again.",
        Some("login_rate_limited") => "Too many attempts. Try again later.",
        Some("password_unavailable") => "This account does not use password sign-in.",
        _ if status == reqwest::StatusCode::NOT_FOUND => {
            "This server needs an update before profile settings are available."
        }
        _ => "Could not update the account. Refresh your profile and try again.",
    };
    bail!(message)
}

pub(super) async fn command(
    api: &ServerApi,
    privacy: &privacy::Privacy,
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    let request = match name {
        "account_profile" => api.request(reqwest::Method::GET, "/v1/accounts/profile"),
        "update_account_profile" => {
            let display_name = super::string_arg(args, "display_name")?;
            let revision = args
                .get("revision")
                .and_then(Value::as_u64)
                .context("Refresh your profile before saving")?;
            let signed = privacy.signed_profile(
                display_name,
                revision
                    .checked_add(1)
                    .context("Invalid profile revision")?,
            )?;
            api.request(reqwest::Method::PATCH, "/v1/accounts/profile")
                .json(&signed)
        }
        "change_account_password" => {
            let password: ChangePasswordRequest = serde_json::from_value(json!({
                "current_password": args.get("current_password"), "new_password": args.get("new_password")
            })).map_err(|_| anyhow::anyhow!("Enter your current and new passwords"))?;
            api.request(reqwest::Method::POST, "/v1/accounts/password")
                .json(&password)
        }
        "recovery_email" => api.request(reqwest::Method::GET, "/v2/accounts/recovery-email"),
        "set_recovery_email" => {
            let email = super::string_arg(args, "email")?;
            let password = super::string_arg(args, "current_password")?;
            api.request(reqwest::Method::POST, "/v2/accounts/recovery-email")
                .json(&json!({"email":email,"current_password":password}))
        }
        _ => bail!("Unknown account action"),
    };
    let reply = request
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Could not reach the account server"))?;
    let reply = response(reply).await?;
    if name == "set_recovery_email" {
        return Ok(json!({"ok":true}));
    }
    if name == "recovery_email" {
        let value: Value = reply
            .json()
            .await
            .map_err(|_| anyhow::anyhow!("Could not read recovery email settings"))?;
        return Ok(json!({"email":value.get("email").and_then(Value::as_str),
            "pending_email":value.get("pending_email").and_then(Value::as_str),
            "verified":value.get("verified").and_then(Value::as_bool).unwrap_or(false),
            "delivery_available":value.get("delivery_available").and_then(Value::as_bool).unwrap_or(false)}));
    }
    if name == "change_account_password" {
        return Ok(json!({"ok":true}));
    }
    let profile: AccountProfile = reply
        .json()
        .await
        .map_err(|_| anyhow::anyhow!("Invalid account profile response"))?;
    Ok(serde_json::to_value(profile)?)
}
