# Use official Rust image as base
FROM rust:1.77-slim

# Create app directory
WORKDIR /app

# Copy your Rust source code
COPY ./gameserver /app

# Build the application in release mode
RUN cargo build --release

# Set the default command to run the binary
CMD ["./target/release/gameserver"]
