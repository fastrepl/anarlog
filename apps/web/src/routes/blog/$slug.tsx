import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import type { Article, ArticleSummary } from "content-collections";
import {
  Children,
  cloneElement,
  isValidElement,
  type ComponentProps,
  type ReactElement,
  type ReactNode,
} from "react";

import { ArrowRight } from "@anlg/ui/components/icons";

import { mdxComponents } from "@/components/mdx-components";
import { SiteFooter } from "@/components/site-footer";
import { formatBlogDate } from "@/lib/blog-date";
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
                { name: "Blog", item: getCanonicalUrl("/blog") },
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

        <Link
          to="/blog/"
          className="mt-16 inline-block text-sm text-[#756b5d] hover:text-[#181613]"
        >
          ← Blog
        </Link>

        <header className="pt-10 pb-12">
          <h1 className="font-hand text-5xl leading-[1.02] font-semibold tracking-normal text-balance text-black md:text-6xl">
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
            <p className="font-hand text-lg font-semibold tracking-normal text-[#756b5d]">
              TL;DR
            </p>
            <p className="font-hand mt-3 text-xl leading-7 font-semibold text-[#363029] md:text-2xl md:leading-8">
              {tldr}
            </p>
          </aside>
        )}

        <article className="blog-prose prose prose-stone prose-headings:font-hand prose-headings:font-semibold prose-headings:text-[#756b5d] prose-p:text-[#363029] prose-a:text-[#181613] prose-a:underline hover:prose-a:text-[#4f4940] prose-strong:text-[#181613] prose-li:text-[#363029] prose-img:rounded-md max-w-none">
          <MDXContent code={article.mdx} components={blogMdxComponents} />
        </article>

        <RelatedArticles articles={relatedArticles} />
        <BlogArticleCta />
      </div>

      <SiteFooter />
    </main>
  );
}

function getRelatedArticles(
  article: Article,
  articleSummaries: ArticleSummary[],
) {
  const sortedArticles = [...articleSummaries].sort(
    (a, b) =>
      new Date(b.date).getTime() - new Date(a.date).getTime() ||
      a.slug.localeCompare(b.slug),
  );
  const articleIndex = sortedArticles.findIndex(
    (candidate) => candidate.slug === article.slug,
  );

  if (articleIndex === -1) {
    return [];
  }

  const neighbors = [
    sortedArticles[articleIndex - 1],
    sortedArticles[articleIndex + 1],
  ].filter((candidate): candidate is ArticleSummary => Boolean(candidate));
  const sameCategory = sortedArticles.find(
    (candidate) =>
      candidate.slug !== article.slug &&
      candidate.category === article.category &&
      !neighbors.some((neighbor) => neighbor.slug === candidate.slug),
  );
  const fallback = sortedArticles.find(
    (candidate) =>
      candidate.slug !== article.slug &&
      !neighbors.some((neighbor) => neighbor.slug === candidate.slug),
  );

  return [...neighbors, sameCategory ?? fallback].filter(
    (candidate): candidate is ArticleSummary => Boolean(candidate),
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
        className="font-hand text-3xl font-semibold tracking-normal text-[#756b5d]"
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
              <p className="font-hand text-xl leading-6 font-semibold text-[#756b5d] group-hover:text-[#4f4940]">
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
  return (
    <div className="my-6 overflow-x-auto">
      <table {...props}>{normalizeTableChildren(children)}</table>
    </div>
  );
}

type ElementWithChildren = ReactElement<{ children?: ReactNode }>;

function normalizeTableChildren(children: ReactNode) {
  return Children.toArray(children).map((child) => {
    const element = getElementWithChildren(child);

    if (!element) {
      return child;
    }

    if (element.type === "thead") {
      const rows = Children.toArray(element.props.children);
      return rows.length > 0 && rows.every(isBlankTableRow) ? null : child;
    }

    if (element.type !== "tbody") {
      return child;
    }

    const rows = Children.toArray(element.props.children);
    const visibleRows = rows.filter((row) => !isBlankTableRow(row));

    if (visibleRows.length === rows.length) {
      return child;
    }

    return visibleRows.length > 0
      ? cloneElement(element, undefined, visibleRows)
      : null;
  });
}

function isBlankTableRow(row: ReactNode) {
  const element = getElementWithChildren(row);

  if (!element || element.type !== "tr") {
    return false;
  }

  const cells = Children.toArray(element.props.children);
  return cells.length > 0 && cells.every(isBlankTableCell);
}

function isBlankTableCell(cell: ReactNode) {
  const element = getElementWithChildren(cell);

  if (!element || (element.type !== "td" && element.type !== "th")) {
    return false;
  }

  return isBlankNode(element.props.children);
}

function isBlankNode(node: ReactNode): boolean {
  if (node === null || node === undefined || typeof node === "boolean") {
    return true;
  }

  if (typeof node === "string" || typeof node === "number") {
    return (
      String(node)
        .replace(/\u00a0/g, " ")
        .trim() === ""
    );
  }

  const element = getElementWithChildren(node);
  if (element) {
    const { children } = element.props;

    if (
      children === null ||
      children === undefined ||
      typeof children === "boolean"
    ) {
      return false;
    }

    return isBlankNode(children);
  }

  const children = Children.toArray(node);
  return children.length === 0 || children.every(isBlankNode);
}

function getElementWithChildren(node: ReactNode): ElementWithChildren | null {
  return isValidElement<{ children?: ReactNode }>(node) ? node : null;
}

function BlogArticleCta() {
  return (
    <aside
      aria-label="Try Anarlog for free"
      className="border-color-subtle mt-20 rounded-sm border bg-[#faf7f1] px-5 py-8 md:px-7"
    >
      <div className="flex flex-col gap-5 md:flex-row md:items-center md:justify-between">
        <div>
          <p className="font-hand text-3xl leading-none font-semibold tracking-normal text-[#756b5d] md:text-4xl">
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
