# Anarlog Cloud plugin

Connect Claude, Cursor, ChatGPT, Codex, and Copilot to Anarlog's hosted, read-only MCP server with OAuth. This is a separate MCP-only plugin; it does not run the Anarlog CLI or bundle the skills from the local `anarlog` plugin.

The hosted server exposes four tools:

- `list_meetings`
- `get_meeting`
- `get_meeting_transcript`
- `get_recurring_meeting_history`

## Prerequisites

1. Sign in to an Anarlog Pro account in the desktop app.
2. Open **Settings → Developers → Cloud API & Connectors**, review the disclosure, and enable it.
3. Wait for your meeting snapshots to upload.

On first use, the host discovers Anarlog's authorization server from `https://api.anarlog.so/mcp`, then opens the Anarlog sign-in and consent flow. No cloud API key or local CLI is required.

## Install from this repository

### Claude Code

```bash
claude plugin marketplace add fastrepl/anarlog
claude plugin install anarlog-cloud@fastrepl
```

Or add the remote server directly. Claude discovers OAuth from the MCP endpoint:

```bash
claude mcp add --transport http anarlog-cloud https://api.anarlog.so/mcp
```

### Cursor

Import `https://github.com/fastrepl/anarlog` as a team marketplace and install **Anarlog Cloud**, or add this MCP server:

```json
{
  "mcpServers": {
    "anarlog-cloud": {
      "url": "https://api.anarlog.so/mcp"
    }
  }
}
```

### GitHub Copilot CLI

```bash
copilot plugin marketplace add fastrepl/anarlog
copilot plugin install anarlog-cloud@fastrepl
```

### ChatGPT and Codex

```bash
codex plugin marketplace add fastrepl/anarlog \
  --sparse .agents/plugins \
  --sparse agent-plugins/anarlog \
  --sparse agent-plugins/anarlog-cloud
```

Restart the ChatGPT desktop app, open the Plugins Directory, select the Fastrepl source, and install **Anarlog Cloud**. Install **Anarlog** instead when you want the local CLI and bundled skill.

## Source install versus publication

This package makes the hosted connector installable from the Fastrepl source. Public directory listing is a separate review step for each host. Creating this package does not submit or publish it.

## Hosted OAuth activation for maintainers

The repository contains the protected-resource metadata, token validation, OAuth consent route, resource audience binding, and plugin configuration. Production already has an asymmetric JWT (ES256 JWKS), the OAuth 2.1 server, PKCE S256, refresh tokens, and dynamic client registration at `https://ijoptyyjrfqwaqhyxkxj.supabase.co/auth/v1`. Confirm these dashboard settings still hold, then deploy:

1. Site URL `https://anarlog.so` and authorization path `/oauth/consent`.
2. Deploy the API, web app, and database migrations together, then verify discovery, PKCE, refresh, and disconnect end to end.

Poke does not complete MCP OAuth. Keep it on per-user `anl_` keys; never put a shared key in a public recipe.
