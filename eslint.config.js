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
    files: ["apps/web/**/*.{ts,tsx}", "apps/desktop/**/*.{ts,tsx}"],
    extends: [
      tseslint.configs.base,
      ...pluginQuery.configs["flat/recommended"],
    ],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "sonner",
              importNames: ["Toaster"],
              message:
                "Use Toaster from @anlg/ui/components/ui/toast for Hugeicons.",
            },
          ],
          patterns: [
            {
              group: [
                "lucide*",
                "@phosphor-icons/*",
                "phosphor-react",
                "react-icons",
                "react-icons/*",
                "@tabler/icons*",
                "@heroicons/*",
                "@radix-ui/react-icons",
                "@fortawesome/*",
                "@hugeicons/*",
              ],
              message:
                "Use @anlg/ui/components/icons so interface icons come from the shared Hugeicons set.",
            },
          ],
        },
      ],
      "no-restricted-syntax": [
        "error",
        {
          selector:
            "JSXOpeningElement[name.name='Streamdown']:not(:has(JSXAttribute[name.name='icons']))",
          message:
            "Pass streamdownIcons from @anlg/ui/components/streamdown-icons so markdown controls use Hugeicons.",
        },
      ],
    },
  },
);
