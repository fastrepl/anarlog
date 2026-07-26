# Anarlog documentation instructions

## Scope

- This is the Mintlify project published at `https://docs.anarlog.so`.
- Write for Anarlog users, developers, and agents using the CLI or MCP server.
- Configuration lives in `docs.json`; content pages are MDX.
- The public agent skill is maintained in `../skills/anarlog/`.

## Sources of truth

- Treat `apps/cli/src/cli.rs` as the CLI command contract.
- Treat `apps/cli/src/mcp.rs` as the MCP tool and resource contract.
- Treat current release automation as the source of truth for installation channels.
- Do not infer product behavior from the raw SQLite schema.

## Writing

- Use active voice and second person.
- Keep headings and sentences concise.
- Put the result before implementation detail.
- Use `Anarlog` for the product and `anarlog` for the executable.
- Use root-relative links between Mintlify pages. Use `https://docs.anarlog.so` in external instructions and agent metadata.

## Accuracy boundaries

- Document only commands, options, tools, resources, and output behavior present in the source.
- Mark planned features and distribution channels as forthcoming.
- Never describe Homebrew, desktop-bundled CLI, or Windows binaries as available until release automation publishes them.
- Never tell users or agents to read, migrate, or modify the SQLite database directly.
- Keep transcript examples bounded. CLI and MCP transcript pages default to 200 words and cap at 500 words.

## Verification

- Check `docs.json` after adding or moving a page.
- Run `pnpm exec dprint fmt docs skills` from the repository root.
- Run `pnpm exec dprint check docs skills` before submitting.
- Run `mint validate` and `mint broken-links --check-anchors --check-redirects` from `docs/` before deploying.
