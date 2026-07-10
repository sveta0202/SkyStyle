use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::error;
use uuid::Uuid;

use crate::DbPool;

/// Тело запроса на добавление/удаление вещей гардероба.
#[derive(Debug, Deserialize)]
pub struct WardrobeInput {
    pub user_id: Uuid,
    pub items: Vec<String>,
}

/// Ответ со списком вещей. Избегаем возврата "голого" массива.
#[derive(Debug, Serialize)]
pub struct WardrobeList {
    pub items: Vec<String>,
}

/// Собирает роутер гардероба: POST — добавить, GET — список, DELETE — удалить.
pub fn wardrobe_routes() -> Router {
    Router::new().route(
        "/wardrobe",
        axum::routing::post(add_items).get(list_items).delete(remove_items),
    )
}

/// POST /wardrobe — добавляет вещи (batch). Дубликаты игнорируются (ON CONFLICT).
pub async fn add_items(
    Extension(pool): Extension<DbPool>,
    Json(input): Json<WardrobeInput>,
) -> impl IntoResponse {
    let mut added = 0i64;
    for raw in &input.items {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        match sqlx::query(
            "INSERT INTO wardrobe (user_id, item) VALUES ($1, $2) \
             ON CONFLICT (user_id, item) DO NOTHING",
        )
        .bind(input.user_id)
        .bind(item)
        .execute(&pool)
        .await
        {
            Ok(r) => {
                if r.rows_affected() > 0 {
                    added += 1;
                }
            }
            Err(e) => {
                error!(error = %e, "ошибка БД при добавлении вещи");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "ok": false, "error": "ошибка БД" })),
                )
                    .into_response();
            }
        }
    }
    (StatusCode::OK, Json(serde_json::json!({ "ok": true, "added": added }))).into_response()
}

/// GET /wardrobe?user_id=<UUID> — возвращает список вещей пользователя.
pub async fn list_items(
    Extension(pool): Extension<DbPool>,
    Query(params): Query<WardrobeQuery>,
) -> impl IntoResponse {
    match load_wardrobe(&pool, params.user_id).await {
        Ok(items) => (StatusCode::OK, Json(WardrobeList { items })).into_response(),
        Err(e) => {
            error!(error = %e, "ошибка БД при чтении гардероба");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "error": "ошибка БД" })),
            )
                .into_response()
        }
    }
}

/// DELETE /wardrobe — удаляет перечисленные вещи пользователя.
pub async fn remove_items(
    Extension(pool): Extension<DbPool>,
    Json(input): Json<WardrobeInput>,
) -> impl IntoResponse {
    let items: Vec<&str> = input
        .items
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if items.is_empty() {
        return (StatusCode::OK, Json(serde_json::json!({ "ok": true, "removed": 0 }))).into_response();
    }

    match sqlx::query("DELETE FROM wardrobe WHERE user_id = $1 AND item = ANY($2)")
        .bind(input.user_id)
        .bind(items)
        .execute(&pool)
        .await
    {
        Ok(r) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "removed": r.rows_affected() })),
        )
            .into_response(),
        Err(e) => {
            error!(error = %e, "ошибка БД при удалении вещей");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "error": "ошибка БД" })),
            )
                .into_response()
        }
    }
}

/// Загружает вещи гардероба пользователя. Переиспользуется модулем подбора образов.
pub async fn load_wardrobe(pool: &DbPool, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query("SELECT item FROM wardrobe WHERE user_id = $1 ORDER BY item")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| r.get::<String, _>("item"))
        .collect())
}

/// Параметры GET-запроса списка гардероба.
#[derive(Debug, Deserialize)]
pub struct WardrobeQuery {
    pub user_id: Uuid,
}
