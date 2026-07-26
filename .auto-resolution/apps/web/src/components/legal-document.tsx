import { MDXContent } from "@content-collections/mdx/react";
import { Link } from "@tanstack/react-router";
import type { Legal } from "content-collections";

import { ANARLOG_SITE_URL } from "@/lib/seo";

import { mdxComponents } from "./mdx-components";

export function legalHead(doc: Legal, path: "/privacy" | "/terms") {
  const url = `${ANARLOG_SITE_URL}${path}`;

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
}

export function LegalDocument({ doc }: { doc: Legal }) {
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
