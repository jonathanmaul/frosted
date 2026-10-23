# Public side in one container: glass-indexer + frosted-api. They share one SQLite file
# (PUBLIC_DB_PATH), and a Railway volume cannot be shared between services, so they must live
# together. Only the API port is exposed. Build context: repository root.
FROM docker.io/library/rust:1.94-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config clang cmake make protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
ENV RUSTUP_TOOLCHAIN=1.94.0
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p frosted-api -p glass-indexer \
 && mkdir /out && cp target/release/frosted-api target/release/glass-indexer /out/

FROM docker.io/library/debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /out/ /usr/local/bin/
COPY deploy/public/entrypoint.sh /entrypoint.sh
# Railway volumes are root-owned; mount the volume at /data and keep RAILWAY_RUN_UID=0.
ENV PUBLIC_DB_PATH=/data/public.db API_BIND=0.0.0.0:8080 LOG_LEVEL=info LOG_IP=false
EXPOSE 8080
ENTRYPOINT ["/entrypoint.sh"]
