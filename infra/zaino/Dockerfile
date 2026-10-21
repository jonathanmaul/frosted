# Zaino (zainod) image for the Frosted regtest stack.
# Same build as upstream zingolabs/zaino Dockerfile at tag 0.10.1, except that apt package
# versions are not pinned: upstream pins (e.g. ca-certificates=20250419~deb12u1) no longer
# exist in the Debian bookworm archive, so the upstream Dockerfile fails to build.
#
# REGTEST ONLY: built with `no_tls_use_unencrypted_traffic` so gRPC can listen on 0.0.0.0
# inside the container without TLS. docker-compose publishes it on 127.0.0.1 only.
# Do not reuse this image for testnet/mainnet.
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

FROM docker.io/library/debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --uid 1000 --create-home zaino \
  && mkdir -p /app/config /app/data && chown -R zaino /app
COPY --from=builder /out/bin/zainod /usr/local/bin/zainod
USER zaino
WORKDIR /app
EXPOSE 8137
ENTRYPOINT ["zainod"]
CMD ["start", "--config", "/app/config/zainod.toml"]
