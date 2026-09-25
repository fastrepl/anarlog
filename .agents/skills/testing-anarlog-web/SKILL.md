---
name: testing-anarlog-web
description: Run the @anlg/web app locally and reach auth-gated pages (integration connect, account) without credentials — via the real desktop-handoff URL or a temporary data stub + temp route.
---

# Testing the anarlog web app (@anlg/web)

## Dev server

`pnpm -F @anlg/web dev` (or `pnpm dev:web` via process-compose) boots on http://localhost:3000 **without any `.env`**: the dev script uses `dotenvx --ignore MISSING_ENV_FILE` and `src/env.ts` marks every var optional in dev (`requiredInProd`). PostHog is disabled without `VITE_POSTHOG_API_KEY` (no crash). Anything that calls `getSupabaseBrowserClient()`/`getAccessToken()` throws without `VITE_SUPABASE_URL`/`VITE_SUPABASE_ANON_KEY` — only code paths that never touch Supabase render.

Infisical env export (`apps/web/AGENTS.md`) requires the `INFISICAL_UNIVERSAL_AUTH_*` identity to be a member of project `87dad7b5-72a6-4791-9228-b3b86b169db1`; the `devin-proxy` identity may return 403 — do not assume `.env` is obtainable.

## Auth gating

`_view/app/route.tsx` `beforeLoad` redirects unauthenticated users to `/auth/` — **except** desktop integration handoffs (`isDesktopIntegrationHandoff`). There is no known test account; plan around it.

## Reaching the integration connect page unauthenticated

This real production path renders the full `ConnectFlow` UI with an **enabled** connect button — no patches needed:

```
/app/integration/?flow=desktop&scheme=anarlog&handoff=nango&action=connect&integration_id=<id>#session_token=<any-string>
```

- `scheme` is required for `flow=desktop` (zod enum: `anarlog`, `char`, etc.).
- `#session_token` is captured by `useNangoSessionHandoffToken` (hash → sessionStorage via `prepareNangoSessionHandoff` in `start.ts`); any non-empty string ≤4096 chars works — it is only sent to Nango when the user clicks connect.
- `integration_id` values: `google-calendar`, `outlook`, `linear`, `github`, `slack`, `notion`, `zoom`, `fathom`, `webex`, `google-meet`, `microsoft-teams`.
- `flow=web` renders the identical layout (`search.flow` only affects analytics + post-success navigation); `flow=web` itself needs auth + paid billing.
- Iconify logos (`logos:*`) load from `api.iconify.design` at runtime — verify CDN reachability if icons render blank; `OutlookIcon` is a local component and always renders.

## Reaching other auth-gated sections

For components whose data comes from Supabase/API (e.g. `IntegrationsSection` in `-account-integrations.tsx`), stub at the component level:

1. Temporarily replace `useQuery`/`useAccountSession` results with plain `{ isPending: false, isError: false, data: [...] }` objects — render order matters (isPending → isError → empty → list), so both must be non-error to reach the list branch.
2. Render the component from a temporary root-level route file (e.g. `src/routes/test-integrations.tsx` → `/test-integrations`) — files outside `_view/app/` are not behind `beforeLoad`. `routeTree.gen.ts` regenerates automatically in dev.
3. Revert both edits afterward (`git checkout -- <file>`, delete the temp route and the `routeTree.gen.ts` delta).

Devin Secrets Needed: none required for the above paths. For real sign-in flows you would need `VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY`, and a provisioned user.
