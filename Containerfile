# ------------------------------
# Stage 1. Build an app
# ------------------------------
FROM rust:1.96.0 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --locked
# ------------------------------
# Stage 2. Build for runtime
# ------------------------------
FROM dhi.io/debian-base:trixie
ARG GIT_REVISION
ARG BUILD_DATE
ARG VERSION
LABEL org.opencontainers.image.title="lsef" \
org.opencontainers.image.description="lsef (List Extended Features) is a Rust-based file listing tool inspired by ls." \
org.opencontainers.image.url="https://github.com/Takayuki-Todo/lsef" \
org.opencontainers.image.source="https://github.com/Takayuki-Todo/lsef" \
org.opencontainers.image.version=${VERSION} \
org.opencontainers.image.revision=${GIT_REVISION} \
org.opencontainers.image.created=${BUILD_DATE} \
org.opencontainers.image.licenses="MIT"
COPY --from=builder /app/target/release/lsef /app/lsef
WORKDIR /opt
ENTRYPOINT [ "/app/lsef" ]
