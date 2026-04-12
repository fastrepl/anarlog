# Overview

Tauri desktop note-taking app (`apps/desktop/`) with a web app (`apps/web/`).
Uses pnpm workspaces.
TinyBase as the primary data store (schema at `packages/store/src/tinybase.ts`), Zustand for UI state, TipTap for the editor. Sessions are the core entity — all notes are backed by sessions.

## Commands

- Format: `pnpm exec dprint fmt`
- Typecheck (TS): `pnpm -r typecheck`
- Typecheck (Rust): `cargo check`
- Desktop dev: `pnpm -F @hypr/desktop tauri:dev`
- Web dev: `pnpm -F @hypr/web dev`
- Dev docs: https://char.com/docs/developers

## Guidelines

- Format via dprint after making changes.
- JavaScript/TypeScript formatting runs through `oxfmt` via dprint's exec plugin.
- Run `pnpm -r typecheck` after TypeScript changes, `cargo check` after Rust changes.
- After editing files, run the relevant verification commands before finishing.
- For `apps/desktop/` TypeScript changes, prefer `pnpm -F desktop typecheck` to match CI.
- After edits, run `pnpm exec dprint fmt`.
- Use `useForm` (tanstack-form) and `useQuery`/`useMutation` (tanstack-query) for form/mutation state. Avoid manual state management (e.g. `setError`).
- Branch naming: `fix/`, `chore/`, `refactor/` prefixes.

## Code Style

- Avoid creating types/interfaces unless shared. Inline function props.
- Do not write comments unless code is non-obvious. Comments should explain "why", not "what".
- Use `cn` from `@hypr/utils` for conditional classNames. Always pass an array, split by logical grouping.
- Use `motion/react` instead of `framer-motion`.

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

## Cursor Cloud specific instructions

### Services

| Service | Start command | Notes |
|---|---|---|
| Web app | `SKIP_NETLIFY=1 pnpm -F @hypr/web dev` | Runs on port 3000. Use `SKIP_NETLIFY=1` in Cloud VMs to avoid the Netlify plugin. No `.env` file needed for basic dev — all `requiredInProd` vars are optional in dev mode. |
| Desktop app | `pnpm -F @hypr/desktop tauri:dev` | Requires macOS or a full GUI + Tauri system deps. Not runnable in headless Cloud VMs. |
| API server | `CXX=g++ CC=gcc cargo run -p api` | Needs `.env` with Supabase, Stripe, Nango keys. Use `CXX=g++ CC=gcc` on Linux to avoid clang/libc++ issues. |

### Caveats

- **Rust `cargo check` on full workspace fails on Linux x86_64** because `cactus-sys` contains ARM-specific SIMD instructions (Apple Silicon targets). Use `cargo check -p api` or `cargo check -p cli` to check individual crates instead.
- **`CXX=g++ CC=gcc`** must be set for Rust builds involving `whisper-rs-sys` or `cactus-sys` on Linux, because the default clang compiler cannot find `<array>` and other C++ headers.
- **dprint fmt** will emit errors for `.swift` files on Linux (Swift formatter not available). These are safe to ignore.
- **`pnpm-workspace.yaml`** controls `onlyBuiltDependencies`. If builds fail due to missing native deps (esbuild, @swc/core, sharp, etc.), check that the package is listed there.
- **Desktop tests** (`pnpm -F @hypr/desktop test`): All 650 unit tests pass; 2 test files report CSS plugin errors related to PostCSS config loading — these are pre-existing and do not affect test results.
- **Lint** (`pnpm lint`): There are 8 pre-existing lint errors (mostly `@tanstack/query` rules). These are in the existing codebase.
