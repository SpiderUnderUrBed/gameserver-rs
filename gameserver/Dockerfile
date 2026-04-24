# --- Stage 1: Rust Build ---
FROM rust:1.94-bookworm AS rust-builder
WORKDIR /app
# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
# Copy only the gameserver node project
COPY gameserver/Cargo.toml gameserver/Cargo.lock ./
COPY gameserver/src ./src
# Build the production binary with full-stack feature
RUN cargo build --release --features full-stack
# --- Stage 2: Final Runtime ---
FROM debian:bookworm-slim
WORKDIR /app
# Install minimal runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    bubblewrap \
    && rm -rf /var/lib/apt/lists/*
# Copy the binary from the rust-builder
COPY --from=rust-builder /app/target/release/gameserver-rs-node ./gameserver-node
EXPOSE 8080 25565
CMD ["./gameserver-node"]