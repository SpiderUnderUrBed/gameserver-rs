# Use an official Rust image as a base
FROM rust:latest

# Set the working directory
WORKDIR /usr/src/app

# Install cargo-watch for live reloading
RUN cargo install cargo-watch

# Copy the Rust project into the container
COPY . .

# Set up a volume for live-editing
VOLUME ["/usr/src/app"]

# Expose the port the app runs on
EXPOSE 8080

# Start with cargo-watch for hot-reloading, ignoring the src/html directory
#CMD ["cargo", "watch", "--ignore", "src/html/*", "--ignore", "src/css/*", "--ignore", "src/scripts/*", "--ignore", "src/gameserver/*", "-x", "run --features full-stack"]
CMD ["cargo", "watch", "--ignore", "src/vanilla/*", "--ignore", "src/svelte/*", "--ignore", "src/gameserver/*", "-x", "run --features full-stack"]
# -x watch
# cargo run .
# cargo watch -x run