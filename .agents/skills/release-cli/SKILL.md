---
name: release-cli
description: Review Anarlog CLI and agent-access coverage, validate shared agent packages, and verify the CLI shipped in each desktop artifact and install path.
metadata:
  internal: true
---

# Release CLI

Review every release. Inherit scope and session authorization from
[Release a New Version](../release-new-version/SKILL.md).

1. Compare user-visible product changes with `apps/cli`, `crates/agent-access`,
   help and CLI contract snapshots. Check commands, flags, JSON fields/errors,
   pagination, local/cloud selection, output/exit codes, and supported formats.
   Implement required parity before candidate freeze; document intentional
   capability differences, reuse reasons, and explicit deferrals.
2. Follow `cli_ci.yaml` and root `AGENTS.md`: run affected locked tests, Clippy
   with the workflow flags, release CLI build, isolated smoke tests, and docs/skill
   validation. Preserve Linux and Windows verification; a local macOS executable
   does not establish other platforms' results. Coordinate public examples with
   [Release Docs](../release-docs/SKILL.md).
3. Complete the shared [Agent packages](references/agent-packages.md) review once
   with [Release MCP](../release-mcp/SKILL.md). The authored skill, generated mirrors,
   public documentation, and installed plugin are separate publication states.
4. Verify the CLI extracted from each final signed desktop artifact. Confirm
   `APP_VERSION` reached the build and packaged `--version` matches the requested
   desktop release. Run `scripts/verify-packaged-cli.py <package-root> <version>`
   as used in the platform workflow, plus affected command checks against isolated
   fixtures. Reuse its MCP results with Release MCP. A developer binary on `PATH`,
   a staging package, or unit tests alone cannot verify the final stable package.
5. Check help, loading/runtime dependencies, output/exit codes and shutdown. After
   upgrade, verify the supported CLI installer resolves the new bundled binary;
   consult `skills/anarlog/references/setup.md` and the current installer. Never
   use user data as a disposable fixture or copy/reset databases and sync identity.

Return parity decisions, source and artifact versions/hashes, checks, supported
install/update evidence, and unavailable platforms or pending work. A mobile-only
release does not authorize publishing an unrelated desktop CLI.
