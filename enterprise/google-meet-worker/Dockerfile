ARG RUST_IMAGE=rust:1.94.0-bookworm@sha256:365468470075493dc4583f47387001854321c5a8583ea9604b297e67f01c5a4f
ARG RUNTIME_IMAGE=debian:bookworm-slim@sha256:abd67ffcfa541b485a3dff59865ab629aa048a6c613e639d36e7456b0b229241

FROM ${RUST_IMAGE} AS builder

WORKDIR /source
COPY . .
RUN cargo build \
    --locked \
    --manifest-path enterprise/Cargo.toml \
    --package anarlog-enterprise-google-meet-worker \
    --bin anarlog-enterprise-google-meet-worker \
    --release

FROM ${RUNTIME_IMAGE}

RUN apt-get update \
    && apt-get install --no-install-recommends --yes \
        ca-certificates \
        chromium \
        fonts-liberation \
        xdotool \
        xvfb \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 anarlog \
    && useradd \
        --uid 10001 \
        --gid 10001 \
        --create-home \
        --home-dir /var/lib/anarlog \
        --shell /usr/sbin/nologin \
        anarlog \
    && mkdir -p /var/lib/anarlog/recordings \
    && chown -R anarlog:anarlog /var/lib/anarlog

COPY --from=builder /source/enterprise/target/release/anarlog-enterprise-google-meet-worker /usr/local/bin/anarlog-enterprise-google-meet-worker

ENV DISPLAY=:99
ENV ANARLOG_ENTERPRISE_CHROMIUM_BINARY=/usr/bin/chromium
ENV ANARLOG_ENTERPRISE_XDOTOOL_BINARY=/usr/bin/xdotool
ENV ANARLOG_ENTERPRISE_RECORDING_ROOT=/var/lib/anarlog/recordings
ENV ANARLOG_ENTERPRISE_DISABLE_SANDBOX=true
ENV RUST_LOG=info

USER 10001:10001
VOLUME ["/var/lib/anarlog/recordings"]

ENTRYPOINT ["/usr/local/bin/anarlog-enterprise-google-meet-worker"]
