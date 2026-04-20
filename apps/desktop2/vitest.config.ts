import path from "node:path";
import { defineConfig } from "vitest/config";

import {
  rendererAlias,
  rendererPlugins,
  rendererRoot,
} from "./renderer.vite.base";

const rendererSetupFile = path.join(rendererRoot, "test-setup.ts");

// Two test surfaces in one invocation:
//   - renderer: jsdom. Reuses the renderer Vite base (root + alias + react
//     plugin) so UI containers can be exercised against the preload mock in
//     `src/renderer/test-setup.ts`.
//   - main: node, for pure logic (e.g. URL allowlist) and subscription-manager
//     unit tests. No DOM, no Vite root rewriting.
export default defineConfig({
  test: {
    projects: [
      {
        root: rendererRoot,
        plugins: rendererPlugins,
        resolve: {
          alias: rendererAlias,
        },
        test: {
          name: "renderer",
          environment: "jsdom",
          // `root` above is `src/renderer`; includes resolve relative to that,
          // so match every `*.test.*` under the root without re-prefixing
          // `src/renderer/`.
          include: ["**/*.test.{ts,tsx}"],
          setupFiles: [rendererSetupFile],
        },
      },
      {
        test: {
          name: "main",
          environment: "node",
          include: ["src/main/**/*.test.ts"],
        },
      },
    ],
  },
});
