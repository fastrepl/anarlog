import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import allArticleSummaries from "article-summaries";
import { useState } from "react";

import { cn } from "@anlg/utils";

import { SiteFooter } from "@/components/site-footer";
import { formatBlogDate } from "@/lib/blog-date";
import { getCanonicalUrl } from "@/lib/seo";

export const Route = createFileRoute("/blog/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: getCanonicalUrl("/blog") }],
    meta: [
      { title: "Anarlog Blog" },
      {
        name: "description",
        content:
          "Guides for AI meeting notes, privacy research, and engineering notes from the Anarlog team.",
      },
      { property: "og:title", content: "Anarlog Blog" },
      { property: "og:url", content: getCanonicalUrl("/blog") },
    ],
  }),
});

function Component() {
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const categories = [
    { label: "All", value: null, icon: "lucide:layout-grid" },
    { label: "Product", value: "Product", icon: "lucide:package" },
    { label: "Comparisons", value: "Comparisons", icon: "lucide:columns-2" },
    { label: "Engineering", value: "Engineering", icon: "lucide:code-2" },
    {
      label: "Founders' notes",
      value: "Founders' notes",
      icon: "lucide:rocket",
    },
    { label: "Guides", value: "Guides", icon: "lucide:book-open" },
  ];
  const sortedArticles = [...allArticleSummaries].sort(
    (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
  );
  const visibleArticles = sortedArticles.filter(
    (article) =>
      selectedCategory === null || article.category === selectedCategory,
  );

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[860px] px-5 py-8 md:px-8 md:py-12">
        <header className="flex items-center justify-between gap-6">
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
        </header>

        <section className="pt-24 pb-16 md:pt-32">
          <h1 className="font-hand text-6xl leading-[0.98] font-semibold tracking-normal text-balance text-black md:text-8xl">
            Blog
          </h1>
          <div
            role="group"
            aria-label="Filter blog articles by category"
            className="mt-8 flex flex-wrap gap-2"
          >
            {categories.map((category) => (
              <button
                key={category.label}
                type="button"
                aria-pressed={selectedCategory === category.value}
                onClick={() => setSelectedCategory(category.value)}
                className={cn([
                  "focus-visible:outline-brand-dark rounded-pill inline-flex items-center gap-2 border px-4 py-2 font-sans text-sm transition-colors focus-visible:outline-2 focus-visible:outline-offset-4",
                  selectedCategory === category.value
                    ? "border-border bg-surface-subtle text-fg"
                    : "border-border-subtle bg-surface text-fg-secondary hover:border-border hover:bg-surface-subtle",
                ])}
              >
                <Icon
                  icon={category.icon}
                  width={18}
                  height={18}
                  aria-hidden="true"
                />
                {category.label}
              </button>
            ))}
          </div>
        </section>

        <p role="status" className="sr-only">
          {visibleArticles.length} articles
          {selectedCategory ? ` in ${selectedCategory}` : " in all categories"}
        </p>
        <ul className="grid gap-9">
          {visibleArticles.map((article) => (
            <li key={article.slug}>
              <Link
                to="/blog/$slug/"
                params={{ slug: article.slug }}
                className="group block"
              >
                <article className="grid gap-3 border-t border-[#eee8df] pt-6">
                  <h2 className="font-hand text-3xl leading-[1.05] font-semibold tracking-normal text-balance text-[#756b5d] group-hover:text-[#4f4940]">
                    {article.title}
                  </h2>
                  {article.meta_description && (
                    <p className="line-clamp-2 leading-7 text-[#4f4940]">
                      {article.meta_description}
                    </p>
                  )}
                  <div className="flex items-center gap-2 text-xs text-[#756b5d]">
                    <span>
                      {Array.isArray(article.author)
                        ? article.author.join(", ")
                        : article.author}
                    </span>
                    <span>·</span>
                    <time dateTime={article.date}>
                      {formatBlogDate(article.date, "short")}
                    </time>
                  </div>
                </article>
              </Link>
            </li>
          ))}
        </ul>
      </div>

      <SiteFooter />
    </main>
  );
}
