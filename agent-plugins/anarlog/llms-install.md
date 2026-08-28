# Install the Anarlog MCP server

1. Confirm the user has Anarlog Pro and has enabled **Settings → Developers → Cloud API & Connectors**.
2. Configure an HTTP MCP server named `anarlog` at `https://api.anarlog.so/mcp`. The host should discover OAuth from that endpoint. Do not paste a cloud API key unless the host cannot complete MCP OAuth.
3. Start the server and confirm it lists `list_meetings`, `get_meeting`, `get_meeting_transcript`, and `get_recurring_meeting_history`.
4. If Cloud returns no meetings the user expects, check whether `anarlog` is on `PATH` with `anarlog --version` and fill the gap with `anarlog --json`. If the CLI is missing, ask the user to install it from **Anarlog → Settings → Developers** or follow <https://docs.anarlog.so/installation>. Do not install software or search the filesystem without permission.
5. Never query or modify Anarlog's SQLite database directly.

The hosted server is read-only. Staging a note or summary edit requires the local CLI.
