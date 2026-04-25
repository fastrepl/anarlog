import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, notFound } from "@tanstack/react-router";
import { allLegals } from "content-collections";

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
    <main className="mx-auto min-h-screen w-full max-w-4xl px-4 py-5 sm:px-6 lg:px-8">
      <header className="border-color-subtle border-b py-10 sm:py-14">
        <a
          href="/legal"
          className="text-color-muted mb-8 inline-block font-mono text-sm"
        >
          Legal
        </a>
        <h1 className="text-color text-4xl sm:text-5xl">{doc.title}</h1>
        <p className="text-color-muted mt-4 max-w-2xl text-lg">{doc.summary}</p>
        <time
          dateTime={doc.date}
          className="text-color-muted mt-5 block font-mono text-sm"
        >
          Updated {formatDate(doc.date)}
        </time>
      </header>
      <article className="prose prose-neutral prose-headings:font-mono prose-a:text-color prose-a:underline max-w-none py-10">
        <MDXContent code={doc.mdx} />
      </article>
    </main>
  );
}

function formatDate(value: string) {
  return new Date(value).toLocaleDateString("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}
