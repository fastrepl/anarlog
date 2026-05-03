import * as fs from "fs";
import * as path from "path";
import { type Sitemap } from "tanstack-router-sitemap";

import { type FileRouteTypes } from "@/routeTree.gen";

export type TRoutes = FileRouteTypes["fullPaths"];

function getArticleSlugs(): string[] {
  const dir = path.resolve(process.cwd(), "content/articles");
  try {
    return fs
      .readdirSync(dir)
      .filter((f) => f.endsWith(".mdx"))
      .map((f) => f.replace(/\.mdx$/, ""));
  } catch {
    return [];
  }
}

export function getSitemap(): Sitemap<TRoutes> {
  const slugs = getArticleSlugs();

  return {
    siteUrl: "https://anarlog.so",
    defaultPriority: 0.5,
    defaultChangeFreq: "monthly",
    routes: {
      "/": {
        priority: 1.0,
        changeFrequency: "monthly",
      },
      "/blog/": {
        priority: 0.8,
        changeFrequency: "weekly",
      },
      "/blog/$slug": slugs.map((slug) => ({
        path: `/blog/${slug}`,
        priority: 0.6,
        changeFrequency: "monthly" as const,
      })),
    },
  };
}
