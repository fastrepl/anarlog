import { createFileRoute, redirect } from "@tanstack/react-router";
import { allHandbooks } from "content-collections";

import { HandbookLayout } from "./-components";
import { handbookStructure } from "./-structure";

export const Route = createFileRoute("/_view/company-handbook/$")({
  component: Component,
  beforeLoad: ({ params }) => {
    const splat = params._splat || "";
    const normalizedSplat = splat.replace(/\/$/, "");

    if (handbookStructure.defaultPages[normalizedSplat]) {
      throw redirect({
        to: "/company-handbook/$/",
        params: {
          _splat: handbookStructure.defaultPages[normalizedSplat],
        },
      });
    }

    let doc = allHandbooks.find((doc) => doc.slug === normalizedSplat);
    if (!doc) {
      doc = allHandbooks.find((doc) => doc.slug === `${normalizedSplat}/index`);
    }

    if (!doc) {
      if (normalizedSplat === "about/what-char-is") {
        return;
      }
      throw redirect({
        to: "/company-handbook/$/",
        params: { _splat: "about/what-char-is" },
      });
    }
  },
  loader: async ({ params }) => {
    const splat = params._splat || "";
    const normalizedSplat = splat.replace(/\/$/, "");

    let doc = allHandbooks.find((doc) => doc.slug === normalizedSplat);
    if (!doc) {
      doc = allHandbooks.find((doc) => doc.slug === `${normalizedSplat}/index`);
    }

    return { doc: doc! };
  },
  head: ({ loaderData }) => {
    if (!loaderData?.doc) {
      return { meta: [] };
    }

    const { doc } = loaderData;
    const url = `https://char.com/company-handbook/${doc.slug}`;

    const params = new URLSearchParams({
      type: "handbook",
      title: doc.title,
      section: doc.section,
    });
    if (doc.summary) {
      params.set("description", doc.summary);
    }
    const ogImage = `/og?${params.toString()}`;

    return {
      meta: [
        { title: `${doc.title} - Company Handbook - Char` },
        { name: "description", content: doc.summary || doc.title },
        {
          property: "og:title",
          content: `${doc.title} - Company Handbook`,
        },
        {
          property: "og:description",
          content: doc.summary || doc.title,
        },
        { property: "og:type", content: "article" },
        { property: "og:url", content: url },
        { property: "og:image", content: ogImage },
        { name: "twitter:card", content: "summary_large_image" },
        { name: "twitter:image", content: ogImage },
      ],
    };
  },
});

function Component() {
  const { doc } = Route.useLoaderData();

  return <HandbookLayout doc={doc} showSectionTitle={true} />;
}
