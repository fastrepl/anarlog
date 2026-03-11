FROM node:22-slim AS node-base
WORKDIR /repo

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
ENV CI=true
RUN corepack enable

FROM node-base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/slack-internal/package.json apps/slack-internal/package.json
COPY packages/agent-core/package.json packages/agent-core/package.json
COPY packages/agent-support/package.json packages/agent-support/package.json
COPY packages/agent-designer/package.json packages/agent-designer/package.json
RUN pnpm fetch --filter @hypr/slack-internal...

FROM deps AS build
COPY apps/slack-internal apps/slack-internal
COPY packages/agent-core packages/agent-core
COPY packages/agent-support packages/agent-support
COPY packages/agent-designer packages/agent-designer
RUN pnpm install --filter @hypr/slack-internal... --frozen-lockfile
RUN pnpm deploy --filter @hypr/slack-internal --prod --legacy /runtime

FROM oven/bun:1 AS runtime
WORKDIR /app
ARG GIT_SHA
ENV NODE_ENV="production"
ENV GIT_SHA=${GIT_SHA}
COPY --from=build /runtime ./
CMD ["bun", "src/index.ts"]
