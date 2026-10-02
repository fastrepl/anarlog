# @anlg/web

[![Open in StackBlitz](https://developer.stackblitz.com/img/open_in_stackblitz.svg)](https://stackblitz.com/fork/github/fastrepl/anarlog/tree/main)

The TanStack Start web app deploys to Vercel using Nitro. The Rust API and billing
service keep their separate Fly deployments. Set the Vercel project root to
`apps/web`; GitHub Actions owns production deployments and release tags.
Successful web CI runs on `main` deploy automatically. Pull requests run checks
without deploying production. Manual dispatch and desktop-release APT publishing
use the same workflow; version calculation, deployment, and tagging are serialized.

`vercel-build-config.ts` owns CDN redirects, documentation proxies, image widths,
and the three five-minute cron schedules. The build preserves prerendered pages,
Pagefind, the sitemap, Sharp, and the fonts used by social preview images.

## Production configuration

Configure `VERCEL_TOKEN` as a GitHub repository secret, and
`ANARLOG_VERCEL_ORG_ID` / `ANARLOG_VERCEL_PROJECT_ID` as repository variables.
Keep the existing Infisical Sentry credentials. The deployment workflow pulls
the Vercel project's production environment before building and uploads source
maps from the final deployment artifact.

Copy the web environment to the new Vercel project using the variable names in
`src/env.ts`, including the existing Supabase, Stripe, Loops, analytics, and
workspace-sharing settings. Keep `VITE_APP_URL=https://anarlog.so` and
`VITE_API_URL=https://api.anarlog.so`. Configure `SENTRY_DSN` for server errors.
`APP_VERSION` is added to the server artifact from the release build version.
Use separate non-production service credentials for previews.

Set a random `CRON_SECRET` of at least 16 characters. Leave
`CRON_JOBS_ENABLED` unset until the old scheduler is disabled: authorized cron
requests return a skipped result while the switch is off. When enabled, the
existing job leases and delivery idempotency remain in place.

## Cutover

1. Provision the Vercel project and environment. Build a preview and verify
   sign-in, checkout, shared notes, OAuth callbacks, images, social previews,
   search, documentation discovery, APT downloads, and redirects. The five-minute
   schedules require a Vercel plan that supports that frequency.
2. Deploy production with jobs disabled. Move the Anarlog web domains to Vercel
   after verifying the deployment. Preserve DNS for the separate API, billing,
   documentation, and model services. Historical domain redirect rules are
   preserved in code; attach only domains intentionally owned by this deployment.
3. Deploy the Cloudflare workspace-share router after the apex domain serves
   Vercel. It forwards to `https://anarlog.so` using the existing shared proxy
   secret, so there is no dependency on a provider-assigned project hostname.
4. Disable the old Netlify scheduled jobs, then set `CRON_JOBS_ENABLED=true` in
   Vercel and redeploy. Verify each job before retiring the old web deployment.
   After retirement, use a previous Vercel deployment for web rollback.

The old Netlify `anarlog` project was deleted on 2026-09-08 after its final
`hyprnote.com` redirect dependency moved to Vercel. The apex uses the existing
host-specific routes; `www.hyprnote.com` redirects to the apex. Both use the
Vercel CNAME target in Cloudflare with proxying disabled. The desktop download
and update aliases remain on Vercel for installed-client compatibility.
Netlify is no longer a deployment, scheduler, or rollback target for Anarlog.

Local development uses `pnpm dev:web` from the repository root. Local image requests
redirect to their original assets; Vercel performs width-based image
optimization in deployment, while CSS controls cropping. A local production
build uses `.output`; `NITRO_PRESET=vercel pnpm -F @anlg/web build` produces
`.vercel/output`. Production builds require the web build environment.

References: [TanStack Start on Vercel](https://vercel.com/kb/guide/deploy-a-tanstack-start-app-to-vercel),
[build output and image configuration](https://vercel.com/docs/build-output-api/configuration),
[cron authentication](https://vercel.com/docs/cron-jobs/manage-cron-jobs).
