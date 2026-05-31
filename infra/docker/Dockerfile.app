FROM rust:1.85-bookworm AS builder

ARG APP=api-gateway

RUN apt-get update && apt-get install -y cmake libclang-dev clang libcurl4-openssl-dev libzstd-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ crates/
COPY apps/ apps/
COPY migrations/ migrations/

ENV BINDGEN_EXTRA_CLANG_ARGS="-I/usr/lib/gcc/aarch64-linux-gnu/12/include -I/usr/lib/gcc/x86_64-linux-gnu/12/include"
RUN cargo build --release -p aetherdb-${APP}

FROM debian:bookworm-slim

ARG APP=api-gateway

RUN apt-get update && apt-get install -y ca-certificates curl libzstd1 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/${APP} /usr/local/bin/${APP}

ENV APP_NAME=${APP}
ENTRYPOINT ["/bin/sh", "-c", "/usr/local/bin/${APP_NAME}"]
