FROM rust:1.88-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations
RUN cargo build --release -p shakti-game-engine

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/shakti-game-engine /usr/local/bin/shakti-game-engine
ENV RUST_LOG=info
ENV APP_PORT=8010
EXPOSE 8010
CMD ["shakti-game-engine"]
