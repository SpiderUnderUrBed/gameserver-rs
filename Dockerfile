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

# Start with cargo-watch for hot-reloading
CMD ["cargo", "watch", "-x", "run"]
