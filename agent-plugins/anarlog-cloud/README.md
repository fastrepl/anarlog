# Anarlog Cloud plugin

Connect ChatGPT and Codex to Anarlog's hosted, read-only MCP server with OAuth. This is a separate MCP-only plugin; it does not run the Anarlog CLI or bundle the skills from the local `anarlog` plugin.

The hosted server exposes four tools:

- `list_meetings`
- `get_meeting`
- `get_meeting_transcript`
- `get_recurring_meeting_history`

## Prerequisites

1. Sign in to an Anarlog Pro account in the desktop app.
2. Open **Settings → Developers → Cloud API & Connectors**, review the disclosure, and enable it.
3. Wait for your meeting snapshots to upload.

On first use, the host opens Anarlog's OAuth sign-in and consent flow. No cloud API key or local CLI is required.

## Install from this repository

Add the Fastrepl source with both Anarlog packages:

```bash
codex plugin marketplace add fastrepl/anarlog \
  --sparse .agents/plugins \
  --sparse agent-plugins/anarlog \
  --sparse agent-plugins/anarlog-cloud
```

Restart the ChatGPT desktop app, open the Plugins Directory, select the Fastrepl source, and install **Anarlog Cloud**. Install **Anarlog** instead when you want the local CLI and bundled skill.

## Source install versus publication

This package makes the hosted connector locally installable. Public listing requires a separate MCP-only submission through the OpenAI Platform with the universal server URL `https://api.anarlog.so/mcp`. Creating this package does not submit or publish it.

## Hosted OAuth activation for maintainers

The repository contains the protected-resource metadata, token validation, OAuth consent route, resource audience binding, and plugin configuration. Activating the hosted flow still requires these Supabase project settings:

1. Use an asymmetric JWT signing key.
2. Enable the Supabase OAuth 2.1 server.
3. Keep the Supabase Site URL on `https://anarlog.so` and set the authorization path to `/oauth/consent`.
4. Enable dynamic client registration.
5. Deploy the API, web app, and database migration together, then verify discovery, PKCE, refresh, and disconnect end to end.

Supabase provides DCR and PKCE for this flow, so no predefined OpenAI client registration is required. These are deployment and external-control-plane changes; installing this package does not perform them. Follow the current [OpenAI MCP authentication requirements](https://developers.openai.com/plugins/build/auth), [OpenAI submission requirements](https://developers.openai.com/plugins/deploy/submission), and [Supabase OAuth server setup](https://supabase.com/docs/guides/auth/oauth-server/getting-started) before activation.
