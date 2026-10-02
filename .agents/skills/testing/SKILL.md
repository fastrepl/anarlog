---
name: testing
description: "Use when adding, changing, deleting, or reviewing tests in Anarlog (TypeScript, Rust, or node --test scripts), deciding whether a change needs a test, fixing a failing test, or pruning test suites. Covers what is worth testing, what to leave untested, test budgets, and the test-weight report."
---

# Testing

Use this skill when adding, changing, deleting, or reviewing tests in Anarlog, deciding whether a change needs a test, or pruning a test suite. Tests are code we pay for on every CI run and every refactor; keep the ones that catch real regressions and nothing else.

## Write a test when

- A bug was fixed: add one regression test that fails without the fix. One, not a matrix of neighbours.
- The behavior is observable by a user or another component: public APIs, tauri-specta bindings, OpenAPI contracts, MCP and CLI output, serializers and parsers, TipTap/ProseMirror JSON conversion.
- Losing it loses data or trust: SQLite migrations and desktop/mobile schema parity, CloudSync and E2EE, auth, billing and Stripe webhooks, permissions and RLS, recording and STT lifecycles.
- It is a real concurrency or recovery invariant: capture start/stop races, crash recovery, retry, cancellation, ordering between actors or tasks.
- It encodes release or store policy that a workflow edit could silently break (for example `scripts/desktop-release-provenance.test.mjs`).

## Do not write a test for

- CSS classes, DOM structure, exact copy, or log text, unless the text is a contract (CLI output parsed by another tool, i18n keys checked by CI).
- Mock call counts or call order when the observable result already proves the behavior.
- Private state, internal helpers already covered through their public caller, or trivial delegation.
- Framework or library behavior (React, TanStack, Tauri, Zustand, Tokio, serde).
- Constants, enum tables, type re-exports, or anything that mirrors source line by line.
- Package or file layout. Structural and import-boundary rules belong in oxlint/ESLint rules, not tests that read files from disk.
- `test.skip` / `#[ignore]` without a linked issue.

## Budget

- Prefer strengthening an existing test over adding a new one. Prefer one table-driven case over five copies only when the rows exercise different branches.
- A new test must name the regression it prevents in its title.
- Deleting a test is fine when the behavior is covered elsewhere, the code is gone, or it matches the "do not write" list. Say which in the commit or PR.
- Keep fixtures and helpers next to the tests that use them; delete them with the last caller.
- If a module's test file outgrows its source several times over, stop adding and consolidate first.

## Test-weight report

Run `node .agents/skills/testing/scripts/test-weight.mjs` from the repo root to list packages and files whose test code most outweighs their source. Use `--help` for thresholds and JSON output. Audit the top entries against the rules above before adding more tests to them.

## Conventions

Which tests run for which component is listed under "Pre-commit verification" in the root `AGENTS.md`; the workflows in `.github/workflows/` are the source of truth. Axum transport choice for API tests follows `../axum-test-transport/SKILL.md`.
