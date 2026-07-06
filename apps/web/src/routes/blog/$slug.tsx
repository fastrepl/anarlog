import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { type Article, allArticles } from "content-collections";
import { ArrowRight } from "lucide-react";
import {
  Children,
  cloneElement,
  isValidElement,
  type ComponentProps,
  type ReactElement,
  type ReactNode,
} from "react";

import { mdxComponents } from "@/components/mdx-components";
import { SiteFooter } from "@/components/site-footer";
import { ANARLOG_SITE_URL, getBlogOgImageUrl } from "@/lib/seo";

const blogMdxComponents = {
  ...mdxComponents,
  table: BlogTable,
};

export const Route = createFileRoute("/blog/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const article = allArticles.find((a: Article) => a.slug === params.slug);
    if (!article) {
      throw notFound();
    }
    return { article };
  },
  head: ({ loaderData }) => {
    const article = loaderData?.article;
    if (!article) return {};
    const url = `${ANARLOG_SITE_URL}/blog/${article.slug}`;
    const imageUrl = getBlogOgImageUrl(article.slug);
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
        { property: "og:image:width", content: "1200" },
        { property: "og:image:height", content: "630" },
        { name: "twitter:card", content: "summary_large_image" },
        { name: "twitter:title", content: article.meta_title || article.title },
        { name: "twitter:description", content: article.meta_description },
        { name: "twitter:image", content: imageUrl },
      ],
    };
  },
});

function Component() {
  const { article } = Route.useLoaderData();
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
              {new Date(article.date).toLocaleDateString("en-US", {
                month: "long",
                day: "numeric",
                year: "numeric",
              })}
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

        <BlogArticleCta />
      </div>

      <SiteFooter />
    </main>
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
      className="border-color-subtle mt-20 border-y bg-[#faf7f1] px-5 py-8 md:px-7"
    >
      <div className="flex flex-col gap-5 md:flex-row md:items-center md:justify-between">
        <div>
          <p className="font-hand text-3xl leading-none font-semibold tracking-normal text-[#756b5d] md:text-4xl">
            Take notes without inviting a bot
          </p>
          <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
            Try Anarlog for private, local-first meeting notes on your Mac.
          </p>
        </div>
        <Link
          to="/download/"
          className="inline-flex h-12 shrink-0 items-center justify-center gap-2 rounded-full bg-[#181613] px-5 text-sm font-semibold text-white transition-colors hover:bg-[#363029]"
        >
          Try for free
          <ArrowRight size={17} strokeWidth={2.2} aria-hidden="true" />
        </Link>
      </div>
    </aside>
  );
}
