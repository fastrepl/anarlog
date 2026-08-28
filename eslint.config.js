import stylexPlugin from "@stylexjs/eslint-plugin";
import pluginQuery from "@tanstack/eslint-plugin-query";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(
  {
    ignores: [
      "**/target/**",
      "**/dist/**",
      "**/node_modules/**",
      "**/*.gen.*",
      "apps/desktop/src/i18n/locales/*/messages.ts",
    ],
  },
  {
    files: [
      "apps/web/**/*.{ts,tsx}",
      "apps/desktop/**/*.{ts,tsx}",
      "packages/changelog/**/*.{ts,tsx}",
      "packages/design-system/**/*.{ts,tsx}",
      "packages/editor/**/*.{ts,tsx}",
      "packages/pricing/**/*.{ts,tsx}",
      "packages/ui/**/*.{ts,tsx}",
    ],
    extends: [
      tseslint.configs.base,
      ...pluginQuery.configs["flat/recommended"],
    ],
    plugins: {
      "@stylexjs": stylexPlugin,
    },
    rules: {
      "@stylexjs/enforce-extension": "error",
      "@stylexjs/no-conflicting-props": "error",
      "@stylexjs/no-legacy-contextual-styles": "error",
      "@stylexjs/no-unused": "error",
      "@stylexjs/valid-shorthands": "error",
      "@stylexjs/valid-styles": "error",
    },
  },
);
