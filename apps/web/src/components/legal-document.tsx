import { MDXContent } from "@content-collections/mdx/react";
import { Link } from "@tanstack/react-router";
import type { Legal } from "content-collections";

import { cn } from "@anlg/utils";

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
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 py-14 md:px-8 md:py-16">
        <Link
          to="/"
          className="mb-10 inline-block text-sm text-[#756b5d] transition-colors hover:text-[#181613]"
        >
          ← Home
        </Link>

        <header className="mb-10">
          <h1 className="font-hand text-5xl leading-none font-semibold text-[#181613]">
            {doc.title}
          </h1>
          {doc.summary ? (
            <p className="mt-5 max-w-2xl text-lg leading-8 text-[#4f4940]">
              {doc.summary}
            </p>
          ) : null}
          <time
            dateTime={doc.date}
            className="mt-3 block text-sm text-[#756b5d]"
          >
            Last updated{" "}
            {new Date(doc.date).toLocaleDateString("en-US", {
              month: "long",
              day: "numeric",
              year: "numeric",
            })}
          </time>
        </header>

        <article
          className={cn([
            "prose prose-lg prose-stone max-w-none",
            "prose-headings:font-hand prose-headings:font-semibold prose-headings:text-[#181613] prose-h2:text-4xl prose-h3:text-2xl",
            "prose-p:text-[#4f4940] prose-li:text-[#4f4940] prose-strong:text-[#181613]",
            "prose-a:text-[#181613] prose-a:underline hover:prose-a:text-[#4f4940]",
            "prose-blockquote:rounded-[3px] prose-blockquote:border prose-blockquote:border-[#eadfce] prose-blockquote:bg-[#fffaf0] prose-blockquote:px-6 prose-blockquote:py-1 prose-blockquote:font-normal prose-blockquote:not-italic prose-blockquote:text-[#363029] prose-blockquote:shadow-[0_18px_50px_rgba(68,54,36,0.08)]",
            "[&_blockquote_p]:before:content-none [&_blockquote_p]:after:content-none",
          ])}
        >
          <MDXContent code={doc.mdx} components={mdxComponents} />
        </article>
      </div>
    </main>
  );
}
