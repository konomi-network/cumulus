FROM rust:1.51.0

RUN apt update && \
    apt install -y cmake pkg-config libssl-dev git build-essential clang libclang-dev curl libz-dev && \
    rustup default stable && \
    rustup update && \
    rustup update nightly && \
    rustup target add wasm32-unknown-unknown --toolchain nightly