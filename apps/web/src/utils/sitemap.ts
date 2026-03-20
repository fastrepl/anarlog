import { type Sitemap } from "tanstack-router-sitemap";

import { type FileRouteTypes } from "@/routeTree.gen";

export type TRoutes = FileRouteTypes["fullPaths"];

export function getSitemap(): Sitemap<TRoutes> {
  return {
    siteUrl: "https://char.com",
    defaultPriority: 0.5,
    defaultChangeFreq: "monthly",
    routes: {
      "/": {
        priority: 1.0,
        changeFrequency: "daily",
      },
      "/pricing": {
        priority: 0.9,
        changeFrequency: "monthly",
      },
      "/docs": {
        priority: 0.9,
        changeFrequency: "weekly",
      },
      "/enterprise": {
        priority: 0.8,
        changeFrequency: "monthly",
      },

      "/blog/": {
        priority: 0.8,
        changeFrequency: "daily",
      },
      "/changelog/": {
        priority: 0.7,
        changeFrequency: "weekly",
      },

      "/opensource": {
        priority: 0.8,
        changeFrequency: "monthly",
      },

      "/about": {
        priority: 0.6,
        changeFrequency: "monthly",
      },
      "/brand": {
        priority: 0.5,
        changeFrequency: "monthly",
      },
      "/company-handbook": {
        priority: 0.6,
        changeFrequency: "weekly",
      },
      "/file-transcription": {
        priority: 0.7,
        changeFrequency: "monthly",
      },
      "/free": {
        priority: 0.7,
        changeFrequency: "monthly",
      },
      "/eval/": {
        priority: 0.7,
        changeFrequency: "weekly",
      },
      "/gallery/": {
        priority: 0.7,
        changeFrequency: "weekly",
      },
      "/oss-friends": {
        priority: 0.6,
        changeFrequency: "monthly",
      },
      "/roadmap/": {
        priority: 0.7,
        changeFrequency: "weekly",
      },
      "/security": {
        priority: 0.6,
        changeFrequency: "monthly",
      },

      "/download/": {
        priority: 0.7,
        changeFrequency: "weekly",
      },
      "/download/apple-intel": {
        priority: 0.7,
        changeFrequency: "weekly",
      },
      "/download/apple-silicon": {
        priority: 0.7,
        changeFrequency: "weekly",
      },

      "/legal/": {
        priority: 0.5,
        changeFrequency: "yearly",
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
          if (!Array.isArray(allArticles)) {
            console.warn("allArticles is not an array:", typeof allArticles);
            return [];
          }
          return allArticles.map((article: any) => ({
            path: `/blog/${article.slug}`,
            priority: 0.7,
            changeFrequency: "weekly" as const,
            lastModified: article.date,
          }));
        } catch (error) {
          console.warn("Failed to load blog articles for sitemap:", error);
          return [];
        }
      },

      "/changelog/$slug": async () => {
        try {
          const path = await import("path");
          const url = await import("url");
          const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
          const modulePath = path.resolve(
            __dirname,
            "../../.content-collections/generated/allChangelogs.js",
          );
          const imported = await import(modulePath);
          const allChangelogs =
            imported.default ?? imported.allChangelogs ?? [];
          if (!Array.isArray(allChangelogs)) return [];
          return allChangelogs.map((changelog: any) => ({
            path: `/changelog/${changelog.slug}`,
            priority: 0.6,
            changeFrequency: "monthly" as const,
            lastModified: changelog.date,
          }));
        } catch (error) {
          console.warn("Failed to load changelogs for sitemap:", error);
          return [];
        }
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
          if (!Array.isArray(allLegals)) return [];
          return allLegals.map((legal: any) => ({
            path: `/legal/${legal.slug}`,
            priority: 0.5,
            changeFrequency: "yearly" as const,
            lastModified: legal.date,
          }));
        } catch (error) {
          console.warn("Failed to load legal docs for sitemap:", error);
          return [];
        }
      },

      "/docs/$": async () => {
        try {
          const path = await import("path");
          const url = await import("url");
          const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
          const modulePath = path.resolve(
            __dirname,
            "../../.content-collections/generated/allDocs.js",
          );
          const imported = await import(modulePath);
          const allDocs = imported.default ?? imported.allDocs ?? [];
          if (!Array.isArray(allDocs)) return [];
          return allDocs.map((doc: any) => ({
            path: `/docs/${doc.slug}`,
            priority: 0.8,
            changeFrequency: "weekly" as const,
            lastModified: doc.date,
          }));
        } catch (error) {
          console.warn("Failed to load docs for sitemap:", error);
          return [];
        }
      },

      "/roadmap/$slug": async () => {
        try {
          const path = await import("path");
          const url = await import("url");
          const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
          const modulePath = path.resolve(
            __dirname,
            "../../.content-collections/generated/allRoadmaps.js",
          );
          const imported = await import(modulePath);
          const allRoadmaps = imported.default ?? imported.allRoadmaps ?? [];
          if (!Array.isArray(allRoadmaps)) return [];
          return allRoadmaps.map((roadmap: any) => ({
            path: `/roadmap/${roadmap.slug}`,
            priority: 0.6,
            changeFrequency: "weekly" as const,
            lastModified: roadmap.date,
          }));
        } catch (error) {
          console.warn("Failed to load roadmap items for sitemap:", error);
          return [];
        }
      },

      "/company-handbook/$": async () => {
        try {
          const path = await import("path");
          const url = await import("url");
          const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
          const modulePath = path.resolve(
            __dirname,
            "../../.content-collections/generated/allHandbooks.js",
          );
          const imported = await import(modulePath);
          const allHandbooks = imported.default ?? imported.allHandbooks ?? [];
          if (!Array.isArray(allHandbooks)) return [];
          return allHandbooks.map((handbook: any) => ({
            path: `/company-handbook/${handbook.slug}`,
            priority: 0.6,
            changeFrequency: "weekly" as const,
            lastModified: handbook.date,
          }));
        } catch (error) {
          console.warn("Failed to load handbook pages for sitemap:", error);
          return [];
        }
      },

      "/gallery/$type/$slug": async () => {
        try {
          const path = await import("path");
          const url = await import("url");
          const __dirname = path.dirname(url.fileURLToPath(import.meta.url));

          const templatesPath = path.resolve(
            __dirname,
            "../../.content-collections/generated/allTemplates.js",
          );
          const shortcutsPath = path.resolve(
            __dirname,
            "../../.content-collections/generated/allShortcuts.js",
          );

          const templatesImported = await import(templatesPath);
          const shortcutsImported = await import(shortcutsPath);
          const allTemplates =
            templatesImported.default ?? templatesImported.allTemplates ?? [];
          const allShortcuts =
            shortcutsImported.default ?? shortcutsImported.allShortcuts ?? [];
          if (!Array.isArray(allTemplates) || !Array.isArray(allShortcuts))
            return [];

          const templateUrls = allTemplates.map((template: any) => ({
            path: `/gallery/template/${template.slug}`,
            priority: 0.7,
            changeFrequency: "weekly" as const,
          }));

          const shortcutUrls = allShortcuts.map((shortcut: any) => ({
            path: `/gallery/shortcut/${shortcut.slug}`,
            priority: 0.7,
            changeFrequency: "weekly" as const,
          }));

          return [...templateUrls, ...shortcutUrls];
        } catch (error) {
          console.warn("Failed to load gallery items for sitemap:", error);
          return [];
        }
      },
    },
  };
}
