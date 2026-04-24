# --- Stage 1: Frontend Build ---
FROM node:24-slim AS frontend-builder
WORKDIR /app
# Copy only package files first to leverage Docker cache
COPY src/frontend/package*.json ./
RUN npm install && mkdir build
# Copy the rest of the frontend source and build
COPY src/frontend/ ./
RUN npm run build

# --- Stage 2: Rust Build ---
FROM rust:1.76-slim AS rust-builder
WORKDIR /app
# Install build dependencies (linker, etc.)
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
# Copy the entire project
COPY . .
# Copy the built assets from the frontend stage (if Rust needs to embed them)
COPY --from=frontend-builder /app/build ./src/frontend/build
# Build the production binary
RUN cargo build --release --features full-stack

# --- Stage 3: Final Runtime ---
FROM debian:bookworm-slim
WORKDIR /app
# Install minimal runtime dependencies (SSL is usually needed for web servers)
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
# Copy the binary from the rust-builder
COPY --from=rust-builder /app/target/release/gameserver-rs ./gameserver
# Copy frontend assets if your binary serves them at runtime
COPY --from=frontend-builder /app/build ./src/frontend/build

# Expose the port the app runs on
EXPOSE 8080

# Run the binary directly
CMD ["./gameserver"]
