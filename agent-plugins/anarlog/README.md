# Anarlog agent plugin

Query Anarlog meetings through hosted Cloud MCP and a bundled skill. The plugin finds meetings and reads notes, summaries, participants, action items, bounded transcript excerpts, and recurring history. When Cloud has no snapshot for a meeting, the skill fills the gap from the local `anarlog` CLI.

## Prerequisites

1. Sign in to an Anarlog Pro account in the desktop app.
2. Open **Settings → Developers → Cloud API & Connectors**, review the disclosure, and enable it.
3. Wait for your meeting snapshots to upload.

On first use, the host discovers Anarlog's authorization server from `https://api.anarlog.so/mcp`, then opens the Anarlog sign-in and consent flow. No cloud API key is required.

If a meeting is missing from Cloud, install the [Anarlog CLI](https://docs.anarlog.so/installation) so the skill can read the local database.

If you previously installed **Anarlog Cloud**, replace it with this plugin.

## Install from this repository

### Claude Code

```bash
claude plugin marketplace add fastrepl/anarlog
claude plugin install anarlog@fastrepl
```

Or add the remote server directly. Claude discovers OAuth from the MCP endpoint:

```bash
claude mcp add --transport http anarlog https://api.anarlog.so/mcp
```

### GitHub Copilot CLI

```bash
copilot plugin marketplace add fastrepl/anarlog
copilot plugin install anarlog@fastrepl
```

### ChatGPT and Codex

```bash
codex plugin marketplace add fastrepl/anarlog \
  --sparse .agents/plugins \
  --sparse agent-plugins/anarlog
```

Restart the ChatGPT desktop app, open the Plugins Directory, select the Fastrepl source, and install **Anarlog**.

### Cursor

Import `https://github.com/fastrepl/anarlog` as a team marketplace, then install **Anarlog**. You can also load `agent-plugins/anarlog` as a local plugin while testing.

## Configure MCP directly

Clients that do not install plugins can add the hosted server:

```json
{
  "mcpServers": {
    "anarlog": {
      "url": "https://api.anarlog.so/mcp"
    }
  }
}
```

For a fully local agent that should not use Cloud, start `anarlog mcp` yourself. See [MCP for agents](https://docs.anarlog.so/agents/mcp). Static remote clients that cannot complete MCP OAuth can follow the [remote MCP setup](https://docs.anarlog.so/reference/api-cloud#remote-mcp) with a cloud API key.

## Data access

Cloud MCP is read-only hosted snapshots. Local CLI and optional local MCP stay on the computer and read the app database through Anarlog's compatibility layer. Cloud access uploads a separate server-readable copy only after the user opts in. See [Data, privacy, and retention](https://docs.anarlog.so/data-and-privacy).
