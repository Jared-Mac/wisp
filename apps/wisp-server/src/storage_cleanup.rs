use crate::{AppState, Duration, json, warn};

impl AppState {
    async fn notify_storage_cleanup(&self, last_revision: &mut i64) -> anyhow::Result<()> {
        let revision: i64 =
            sqlx::query_scalar("SELECT revision FROM storage_cleanup_revision WHERE id=1")
                .fetch_one(&self.pool)
                .await?;
        if revision != *last_revision {
            self.emit("message_deleted", json!({"changed": true})).await;
            *last_revision = revision;
        }
        Ok(())
    }

    /// A committed dashboard cleanup invalidates connected clients' snapshots.
    /// Poll a single durable counter, not message contents or per-client history.
    pub async fn maintain_storage_cleanup(self) {
        let mut revision = 0;
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = self.notify_storage_cleanup(&mut revision).await {
                warn!(%error, "storage cleanup notification failed; will retry");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TEST_MEMBER_A_ID, TEST_OWNER_ID, find_or_create_direct, tests::test_config};

    #[tokio::test]
    async fn committed_external_cleanup_notifies_once_and_refresh_removes_deleted_message() {
        let state = AppState::new(test_config()).await.unwrap();
        let user = TEST_OWNER_ID.parse().unwrap();
        let conversation =
            find_or_create_direct(&state.pool, user, TEST_MEMBER_A_ID.parse().unwrap())
                .await
                .unwrap();
        sqlx::query("INSERT INTO messages(id,conversation_id,sender_id,created_at,content_type,payload,encryption_version) VALUES (?,?,?,?,?,?,0)")
            .bind("fe7124c9-6b18-4d39-812e-df2fa1fc92f7").bind(&conversation).bind(TEST_OWNER_ID)
            .bind(crate::Utc::now().to_rfc3339()).bind("text/plain").bind("\"fixture\"")
            .execute(&state.pool).await.unwrap();
        assert_eq!(state.snapshot(user).await.unwrap().messages.len(), 1);
        let mut events = state.events.subscribe();
        let mut seen = 0;
        state.notify_storage_cleanup(&mut seen).await.unwrap();
        assert!(events.try_recv().is_err());
        let mut tx = state.pool.begin().await.unwrap();
        sqlx::query("DELETE FROM messages WHERE conversation_id=?")
            .bind(&conversation)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("UPDATE storage_cleanup_revision SET revision=revision+1 WHERE id=1")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        state.notify_storage_cleanup(&mut seen).await.unwrap();
        let event = events.try_recv().unwrap();
        assert_eq!(event.name, "message_deleted");
        assert_eq!(event.payload, json!({"changed":true}));
        assert!(state.snapshot(user).await.unwrap().messages.is_empty());
        state.notify_storage_cleanup(&mut seen).await.unwrap();
        assert!(events.try_recv().is_err());
    }

    #[tokio::test]
    async fn rolled_back_cleanup_does_not_notify() {
        let state = AppState::new(test_config()).await.unwrap();
        let mut events = state.events.subscribe();
        let mut tx = state.pool.begin().await.unwrap();
        sqlx::query("UPDATE storage_cleanup_revision SET revision=revision+1 WHERE id=1")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        state.notify_storage_cleanup(&mut 0).await.unwrap();
        assert!(events.try_recv().is_err());
    }
}
