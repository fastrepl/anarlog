import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { type Article, allArticles } from "content-collections";
import { Download } from "lucide-react";
import type { ComponentProps } from "react";

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

        <article className="blog-prose prose prose-stone prose-headings:font-hand prose-headings:font-semibold prose-headings:text-[#756b5d] prose-p:text-[#363029] prose-a:text-[#181613] prose-a:underline hover:prose-a:text-[#4f4940] prose-strong:text-[#181613] prose-li:text-[#363029] prose-img:rounded-md prose-img:border prose-img:border-[#eee8df] max-w-none">
          <MDXContent code={article.mdx} components={blogMdxComponents} />
        </article>

        <BlogDownloadCta />
      </div>

      <SiteFooter />
    </main>
  );
}

function BlogTable({ children, ...props }: ComponentProps<"table">) {
  return (
    <div className="my-8 overflow-x-auto">
      <table {...props}>{children}</table>
    </div>
  );
}

function BlogDownloadCta() {
  return (
    <aside
      aria-label="Download Anarlog"
      className="mt-20 border-y border-[#eee8df] bg-[#faf7f1] px-5 py-8 md:px-7"
    >
      <div className="flex flex-col gap-5 md:flex-row md:items-center md:justify-between">
        <div>
          <p className="font-hand text-3xl leading-none font-semibold tracking-normal text-[#756b5d] md:text-4xl">
            Take notes without inviting a bot
          </p>
          <p className="mt-3 max-w-xl text-base leading-7 text-[#4f4940]">
            Download Anarlog for private, local-first meeting notes on your Mac.
          </p>
        </div>
        <Link
          to="/download/"
          className="inline-flex h-12 shrink-0 items-center justify-center gap-2 rounded-full bg-[#181613] px-5 text-sm font-semibold text-white transition-colors hover:bg-[#363029]"
        >
          <Download size={17} strokeWidth={2.2} aria-hidden="true" />
          Download app
        </Link>
      </div>
    </aside>
  );
}
