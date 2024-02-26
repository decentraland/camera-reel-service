FROM rust:1.74 as chef
RUN cargo install --version 0.1.62 cargo-chef --locked

WORKDIR /app

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder 
ARG PROJECT
RUN apt update && apt-get install -y protobuf-compiler
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM gcr.io/distroless/cc-debian11 as runtime
ARG PROJECT
COPY --from=builder /app/target/release/camera-reel-service /usr/local/bin/camera-reel-service

ENTRYPOINT [ "/usr/local/bin/camera-reel-service" ]
