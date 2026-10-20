# Build stage
FROM rust:bookworm AS builder

WORKDIR /build

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked --bin commy

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/commy /usr/local/bin/commy

# Durable state (services, grants, audit log, tenant auth databases) lives under /var/lib/commy;
# mount a volume there. Service files are created under tenant_<id>/ in the working directory.
RUN mkdir -p /var/lib/commy /etc/commy
ENV COMMY_STATE_DIR=/var/lib/commy
VOLUME ["/var/lib/commy"]

EXPOSE 8443

# The server has no HTTP health endpoint; healthy means the WSS port accepts a TCP connection.
HEALTHCHECK --interval=10s --timeout=5s --retries=3 \
    CMD ["bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/${COMMY_LISTEN_PORT:-8443}"]

CMD ["commy"]
