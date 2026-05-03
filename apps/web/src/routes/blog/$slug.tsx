import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { type Article, allArticles } from "content-collections";

import { mdxComponents } from "@/components/mdx-components";
import { CHAR_SITE_URL } from "@/lib/seo";

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
    const url = `${CHAR_SITE_URL}/blog/${article.slug}`;
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
        ...(article.coverImage
          ? [{ property: "og:image", content: article.coverImage }]
          : []),
      ],
    };
  },
});

function Component() {
  const { article } = Route.useLoaderData();
  const authors = Array.isArray(article.author)
    ? article.author.join(", ")
    : article.author;

  return (
    <main className="mx-auto max-w-3xl px-6 py-16">
      <Link
        to="/blog/"
        className="mb-8 inline-block text-sm text-neutral-500 hover:text-neutral-800"
      >
        ← Blog
      </Link>

      <header className="mb-10">
        <h1 className="mb-4 font-mono text-3xl leading-tight text-stone-800 sm:text-4xl">
          {article.title}
        </h1>
        <div className="flex items-center gap-2 text-sm text-neutral-500">
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

      {article.coverImage && (
        <img
          src={article.coverImage}
          alt={article.title}
          className="mb-10 w-full rounded-md border border-neutral-200"
        />
      )}

      <article className="prose prose-stone prose-headings:font-mono prose-headings:text-stone-800 prose-a:text-stone-800 prose-a:underline hover:prose-a:text-stone-600 prose-img:rounded-md prose-img:border prose-img:border-neutral-200 max-w-none">
        <MDXContent code={article.mdx} components={mdxComponents} />
      </article>
    </main>
  );
}
