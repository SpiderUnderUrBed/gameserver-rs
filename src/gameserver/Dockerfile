FROM rust:slim AS builder


RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    build-essential \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*


ENV OPENSSL_DIR="/usr"
ENV OPENSSL_INCLUDE_DIR="/usr/include"
ENV OPENSSL_LIB_DIR="/usr/lib/x86_64-linux-gnu"


RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /usr/src/app


COPY src/gameserver/ .


RUN cargo build --release --target=x86_64-unknown-linux-musl


FROM debian:bookworm-slim


RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*


COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/myapp /usr/local/bin/myapp

ENTRYPOINT ["/usr/local/bin/myapp"]
