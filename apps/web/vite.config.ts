import contentCollections from "@content-collections/vite";
import tailwindcss from "@tailwindcss/vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { nitro } from "nitro/vite";
import { fileURLToPath } from "node:url";
import { generateSitemap } from "tanstack-router-sitemap";
import { defineConfig, type UserConfig } from "vite";

import { publishedChangelogs } from "./changelog-build.ts";
import { getSitemap } from "./src/utils/sitemap";
import { vercelBuildConfig } from "./vercel-build-config";

const config = defineConfig(async ({ command }): Promise<UserConfig> => {
  const generateSourceMaps = Boolean(
    process.env.SENTRY_BUILD_SOURCEMAPS === "1" && process.env.VITE_APP_VERSION,
  );

  return {
    build: {
      sourcemap: generateSourceMaps ? ("hidden" as const) : false,
      rolldownOptions: { external: ["sharp"] },
    },
    plugins: [
      await publishedChangelogs(command),
      contentCollections(),
      tailwindcss(),
      tanstackStart({
        sitemap: {
          host: "https://anarlog.so",
        },
        prerender: {
          enabled: true,
          concurrency: 3,
          crawlLinks: true,
          autoStaticPathsDiscovery: true,
          filter: ({ path }) => {
            return [
              "/",
              "/download",
              "/download/",
              "/blog",
              "/blog/",
              "/blog/char-is-now-anarlog",
              "/blog/char-is-now-anarlog/",
            ].includes(path);
          },
        },
      }),
      viteReact(),
      generateSitemap(getSitemap()),
      nitro({
        sourcemap: generateSourceMaps,
        vercel: {
          config: vercelBuildConfig,
          functions: {
            runtime: "nodejs22.x",
            regions: ["sfo1"],
            maxDuration: 300,
            environment: { APP_VERSION: process.env.VITE_APP_VERSION ?? "dev" },
          },
        },
      }),
      {
        name: "local-image-redirect",
        apply: "serve",
        configureServer(server) {
          server.middlewares.use("/_vercel/image", (request, response) => {
            const source = new URL(
              request.url ?? "/",
              "http://localhost",
            ).searchParams.get("url");
            if (
              !source?.startsWith("/") ||
              source.startsWith("//") ||
              source.startsWith("/_vercel/")
            ) {
              response.statusCode = 400;
            } else {
              response.statusCode = 307;
              response.setHeader("Location", source);
            }
            response.end();
          });
        },
      },
    ],
    ssr: {
      external: ["sharp"],
      noExternal: ["posthog-js", "@posthog/react", "react-tweet"],
    },
    resolve: {
      alias: {
        "article-summaries": fileURLToPath(
          new URL(
            "./.content-collections/generated/allArticleSummaries.js",
            import.meta.url,
          ),
        ),
      },
      tsconfigPaths: true,
    },
    preview: {
      host: "127.0.0.1",
    },
  };
});

export default config;
