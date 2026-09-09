//! Account-owned profile pictures. Only normalized, bounded PNGs reach storage.
use super::{ApiError, AppState, HeaderMap, Json, Path, State, Uuid, Value, authenticate_headers};
use axum::response::{IntoResponse, Response};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::json;
use std::io::Cursor;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Upload {
    data: String,
}

pub(super) async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Upload>,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    if request.data.len() > 2_796_204 {
        return Err(ApiError::bad_request(
            "image_too_large",
            "Choose an image under 2 MB",
        ));
    }
    let bytes = STANDARD
        .decode(request.data)
        .map_err(|_| ApiError::bad_request("invalid_image", "Invalid profile picture"))?;
    if bytes.len() > 2_097_152 {
        return Err(ApiError::bad_request(
            "image_too_large",
            "Choose an image under 2 MB",
        ));
    }
    let png = tokio::task::spawn_blocking(move || normalize(bytes))
        .await
        .map_err(ApiError::internal)?
        .map_err(|_| {
            ApiError::bad_request(
                "invalid_image",
                "Choose a PNG, JPEG, GIF or WebP image up to 4096 × 4096",
            )
        })?;
    sqlx::query("INSERT INTO account_avatars(user_id,png) VALUES (?,?) ON CONFLICT(user_id) DO UPDATE SET png=excluded.png")
        .bind(user.to_string()).bind(png).execute(&state.pool).await.map_err(ApiError::internal)?;
    state
        .emit("account_avatar_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

fn normalize(bytes: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()?
        .resize_to_fill(256, 256, image::imageops::FilterType::Lanczos3);
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}

pub(super) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let user = authenticate_headers(&state, &headers).await?;
    sqlx::query("DELETE FROM account_avatars WHERE user_id=?")
        .bind(user.to_string())
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    state
        .emit("account_avatar_changed", json!({"changed":true}))
        .await;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    authenticate_headers(&state, &headers).await?;
    let png: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT png FROM account_avatars WHERE user_id=?")
            .bind(id.to_string())
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?;
    Ok((
        [
            ("content-type", "image/png"),
            ("cache-control", "private, no-store"),
            ("x-content-type-options", "nosniff"),
        ],
        png.ok_or_else(|| ApiError::not_found("No profile picture"))?,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TEST_MEMBER_A_ID, TEST_MEMBER_B_ID, TEST_OWNER_ID, router, tests::test_config,
        text_tests::request,
    };
    use axum::{body::to_bytes, http::StatusCode};
    #[tokio::test]
    async fn avatars_are_normalized_shared_and_writable_only_by_their_owner() {
        let state = AppState::new(test_config()).await.unwrap();
        let app = router(state);
        let mut png = Cursor::new(Vec::new());
        image::RgbImage::from_pixel(400, 100, image::Rgb([30, 100, 200]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let data = STANDARD.encode(png.into_inner());
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/accounts/avatar",
                TEST_MEMBER_A_ID,
                json!({"data":data,"user_id":TEST_OWNER_ID})
            )
            .await
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            request(
                &app,
                "POST",
                "/v1/accounts/avatar",
                TEST_MEMBER_A_ID,
                json!({"data":data})
            )
            .await
            .status(),
            StatusCode::OK
        );
        let path = format!("/v1/accounts/{TEST_MEMBER_A_ID}/avatar");
        let downloaded = request(&app, "GET", &path, TEST_MEMBER_B_ID, json!({})).await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        assert_eq!(downloaded.headers()["content-type"], "image/png");
        let bytes = to_bytes(downloaded.into_body(), 512 * 1024).await.unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (256, 256));
        assert_eq!(
            request(
                &app,
                "DELETE",
                "/v1/accounts/avatar",
                TEST_MEMBER_B_ID,
                json!({})
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            request(&app, "GET", &path, TEST_MEMBER_B_ID, json!({}))
                .await
                .status(),
            StatusCode::OK
        );
        for data in [
            STANDARD.encode(b"<svg/>"),
            "not base64".into(),
            "A".repeat(2_796_208),
        ] {
            assert_eq!(
                request(
                    &app,
                    "POST",
                    "/v1/accounts/avatar",
                    TEST_MEMBER_A_ID,
                    json!({"data":data})
                )
                .await
                .status(),
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(
            request(
                &app,
                "DELETE",
                "/v1/accounts/avatar",
                TEST_MEMBER_A_ID,
                json!({})
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            request(&app, "GET", &path, TEST_MEMBER_B_ID, json!({}))
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        assert!(normalize(Vec::new()).is_err());
    }
}
