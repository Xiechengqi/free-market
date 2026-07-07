# syntax=docker/dockerfile:1.7

FROM rust:1.83-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock build.sh ./
COPY migrations ./migrations
COPY assets ./assets
COPY templates ./templates
COPY src ./src
RUN cargo build --release && \
    strip target/release/free-market

FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates tzdata && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/target/release/free-market /app/free-market
COPY --from=builder /src/config.example.toml /app/config.example.toml
RUN useradd --system --create-home --shell /usr/sbin/nologin freemarket && \
    mkdir -p /app/data /app/uploads /app/logs && \
    chown -R freemarket:freemarket /app
USER freemarket
ENV FREEMARKET_CONFIG=/app/config.toml
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["/app/free-market", "--healthcheck"]
ENTRYPOINT ["/app/free-market"]
