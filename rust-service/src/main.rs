use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use tracing::{error, info, instrument};
use uuid::Uuid;

mod config;
mod auth;
mod wardrobe;
mod weather;
mod outfits;

use config::AppConfig;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    id: Uuid,
    name: String,
    mail: String,
}

#[derive(Deserialize)]
struct CreateUserInput {
    name: String,
    mail: String,
}

pub type DbPool = Pool<Postgres>;

#[instrument(skip(pool, input), fields(user_name = %input.name, user_mail = %input.mail))]
async fn create_user(
    Extension(pool): Extension<DbPool>,
    axum::Json(input): axum::Json<CreateUserInput>,
) -> impl IntoResponse {
    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (name, mail) VALUES ($1, $2) RETURNING id, name, mail"#,
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
            )
                .into_response()
        }
    }
}

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
            )
                .into_response()
        }
    }
}

async fn init_db(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::query(r#"CREATE EXTENSION IF NOT EXISTS pgcrypto;"#)
        .execute(pool)
        .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS users (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            name TEXT NOT NULL,
            mail TEXT NOT NULL UNIQUE,
            password_hash TEXT
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS wardrobe (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            item TEXT NOT NULL,
            UNIQUE (user_id, item)
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS outfits (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )"#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL должен быть задан в .env или переменных окружения");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    init_db(&pool).await?;
    info!("подключение к БД установлено, таблицы users/wardrobe/outfits готовы");

    let config = AppConfig::from_env();

    let app = Router::new()
        .route("/users", post(create_user).get(get_users))
        .merge(auth::auth_routes())
        .merge(wardrobe::wardrobe_routes())
        .merge(weather::weather_routes())
        .merge(outfits::outfit_routes())
        .layer(Extension(pool))
        .layer(Extension(config));

    info!("сервер слушает на http://0.0.0.0:8080");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;

    Ok(())
}