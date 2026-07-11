#FROM rust:latest AS builder
#
#WORKDIR /app
#
#COPY Cargo.toml Cargo.lock ./
#COPY src ./src
#
#RUN cargo build --release
#
#FROM debian:bookworm-slim
#
#WORKDIR /app
#
#RUN apt-get update \
#    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
#    && rm -rf /var/lib/apt/lists/*
#
#COPY --from=builder /app/target/release/rust-service ./rust-service
#
#EXPOSE 8080
#
#CMD ["./rust-service"]
FROM rust:1.79 as builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rust-service /app/rust-service

CMD ["./rust-service"]