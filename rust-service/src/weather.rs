use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppConfig;

/// Нормализованная погода, которую отдаём клиенту и используем в подборе образов.
#[derive(Debug, Clone, Serialize)]
pub struct WeatherInfo {
    pub city: String,
    pub temp: f64,
    pub feels_like: f64,
    pub description: String,
    pub humidity: i64,
    pub wind: f64,
    pub icon: String,
}

/// Ответ OpenWeatherMap (берём только нужные поля).
#[derive(Debug, Deserialize)]
struct OwmMain {
    temp: f64,
    feels_like: f64,
    humidity: i64,
}
#[derive(Debug, Deserialize)]
struct OwmWeather {
    description: String,
    icon: String,
}
#[derive(Debug, Deserialize)]
struct OwmWind {
    speed: f64,
}
#[derive(Debug, Deserialize)]
struct OwmResponse {
    weather: Vec<OwmWeather>,
    main: OwmMain,
    wind: OwmWind,
    name: String,
}

/// Ошибки получения погоды (текст + код для маппинга в HTTP-статус).
#[derive(Debug)]
pub enum WeatherError {
    Request(reqwest::Error),
    Status(u16, String),
    NoKey,
}

impl std::fmt::Display for WeatherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeatherError::Request(e) => write!(f, "ошибка запроса погоды: {e}"),
            WeatherError::Status(code, body) => write!(f, "OpenWeatherMap {code}: {body}"),
            WeatherError::NoKey => write!(f, "OPENWEATHER_API_KEY не задан"),
        }
    }
}

/// GET /weather?city=<город> — проксирует запрос к OpenWeatherMap.
pub async fn get_weather(
    Extension(cfg): Extension<AppConfig>,
    Query(q): Query<WeatherQuery>,
) -> impl IntoResponse {
    match fetch_weather(&cfg.client, &cfg.weather_key, &q.city).await {
        Ok(w) => (StatusCode::OK, Json(w)).into_response(),
        Err(e) => {
            let (status, msg) = match &e {
                WeatherError::NoKey => (StatusCode::BAD_GATEWAY, e.to_string()),
                WeatherError::Request(_) => (StatusCode::BAD_GATEWAY, e.to_string()),
                WeatherError::Status(code, _) => {
                    let s = if *code == 404 {
                        StatusCode::NOT_FOUND
                    } else {
                        StatusCode::BAD_GATEWAY
                    };
                    (s, e.to_string())
                }
            };
            (status, Json(json!({ "error": msg }))).into_response()
        }
    }
}

/// Запрашивает погоду через OpenWeatherMap и нормализует ответ.
pub async fn fetch_weather(
    client: &reqwest::Client,
    key: &str,
    city: &str,
) -> Result<WeatherInfo, WeatherError> {
    if key.trim().is_empty() {
        return Err(WeatherError::NoKey);
    }

    let resp = client
        .get("https://api.openweathermap.org/data/2.5/weather")
        .query(&[
            ("q", city),
            ("units", "metric"),
            ("lang", "ru"),
            ("appid", key),
        ])
        .send()
        .await
        .map_err(WeatherError::Request)?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(WeatherError::Status(status.as_u16(), body));
    }

    let data: OwmResponse = resp.json().await.map_err(WeatherError::Request)?;
    let w = data
        .weather
        .into_iter()
        .next()
        .unwrap_or(OwmWeather {
            description: String::new(),
            icon: String::new(),
        });

    Ok(WeatherInfo {
        city: data.name,
        temp: data.main.temp,
        feels_like: data.main.feels_like,
        description: w.description,
        humidity: data.main.humidity,
        wind: data.wind.speed,
        icon: w.icon,
    })
}

/// Собирает роутер погоды.
pub fn weather_routes() -> Router {
    Router::new().route("/weather", axum::routing::get(get_weather))
}

#[derive(Debug, Deserialize)]
pub struct WeatherQuery {
    pub city: String,
}
