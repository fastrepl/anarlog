import { createFileRoute, Link } from "@tanstack/react-router";
import { allArticles } from "content-collections";

import { CHAR_SITE_URL } from "@/lib/seo";

export const Route = createFileRoute("/blog/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: `${CHAR_SITE_URL}/blog` }],
    meta: [
      { title: "Anarlog Blog" },
      {
        name: "description",
        content:
          "Guides for AI meeting notes, privacy research, and engineering notes from the Anarlog team.",
      },
      { property: "og:title", content: "Anarlog Blog" },
      { property: "og:url", content: `${CHAR_SITE_URL}/blog` },
    ],
  }),
});

function Component() {
  const sortedArticles = [...allArticles].sort(
    (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
  );

  return (
    <main className="mx-auto max-w-3xl px-6 py-16">
      <header className="mb-12 border-b border-neutral-200 pb-8">
        <h1 className="mb-3 font-mono text-4xl text-stone-800">Blog</h1>
        <p className="text-lg text-neutral-600">
          Guides, comparisons, and engineering notes from the Anarlog team.
        </p>
      </header>

      <ul className="divide-y divide-neutral-100">
        {sortedArticles.map((article) => (
          <li key={article.slug}>
            <Link
              to="/blog/$slug/"
              params={{ slug: article.slug }}
              className="group block py-5 transition-colors hover:bg-stone-50/50"
            >
              <div className="flex flex-col gap-1">
                <h2 className="font-mono text-lg text-stone-800 group-hover:text-stone-600">
                  {article.title}
                </h2>
                {article.meta_description && (
                  <p className="line-clamp-2 text-sm text-neutral-600">
                    {article.meta_description}
                  </p>
                )}
                <div className="mt-1 flex items-center gap-2 text-xs text-neutral-500">
                  <span>
                    {Array.isArray(article.author)
                      ? article.author.join(", ")
                      : article.author}
                  </span>
                  <span>·</span>
                  <time dateTime={article.date}>
                    {new Date(article.date).toLocaleDateString("en-US", {
                      month: "short",
                      day: "numeric",
                      year: "numeric",
                    })}
                  </time>
                </div>
              </div>
            </Link>
          </li>
        ))}
      </ul>
    </main>
  );
}
