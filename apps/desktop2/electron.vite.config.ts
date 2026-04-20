import { defineConfig } from "electron-vite";

import {
  rendererAlias,
  rendererPlugins,
  rendererRoot,
  rendererServerFs,
} from "./renderer.vite.base";

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
    root: rendererRoot,
    plugins: rendererPlugins,
    resolve: {
      alias: rendererAlias,
      preserveSymlinks: false,
    },
    server: {
      host: "127.0.0.1",
      fs: rendererServerFs,
    },
    build: {
      outDir: "../../dist/ui",
      emptyOutDir: true,
      sourcemap: true,
    },
  },
});
