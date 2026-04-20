/// <reference types="vitest" />
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  root: "./src/renderer",
  plugins: [tailwindcss(), react()],
  resolve: {
    alias: {
      "~": "/",
    },
    preserveSymlinks: false,
  },
  server: {
    fs: {
      allow: ["..", "../..", "../../.."],
    },
  },
  build: {
    outDir: "../../dist/ui",
    emptyOutDir: true,
    sourcemap: true,
  },
  test: {
    environment: "jsdom",
    passWithNoTests: true,
    setupFiles: ["./test-setup.ts"],
  },
});
