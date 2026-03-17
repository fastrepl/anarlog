import { createFileRoute, notFound } from "@tanstack/react-router";
import { allLegals } from "content-collections";

import { LegalLayout } from "./-components";

export const Route = createFileRoute("/_view/legal/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const doc = allLegals.find((doc) => doc.slug === params.slug);
    if (!doc) {
      throw notFound();
    }

    return { doc };
  },
  head: ({ loaderData }) => {
    if (!loaderData?.doc) {
      return { meta: [] };
    }

    const { doc } = loaderData;
    const url = `https://char.com/legal/${doc.slug}`;

    return {
      meta: [
        { title: `${doc.title} - Char` },
        { name: "description", content: doc.summary },
        { property: "og:title", content: doc.title },
        { property: "og:description", content: doc.summary },
        { property: "og:type", content: "article" },
        { property: "og:url", content: url },
        { name: "twitter:card", content: "summary" },
        { name: "twitter:title", content: doc.title },
        { name: "twitter:description", content: doc.summary },
      ],
    };
  },
});

function Component() {
  const { doc } = Route.useLoaderData();

  return <LegalLayout doc={doc} />;
}
