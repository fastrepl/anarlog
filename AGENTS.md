# Overview

Tauri desktop note-taking app (`apps/desktop/`) with a web app (`apps/web/`).
Uses pnpm workspaces.
SQLite is the primary data store (schema and migrations in `crates/db-app/`, desktop transport in `plugins/db/`), Zustand is used for UI state, and ProseMirror powers the editor (`packages/editor`, via `@handlewithcare/react-prosemirror`); documents are stored as TipTap-dialect ProseMirror JSON (converters/validation in `crates/tiptap`). Sessions are the core entity — all notes are backed by sessions.

## Commands

- Format: `pnpm exec dprint fmt`
- Typecheck (TS): `pnpm -r typecheck`
- Typecheck (Rust): `cargo check`
- Desktop dev: `turbo dev:desktop`
- Web dev: `turbo dev:web`
- Dev docs: https://docs.anarlog.so

## Pre-commit verification

- Before every commit, run the locally available checks from the CI workflows affected by the changed paths. Do not defer routine validation to CI.
- Always run `pnpm exec dprint fmt`, then `pnpm fmt:check`.
- For TypeScript changes, run the affected package's typecheck and tests. For desktop changes, run `pnpm -F desktop typecheck` and `pnpm -F desktop test`; use `pnpm -r typecheck` when changes span packages.
- For desktop TypeScript changes, run the CI lint command: `pnpm exec oxlint --quiet --format=github apps/desktop/src/`.
- When desktop translated copy, message extraction, or catalogs may change, run `pnpm -F desktop exec lingui extract --clean --workers 1` and `pnpm -F desktop exec lingui compile --strict --workers 1`, include all generated `apps/desktop/src/i18n/locales` changes, and rerun until generation is stable. When there is no intentional uncommitted catalog diff, run the exact CI check: `pnpm -F desktop i18n:check`.
- For Rust changes, run `cargo check` and the affected package tests. Match stricter workflow commands such as `cargo clippy --locked ... -D warnings` when the changed paths trigger them.
- Check the relevant `.github/workflows/*_ci.*`, `fmt.yaml`, and `lint.yaml` files for package-specific tests, generated-file checks, or build validation, and run the locally reproducible commands before committing.
- If a required check fails, fix it before committing. If a check cannot run locally because of platform, secrets, or unavailable infrastructure, report the exact skipped command and reason; do not claim full validation.

## Guidelines

- JavaScript/TypeScript formatting runs through `oxfmt` via dprint's exec plugin.
- Use `useForm` (tanstack-form) and `useQuery`/`useMutation` (tanstack-query) for form/mutation state. Avoid manual state management (e.g. `setError`).
- For `plugins/db` live queries, keep schema creation, migrations, and DB initialization on the Rust side; TypeScript should only consume `execute`/`subscribe` APIs.
- New SQLite migrations must be downgrade-safe (older builds tolerate newer schemas): additive only, new columns nullable or with a DEFAULT. If a migration can't be downgrade-safe, add a `-- breaking` line to the leading comment block of its `.sql` file so older builds refuse the database with an update prompt.
- Branch naming: `fix/`, `chore/`, `refactor/` prefixes.

## Code Style

- Avoid creating types/interfaces unless shared. Inline function props.
- Do not write comments unless code is non-obvious. Comments should explain "why", not "what".
- Use `cn` from `@anlg/utils` for conditional classNames. Always pass an array, split by logical grouping.
- Use `motion/react` instead of `framer-motion`.
- Prefer DOM order for local overlap and portals for cross-tree floating UI. Use `z-index` only inside a bounded stacking context with explicit sibling ordering; do not add arbitrary global or escalating values.

## CLI TUI Command Architecture

Choose the lightest command structure that fits the workflow.

Use the full reducer/effect/runtime split only when the command has async orchestration, a multi-step workflow, or substantial state transitions that benefit from reducer-style tests.

```
commands/<name>/
  mod.rs        -- Screen impl, Args, run()          [glue]
  app.rs        -- App or screen-local state          [optional]
  action.rs     -- Action enum                        [optional]
  effect.rs     -- Effect enum                        [optional]
  runtime.rs    -- Runtime, RuntimeEvent              [async I/O]
  ui.rs         -- draw(frame, app)                   [rendering]
```

Naming rules:

- Types drop the command prefix: `App`, `Action`, `Effect`, `Runtime`, `RuntimeEvent`
- `app.rs` → `app/mod.rs` with private submodules when state is complex
- `ui.rs` → `ui/mod.rs` with sub-files when rendering is complex
- `action.rs`/`effect.rs` are siblings of `mod.rs` when they exist; do not create them by default for simple list/detail screens
- `app.rs` contains no rendering logic, no API calls, no async code when using the reducer pattern
- Prefer `screen.rs` plus a small local state struct for simple browse/select flows
- Do not add parent-level action/effect translation layers that proxy child workflows through another command's reducer

## Misc

- Do not create summary docs or example code files unless requested.
