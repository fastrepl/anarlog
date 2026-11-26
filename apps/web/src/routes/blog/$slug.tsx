import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import type { Article, ArticleSummary } from "content-collections";
import type { ComponentProps } from "react";

import { ArrowLeft, ArrowRight } from "@anlg/ui/components/icons";

import { mdxComponents } from "@/components/mdx-components";
import { SiteFooter } from "@/components/site-footer";
import { formatBlogDate } from "@/lib/blog-date";
import { getRelatedArticles, isTranscriptionArticle } from "@/lib/blog-related";
import { prepareBlogTable } from "@/lib/blog-table";
import {
  getBlogOgImageUrl,
  getBlogPostingJsonLd,
  getBreadcrumbListJsonLd,
  getCanonicalUrl,
  getStructuredDataGraph,
} from "@/lib/seo";

const blogMdxComponents = {
  ...mdxComponents,
  table: BlogTable,
};

export const Route = createFileRoute("/blog/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const { allArticles, allArticleSummaries } =
      await import("content-collections");
    const article = allArticles.find((a: Article) => a.slug === params.slug);
    if (!article) {
      throw notFound();
    }
    return {
      article,
      relatedArticles: getRelatedArticles(article, allArticleSummaries),
    };
  },
  head: ({ loaderData }) => {
    const article = loaderData?.article;
    if (!article) return {};
    const url = getCanonicalUrl(`/blog/${article.slug}`);
    const imageUrl = getBlogOgImageUrl(article.slug);
    const authors = Array.isArray(article.author)
      ? article.author
      : [article.author];
    return {
      links: [{ rel: "canonical", href: url }],
      meta: [
        { title: article.meta_title || article.title },
        { name: "description", content: article.meta_description },
        {
          property: "og:title",
          content: article.meta_title || article.title,
        },
        { property: "og:description", content: article.meta_description },
        { property: "og:url", content: url },
        { property: "og:type", content: "article" },
        { property: "og:image", content: imageUrl },
        { property: "og:image:type", content: "image/png" },
        { property: "og:image:width", content: "1200" },
        { property: "og:image:height", content: "630" },
        {
          property: "og:image:alt",
          content: `Preview of ${article.title}`,
        },
        { name: "twitter:card", content: "summary_large_image" },
        { name: "twitter:title", content: article.meta_title || article.title },
        { name: "twitter:description", content: article.meta_description },
        { name: "twitter:image", content: imageUrl },
        { name: "twitter:image:alt", content: `Preview of ${article.title}` },
      ],
      scripts: [
        {
          type: "application/ld+json",
          children: JSON.stringify(
            getStructuredDataGraph([
              getBlogPostingJsonLd({
                url,
                headline: article.title,
                description: article.meta_description,
                image: imageUrl,
                datePublished: article.date,
                authors,
              }),
              getBreadcrumbListJsonLd([
                { name: "Home", item: getCanonicalUrl() },
                isTranscriptionArticle(article.slug)
                  ? {
                      name: "Transcription guides",
                      item: getCanonicalUrl("/guides/transcription"),
                    }
                  : { name: "Blog", item: getCanonicalUrl("/blog") },
                { name: article.title, item: url },
              ]),
            ]),
          ),
        },
      ],
    };
  },
});

function Component() {
  const { article, relatedArticles } = Route.useLoaderData();
  const authors = Array.isArray(article.author)
    ? article.author.join(", ")
    : article.author;
  const tldr = article.meta_description.trim();

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[860px] px-5 py-8 md:px-8 md:py-12">
        <header className="flex items-center justify-between gap-6">
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
        </header>

        <nav
          aria-label="Article navigation"
          className="text-brand-dark mt-16 flex flex-wrap items-center gap-5 text-sm"
        >
          <Link
            to="/blog/"
            className="inline-flex items-center gap-1 hover:underline"
          >
            <ArrowLeft size={14} aria-hidden="true" />
            Blog
          </Link>
          {isTranscriptionArticle(article.slug) && (
            <Link to="/guides/transcription/" className="hover:underline">
              Transcription guides
            </Link>
          )}
        </nav>

        <header className="pt-10 pb-12">
          <h1 className="text-3xl leading-[1.08] font-semibold tracking-[-0.025em] text-pretty text-black md:text-5xl">
            {article.title}
          </h1>
          <div className="mt-6 flex items-center gap-2 text-sm text-[#756b5d]">
            <span>{authors}</span>
            <span>·</span>
            <time dateTime={article.date}>
              {formatBlogDate(article.date, "long")}
            </time>
          </div>
        </header>

        {tldr && (
          <aside
            aria-label="TLDR"
            className="mb-12 border-y border-[#eee8df] py-5"
          >
            <p className="text-xs font-semibold tracking-[0.08em] text-[#756b5d] uppercase">
              TL;DR
            </p>
            <p className="mt-2.5 text-lg leading-7 font-medium tracking-tight text-[#363029] md:text-xl md:leading-8">
              {tldr}
            </p>
          </aside>
        )}

        <article className="blog-prose prose prose-stone prose-headings:font-sans prose-headings:font-semibold prose-headings:tracking-tight prose-headings:text-[#756b5d] prose-p:text-[#363029] prose-a:text-[#181613] prose-a:underline hover:prose-a:text-[#4f4940] prose-strong:text-[#181613] prose-li:text-[#363029] prose-img:rounded-md max-w-none">
          <MDXContent code={article.mdx} components={blogMdxComponents} />
        </article>

        <RelatedArticles articles={relatedArticles} />
        <BlogArticleCta />
      </div>

      <SiteFooter />
    </main>
  );
}

function RelatedArticles({ articles }: { articles: ArticleSummary[] }) {
  if (articles.length === 0) {
    return null;
  }

  return (
    <aside aria-labelledby="keep-reading-heading" className="mt-20">
      <h2
        id="keep-reading-heading"
        className="text-2xl leading-tight font-semibold tracking-tight text-[#756b5d]"
      >
        Keep reading
      </h2>
      <ul className="mt-5 grid gap-5 md:grid-cols-3">
        {articles.map((relatedArticle) => (
          <li key={relatedArticle.slug}>
            <Link
              to="/blog/$slug/"
              params={{ slug: relatedArticle.slug }}
              className="group block border-t border-[#eee8df] pt-4"
            >
              <p className="text-base leading-6 font-semibold tracking-tight text-[#756b5d] group-hover:text-[#4f4940]">
                {relatedArticle.title}
              </p>
              <time
                dateTime={relatedArticle.date}
                className="mt-2 block text-xs text-[#756b5d]"
              >
                {formatBlogDate(relatedArticle.date, "short")}
              </time>
            </Link>
          </li>
        ))}
      </ul>
    </aside>
  );
}

function BlogTable({ children, ...props }: ComponentProps<"table">) {
  const table = prepareBlogTable(children);
  return (
    <div className="my-6 overflow-x-auto">
      <table
        {...props}
        role="table"
        data-mobile-stack={table.stackOnMobile || undefined}
      >
        {table.children}
      </table>
    </div>
  );
}

function BlogArticleCta() {
  return (
    <aside
      aria-label="Try Anarlog for free"
      className="border-color-subtle mt-20 rounded-sm border bg-[#faf7f1] px-5 py-8 md:px-7"
    >
      <div className="flex flex-col gap-5 md:flex-row md:items-center md:justify-between">
        <div>
          <p className="text-2xl leading-tight font-semibold tracking-tight text-[#756b5d] md:text-3xl">
            Take notes without inviting a bot
          </p>
          <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
            Try Anarlog for private, local-first meeting notes on your desktop.
          </p>
        </div>
        <Link
          to="/download/"
          className="inline-flex h-12 shrink-0 items-center justify-center gap-2 rounded-full bg-[#181613] px-5 text-sm font-semibold text-white transition-colors hover:bg-[#363029]"
        >
          Try for free
          <ArrowRight size={17} weight="bold" aria-hidden="true" />
        </Link>
      </div>
    </aside>
  );
}
