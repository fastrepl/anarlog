---
name: add-plugin
description: Scaffold a new Tauri plugin in this repository when asked to add or create a plugin under `plugins/`. Use this for plugin generation and repository integration work, not for editing an existing plugin unless the request is specifically about bringing a freshly generated plugin in line with project conventions.
metadata:
  internal: true
---

Create a new plugin with:

```bash
npx @tauri-apps/cli plugin new NAME \
--no-example \
--directory ./plugins/NAME
```

Decide `NAME` from the user request, then run the generator.

Follow the style and conventions used in `plugins/analytics`:

- remove generated `rollup.config.js`
- remove generated `README.md`
- update `tsconfig.json`
- update `package.json`

If the user does not ask for different command behavior, keep the single `ping` function in both `ext.rs` and `commands.rs`.

After the code changes are in place, generate bindings:

```bash
pnpm -F @hypr/plugin-<NAME> codegen
```

Finish by updating these integration points:

- `Cargo.toml`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/capabilities/default.json`

Run `pnpm i` after updating `apps/desktop/package.json`.
