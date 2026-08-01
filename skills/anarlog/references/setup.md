# Setup

## MCP

Run the local stdio server with:

```bash
anarlog mcp
```

A generic client configuration is:

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

Restart the client after changing its MCP configuration.

## CLI

On macOS, open **Anarlog → Settings → Developers** and select **Install**. Anarlog installs the bundled command at `~/.local/bin/anarlog`.

To build from source instead:

```bash
git clone https://github.com/fastrepl/anarlog.git
cd anarlog
cargo install --locked --path apps/cli
anarlog --version
```

Run the Anarlog desktop app once so its local database exists. The CLI and local MCP server work while the app is closed after that.

Homebrew, standalone release binaries, Windows package-manager distribution, and bundled CLI installation on platforms other than macOS are not yet available.

Use `--db-path FILE` or `ANARLOG_DB_PATH` only when the database is outside Anarlog's default application-data location.
