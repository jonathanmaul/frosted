# One container, two processes: zebrad (testnet) + zainod. Build context: deploy/node.
# zainod is built with no_tls_use_unencrypted_traffic so gRPC can listen beyond localhost; it is
# reachable only through Railway private networking (zaino gRPC on port 8137, no public domain).
ARG ZAINO_REV=3244a74bb09fa6a09a4b2deeb6be53bab0890747

FROM docker.io/library/rust:1.98.0-bookworm AS builder
ARG ZAINO_REV
RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config clang cmake make ca-certificates protobuf-compiler git \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /src
RUN git clone https://github.com/zingolabs/zaino.git . && git checkout "${ZAINO_REV}"
# Limit parallelism to fit small Docker VMs (zainod is a large build).
ENV CARGO_BUILD_JOBS=2
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo install --locked --path packages/zainod --bin zainod --root /out --features no_tls_use_unencrypted_traffic

FROM docker.io/zfnd/zebra:6.4.2
USER root
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl libgcc-s1 \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /out/bin/zainod /usr/local/bin/zainod
COPY zebra.testnet.toml /etc/zebrad/zebrad.toml
COPY zainod.testnet.toml /etc/zainod/zainod.toml
COPY entrypoint.sh /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
