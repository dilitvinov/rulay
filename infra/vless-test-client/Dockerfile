FROM ghcr.io/sagernet/sing-box:latest AS singbox

FROM alpine:3.20

RUN apk add --no-cache curl

COPY --from=singbox /usr/local/bin/sing-box /usr/local/bin/sing-box
COPY config.json /etc/sing-box/config.json
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
