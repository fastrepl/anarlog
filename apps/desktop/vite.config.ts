/// <reference types="vitest" />
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, type UserConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

import { relayShim } from "@hypr/plugin-relay/vite";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [
    tsconfigPaths(),
    relayShim(),
    tanstackRouter({ target: "react", autoCodeSplitting: false }),
    react({
      babel: {
        plugins: [
          // [
          //   "babel-plugin-react-compiler",
          //   { compilationMode: "infer", target: "19" },
          // ],
        ],
      },
    }),
  ],
  resolve: {
    alias:
      process.env.NODE_ENV === "development"
        ? {
            "@tauri-apps/plugin-updater": "/src/shared/mock-updater.ts",
          }
        : {},
    dedupe: [
      "@codemirror/state",
      "@codemirror/view",
      "@codemirror/autocomplete",
      "@codemirror/language",
      "@codemirror/lint",
      "@codemirror/lang-jinja",
      "codemirror-readonly-ranges",
      "@uiw/react-codemirror",
    ],
  },
  test: {
    reporters: "default",
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    onConsoleLog: (_, type) => {
      return type === "stderr";
    },
    exclude: ["**/node_modules/**", "**/src-tauri/**"],
  },
  ...tauri,
}));

// https://v2.tauri.app/start/frontend/vite/#update-vite-configuration
const tauri: UserConfig = {
  clearScreen: false,
  server: {
    port: 1422,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1423,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    outDir: "./dist",
    chunkSizeWarningLimit: 500 * 10,
    target:
      process.env.TAURI_ENV_PLATFORM == "windows" ? "chrome105" : "safari13",
    // minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    minify: false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
};
