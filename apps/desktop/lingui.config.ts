import { defineConfig } from "@lingui/cli";

import { SUPPORTED_DISPLAY_LOCALES } from "./src/i18n/locales";

export default defineConfig({
  sourceLocale: "en",
  locales: [...SUPPORTED_DISPLAY_LOCALES],
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
