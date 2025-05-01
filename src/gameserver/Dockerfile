
FROM rust:alpine AS builder



RUN apk add --no-cache \
    build-base \
    openssl-dev \
    pkgconfig \
    perl

WORKDIR /usr/src/app


COPY src/gameserver/Cargo.toml src/gameserver/Cargo.lock ./
COPY src/gameserver/src ./src/



RUN cargo build --release


FROM alpine:latest


RUN apk add --no-cache ca-certificates



COPY --from=builder /usr/src/app/target/release/myapp /usr/local/bin/myapp

ENTRYPOINT ["/usr/local/bin/myapp"]
