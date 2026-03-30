# =============================================================================
# AI Endpoint Simulator — Multi-stage build (static musl binary)
# =============================================================================

FROM rust:1.93-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y \
    build-essential pkg-config libssl-dev musl-tools \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add x86_64-unknown-linux-musl

COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Static build — no libc dependency at runtime
RUN CC=musl-gcc cargo build --release --target x86_64-unknown-linux-musl

# Runtime — minimal image
FROM alpine:3.19
WORKDIR /app

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/ai-endpoint-simulator ./
COPY config.docker.yml ./config.yml
COPY zresponse ./zresponse

EXPOSE 4545
CMD ["./ai-endpoint-simulator"]
