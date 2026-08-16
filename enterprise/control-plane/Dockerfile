ARG RUST_IMAGE=rust:1.94.0-bookworm@sha256:365468470075493dc4583f47387001854321c5a8583ea9604b297e67f01c5a4f
ARG RUNTIME_IMAGE=debian:bookworm-slim@sha256:abd67ffcfa541b485a3dff59865ab629aa048a6c613e639d36e7456b0b229241

FROM ${RUST_IMAGE} AS builder

WORKDIR /source
COPY . .
RUN cargo build \
    --locked \
    --manifest-path enterprise/Cargo.toml \
    --package anarlog-enterprise-control-plane \
    --release

FROM ${RUNTIME_IMAGE}

RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 anarlog \
    && useradd \
        --uid 10001 \
        --gid 10001 \
        --no-create-home \
        --home-dir /nonexistent \
        --shell /usr/sbin/nologin \
        anarlog

COPY --from=builder /source/enterprise/target/release/anarlog-enterprise-control-plane /usr/local/bin/anarlog-enterprise-control-plane

ENV ANARLOG_ENTERPRISE_BIND_ADDRESS=0.0.0.0:8080
ENV RUST_LOG=info,tower_http=info

USER 10001:10001
EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=6 \
    CMD ["curl", "--fail", "--silent", "--show-error", "http://127.0.0.1:8080/health/ready"]

ENTRYPOINT ["/usr/local/bin/anarlog-enterprise-control-plane"]
