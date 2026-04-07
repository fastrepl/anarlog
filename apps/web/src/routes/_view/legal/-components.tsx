import { MDXContent } from "@content-collections/mdx/react";
import { Link } from "@tanstack/react-router";

import { defaultMDXComponents } from "@/components/mdx";
import { TableOfContents } from "@/components/table-of-contents";

import { getOrderedLegals } from "./-structure";

const orderedLegals = getOrderedLegals();

export function LegalLayout({ doc }: { doc: any }) {
  return (
    <>
      <main className="max-w-200 px-4 py-6">
        <ArticleHeader doc={doc} />
        <ArticleContent doc={doc} />
        <PageNavigation currentSlug={doc.slug} />
      </main>
      <TableOfContents toc={doc.toc} />
    </>
  );
}

export function LegalIndexLayout() {
  return (
    <main className="max-w-200 px-4 py-6">
      <header className="mb-8 lg:mb-12">
        <div className="mb-4 inline-flex items-center gap-2 text-sm text-neutral-500">
          <span>Legal</span>
        </div>
        <h1 className="mb-4 font-serif text-3xl text-stone-700 sm:text-4xl">
          Legal
        </h1>
        <p className="max-w-3xl text-lg leading-relaxed text-neutral-600 lg:text-xl">
          Terms, privacy policy, and related legal documents for Char.
        </p>
      </header>

      <div className="overflow-hidden rounded-xs bg-white">
        {orderedLegals.map((doc, index) => (
          <LegalDocumentRow
            key={doc.slug}
            doc={doc}
            isLast={index === orderedLegals.length - 1}
          />
        ))}
      </div>
    </main>
  );
}

function ArticleHeader({ doc }: { doc: any }) {
  return (
    <header className="mb-8 lg:mb-12">
      <div className="text-fg-muted mb-4 inline-flex items-center gap-2 text-sm">
        <span>Legal</span>
      </div>
      <h1 className="text-fg mb-4 font-serif text-3xl sm:text-4xl">
        {doc.title}
      </h1>
      {doc.summary && (
        <p className="text-fg-muted mb-6 text-lg leading-relaxed lg:text-xl">
          {doc.summary}
        </p>
      )}

      <div className="text-fg-muted flex items-center gap-4 text-sm">
        <time dateTime={doc.date}>
          Updated{" "}
          {new Date(doc.date).toLocaleDateString("en-US", {
            year: "numeric",
            month: "long",
            day: "numeric",
          })}
        </time>
      </div>
    </header>
  );
}

function ArticleContent({ doc }: { doc: any }) {
  return (
    <article className="prose prose-stone prose-headings:font-serif prose-headings:font-semibold prose-h1:text-3xl prose-h1:mt-12 prose-h1:mb-6 prose-h2:text-2xl prose-h2:mt-10 prose-h2:mb-5 prose-h3:text-xl prose-h3:mt-8 prose-h3:mb-4 prose-h4:text-lg prose-h4:mt-6 prose-h4:mb-3 prose-a:text-stone-600 prose-a:underline prose-a:decoration-dotted hover:prose-a:text-stone-800 prose-headings:no-underline prose-headings:decoration-transparent prose-code:bg-stone-50 prose-code:border prose-code:border-neutral-200 prose-code:rounded prose-code:px-1.5 prose-code:py-0.5 prose-code:text-sm prose-code:font-mono prose-code:text-stone-700 prose-pre:bg-stone-50 prose-pre:border prose-pre:border-neutral-200 prose-pre:rounded-xs prose-pre:prose-code:bg-transparent prose-pre:prose-code:border-0 prose-pre:prose-code:p-0 prose-img:rounded-xs prose-img:my-8 max-w-none">
      <MDXContent code={doc.mdx} components={defaultMDXComponents} />
    </article>
  );
}

function LegalDocumentRow({ doc, isLast }: { doc: any; isLast: boolean }) {
  return (
    <Link
      to="/legal/$slug/"
      params={{ slug: doc.slug }}
      className={`group block px-5 py-5 transition-colors sm:px-6 ${!isLast ? "border-b border-neutral-100" : ""} hover:bg-stone-50/70`}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <h2 className="text-fg group-hover:text-fg font-serif text-xl transition-colors">
            {doc.title}
          </h2>
          <p className="text-fg mt-2 max-w-2xl text-sm leading-6 sm:text-base">
            {doc.summary}
          </p>
        </div>

        <span className="text-fg-muted group-hover:text-fg shrink-0 text-sm transition-colors">
          Read
        </span>
      </div>

      <p className="text-fg-muted mt-4 text-sm">
        Updated{" "}
        {new Date(doc.date).toLocaleDateString("en-US", {
          month: "long",
          day: "numeric",
          year: "numeric",
        })}
      </p>
    </Link>
  );
}

function PageNavigation({ currentSlug }: { currentSlug: string }) {
  const currentIndex = orderedLegals.findIndex(
    (doc) => doc.slug === currentSlug,
  );
  const prev = currentIndex > 0 ? orderedLegals[currentIndex - 1] : null;
  const next =
    currentIndex >= 0 && currentIndex < orderedLegals.length - 1
      ? orderedLegals[currentIndex + 1]
      : null;

  if (!prev && !next) return null;

  return (
    <nav className="mt-12 flex items-center justify-between gap-4 border-t border-neutral-200 pt-6">
      {prev ? (
        <Link
          to="/legal/$slug/"
          params={{ slug: prev.slug }}
          className="group flex flex-col items-start gap-1 text-sm"
        >
          <span className="text-neutral-400 transition-colors group-hover:text-neutral-500">
            Previous
          </span>
          <span className="font-medium text-stone-600 transition-colors group-hover:text-stone-800">
            {prev.title}
          </span>
        </Link>
      ) : (
        <div />
      )}
      {next ? (
        <Link
          to="/legal/$slug/"
          params={{ slug: next.slug }}
          className="group flex flex-col items-end gap-1 text-right text-sm"
        >
          <span className="text-neutral-400 transition-colors group-hover:text-neutral-500">
            Next
          </span>
          <span className="font-medium text-stone-600 transition-colors group-hover:text-stone-800">
            {next.title}
          </span>
        </Link>
      ) : (
        <div />
      )}
    </nav>
  );
}
