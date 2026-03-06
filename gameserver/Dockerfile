FROM rust:latest

RUN apt-get update && apt-get install -y git

WORKDIR /usr/src

RUN cargo install cargo-watch

RUN git clone https://github.com/SpiderUnderUrBed/gameserver-rs.git

WORKDIR /usr/src/gameserver-rs/gameserver

EXPOSE 8080

CMD ["cargo", "watch", "--ignore", "server/*", "-x", "run --features full-stack"]