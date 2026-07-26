# Anarlog documentation

This Mintlify project is published at [docs.anarlog.so](https://docs.anarlog.so).

## Deployment

Configure the Mintlify project with `docs/` as its documentation directory and `docs.anarlog.so` as its custom domain. In Mintlify's domain setup:

1. Add the verification records shown in the dashboard.
2. Wait for both records and TLS provisioning to verify.
3. Point the `docs` CNAME to the target shown by Mintlify.

The website redirects the previous `anarlog.so/docs/*` routes to the matching path on the custom domain.

## Local preview

Install the Mintlify CLI, then run it from this directory:

```bash
npm install --global mint
cd docs
mint dev
```

Update `docs.json` whenever a page is added, moved, or removed. Keep CLI and MCP reference content aligned with `apps/cli/src/cli.rs` and `apps/cli/src/mcp.rs`.

Before deploying, run:

```bash
mint validate
mint broken-links --check-anchors --check-redirects
```
