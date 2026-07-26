# Errors

## Meeting not found

List or search meetings again and use the returned ID. Do not retry a guessed ID.

## Database not found

Run `anarlog --json doctor`. Ask the user to open Anarlog once if the database does not exist. If they keep data in a custom location, use `--db-path FILE` or `ANARLOG_DB_PATH` after they provide the path.

## Database operation failed

Confirm the desktop app and CLI come from compatible revisions. Do not run migrations or write SQL from the agent.

## Export output exists

Choose a new path. Pass `--force` only when the user explicitly approves replacing that exact file.

## MCP server exits

Run `anarlog --json meetings list` to distinguish database access from client configuration. Confirm the MCP command is `anarlog` and its only required argument is `mcp`.

With `--json`, errors contain `schema_version` and an `error` object with `code`, `message`, and `exit_code`. CLI exit codes are `1` for an operation failure, `2` for missing data, `3` for a missing database, and `4` for an existing export target. Invalid CLI arguments use Clap's exit code and the `invalid_arguments` error code.
