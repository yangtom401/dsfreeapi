FROM alpine:3.21
RUN apk add --no-cache ca-certificates && update-ca-certificates

WORKDIR /app

ARG TARGETARCH
COPY target/docker/${TARGETARCH}/ds-free-api /app/ds-free-api
COPY config.example.toml /app/config/config.toml

ENV RUST_LOG=info \
    DS_DATA_DIR=/app/data \
    DS_CONFIG_PATH=/app/config/config.toml \
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
    SSL_CERT_DIR=/etc/ssl/certs

EXPOSE 22217

ENTRYPOINT ["/app/ds-free-api"]
