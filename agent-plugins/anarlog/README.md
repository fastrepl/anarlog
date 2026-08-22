# Anarlog agent plugin

Query Anarlog meetings through a read-only agent skill and local MCP server. The plugin can find meetings, read notes and summaries, inspect participants and action items, page through transcript excerpts, and review recurring meeting history.

## Prerequisites

1. Install [Anarlog](https://anarlog.so/download) and open it once.
2. Make the `anarlog` CLI available on `PATH`. In the desktop app, open **Settings → Developers** and install the CLI when that action is available. You can also [build the CLI from source](https://docs.anarlog.so/installation#build-from-source).
3. Confirm the local data is ready:

   ```bash
   anarlog --json doctor
   ```

The bundled MCP configuration starts `anarlog mcp`. If the client cannot find the command, use the executable's absolute path in that client's MCP settings.

## Install from this repository

### Claude Code

```bash
claude plugin marketplace add fastrepl/anarlog
claude plugin install anarlog@fastrepl
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

Restart the ChatGPT desktop app, open the Plugins Directory, select the Fastrepl source, and install Anarlog.

### Cursor

Import `https://github.com/fastrepl/anarlog` as a team marketplace, or load `agent-plugins/anarlog` as a local plugin while testing.

## Configure MCP directly

Clients that do not install plugins can start the local stdio server with:

```json
{
  "mcpServers": {
    "anarlog": {
      "command": "anarlog",
      "args": ["mcp"]
    }
  }
}
```

For a remote agent, enable **Cloud API & Connectors** in Anarlog and follow the [remote MCP setup](https://docs.anarlog.so/reference/api-cloud#remote-mcp). Hosted access requires Anarlog Pro, explicit opt-in, and a cloud API key.

## Data access

Every Anarlog tool is read-only. Local CLI and MCP requests stay on the computer and read the app database through Anarlog's compatibility layer. Cloud access uploads a separate server-readable copy only after the user opts in. See [Data, privacy, and retention](https://docs.anarlog.so/data-and-privacy).
