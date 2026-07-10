use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use tracing::error;
use uuid::Uuid;

use crate::wardrobe::load_wardrobe;
use crate::weather::{fetch_weather, WeatherInfo};
use crate::{AppConfig, DbPool};

/// Тело запроса на подбор образа.
#[derive(Debug, Deserialize)]
pub struct GenerateInput {
    pub user_id: Uuid,
    pub city: String,
    /// Цель выхода (необязательно): праздник, улица, офис, театр и т.п.
    pub goal: Option<String>,
    /// Тональность (необязательно): тёплые, холодные, яркие, смешанные.
    pub tone: Option<String>,
}

/// Ответ /outfits/generate: текст от нейросети + использованные погода и гардероб.
#[derive(Debug, Serialize)]
pub struct GenerateResponse {
    pub outfit: String,
    pub weather: WeatherInfo,
    pub wardrobe: Vec<String>,
}

/// Сохранённый образ из таблицы outfits.
#[derive(Debug, FromRow, Serialize)]
pub struct SavedOutfit {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct OutfitList {
    pub outfits: Vec<SavedOutfit>,
}

#[derive(Debug, Deserialize)]
pub struct OutfitQuery {
    pub user_id: Uuid,
}

/// Собирает роутер подбора/списка образов.
pub fn outfit_routes() -> Router {
    Router::new()
        .route("/outfits/generate", axum::routing::post(generate))
        .route("/outfits", axum::routing::get(list_outfits))
}

/// Делит ответ нейросети на заголовок (первая строка) и полное описание.
fn split_outfit(text: &str) -> (String, String) {
    let text = text.trim();
    let mut title = text
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('#')
        .trim()
        .to_string();
    if title.len() > 120 {
        title.truncate(120);
    }
    (title, text.to_string())
}

/// POST /outfits/generate — гардероб + погода → промпт → OpenAI-совместимый LLM.
/// Сохраняет результат в таблицу outfits (нефатально: при ошибке БД вернём текст).
pub async fn generate(
    Extension(pool): Extension<DbPool>,
    Extension(cfg): Extension<AppConfig>,
    Json(input): Json<GenerateInput>,
) -> impl IntoResponse {
    let wardrobe = match load_wardrobe(&pool, input.user_id).await {
        Ok(w) => w,
        Err(e) => {
            error!(error = %e, "ошибка БД при чтении гардероба");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": "ошибка чтения гардероба" })),
            )
                .into_response();
        }
    };

    let weather = match fetch_weather(&cfg.client, &cfg.weather_key, &input.city).await {
        Ok(w) => w,
        Err(e) => {
            error!(error = %e, "не удалось получить погоду");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "ok": false, "error": format!("не удалось получить погоду: {e}") })),
            )
                .into_response();
        }
    };

    let wardrobe_text = if wardrobe.is_empty() {
        "пусто".to_string()
    } else {
        wardrobe.join(", ")
    };

    let system = "Ты — персональный стилист приложения SkyStyle. Подбирай образы на основе \
погоды и вещей из гардероба пользователя. Отвечай на русском, кратко и по делу: \
сначала одной строкой назови образ, затем перечисли конкретные вещи для выхода, \
затем одной строкой поясни выбор.";

    let mut user = format!(
        "Город: {city}. Погода: {temp:.0}°C (ощущается как {feels:.0}°C), {desc}. \
Влажность {hum}%, ветер {wind:.0} м/с.\nГардероб пользователя: {wardrobe}.",
        city = weather.city,
        temp = weather.temp,
        feels = weather.feels_like,
        desc = weather.description,
        hum = weather.humidity,
        wind = weather.wind,
        wardrobe = wardrobe_text,
    );

    if let Some(goal) = input.goal.as_deref().filter(|s| !s.trim().is_empty()) {
        user.push_str(&format!("\nЦель выхода: {goal}. Учти её при подборе образа."));
    }
    if let Some(tone) = input.tone.as_deref().filter(|s| !s.trim().is_empty()) {
        user.push_str(&format!("\nПредпочитаемые тона: {tone}."));
    }

    user.push_str("\nПодбери подходящий образ на сегодня и кратко поясни выбор.");

    let body = json!({
        "model": cfg.llm_model,
        "temperature": 0.7,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });

    let base = cfg.llm_base_url.trim_end_matches('/');
    let resp = match cfg
        .client
        .post(format!("{base}/chat/completions"))
        .bearer_auth(&cfg.llm_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "ошибка запроса к LLM");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "ok": false, "error": format!("ошибка запроса к LLM: {e}") })),
            )
                .into_response();
        }
    };

    let status = resp.status();
    let resp_json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            error!(error = %e, "некорректный JSON от LLM");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "ok": false, "error": "некорректный ответ LLM" })),
            )
                .into_response();
        }
    };

    if !status.is_success() {
        let msg = resp_json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("ошибка LLM")
            .to_string();
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "ok": false, "error": msg })),
        )
            .into_response();
    }

    let outfit = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if outfit.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "ok": false, "error": "LLM вернул пустой ответ" })),
        )
            .into_response();
    }

    let (title, description) = split_outfit(&outfit);
    if let Err(e) = sqlx::query_as::<_, SavedOutfit>(
        "INSERT INTO outfits (user_id, title, description) VALUES ($1, $2, $3) \
         RETURNING id, user_id, title, description, created_at",
    )
    .bind(input.user_id)
    .bind(&title)
    .bind(&description)
    .fetch_one(&pool)
    .await
    {
        error!(error = %e, "не удалось сохранить образ (вернём текст без сохранения)");
    }

    (
        StatusCode::OK,
        Json(GenerateResponse {
            outfit,
            weather,
            wardrobe,
        }),
    )
        .into_response()
}

/// GET /outfits?user_id=<UUID> — список сохранённых образов (новые сверху).
pub async fn list_outfits(
    Extension(pool): Extension<DbPool>,
    Query(q): Query<OutfitQuery>,
) -> impl IntoResponse {
    match sqlx::query_as::<_, SavedOutfit>(
        "SELECT id, user_id, title, description, created_at \
         FROM outfits WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(q.user_id)
    .fetch_all(&pool)
    .await
    {
        Ok(list) => (
            StatusCode::OK,
            Json(OutfitList { outfits: list }),
        )
            .into_response(),
        Err(e) => {
            error!(error = %e, "ошибка БД при чтении образов");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": "ошибка БД" })),
            )
                .into_response()
        }
    }
}
