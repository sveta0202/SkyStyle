
FROM rust:1.79 as builder

WORKDIR /app

COPY rust-service/Cargo.toml rust-service/Cargo.lock ./
COPY rust-service/src ./src

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rust-service /app/rust-service

CMD ["./rust-service"]
