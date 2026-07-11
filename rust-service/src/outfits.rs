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

#[derive(Debug, Deserialize)]
pub struct GenerateInput {
    pub user_id: Uuid,
    pub city: String,
    pub goal: Option<String>,
    pub tone: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GenerateResponse {
    pub ok: bool,
    pub title: String,
    pub items: Vec<String>,
    pub note: String,
    pub weather: WeatherInfo,
    pub wardrobe: Vec<String>,
    pub image_base64: Option<String>,
}

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

pub fn outfit_routes() -> Router {
    Router::new()
        .route("/outfits/generate", axum::routing::post(generate))
        .route("/outfits", axum::routing::get(list_outfits))
}

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

    if title.is_empty() {
        title = "Новый образ".to_string();
    }

    if title.len() > 120 {
        title.truncate(120);
    }

    (title, text.to_string())
}

fn normalize_outfit(text: &str) -> (String, Vec<String>, String) {
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.trim().trim_start_matches('#').trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    if lines.is_empty() {
        return (
            "Новый образ".to_string(),
            vec!["Не удалось разобрать состав образа".to_string()],
            "".to_string(),
        );
    }

    let title = lines[0].clone();
    let mut items = Vec::new();
    let mut note_parts = Vec::new();

    for line in lines.iter().skip(1) {
        let cleaned = line
            .trim_start_matches("-")
            .trim_start_matches("•")
            .trim()
            .to_string();

        let lower = cleaned.to_lowercase();

        if lower.starts_with("почему")
            || lower.starts_with("объяснение")
            || lower.starts_with("пояснение")
            || lower.starts_with("потому")
        {
            note_parts.push(cleaned);
        } else if cleaned.contains(':') || cleaned.len() < 90 {
            items.push(cleaned);
        } else {
            note_parts.push(cleaned);
        }
    }

    if items.is_empty() {
        for line in lines.iter().skip(1).take(4) {
            items.push(line.clone());
        }
    }

    let note = note_parts.join(" ");

    (title, items, note)
}

async fn generate_image_base64(
    cfg: &AppConfig,
    title: &str,
    items: &[String],
    note: &str,
    goal: Option<&str>,
    tone: Option<&str>,
    weather: &WeatherInfo,
) -> Result<Option<String>, String> {
    let goal_text = goal.unwrap_or("повседневный выход");
    let tone_text = tone.unwrap_or("смешанные");
    let items_text = if items.is_empty() {
        "без уточненного списка".to_string()
    } else {
        items.join(", ")
    };

    let prompt = format!(
        "Создай fashion flat lay фото стильного аутфита. \
Название образа: {title}. \
Цель: {goal}. Предпочитаемые тона: {tone}. \
Погода: {temp:.0}°C, {desc}. \
В образе должны быть: {items}. \
Стиль: современная fashion editorial съемка одежды сверху, аккуратная композиция, светлый чистый фон, реалистичная одежда, без людей, premium styling. \
Дополнительный контекст: {note}",
        title = title,
        goal = goal_text,
        tone = tone_text,
        temp = weather.temp,
        desc = weather.description,
        items = items_text,
        note = note
    );

    let base = cfg.image_base_url.trim_end_matches('/');
    let body = json!({
        "model": cfg.image_model,
        "prompt": prompt,
        "size": "1024x1024"
    });

    let resp = cfg
        .client
        .post(format!("{base}/images/generations"))
        .bearer_auth(&cfg.image_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("ошибка запроса к image api: {e}"))?;

    let status = resp.status();

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("некорректный JSON от image api: {e}"))?;

    if !status.is_success() {
        let msg = resp_json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("ошибка image api")
            .to_string();

        return Err(msg);
    }

    let image_b64 = resp_json["data"][0]["b64_json"]
        .as_str()
        .map(|s| s.to_string());

    Ok(image_b64)
}

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
погоды и вещей из гардероба пользователя. Отвечай на русском, кратко и структурно: \
1) первая строка — короткое название образа, \
2) дальше 3-6 строк с конкретными вещами, \
3) последняя строка — короткое пояснение выбора.";

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

    let (db_title, description) = split_outfit(&outfit);
    let (title, items, note) = normalize_outfit(&outfit);

    if let Err(e) = sqlx::query_as::<_, SavedOutfit>(
        "INSERT INTO outfits (user_id, title, description) VALUES ($1, $2, $3) \
         RETURNING id, user_id, title, description, created_at",
    )
    .bind(input.user_id)
    .bind(&db_title)
    .bind(&description)
    .fetch_one(&pool)
    .await
    {
        error!(error = %e, "не удалось сохранить образ (вернём текст без сохранения)");
    }

    let image_base64 = match generate_image_base64(
        &cfg,
        &title,
        &items,
        &note,
        input.goal.as_deref(),
        input.tone.as_deref(),
        &weather,
    )
    .await
    {
        Ok(image) => image,
        Err(e) => {
            error!(error = %e, "ошибка генерации картинки");
            None
        }
    };

    (
        StatusCode::OK,
        Json(GenerateResponse {
            ok: true,
            title,
            items,
            note,
            weather,
            wardrobe,
            image_base64,
        }),
    )
        .into_response()
}

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