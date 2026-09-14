# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.94.0

FROM rust:${RUST_VERSION}-bookworm AS planner
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN sed -i '/^members = \[/,/^\]/c\members = ["apps/api", "crates/*"]' Cargo.toml
COPY crates crates
COPY apps/api/Cargo.toml apps/api/Cargo.toml
RUN mkdir -p apps/api/src && echo "fn main() {}" > apps/api/src/main.rs
RUN cargo chef prepare --recipe-path recipe.json

FROM rust:${RUST_VERSION}-bookworm AS build
ARG APP_VERSION
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev libclang-dev ca-certificates && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
ENV APP_VERSION=${APP_VERSION}
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json -p api
COPY Cargo.toml Cargo.lock ./
RUN sed -i '/^members = \[/,/^\]/c\members = ["apps/api", "crates/*"]' Cargo.toml
COPY crates crates
COPY packages/pricing/src/trial-policy.json packages/pricing/src/trial-policy.json
COPY apps/api apps/api
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo clean -p progenitor-utils
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --locked --release -p api

FROM debian:bookworm-slim AS api-runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
RUN groupadd -g 1001 appgroup && \
    useradd -u 1001 -g appgroup -d /nonexistent -s /usr/sbin/nologin -M appuser
COPY --from=build --chown=appuser:appgroup /app/target/release/api /usr/local/bin/api
USER appuser
WORKDIR /app
EXPOSE 3001
ENTRYPOINT ["/usr/local/bin/api"]

FROM node:22-slim AS billing-node-base
WORKDIR /repo

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
ENV CI=true
RUN corepack enable

FROM billing-node-base AS billing-deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY patches patches
COPY apps/stripe/package.json apps/stripe/package.json
COPY packages/supabase/package.json packages/supabase/package.json
COPY packages/user-error/package.json packages/user-error/package.json
RUN pnpm fetch --prod --filter @anlg/stripe

FROM billing-deps AS billing-build
COPY apps/stripe apps/stripe
COPY packages/supabase packages/supabase
COPY packages/user-error packages/user-error
RUN pnpm install --filter @anlg/stripe --frozen-lockfile
# The filtered Stripe runtime excludes mobile-only patched dependencies.
RUN pnpm --config.allowUnusedPatches=true deploy --filter @anlg/stripe --prod --legacy /runtime

FROM oven/bun:1 AS bun-runtime

FROM api-runtime AS billing-runtime
ARG APP_VERSION
ENV APP_VERSION=${APP_VERSION}
COPY --from=bun-runtime /usr/local/bin/bun /usr/local/bin/bun
COPY --from=billing-build --chown=appuser:appgroup /runtime /app/billing
WORKDIR /app/billing
ENTRYPOINT ["bun", "src/index.ts"]

FROM api-runtime AS runtime
