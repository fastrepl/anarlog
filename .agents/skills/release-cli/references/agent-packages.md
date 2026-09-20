# Agent packages

[Release CLI](../SKILL.md) and [Release MCP](../../release-mcp/SKILL.md) share
this review and publish each affected package once. Inherit release authorization;
do not assume generating local files publishes or installs anything.

1. Compare authored `skills/anarlog/` and its CLI/MCP/setup references with current
   contracts. Review generated `agent-plugins/anarlog/`, `docs/skill.md`, native
   manifests, and repository marketplace entries. CLI owns command/install
   examples; MCP owns protocol/auth setup, discovery and host manifests.
2. Edit authored sources and regenerate mirrors:

   ```bash
   node scripts/publish-anarlog-skill.mjs
   node scripts/publish-anarlog-skill.mjs --check
   node --test scripts/publish-anarlog-skill.test.mjs
   ```

   Confirm generation is stable. Bump the plugin's own version when its package
   changes; do not tie it artificially to the desktop version. Include intended
   generated changes before candidate freeze. Run the current `cli_ci.yaml`
   manifest, referenced-file, discovery, and skill validation checks.
3. Verify the actual publication mechanism for each affected repository/catalog.
   Publish within existing session authorization and follow it to completion;
   if a destination or approval is missing, prepare the change fully and record
   that specific blocker. Never invent a catalog publish command.
4. Coordinate `docs/skill.md` and discovery publication with
   [Release Docs](../../release-docs/SKILL.md). Verify the supported fresh install
   or update resolves the published package and capabilities. Catalog submission,
   approval, repository merge, and the user's installed cache are distinct states.

Return source/package version, update or reuse reasons, validation, publication
destination, install evidence, and pending actions to both owning skills.
