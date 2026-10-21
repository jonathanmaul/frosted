# Regtest test wallet (zcash-devtool) for the Frosted dev stack.
# Built in Docker so it does not depend on host protoc/abseil. Not published on crates.io; commit pinned.
ARG ZCASH_DEVTOOL_REV=0b3436bb8d235c67cd36b800cb45a7b1b489c415

FROM docker.io/library/rust:1.98.0-bookworm AS builder
ARG ZCASH_DEVTOOL_REV
RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config clang cmake protobuf-compiler git ca-certificates \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /src
RUN git clone https://github.com/zcash/zcash-devtool.git . && git checkout "${ZCASH_DEVTOOL_REV}"
ENV CARGO_BUILD_JOBS=2
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --features regtest_support \
    && cp target/release/zcash-devtool /usr/local/bin/zcash-devtool

FROM docker.io/library/debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/bin/zcash-devtool /usr/local/bin/zcash-devtool
WORKDIR /work
ENTRYPOINT ["zcash-devtool"]
