import { createFileRoute, redirect } from "@tanstack/react-router";
import { allDocs } from "content-collections";

import { DocLayout } from "./-components";
import { docsStructure } from "./-structure";

export const Route = createFileRoute("/_view/docs/$")({
  component: Component,
  beforeLoad: ({ params }) => {
    const splat = params._splat || "";
    const normalizedSplat = splat.replace(/\/$/, "");

    if (docsStructure.defaultPages[normalizedSplat]) {
      throw redirect({
        to: "/docs/$/",
        params: { _splat: docsStructure.defaultPages[normalizedSplat] },
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
    const url = `https://char.com/docs/${doc.slug}`;
    const ogImageUrl = `https://char.com/og?type=docs&title=${encodeURIComponent(doc.title)}&section=${encodeURIComponent(doc.section)}${doc.summary ? `&description=${encodeURIComponent(doc.summary)}` : ""}&v=1`;

    return {
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
