import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, notFound } from "@tanstack/react-router";
import { allLegals } from "content-collections";

import { defaultMDXComponents } from "@/components/mdx";

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

  return (
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white px-4 py-16 sm:px-6 lg:px-8">
        <article className="prose prose-stone prose-lg max-w-none">
          <h1>{doc.title}</h1>

          <p className="mb-8 text-xl text-neutral-600">
            Last updated:{" "}
            {new Date(doc.date).toLocaleDateString("en-US", {
              year: "numeric",
              month: "long",
              day: "numeric",
            })}
          </p>

          <MDXContent code={doc.mdx} components={defaultMDXComponents} />
        </article>
      </div>
    </div>
  );
}
