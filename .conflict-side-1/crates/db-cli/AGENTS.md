# `db-cli`

## Role

CLI subcommand for inspecting and managing the desktop SQLite database. Exposes `db-app` operations via clap.

## Owns

- `Args` and clap subcommand definitions.
- DB path resolution (`--base` / `--db-path`).
- CLI output formatting.

## Does Not Own

- Schema, migrations, CRUD logic (owned by `db-app`).
- Database opening policy (owned by `db-core`).
