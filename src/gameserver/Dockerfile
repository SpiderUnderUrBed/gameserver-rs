FROM rust:latest

WORKDIR /usr/src/app

RUN cargo install cargo-watch

COPY . .

VOLUME ["/usr/src/app"]

EXPOSE 8080

CMD ["cargo", "watch", "--ignore", "server/*", "-x", "run"]
#CMD ["sleep", "infinity"]