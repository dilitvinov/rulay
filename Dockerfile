FROM rust:1.86-slim-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

COPY --from=builder /app/target/release/rulay /usr/local/bin/rulay
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

RUN chmod +x /usr/local/bin/rulay /usr/local/bin/docker-entrypoint.sh

ENV MODE=""
ENV UPSTREAM_SERVER=""
ENV UPSTREAM_PORT=""
ENV DOWNSTREAM_SERVER=""
ENV DOWNSTREAM_PORT=""

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
