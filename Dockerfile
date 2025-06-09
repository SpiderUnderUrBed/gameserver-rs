# Use official Rust image as base
FROM rust:latest

# Install system dependencies for Node.js and npm
RUN apt-get update && \
    apt-get install -y curl gnupg && \
    curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get install -y nodejs

# Set the working directory
WORKDIR /usr/src/app

# Install cargo-watch for hot reloading
RUN cargo install cargo-watch

# Copy the entire project
COPY . .

# Build the Svelte frontend (assumes src/svelte/package.json exists)
WORKDIR /usr/src/app/src/svelte
RUN npm install && npm run build

# Go back to Rust project root
WORKDIR /usr/src/app

# Expose the port your app listens on
EXPOSE 8080

# Start with cargo-watch for hot reloading, ignoring non-Rust frontend folders
CMD ["cargo", "watch", "--ignore", "src/vanilla/*", "--ignore", "src/svelte/*", "--ignore", "src/gameserver/*", "-x", "run --features full-stack"]
