## Plugin Runtime Notes

Plugins in `examples/plugins/*` run inside the desktop renderer process as plain browser scripts.

### React and JSX

- Plugins do not import React from npm at runtime.
- Use `window.__char_react` in plugin source.
- Build JSX with classic transform for IIFE bundles:
  - `jsx: "transform"`
  - `jsxFactory: "React.createElement"`
  - `jsxFragment: "React.Fragment"`
- Do not use `jsx: "automatic"` for these plugin bundles. It can emit `react/jsx-runtime` usage, which is not available in this runtime.

### Why a bad plugin can freeze the app

- Desktop loads installed plugins from `~/Library/Application Support/com.hyprnote.dev/plugins`.
- Plugin scripts are injected into the main renderer thread.
- `onload` runs during app startup, and many examples call `ctx.openTab(...)` immediately.
- Blocking logic (infinite loops, heavy sync work, runaway rerenders/listeners) can freeze the whole app.

### Recovery and debug workflow

- Rebuild and reinstall plugin after changes:
  - `pnpm --dir examples/plugins/hello-world build`
  - `pnpm --dir examples/plugins/hello-world install:dev`
- If startup is bricked, remove installed plugins directory and restart:
  - `rm -rf ~/Library/Application\ Support/com.hyprnote.dev/plugins`
