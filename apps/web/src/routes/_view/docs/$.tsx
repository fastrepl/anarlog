import { createFileRoute, redirect } from "@tanstack/react-router";
import { allDocs } from "content-collections";

import {
  CHAR_SITE_URL,
  getBreadcrumbListJsonLd,
  getOrganizationJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";

import { DocLayout } from "./-components";
import { docsStructure } from "./-structure";

export const Route = createFileRoute("/_view/docs/$")({
  component: Component,
  beforeLoad: ({ params }) => {
    const splat = params._splat || "";
    const normalizedSplat = splat.replace(/\/$/, "");
    const defaultPage = docsStructure.defaultPages[normalizedSplat];

    if (defaultPage && defaultPage !== normalizedSplat) {
      throw redirect({
        to: "/docs/$/",
        params: { _splat: defaultPage },
      });
    }

    let doc = allDocs.find((doc) => doc.slug === normalizedSplat);
    if (!doc) {
      doc = allDocs.find((doc) => doc.slug === `${normalizedSplat}/index`);
    }

    if (!doc) {
      if (normalizedSplat === "about/hello-world") {
        return;
      }
      throw redirect({
        to: "/docs/$/",
        params: { _splat: "about/hello-world" },
      });
    }
  },
  loader: async ({ params }) => {
    const splat = params._splat || "";
    const normalizedSplat = splat.replace(/\/$/, "");

    let doc = allDocs.find((doc) => doc.slug === normalizedSplat);
    if (!doc) {
      doc = allDocs.find((doc) => doc.slug === `${normalizedSplat}/index`);
    }

    return { doc: doc! };
  },
  head: ({ loaderData }) => {
    if (!loaderData?.doc) {
      return { meta: [] };
    }

    const { doc } = loaderData;
    const url = `${CHAR_SITE_URL}/docs/${doc.slug}`;
    const ogImageUrl = `${CHAR_SITE_URL}/og?type=docs&title=${encodeURIComponent(doc.title)}&section=${encodeURIComponent(doc.section)}${doc.summary ? `&description=${encodeURIComponent(doc.summary)}` : ""}&v=1`;

    return {
      links: [{ rel: "canonical", href: url }],
      scripts: [
        {
          type: "application/ld+json",
          children: JSON.stringify(
            getStructuredDataGraph([
              {
                "@type": "TechArticle",
                headline: doc.title,
                name: doc.title,
                description: doc.summary || doc.title,
                url,
                image: [ogImageUrl],
                about: {
                  "@type": "Thing",
                  name: doc.section,
                },
                isPartOf: {
                  "@type": "WebSite",
                  name: "Char Documentation",
                  url: `${CHAR_SITE_URL}/docs`,
                },
                publisher: getOrganizationJsonLd(),
              },
              getBreadcrumbListJsonLd([
                { name: "Home", item: CHAR_SITE_URL },
                { name: "Docs", item: `${CHAR_SITE_URL}/docs` },
                { name: doc.title, item: url },
              ]),
            ]),
          ),
        },
      ],
      meta: [
        { title: `${doc.title} - Char Documentation` },
        { name: "description", content: doc.summary || doc.title },
        {
          property: "og:title",
          content: `${doc.title} - Char Documentation`,
        },
        {
          property: "og:description",
          content: doc.summary || doc.title,
        },
        { property: "og:type", content: "article" },
        { property: "og:url", content: url },
        { property: "og:image", content: ogImageUrl },
        { name: "twitter:card", content: "summary_large_image" },
        {
          name: "twitter:title",
          content: `${doc.title} - Char Documentation`,
        },
        {
          name: "twitter:description",
          content: doc.summary || doc.title,
        },
        { name: "twitter:image", content: ogImageUrl },
      ],
    };
  },
});

function Component() {
  const { doc } = Route.useLoaderData();

  return <DocLayout doc={doc} showSectionTitle={true} />;
}
