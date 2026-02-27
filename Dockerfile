# === Chef: install cargo-chef ===
FROM rust:bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# === Planner: compute dependency recipe ===
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# === Builder: build dependencies then application ===
FROM chef AS builder
RUN apt-get update && apt-get install -y cmake && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin ai-proxy

# === Runtime ===
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -g 1001 aiproxy && useradd -u 1001 -g aiproxy -s /bin/false aiproxy
COPY --from=builder /app/target/release/ai-proxy /usr/local/bin/ai-proxy

USER aiproxy
EXPOSE 8317

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8317/health || exit 1

ENTRYPOINT ["ai-proxy"]
CMD ["--config", "/etc/ai-proxy/config.yaml"]
