# Meeting notes Worker

This Worker accepts transcript text only. The mobile app must show its first-send disclosure and require an explicit Generate summary action before calling this endpoint. It never accepts audio, stores request content, or logs prompts/results.

## Configure and deploy

1. For local development, put `SUPABASE_URL` and `SUPABASE_ANON_KEY` in `.dev.vars`.
2. Run `pnpm install` from the repository root.
3. From this directory, run `pnpm types` after changing bindings or Wrangler configuration.
4. Before deployment, set both runtime values as Worker secrets with `wrangler secret put SUPABASE_URL` and `wrangler secret put SUPABASE_ANON_KEY`. The URL and key are intentionally absent from the checked-in config.
5. Run `pnpm typecheck`, then deploy with `pnpm deploy` after checking the Cloudflare account and Worker name.

## Deployment readiness check

On 2026-11-03, `wrangler deploy --dry-run` completed with the pinned Wrangler
4.156.2 and resolved the AI and rate-limit bindings. This validates the bundle
and local configuration only. The currently selected Cloudflare account does
not contain `anarlog-meeting-notes` (`wrangler secret list` returned Worker
not found), so no production Worker or secrets were created. Before a real
deployment, verify the target account and provision `SUPABASE_URL` and
`SUPABASE_ANON_KEY` as Worker secrets.

The model selector includes the plan's two Qwen candidates plus an active Llama 3.3 JSON Mode control. Cloudflare's listed context windows and prices were checked on 2026-11-03:

| Model | Purpose | Context | Input / output per million tokens |
| --- | --- | ---: | ---: |
| [`@cf/qwen/qwen3-30b-a3b-fp8`](https://developers.cloudflare.com/workers-ai/models/qwen3-30b-a3b-fp8/) | Lower-cost candidate | 32,768 | $0.0509 / $0.335 |
| [`@cf/qwen/qwen3.8-27b`](https://developers.cloudflare.com/workers-ai/models/qwen3.8-27b/) | Long-context candidate | 262,144 | $0.45 / $3.20 |
| [`@cf/meta/llama-3.3-70b-instruct-fp8-fast`](https://developers.cloudflare.com/workers-ai/models/llama-3.3-70b-instruct-fp8-fast/) | JSON Mode control | 24,000 | $0.293 / $2.253 |

Cloudflare lists JSON Mode for Llama 3.3, not the Qwen candidates ([supported list](https://developers.cloudflare.com/workers-ai/features/json-mode/)); the formerly configured Llama 3.1 8B model was deprecated in May 2026. For Qwen, the Worker asks for schema-only JSON, disables reasoning tokens, parses the response, validates it with Zod, and checks every evidence ID against the exact transcript revision. This keeps the planned Qwen comparison available without relying on undocumented JSON Mode support. The Qwen options remain evaluation candidates; they have not been shown to match the control on Vietnamese meeting data.

The Worker first validates the bearer token through Supabase Auth and then applies Anarlog's Pro/trial claim rules to that same validated token. Its per-user rate limit is approximate and per Cloudflare location, so it is an abuse guard rather than a strict global quota.
