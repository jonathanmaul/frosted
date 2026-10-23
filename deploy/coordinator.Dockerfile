# Private side: payout-coordinator + wallet-helper + operator CLI in one image (the coordinator
# spawns the helper as a subprocess). No inbound port: the coordinator binds loopback only and
# pushes signed data to the public API over HTTPS. Operator commands run inside the container
# (`railway ssh`). Build context: repository root.
FROM docker.io/library/rust:1.94-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config clang cmake make protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY wallet-helper ./wallet-helper
ENV RUSTUP_TOOLCHAIN=1.94.0
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    --mount=type=cache,target=/src/wallet-helper/target \
    cargo build --release --locked -p payout-coordinator -p frosted-operator -p frosted-verify \
 && cargo build --release --locked --manifest-path wallet-helper/Cargo.toml \
 && mkdir /out \
 && cp target/release/payout-coordinator target/release/frosted-operator target/release/frosted-verify /out/ \
 && cp wallet-helper/target/release/frosted-wallet-helper /out/

FROM docker.io/library/debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /out/ /usr/local/bin/
# Secrets live on the volume at /data/private, never in the image or in git.
ENV WALLET_HELPER_BIN=/usr/local/bin/frosted-wallet-helper \
    COORDINATOR_DB_PATH=/data/coordinator.db COORDINATOR_INDEXER_DB_PATH=/data/coordinator-indexer.db \
    SECRETS_KEY_FILE=/data/private/secrets.key COORDINATOR_KEY_FILE=/data/private/coordinator.key \
    ENVELOPE_SK_FILE=/data/private/envelope.sk HOT_WALLET_SEED_FILE=/data/private/hot-wallet.seed \
    COORDINATOR_BIND=127.0.0.1:8090 LOG_LEVEL=info
CMD ["payout-coordinator"]
