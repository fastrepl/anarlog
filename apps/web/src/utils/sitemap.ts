import { type Sitemap } from "tanstack-router-sitemap";

import { type FileRouteTypes } from "@/routeTree.gen";

export type TRoutes = FileRouteTypes["fullPaths"];

export function getSitemap(): Sitemap<TRoutes> {
  return {
    siteUrl: "https://char.com",
    defaultPriority: 1,
    defaultChangeFreq: "daily",
    routes: {
      "/": {
        priority: 1,
        changeFrequency: "daily",
      },
    },
  };
}
