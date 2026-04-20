import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "electron-vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  main: {
    resolve: {
      preserveSymlinks: false,
    },
    build: {
      outDir: "dist/electron",
      emptyOutDir: true,
      lib: {
        entry: "src/main/main.ts",
        formats: ["es"],
      },
      rollupOptions: {
        external: ["@hypr/napi-sdk", "electron-updater"],
        output: {
          entryFileNames: "main.js",
        },
      },
    },
  },
  preload: {
    resolve: {
      preserveSymlinks: false,
    },
    build: {
      outDir: "dist/electron",
      emptyOutDir: false,
      lib: {
        entry: "src/preload/preload.cts",
        formats: ["cjs"],
      },
      rollupOptions: {
        output: {
          entryFileNames: "preload.cjs",
        },
      },
    },
  },
  renderer: {
    root: "src/renderer",
    plugins: [tailwindcss(), react()],
    resolve: {
      alias: {
        "~": path.join(rootDir, "src/renderer"),
      },
      preserveSymlinks: false,
    },
    server: {
      host: "127.0.0.1",
      fs: {
        allow: ["..", "../..", "../../.."],
      },
    },
    build: {
      outDir: "../../dist/ui",
      emptyOutDir: true,
      sourcemap: true,
    },
  },
});
