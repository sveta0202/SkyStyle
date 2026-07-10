# Этап сборки: компилируем release-бинарь.
# Используем rust:latest, т.к. транзитивные зависимости (idna/icu/home) быстро
# повышают требуемую мин. версию rustc — фиксированный старый тег ломает сборку.
FROM rust:latest AS builder

WORKDIR /app

# Копируем сначала манифесты — чтобы кэшировать слой зависимостей
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Собираем. Чтобы не качать весь registry каждый раз, можно добавить
# cargo-chef, но для учебного проекта прямая сборка достаточна.
RUN cargo build --release

# Этап запуска: минимальный образ
FROM debian:bookworm-slim

WORKDIR /app

# Runtime-зависимости для бинаря на Rust (SSL и т.п.)
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Копируем собранный бинарь из этапа builder
COPY --from=builder /app/target/release/rust-service ./rust-service

EXPOSE 8080

CMD ["./rust-service"]
