import { createFileRoute } from "@tanstack/react-router";
import { allArticles, type Article } from "content-collections";

import { CHAR_SITE_URL } from "@/lib/seo";

type ArticleBucket = {
  label: string;
  description: string;
  matches: (article: Article) => boolean;
};

const ARTICLE_BUCKETS: ArticleBucket[] = [
  {
    label: "Comparisons",
    description: "Alternatives, reviews, and buying decisions.",
    matches: (article) => article.category === "Comparisons",
  },
  {
    label: "Privacy and local AI",
    description: "Data retention, local models, and private workflows.",
    matches: (article) =>
      article.category === "Guides" &&
      /(privacy|retention|legal|local|cloud|safe)/i.test(
        `${article.title} ${article.meta_description}`,
      ),
  },
  {
    label: "Guides",
    description: "Practical meeting, transcription, and note-taking guides.",
    matches: (article) => article.category === "Guides",
  },
  {
    label: "Building Char",
    description: "Engineering notes, product thinking, and company writing.",
    matches: (article) =>
      article.category === "Engineering" ||
      article.category === "Product" ||
      article.category === "Founders' notes",
  },
];

export const Route = createFileRoute("/_view/blog/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: `${CHAR_SITE_URL}/blog` }],
    meta: [
      { title: "Articles - Char" },
      {
        name: "description",
        content:
          "Articles from Char on AI meeting notes, privacy, local AI, and building the product.",
      },
      { property: "og:title", content: "Articles - Char" },
      {
        property: "og:description",
        content:
          "Articles from Char on AI meeting notes, privacy, local AI, and building the product.",
      },
    ],
  }),
});

function Component() {
  const articles = [...allArticles].sort(
    (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
  );

  const buckets = ARTICLE_BUCKETS.map((bucket) => ({
    ...bucket,
    articles: articles.filter(bucket.matches),
  })).filter((bucket) => bucket.articles.length > 0);

  return (
    <main className="mx-auto min-h-screen w-full max-w-6xl px-4 py-5 sm:px-6 lg:px-8">
      <PageHeader articleCount={articles.length} />
      <div className="space-y-12 py-10">
        {buckets.map((bucket) => (
          <section key={bucket.label}>
            <div className="border-color-subtle mb-5 border-b pb-4">
              <h2 className="text-color text-2xl">{bucket.label}</h2>
              <p className="text-color-muted mt-2 text-base">
                {bucket.description}
              </p>
            </div>
            <div className="border-color-subtle bg-border-subtle grid gap-px overflow-hidden rounded-lg border md:grid-cols-2">
              {bucket.articles.map((article) => (
                <ArticleCard key={article.slug} article={article} />
              ))}
            </div>
          </section>
        ))}
      </div>
    </main>
  );
}

function PageHeader({ articleCount }: { articleCount: number }) {
  return (
    <header className="border-color-subtle border-b py-10 sm:py-14">
      <a
        href="/"
        className="text-color-muted mb-8 inline-block font-mono text-sm"
      >
        Char
      </a>
      <h1 className="text-color text-4xl sm:text-5xl">Articles</h1>
      <p className="text-color-muted mt-4 max-w-2xl text-lg">
        {articleCount} articles arranged into the buckets we use to explain
        Char: comparisons, privacy, practical guides, and how we build.
      </p>
    </header>
  );
}

function ArticleCard({ article }: { article: Article }) {
  return (
    <a
      href={`/blog/${article.slug}`}
      className="block bg-white p-5 transition hover:bg-neutral-50"
    >
      <div className="text-color-muted mb-4 flex flex-wrap items-center gap-2 font-mono text-xs">
        {article.category && <span>{article.category}</span>}
        <span>{formatDate(article.date)}</span>
      </div>
      <h3 className="text-color text-xl">{article.title}</h3>
      {article.meta_description && (
        <p className="text-color-muted mt-3 line-clamp-3 text-base">
          {article.meta_description}
        </p>
      )}
    </a>
  );
}

function formatDate(value: string) {
  return new Date(value).toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}
