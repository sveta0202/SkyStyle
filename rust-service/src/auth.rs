use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    Json,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::{error, info, instrument};
use uuid::Uuid;
use bcrypt::{hash, verify, DEFAULT_COST};
use crate::DbPool;


#[derive(Debug, Deserialize)]
pub struct AuthInput {
    pub name: String,
    pub mail: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub id: Uuid,
    pub name: String,
    pub mail: String,
}

#[derive(Debug, FromRow)]
struct UserRow {
    id: Uuid,
    name: String,
    mail: String,
    password_hash: String,
}


pub fn auth_routes() -> Router {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/registr", post(register))
        .route("/auth/login", post(login))
}


fn validate(input: &AuthInput) -> Result<(), &'static str> {
    if input.name.trim().is_empty() {
        return Err("имя не может быть пустым");
    }
    if input.mail.trim().is_empty() || !input.mail.contains('@') {
        return Err("некорректный email");
    }
    if input.password.trim().len() < 6 {
        return Err("пароль должен быть не короче 6 символов");
    }
    Ok(())
}

#[instrument(skip(pool, input), fields(mail = %input.mail))]
pub async fn register(
    Extension(pool): Extension<DbPool>,
    Json(input): Json<AuthInput>,
) -> impl IntoResponse {
    if let Err(msg) = validate(&input) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    let exists = sqlx::query("SELECT 1 FROM users WHERE mail = $1")
        .bind(&input.mail)
        .fetch_optional(&pool)
        .await;

    match exists {
        Ok(Some(_)) => {
            return (StatusCode::CONFLICT,
                Json(serde_json::json!({ "error": "пользователь с таким email уже существует" })))
                .into_response();
        }
        Ok(None) => {}
        Err(e) => {
            error!(error = %e, "ошибка БД при проверке пользователя");
            return (StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "ошибка сервера" }))).into_response();
        }
    }

    let password_hash = match hash(&input.password, DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            error!(error = %e, "не удалось хэшировать пароль");
            return (StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "ошибка сервера" }))).into_response();
        }
    };

    let result = sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (name, mail, password_hash) VALUES ($1, $2, $3) RETURNING id, name, mail, password_hash"
    )
    .bind(&input.name)
    .bind(&input.mail)
    .bind(&password_hash)
    .fetch_one(&pool)
    .await;

    match result {
        Ok(u) => {
            info!(user_id = %u.id, "пользователь зарегистрирован");
            let resp = AuthResponse { id: u.id, name: u.name, mail: u.mail };
            (StatusCode::CREATED, Json(resp)).into_response()
        }
        Err(e) => {
            error!(error = %e, "ошибка БД при регистрации");
            (StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "не удалось создать пользователя" }))).into_response()
        }
    }
}

#[instrument(skip(pool, input), fields(mail = %input.mail))]
pub async fn login(
    Extension(pool): Extension<DbPool>,
    Json(input): Json<AuthInput>,
) -> impl IntoResponse {
    if let Err(msg) = validate(&input) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": msg }))).into_response();
    }

    let result = sqlx::query_as::<_, UserRow>(
        "SELECT id, name, mail, password_hash FROM users WHERE mail = $1"
    )
    .bind(&input.mail)
    .fetch_optional(&pool)
    .await;

    let user = match result {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "пользователь не найден" }))).into_response();
        }
        Err(e) => {
            error!(error = %e, "ошибка БД при входе");
            return (StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "ошибка сервера" }))).into_response();
        }
    };

    match verify(&input.password, &user.password_hash) {
        Ok(true) => {
            info!(user_id = %user.id, "успешный вход");
            let resp = AuthResponse { id: user.id, name: user.name, mail: user.mail };
            (StatusCode::OK, Json(resp)).into_response()
        }
        Ok(false) => {
            (StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "неверный пароль" }))).into_response()
        }
        Err(e) => {
                Json(serde_json::json!({ "error": "ошибка сервера" })).into_response()
        }
    }
}