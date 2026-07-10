// ─────────────────────────────────────────────────────────────────────────────
//  SkyStyle — Rust-микросервис (бэкенд)
//  Связывает: HTTP-роутер (axum) + модуль авторизации (auth.rs)
//              + пул PostgreSQL (sqlx) + трейсинг (tracing)
//
//  Общая схема взаимодействия микросервисов:
//    Браузер → Flask (Python :8000) → HTTP → Этот сервис (:8080) → PostgreSQL
//  Flask проксирует /login и /register сюда (см. python-service/app.py).
// ─────────────────────────────────────────────────────────────────────────────

use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tracing::{error, info, instrument};
use uuid::Uuid;
use std::env;

/// Подключаем модуль авторизации/регистрации.
mod auth;

// ─── Структуры данных ───

/// Пользователь (мапится на таблицу users).
/// FromRow — авто-маппинг строки БД в struct через sqlx::query_as.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: Uuid,
    name: String,
    mail: String,
}

/// Входные данные для POST /users (id генерируется БД).
#[derive(Deserialize)]
struct CreateUserInput {
    name: String,
    mail: String,
}

/// Пул соединений к PostgreSQL (sqlx + Tokio).
/// `pub` — чтобы модуль `auth` мог использовать этот тип.
pub type DbPool = Pool<Postgres>;

// ─── Обработчики (legacy CRUD) ───

/// POST /users — создать пользователя (без пароля).
#[instrument(skip(pool, input), fields(user_name = %input.name, user_mail = %input.mail))]
async fn create_user(
    Extension(pool): Extension<DbPool>,
    axum::Json(input): axum::Json<CreateUserInput>,
) -> impl IntoResponse {
    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (name, mail) VALUES ($1, $2) RETURNING id, name, mail"#
    )
    .bind(&input.name)
    .bind(&input.mail)
    .fetch_one(&pool)
    .await;

    match user {
        Ok(u) => {
            info!("пользователь создан");
            (StatusCode::CREATED, axum::Json(u)).into_response()
        }
        Err(e) => {
            error!(error = %e, "ошибка БД при создании пользователя");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({ "error": "failed to create user" })),
            ).into_response()
        }
    }
}

/// GET /users — список всех пользователей.
#[instrument(skip(pool))]
async fn get_users(Extension(pool): Extension<DbPool>) -> impl IntoResponse {
    let users = sqlx::query_as::<_, User>("SELECT id, name, mail FROM users")
        .fetch_all(&pool)
        .await;

    match users {
        Ok(list) => {
            info!(count = list.len(), "список пользователей получен");
            (StatusCode::OK, axum::Json(list)).into_response()
        }
        Err(e) => {
            error!(error = %e, "ошибка БД при получении пользователей");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({ "error": "failed to fetch users" })),
            ).into_response()
        }
    }
}

// ─── Инициализация БД ───

/// Создаёт таблицу users, если её нет. Вызывается при старте.
/// password_hash — необязательный (legacy /users не передаёт пароль,
/// а /auth/register — передаёт и хэширует через bcrypt).
async fn init_db(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS users (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            name TEXT NOT NULL,
            mail TEXT NOT NULL UNIQUE,
            password_hash TEXT
        )"#
    )
    .execute(pool)
    .await?;
    Ok(())
}

// ─── Main ───

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Tracing (логирование). Уровень берётся из RUST_LOG.
    // RUST_LOG=info cargo run (bash) / $env:RUST_LOG="info"; cargo run (PowerShell)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .with_file(true)
        .with_file(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();

    // 2. Загрузка .env (локально; в Docker переменные задаёт docker-compose).
    dotenvy::dotenv().ok();

    // 3. Подключение к БД (пул соединений).
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL должен быть задан в .env или переменных окружения");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // 4. Инициализация схемы (создаём таблицу, если нет).
    init_db(&pool).await?;
    info!("подключение к БД установлено, таблица users готова");

    // 5. Router — собираем все маршруты вместе.
    //    /users          — legacy CRUD (этот файл)
    //    /auth/*         — регистрация/вход (модуль auth)
    // Extension(pool) — кладём пул в расширения, хендлеры берут через Extension.
    let app = Router::new()
        .route("/users", post(create_user).get(get_users))
        .merge(auth::auth_routes())
        .layer(Extension(pool));

    // 6. Запуск сервера.
    //    0.0.0.0 — слушать все интерфейсы (нужно внутри Docker-контейнера).
    println!("Сервер слушает на http://0.0.0.0:8080");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
