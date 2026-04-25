import { type Sitemap } from "tanstack-router-sitemap";

import { type FileRouteTypes } from "@/routeTree.gen";

export type TRoutes = FileRouteTypes["fullPaths"];

export function getSitemap(): Sitemap<TRoutes> {
  return {
    siteUrl: "https://char.com",
    defaultPriority: 0.6,
    defaultChangeFreq: "monthly",
    routes: {
      "/": {
        priority: 1,
        changeFrequency: "daily",
      },
      "/blog/": {
        priority: 0.8,
        changeFrequency: "weekly",
      },
      "/blog/$slug": async () => {
        try {
          const path = await import("path");
          const url = await import("url");
          const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
          const modulePath = path.resolve(
            __dirname,
            "../../.content-collections/generated/allArticles.js",
          );
          const imported = await import(modulePath);
          const allArticles = imported.default ?? imported.allArticles ?? [];
          return allArticles.map((article: any) => ({
            path: `/blog/${article.slug}`,
            priority: 0.7,
            changeFrequency: "monthly" as const,
            lastModified: article.date,
          }));
        } catch {
          return [];
        }
      },
      "/legal/": {
        priority: 0.5,
        changeFrequency: "yearly",
      },
      "/legal/$slug": async () => {
        try {
          const path = await import("path");
          const url = await import("url");
          const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
          const modulePath = path.resolve(
            __dirname,
            "../../.content-collections/generated/allLegals.js",
          );
          const imported = await import(modulePath);
          const allLegals = imported.default ?? imported.allLegals ?? [];
          return allLegals.map((doc: any) => ({
            path: `/legal/${doc.slug}`,
            priority: 0.5,
            changeFrequency: "yearly" as const,
            lastModified: doc.date,
          }));
        } catch {
          return [];
        }
      },
      "/oss-friends": {
        priority: 0.6,
        changeFrequency: "monthly",
      },
    },
  };
}
