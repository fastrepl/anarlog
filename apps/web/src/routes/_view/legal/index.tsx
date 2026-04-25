import { createFileRoute } from "@tanstack/react-router";
import { allLegals } from "content-collections";

const LEGAL_ORDER = ["terms", "privacy", "cookies", "dpa"];

export const Route = createFileRoute("/_view/legal/")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Legal - Char" },
      {
        name: "description",
        content:
          "Terms, privacy policy, cookie policy, and data processing agreement for Char.",
      },
      { property: "og:title", content: "Legal - Char" },
      {
        property: "og:description",
        content:
          "Terms, privacy policy, cookie policy, and data processing agreement for Char.",
      },
    ],
  }),
});

function Component() {
  const docs = [...allLegals].sort((a, b) => {
    const aIndex = LEGAL_ORDER.indexOf(a.slug);
    const bIndex = LEGAL_ORDER.indexOf(b.slug);
    return (aIndex === -1 ? 999 : aIndex) - (bIndex === -1 ? 999 : bIndex);
  });

  return (
    <main className="mx-auto min-h-screen w-full max-w-5xl px-4 py-5 sm:px-6 lg:px-8">
      <header className="border-color-subtle border-b py-10 sm:py-14">
        <a
          href="/"
          className="text-color-muted mb-8 inline-block font-mono text-sm"
        >
          Char
        </a>
        <h1 className="text-color text-4xl sm:text-5xl">Legal</h1>
        <p className="text-color-muted mt-4 max-w-2xl text-lg">
          Terms, privacy, cookies, and data processing documents.
        </p>
      </header>
      <section className="border-color-subtle bg-border-subtle my-10 grid gap-px overflow-hidden rounded-lg border md:grid-cols-2">
        {docs.map((doc) => (
          <a
            key={doc.slug}
            href={`/legal/${doc.slug}`}
            className="block bg-white p-5 transition hover:bg-neutral-50"
          >
            <h2 className="text-color text-xl">{doc.title}</h2>
            <p className="text-color-muted mt-3 text-base">{doc.summary}</p>
            <time
              dateTime={doc.date}
              className="text-color-muted mt-5 block font-mono text-xs"
            >
              Updated {formatDate(doc.date)}
            </time>
          </a>
        ))}
      </section>
    </main>
  );
}

function formatDate(value: string) {
  return new Date(value).toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}
