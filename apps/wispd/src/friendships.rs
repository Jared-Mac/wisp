use crate::{CommandEnvelope, Context, Daemon, Value, decode, ensure, string_arg};

impl Daemon {
    pub(crate) async fn friendship_command(
        &self,
        command: &CommandEnvelope,
    ) -> anyhow::Result<Value> {
        let server_id = command.args["server_id"]
            .as_str()
            .unwrap_or(&self.primary_server.id);
        let linked = if server_id == self.primary_server.id {
            None
        } else {
            Some(
                self.linked_servers
                    .read()
                    .await
                    .get(server_id)
                    .context("Server is not connected")?
                    .clone(),
            )
        };
        let api = linked.as_ref().map_or(&self.api, |server| &server.api);
        let (method, path) = if command.name == "list_people" {
            (reqwest::Method::GET, "/v1/people".to_owned())
        } else {
            let user: uuid::Uuid = string_arg(&command.args, "user_id")?.parse()?;
            let accept = command.name == "accept_friend_request";
            ensure!(
                accept
                    || command.name == "send_friend_request"
                    || command.name == "dismiss_friend_request"
                    || command.name == "remove_friend",
                "Unknown friend action"
            );
            (
                if matches!(
                    command.name.as_str(),
                    "dismiss_friend_request" | "remove_friend"
                ) {
                    reqwest::Method::DELETE
                } else {
                    reqwest::Method::POST
                },
                if command.name == "remove_friend" {
                    format!("/v1/friends/{user}")
                } else {
                    format!(
                        "/v1/friend-requests/{user}{}",
                        if accept { "/accept" } else { "" }
                    )
                },
            )
        };
        let response = api.request(method, &path).send().await?;
        ensure!(
            command.name != "remove_friend" || response.status() != reqwest::StatusCode::NOT_FOUND,
            "This server needs an update before friends can be removed."
        );
        let result: Value = decode(response).await?;
        // Refresh through the normal contact enrollment and pinned-key checks.
        // A pending request alone never enters the encrypted friend directory.
        if command.name != "list_people" {
            let event = if matches!(
                command.name.as_str(),
                "accept_friend_request" | "remove_friend"
            ) {
                "friendship_changed"
            } else {
                "friend_requests_changed"
            };
            if let Some(server) = linked {
                self.refresh_linked(&server, event).await?;
            } else {
                self.refresh(event).await?;
            }
        }
        Ok(result)
    }
}
