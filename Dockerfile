FROM debian:bookworm-slim

WORKDIR /app

COPY target/x86_64-unknown-linux-gnu/release/rulay /usr/local/bin/rulay
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

RUN chmod +x /usr/local/bin/rulay /usr/local/bin/docker-entrypoint.sh

ENV MODE=""
ENV UPSTREAM_SERVER=""
ENV UPSTREAM_PORT=""
ENV DOWNSTREAM_SERVER=""
ENV DOWNSTREAM_PORT=""

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
