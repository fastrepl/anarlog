# General

- When input variables change, structs in `crates/template2/src` should be updated, and TS binding should be updated with `cargo test -p tauri-plugin-template`.
- `.md.jinja` files should be written in structured-markdown format.
- Any instructions should be written as very concise, minimal descriptions. No weird prompting tricks or ALL CAPITALS. Human-readability is important.

# Syntax

- Read [docs](https://askama.readthedocs.io/en/stable/template_syntax.html) for template syntax.
- Our custom filters are defined in `crates/askama-utils/src/filters.rs`.

# Tooling

- If `cargo insta` is not available, run `cargo install cargo-insta` first.
- `cargo test -p template2 -q; cargo insta accept` combo is helpful to iteratively adjust template and get feedback from the snapshot.
