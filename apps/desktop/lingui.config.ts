import { defineConfig } from "@lingui/cli";

export default defineConfig({
  sourceLocale: "en",
  locales: ["en", "de", "es", "fr", "it", "ja", "ko", "pt", "zh"],
  compileNamespace: "ts",
  fallbackLocales: {
    default: "en",
  },
  catalogs: [
    {
      path: "<rootDir>/src/i18n/locales/{locale}/messages",
      include: ["<rootDir>/src"],
      exclude: ["**/*.test.*", "**/routeTree.gen.ts", "**/i18n/locales/**"],
    },
  ],
});
