import mdx from "@astrojs/mdx";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://cli.char.com",
  integrations: [mdx()],
  vite: {
    plugins: [tailwindcss()],
  },
});
