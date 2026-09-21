# Contributing

Issues, pull requests, bug reports, and documentation fixes are welcome.

## Before you start

- Search existing issues and pull requests before starting a large change.
- Keep changes focused. Add tests for behavior that can regress.
- Never commit credentials, customer configuration, meeting content, or other private data.
- Use [docs.anarlog.so](https://docs.anarlog.so) for product, CLI, and MCP behavior. Use this file and the repository's `AGENTS.md` files for development guidance.

## Set up the repository

You need:

- Node.js 22 or later
- pnpm 11.1.1
- Rust 1.94.0
- [process-compose](https://f1bonacc1.github.io/process-compose/installation/) 1.122.0 or later (`brew install process-compose` on macOS)
- The [Tauri v2 system dependencies](https://v2.tauri.app/start/prerequisites/)

On Debian or Ubuntu, install the supported toolchains and system packages with:

```bash
bash scripts/setup-linux.sh
```

Then install the workspace:

```bash
pnpm install --frozen-lockfile
```

The desktop app and website start without secrets for local-first workflows.

## Run the apps

```bash
# Tauri desktop app
pnpm dev:desktop

# Website
pnpm dev:web
```

Process Compose manages the processes and logs. It runs the shared UI build through Turbo before starting either app. The existing `pnpm exec turbo dev:desktop` and `pnpm exec turbo dev:web` commands also work.

On macOS, install and select the full Xcode application, including its Metal
toolchain (`xcodebuild -downloadComponent MetalToolchain`). Command Line Tools
alone cannot build the on-device transcription dependencies. If the checkout
path contains spaces, set `CARGO_TARGET_DIR` to a path without spaces before
starting the desktop app; some native dependencies use Autotools, which rejects
build directories containing spaces.

For the combined desktop, web, API, and local Supabase stack:

```bash
pnpm dev

# Only local Supabase and the API
pnpm dev:api
```

These commands require Docker and [Task](https://taskfile.dev/installation/) (`brew install go-task` on macOS), plus the API configuration described in [apps/api/AGENTS.md](apps/api/AGENTS.md). The Linux setup installs Task and process-compose. The combined stack runs `task supabase-start` to prepare local Supabase and `.env.supabase`, waits for API readiness, then starts web and desktop. Package `.env` files are still loaded by the apps; Process Compose does not inject the root `.env` into every process.

Provider credentials and service-specific configuration are not required for `pnpm dev:desktop` or `pnpm dev:web`. The underlying `task supabase-start` and `cargo run -p api` commands remain available. Supabase's Docker containers persist when Process Compose exits; stop them explicitly with `task supabase-stop`.

Use the Process Compose TUI to view logs or restart individual processes. Logs are also written under `.process-compose/<target>/`, with rotation; set `PC_LOG_DIR` to override that directory. The launcher disables the TUI automatically without a terminal, or you can set `PC_DISABLE_TUI=1`.

The control ports are 18080 for `pnpm dev`, 18081 for desktop, 18082 for web, and 18083 for API. Set `PC_PORT_NUM` to override a control port; app ports remain 1422 (desktop Vite), 3000 (web), and 3001 (API). Stop other apps using those app ports before starting the same services here.

```bash
# Validate configuration without starting apps
pnpm dev --dry-run

# Start web without the TUI and restart it from another terminal
pnpm dev:web -t=false
process-compose -p 18082 process restart web
process-compose -p 18082 down
```

### Develop the Google Drive automation

Run the desktop, web (`:3000`), and API (`:3001`) development commands above in
separate terminals. Configure the existing Nango `google-drive` integration with
`https://www.googleapis.com/auth/drive.file` and the team's development OAuth
client. Enable the Google Drive and Google Picker APIs in that client's Google
Cloud project. Existing connections with broader scopes must be reauthorized.

To test hosted transcription and summaries while keeping Drive requests local,
set `VITE_API_URL=http://localhost:3001` and
`VITE_AI_API_URL=https://api.anarlog.so` in `apps/desktop/.env`, then restart the
desktop dev process. This requires an account and Pro entitlement accepted by
the hosted service. The local API can remain `ANARLOG_SERVICE=core`. Omitting
`VITE_AI_API_URL` keeps AI requests on `VITE_API_URL`. Explicit provider base URL
settings still take precedence. Audio and summary requests use the hosted service
and its normal usage limits.

Set `GOOGLE_DRIVE_PICKER_REDIRECT_URI` in `apps/api/.env` to
`http://localhost:3000/app/google-drive-picker` for local development. Register
that exact URI on the **same Google OAuth client configured in Nango**, in its
authorized redirect URIs. Keep Nango's existing callback URI as well. Production
must use the deployed web origin with HTTPS and this same route path.
For production, add `GOOGLE_DRIVE_PICKER_REDIRECT_URI=https://anarlog.so/app/google-drive-picker`
to the production `/anarlog/ai` Infisical runtime view. The Core deployment passes
this setting through to the API. Register that URI on the production Nango
integration's Google OAuth client, retaining Nango's callback. Deploy Core, then Gateway, then Web before distributing a desktop build that
includes this action. Gateway also compiles the route table and must know the
new `/nango/google-drive/*` routes; updating Core alone is insufficient. Desktop
production builds can leave `VITE_AI_API_URL` unset to use the existing API URL.

Folder selection opens Google's OAuth page directly using `trigger_onepick` and
`drive.file`; it does not embed the JavaScript Picker. No Picker API key or
`VITE_GOOGLE_DRIVE_PICKER_*` variables are needed. The API obtains the OAuth
client credentials from Nango, validates signed user-bound state and PKCE,
checks that the selected Google account matches the existing connection, and
imports the new credentials into that same Nango connection. The Nango API key
must permit reading integration credentials and importing connections.

The web page uses same-tab session storage only for return routing, clears the
OAuth code from the URL before telemetry, and hands it to the authenticated
desktop for API completion. Access and refresh tokens stay on the server.
Restart selection from the desktop if the callback tab is refreshed or closed.
Nango's new-connection webhook still needs a reachable local tunnel; the direct
folder-selection callback goes to the web app and does not use that tunnel.

Validate with a test My Drive folder and a test shared-drive folder. Verify
folder selection, a first upload, an update after regenerating a summary, and a
failed upload retry. Deploy the API and web support before distributing a desktop
build that exposes the action.

## Find the right code

| Path             | Scope                                                   |
| ---------------- | ------------------------------------------------------- |
| `apps/desktop`   | React desktop UI and Tauri application                  |
| `apps/web`       | Marketing site, account portal, and shared-note pages   |
| `apps/api`       | Hosted API routes                                       |
| `apps/cli`       | CLI and MCP server                                      |
| `apps/mobile`    | Expo mobile client                                      |
| `plugins/*`      | Tauri plugin boundaries                                 |
| `crates/*`       | Rust libraries and services                             |
| `packages/*`     | Shared TypeScript packages                              |
| `crates/db-app`  | SQLite schema and migrations                            |
| `supabase`       | Hosted database schema, functions, and tests            |
| `skills/anarlog` | Published CLI and MCP agent skill                       |
| `enterprise`     | Commercially licensed capture and deployment components |
| `docs`           | Mintlify product and reference documentation            |

Sessions are the core data entity. Notes, transcripts, and summaries are all backed by sessions. ProseMirror documents use the TipTap JSON dialect.

## Validate your change

Always format before committing:

```bash
pnpm exec dprint fmt
pnpm fmt:check
```

On Linux, the full format check cannot run the macOS-only Swift formatter. Run a scoped dprint check for every changed non-Swift path and report the skipped Swift check.

Run checks for every package you changed. Common commands include:

```bash
# Desktop TypeScript
pnpm -F desktop typecheck
pnpm -F desktop test
pnpm exec oxlint --quiet --format=github apps/desktop/src/

# Changes spanning TypeScript packages
pnpm -r typecheck

# Rust
cargo check
cargo test -p <affected-package>
```

For documentation changes:

```bash
pnpm exec dprint fmt 'docs/**/*'
pnpm exec dprint check 'docs/**/*'
cd docs
mint validate
mint broken-links --check-anchors --check-redirects
```

Check the affected workflow under `.github/workflows/` for stricter package-specific commands.

## Open a pull request

- Write the title as a specific action that states the intended outcome. Do not use a file name, ticket number, or a generic label as the title.
- Write one or two sentences on the labeled `Intent` line: what problem you saw, and what this change should do instead. Cubic already summarizes the code diff, so do not paste a generated commit log, a file-by-file recap, or a long write-up: keep it short.
- Attach a short screen recording or GIF in the `Demo` section showing the problem and the fix in action. This, plus the one-line intent, is what lets a maintainer understand a contribution without reading a wall of text. Docs-only or non-functional changes can skip the video by writing `N/A` and a one-line reason.
- List the commands and manual checks you used to verify the change.
- CI enforces the title, the labeled `Intent` line, and the `Demo` section for external contributors. Org members, collaborators, owners, and bots are not gated.
- External contributors must sign the [Fastrepl Contributor License Agreement](https://gist.github.com/ComputelessComputer/9d8243ec8e2ce92541c5b67462f092a0) through CLA Assistant when prompted. Org members, collaborators, and owners skip the `license/cla` status check.

## Licensing and contribution boundary

By submitting a contribution outside `enterprise/`, you agree that it may be distributed under the repository's [MIT License](LICENSE). Only submit material you have the right to license this way.

Do not submit changes under `enterprise/` unless Fastrepl has confirmed the applicable contribution terms in writing. Never include customer configuration, credentials, confidential material, or untracked third-party code. Record the immutable upstream revision and license before reusing third-party material. See [Licensing and product boundary](LICENSING.md) for the component-placement and provenance rules.
