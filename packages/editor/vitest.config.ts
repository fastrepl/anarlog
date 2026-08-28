import stylex from "@stylexjs/unplugin";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [
    stylex.vite({
      runtimeInjection: false,
      unstable_moduleResolution: {
        type: "commonJS",
        rootDir: fileURLToPath(new URL("../..", import.meta.url)),
      },
    }),
  ],
});
