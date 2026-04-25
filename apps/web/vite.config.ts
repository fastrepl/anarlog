import netlify from "@netlify/vite-plugin-tanstack-start";
import tailwindcss from "@tailwindcss/vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { generateSitemap } from "tanstack-router-sitemap";
import { defineConfig } from "vite";

import { getSitemap } from "./src/utils/sitemap";

const config = defineConfig(() => ({
  plugins: [
    tailwindcss(),
    tanstackStart({
      sitemap: {
        host: "https://char.com",
      },
      prerender: {
        enabled: true,
        concurrency: 3,
        crawlLinks: true,
        autoStaticPathsDiscovery: true,
        filter: ({ path }) => path === "/",
      },
    }),
    viteReact(),
    generateSitemap(getSitemap()),
    process.env.SKIP_NETLIFY === "1"
      ? null
      : netlify({
          dev: { images: { enabled: true }, edgeFunctions: { enabled: false } },
        }),
  ],
  ssr: {
    noExternal: [
      "posthog-js",
      "@posthog/react",
      "react-tweet",
      "@content-collections/mdx",
    ],
  },
  resolve: {
    tsconfigPaths: true,
  },
  preview: {
    host: "127.0.0.1",
  },
}));

export default config;
