FROM rust:1.86 AS chef
RUN cargo install --version 0.1.62 cargo-chef --locked

WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM debian:13-slim AS runtime
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3t64 \
    && rm -rf /var/lib/apt/lists/*

# Run as an unprivileged user
RUN useradd --system --no-create-home --uid 10001 appuser

COPY --from=builder /app/target/release/camera-reel-service /usr/local/bin/camera-reel-service

USER appuser
EXPOSE 3000

ENTRYPOINT [ "/usr/local/bin/camera-reel-service" ]
