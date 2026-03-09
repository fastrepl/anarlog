import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "wxt";

const NO_BROWSER = process.env.WXT_NO_BROWSER === "1";

export default defineConfig({
  modules: ["@wxt-dev/module-react"],
  vite: () => ({
    plugins: [tailwindcss()],
  }),
  webExt: NO_BROWSER ? { disabled: true } : undefined,
  manifest: {
    name: "Char for Google Meet",
    description:
      "POC extension: Google Meet DOM parsing and native messaging bridge to Char desktop.",
    permissions: ["nativeMessaging"],
  },
});
