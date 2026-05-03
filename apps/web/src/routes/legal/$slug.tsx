import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { allLegals } from "content-collections";

import { mdxComponents } from "@/components/mdx-components";
import { ANARLOG_SITE_URL } from "@/lib/seo";

export const Route = createFileRoute("/legal/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const doc = allLegals.find((d) => d.slug === params.slug);
    if (!doc) {
      throw notFound();
    }
    return { doc };
  },
  head: ({ loaderData }) => {
    const doc = loaderData?.doc;
    if (!doc) return {};
    const url = `${ANARLOG_SITE_URL}/legal/${doc.slug}`;
    return {
      links: [{ rel: "canonical", href: url }],
      meta: [
        { title: `${doc.title} — Anarlog` },
        { name: "description", content: doc.summary || doc.title },
        { property: "og:title", content: doc.title },
        { property: "og:description", content: doc.summary || doc.title },
        { property: "og:url", content: url },
      ],
    };
  },
});

function Component() {
  const { doc } = Route.useLoaderData();

  return (
    <main className="mx-auto max-w-3xl px-6 py-16">
      <Link
        to="/"
        className="mb-8 inline-block text-sm text-neutral-500 hover:text-neutral-800"
      >
        ← Home
      </Link>

      <header className="mb-10">
        <h1 className="mb-2 font-mono text-3xl leading-tight text-stone-800 sm:text-4xl">
          {doc.title}
        </h1>
        <time dateTime={doc.date} className="text-sm text-neutral-500">
          Last updated{" "}
          {new Date(doc.date).toLocaleDateString("en-US", {
            month: "long",
            day: "numeric",
            year: "numeric",
          })}
        </time>
      </header>

      <article className="prose prose-stone prose-headings:font-mono prose-headings:text-stone-800 prose-a:text-stone-800 prose-a:underline hover:prose-a:text-stone-600 max-w-none">
        <MDXContent code={doc.mdx} components={mdxComponents} />
      </article>
    </main>
  );
}
