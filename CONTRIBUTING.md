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
pnpm exec turbo dev:desktop

# Website
pnpm exec turbo dev:web
```

Turbo builds shared UI packages before starting either app.

CloudSync, hosted AI, authentication, billing, and connected integrations require the optional local services:

```bash
task supabase-start
cargo run -p api
```

The Supabase stack requires Docker. Provider credentials and service-specific configuration are not required for local notes, recording, or on-device features.

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
- Write the description yourself as a concise executive summary: on the labeled `Problem` and `Fix` lines, explain the problem, why it mattered, and how the change fixes it. Do not paste a generated commit log or a file-by-file recap.
- List the commands and manual checks you used to verify the change.
- CI enforces the title and labeled `Problem` / `Fix` summary for external contributors. Org members, collaborators, owners, and bots are not gated.
- External contributors must sign the [Fastrepl Contributor License Agreement](https://gist.github.com/ComputelessComputer/9d8243ec8e2ce92541c5b67462f092a0) through CLA Assistant when prompted. Org members, collaborators, and owners skip the `license/cla` status check.

## Licensing and contribution boundary

By submitting a contribution outside `enterprise/`, you agree that it may be distributed under the repository's [MIT License](LICENSE). Only submit material you have the right to license this way.

Do not submit changes under `enterprise/` unless Fastrepl has confirmed the applicable contribution terms in writing. Never include customer configuration, credentials, confidential material, or untracked third-party code. Record the immutable upstream revision and license before reusing third-party material. See [Licensing and product boundary](LICENSING.md) for the component-placement and provenance rules.
