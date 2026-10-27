# syntax=docker/dockerfile:1

# ---- Build -------------------------------------------------------------
FROM rust:1-bookworm AS build
WORKDIR /src
# OpenSSL wird aus den Quellen gebaut (Feature "vendored"); perl und make bringt das Image mit.
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY apps/api apps/api
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p tickly-api \
 && cp target/release/tickly-api /tickly-api \
 && mkdir /backups && chown 65532:65532 /backups

# ---- Laufzeit ----------------------------------------------------------
# distroless/cc: glibc, keine Shell, läuft als unprivilegierter Benutzer
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=build /tickly-api /app/tickly-api
COPY --from=build --chown=65532:65532 /backups /backups
COPY web /app/web
ENV BIND=0.0.0.0:3000 PORT=3000 WEB_DIR=/app/web
EXPOSE 3000
USER nonroot
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s CMD ["/app/tickly-api", "healthcheck"]
ENTRYPOINT ["/app/tickly-api"]
