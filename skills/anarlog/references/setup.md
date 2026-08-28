# Setup

## Cloud MCP

The Anarlog plugin connects to the hosted server:

```text
https://api.anarlog.so/mcp
```

The account needs Anarlog Pro. In the desktop app, open **Settings → Developers → Cloud API & Connectors**, review the disclosure, and enable it. Wait for meeting snapshots to upload. The first tool use opens Anarlog sign-in and consent. No cloud API key or local CLI is required for Cloud reads.

## Local CLI for missing snapshots

Cloud only has opted-in snapshots. If a meeting is missing there, use the local CLI with `--json`.

Open **Anarlog → Settings → Developers** and select **Install**. Direct-download builds install:

- macOS and Linux (DEB / AppImage): `~/.local/bin/anarlog`
- Windows: `%LOCALAPPDATA%\Anarlog\bin\anarlog.exe`

The Mac App Store build does not bundle CLI installation. Build from source instead.

To build from source instead:

```bash
git clone https://github.com/fastrepl/anarlog.git
cd anarlog
cargo install --locked --path apps/cli
anarlog --version
```

Run the Anarlog desktop app once so its local database exists. The CLI works while the app is closed after that.

Cloud sign-in does not require a graphical session on the Anarlog machine. Run `anarlog auth login`, open the printed URL in any browser, and paste the copied callback URL into the CLI prompt. Confirm the session with `anarlog auth status`. With `--json`, the login URL is printed to stderr and the callback is read from stdin.

On Flatpak, the host command is `anarlog-cli`. On DEB, AppImage, macOS, Windows, and Settings-installed builds, the command is `anarlog`.

Homebrew, standalone release binaries, and Windows package-manager distribution are not yet available.

Use `--db-path FILE` or `ANARLOG_DB_PATH` only when the database is outside Anarlog's default application-data location.

## Optional local MCP

For a fully local agent that should not use Cloud, start the stdio server yourself:

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

Restart the client after changing its MCP configuration. The marketplace plugin does not start this server.
