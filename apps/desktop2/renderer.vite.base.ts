import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Single source of truth for the renderer's Vite surface. Consumed by:
//   - `electron.vite.config.ts` for dev/build under electron-vite.
//   - `vitest.config.ts` for the renderer test project (jsdom).
// Any renderer-wide Vite concern (root, alias, plugins, dev-server fs allow)
// belongs here, not duplicated in each config.

const rootDir = path.dirname(fileURLToPath(import.meta.url));

export const rendererRoot = path.join(rootDir, "src/renderer");

export const rendererAlias = {
  "~": rendererRoot,
};

export const rendererPlugins = [tailwindcss(), react()];

export const rendererServerFs = {
  allow: ["..", "../..", "../../.."],
};
