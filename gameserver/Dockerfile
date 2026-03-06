FROM rust:latest

WORKDIR /usr/src/app

RUN cargo install cargo-watch

COPY . .

VOLUME ["/usr/src/app/server"]

EXPOSE 8080

CMD ["cargo", "run", "--features", "full-stack"]
