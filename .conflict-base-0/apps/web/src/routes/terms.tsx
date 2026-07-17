import { createFileRoute, notFound } from "@tanstack/react-router";
import { allLegals } from "content-collections";

import { LegalDocument, legalHead } from "@/components/legal-document";

export const Route = createFileRoute("/terms")({
  component: Component,
  loader: async () => {
    const doc = allLegals.find((legal) => legal.slug === "terms");
    if (!doc) {
      throw notFound();
    }
    return { doc };
  },
  head: ({ loaderData }) => {
    const doc = loaderData?.doc;
    if (!doc) return {};
    return legalHead(doc, "/terms");
  },
});

function Component() {
  const { doc } = Route.useLoaderData();

  return <LegalDocument doc={doc} />;
}
