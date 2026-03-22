import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { allLegals } from "content-collections";

export const Route = createFileRoute("/_view/legal/")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Legal - Char" },
      {
        name: "description",
        content: "Terms, privacy policy, and other legal documents for Char",
      },
      { property: "og:title", content: "Legal - Char" },
      {
        property: "og:description",
        content: "Terms, privacy policy, and other legal documents for Char",
      },
      { property: "og:type", content: "website" },
      { property: "og:url", content: "https://char.com/legal" },
      { name: "twitter:card", content: "summary" },
      { name: "twitter:title", content: "Legal - Char" },
      {
        name: "twitter:description",
        content: "Terms, privacy policy, and other legal documents for Char",
      },
    ],
  }),
});

function Component() {
  return (
    <div className="min-h-screen">
      <div className="mx-auto px-4 py-16 lg:py-24">
        <header className="mb-12">
          <h1 className="text-color mb-4 font-mono text-4xl tracking-tight sm:text-5xl">
            Legal
          </h1>
          <p className="text-fg-muted text-lg sm:text-xl">
            Terms, privacy policy, and other legal documents
          </p>
        </header>

        <div className="grid gap-4 md:grid-cols-2">
          {allLegals.map((doc) => (
            <LegalCard key={doc.slug} doc={doc} />
          ))}
        </div>
      </div>
    </div>
  );
}

function LegalCard({ doc }: { doc: (typeof allLegals)[number] }) {
  return (
    <Link
      to="/legal/$slug/"
      params={{ slug: doc.slug }}
      className="group block"
    >
      <article className="h-full rounded-xs border border-neutral-100 bg-white p-6 transition-all duration-300 hover:border-neutral-200 hover:shadow-md">
        <div className="mb-3 flex items-start gap-3">
          <Icon
            icon="mdi:file-document-outline"
            className="mt-0.5 shrink-0 text-xl text-stone-700 transition-colors group-hover:text-stone-800"
          />
          <div className="min-w-0 flex-1">
            <h3 className="mb-2 font-mono text-lg text-stone-700 transition-colors group-hover:text-stone-800">
              {doc.title}
            </h3>
            <p className="line-clamp-2 text-sm text-neutral-500">
              {doc.summary}
            </p>
          </div>
        </div>

        <div className="mt-4 flex items-center justify-between border-t border-neutral-100 pt-4 text-sm text-neutral-500">
          <span className="text-xs">
            Updated{" "}
            {new Date(doc.date).toLocaleDateString("en-US", {
              month: "short",
              day: "numeric",
              year: "numeric",
            })}
          </span>
          <span className="font-medium transition-colors group-hover:text-stone-600">
            Read →
          </span>
        </div>
      </article>
    </Link>
  );
}
