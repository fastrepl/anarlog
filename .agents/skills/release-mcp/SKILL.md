---
name: release-mcp
description: Review and verify Anarlog local and hosted MCP protocols, contracts, authentication, access boundaries, and shipped runtime behavior during releases.
metadata:
  internal: true
---

# Release MCP

Review every release. Inherit scope and session authorization from
[Release a New Version](../release-new-version/SKILL.md).

1. Compare product changes with `apps/cli/src/mcp.rs`,
   `crates/api-cloud/src/mcp.rs`, `crates/mcp`, shared `crates/agent-access`, and
   current contract tests. Review initialization, tools/resources, schemas,
   pagination, errors, capability exposure, and local/hosted differences. Derive
   expected tools from current source, never a remembered tool count.
2. Preserve local proposal-approval and hosted read-only boundaries, authentication,
   scopes, and workspace isolation. Implement required coverage before candidate
   freeze or document the intentional difference/explicit deferral. Do not infer
   supported agent behavior from the database schema alone.
3. Run affected local and hosted contract/auth tests from `cli_ci.yaml` and
   `api_ci.yaml`, including allowed and denied paths. Reuse the CLI test evidence
   rather than rerunning identical shared checks.
4. For each shipped desktop platform, use the packaged CLI and isolated fixture
   database for a real stdio initialize/discovery exchange and affected calls.
   Keep stdout protocol-only and verify clean shutdown. Reuse the artifact and
   `verify-packaged-cli.py` evidence from [Release CLI](../release-cli/SKILL.md).
5. Publish required hosted changes through [Release API](../release-api/SKILL.md).
   Verify the serving revision, authentication discovery/OAuth resource metadata,
   and authenticated discovery plus an affected operation with an authorized test
   account. Health/401 responses or mocks are not hosted runtime acceptance.
   Keep writes confined to supported local disposable fixtures and approval rules.
6. Review shared host manifests/setup references once via
   [Agent packages](../release-cli/references/agent-packages.md). Update public
   setup instructions through [Release Docs](../release-docs/SKILL.md).

Return local and hosted revisions, contract decisions, protocol/auth evidence,
package publication or reuse, and unavailable credentials or runtime checks.
