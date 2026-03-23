# https://pnpm.io/docker#example-3-build-on-cicd
FROM node:22-slim AS node-base
WORKDIR /repo

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
ENV CI=true
RUN corepack enable

FROM node-base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/stripe/package.json apps/stripe/package.json
COPY packages/supabase/package.json packages/supabase/package.json
RUN pnpm fetch --prod --filter @hypr/stripe

FROM deps AS build
COPY apps/stripe apps/stripe
COPY packages/supabase packages/supabase
RUN pnpm install --filter @hypr/stripe --frozen-lockfile
RUN pnpm deploy --filter @hypr/stripe --prod --legacy /runtime

FROM oven/bun:1 AS runtime
WORKDIR /app

COPY --from=build /runtime ./

EXPOSE 8788
CMD ["bun", "src/index.ts"]
